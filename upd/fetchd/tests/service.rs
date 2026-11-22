// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! The service against a server of the test's own: checks, refusals
//! that change nothing, a download cut and resumed, a file of the wrong
//! hash removed with its code, the limit, the automatic check's times.

use std::collections::HashMap;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use ed25519_dalek::SigningKey;
use fetchd::proto::FileState;
use fetchd::settings::Check;
use fetchd::{Config, Get, Service, Stream};
use sha2::{Digest, Sha256};

/// What the test's server holds, and how much of a file it sends before
/// it cuts the connection (once).
#[derive(Default)]
struct Server {
    files: HashMap<String, Vec<u8>>,
    cut_after: Option<usize>,
    asked: Vec<String>,
}

#[derive(Clone, Default)]
struct Fake(Arc<Mutex<Server>>);

impl Get for Fake {
    fn get(&self, url: &str, limit: u64) -> Result<Vec<u8>, String> {
        let mut s = self.0.lock().unwrap();
        s.asked.push(url.to_string());
        let b = s.files.get(url).cloned().ok_or("the server answered 404")?;
        if b.len() as u64 > limit {
            return Err("too large".into());
        }
        Ok(b)
    }
    fn fetch(&self, url: &str, path: &Path, limit: u64) -> Result<(), String> {
        let mut s = self.0.lock().unwrap();
        s.asked.push(url.to_string());
        let b = s.files.get(url).cloned().ok_or("the server answered 404")?;
        if b.len() as u64 > limit {
            return Err("too large".into());
        }
        let have = usize::try_from(std::fs::metadata(path).map_or(0, |m| m.len())).unwrap();
        let mut f = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .unwrap();
        let end = match s.cut_after.take() {
            Some(n) => (have + n).min(b.len()),
            None => b.len(),
        };
        f.write_all(&b[have.min(b.len())..end]).unwrap();
        if end < b.len() {
            return Err("the connection was cut".into());
        }
        Ok(())
    }
}

const SERVER: &str = "http://mirror.test";

struct World {
    dir: PathBuf,
    fake: Fake,
    posted: Arc<Mutex<Vec<String>>>,
    key: SigningKey,
}

impl World {
    fn new(name: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("fetchd-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("keys")).unwrap();
        let key = SigningKey::from_bytes(&[7; 32]);
        for s in Stream::ALL {
            std::fs::write(
                s.keys_file(&dir.join("keys")),
                format!("{}\n", hex::encode(key.verifying_key().to_bytes())),
            )
            .unwrap();
        }
        Self {
            dir,
            fake: Fake::default(),
            posted: Arc::default(),
            key,
        }
    }

    fn service(&self) -> Service {
        let posted = self.posted.clone();
        let s = Service::start(Config {
            dir: self.dir.join("fetch"),
            files: self.dir.join("updates"),
            keys: self.dir.join("keys"),
            limit: 10_000,
            running: "V1R0M0".parse().unwrap(),
            clock: Box::new(|| 1_800_000_000),
            offset: Box::new(|| 0),
            post: Box::new(move |c, d| posted.lock().unwrap().push(format!("{} {d}", c.code))),
            log: Box::new(|_| {}),
            get: Box::new(self.fake.clone()),
        });
        s.configure(
            &fetchd::proto::Config {
                server: SERVER.into(),
                check: Check::Never,
                download: false,
            },
            "qsadmin",
        )
        .unwrap();
        s
    }

    fn publish(&self, stream: &str, sequence: u64, file: &str, bytes: &[u8]) {
        let index = stream::Index {
            format: stream::FORMAT,
            stream: stream.into(),
            sequence,
            generated: 1_800_000_000,
            releases: vec![stream::Entry {
                release: "V1R0M1".into(),
                build: "t1".into(),
                fixes: vec![],
                updates_from: "V1R0M0".into(),
                published: 1_800_000_000,
                update: file.into(),
                update_bytes: bytes.len() as u64,
                update_sha256: hex::encode(Sha256::digest(bytes)),
                iso: "x.iso".into(),
                iso_bytes: 1,
                iso_sha256: "00".repeat(32),
                released: "2027-01-02".into(),
                notes: "NOTES.md".into(),
            }],
        };
        let mut s = self.fake.0.lock().unwrap();
        s.files.insert(
            format!("{SERVER}/{stream}/v1/index"),
            stream::sign(&index, &self.key).into_bytes(),
        );
        s.files
            .insert(format!("{SERVER}/{stream}/v1/file/{file}"), bytes.to_vec());
        s.files.insert(
            format!("{SERVER}/{stream}/v1/file/NOTES.md"),
            b"# Notes".to_vec(),
        );
    }

    fn posted(&self, code: &str) -> usize {
        self.posted
            .lock()
            .unwrap()
            .iter()
            .filter(|p| p.starts_with(code))
            .count()
    }

    fn wait(svc: &Service, name: &str) -> fetchd::proto::File {
        for _ in 0..200 {
            let st = svc.status();
            if let Some(f) = st.files.iter().find(|f| f.name == name)
                && f.state != FileState::Downloading
            {
                return f.clone();
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        panic!("{name} did not end");
    }
}

impl Drop for World {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

#[test]
fn a_check_takes_what_verifies_and_a_refusal_changes_nothing() {
    let w = World::new("check");
    w.publish("qsmc", 3, "QSMC-V1R0M1-update-a.upd", b"image");
    w.publish("qsvs", 2, "QSVS-V1R0M1-update-b.upd", b"host");
    let svc = w.service();
    let known = svc.check("qsadmin");
    let qsmc = &known[0];
    assert_eq!((qsmc.sequence, qsmc.error.as_ref()), (3, None));
    assert_eq!(qsmc.entries[0].update, "QSMC-V1R0M1-update-a.upd");
    // qs400 has no catalog on this server: not reached, and said.
    assert_eq!(known[2].error.as_ref().unwrap().id, "QMC7204");
    assert!(w.posted("C3007100") == 1 && w.posted("A3007105") == 1);
    assert!(w.dir.join("updates/qsmc/notes/NOTES.md").exists());

    // An older sequence: refused, and what verified before stays.
    w.publish("qsmc", 2, "QSMC-V1R0M1-update-a.upd", b"image");
    let known = svc.check("qsadmin");
    assert_eq!(known[0].error.as_ref().unwrap().id, "QMC7202");
    assert_eq!(known[0].sequence, 3);
    // A bad signature: the same.
    {
        let mut s = w.fake.0.lock().unwrap();
        let url = format!("{SERVER}/qsmc/v1/index");
        let b = s.files.get_mut(&url).unwrap();
        let at = b.len() - 3;
        b[at] = if b[at] == b'0' { b'1' } else { b'0' };
    }
    let known = svc.check("qsadmin");
    assert_eq!(known[0].error.as_ref().unwrap().id, "QMC7201");
    assert_eq!(known[0].entries.len(), 1);
    assert_eq!(w.posted("A3007100"), 2);

    // A restart knows what verified, and the sequence seen.
    drop(svc);
    let svc = w.service();
    let st = svc.status();
    assert_eq!(st.streams[0].sequence, 3);
    w.publish("qsmc", 1, "QSMC-V1R0M1-update-a.upd", b"image");
    assert_eq!(
        svc.check("qsadmin")[0].error.as_ref().unwrap().id,
        "QMC7202"
    );
}

#[test]
fn a_download_cut_resumes_and_a_wrong_file_is_removed() {
    let w = World::new("download");
    w.publish("qsmc", 1, "QSMC-V1R0M1-update-a.upd", b"0123456789");
    let svc = w.service();
    svc.check("qsadmin");
    w.fake.0.lock().unwrap().cut_after = Some(4);
    svc.download(Stream::Qsmc, "QSMC-V1R0M1-update-a.upd", "qsadmin")
        .unwrap();
    let f = World::wait(&svc, "QSMC-V1R0M1-update-a.upd");
    assert_eq!((f.state, f.have), (FileState::Stopped, 4));
    assert!(!w.dir.join("updates/qsmc/QSMC-V1R0M1-update-a.upd").exists());
    svc.download(Stream::Qsmc, "QSMC-V1R0M1-update-a.upd", "qsadmin")
        .unwrap();
    let f = World::wait(&svc, "QSMC-V1R0M1-update-a.upd");
    assert_eq!((f.state, f.have), (FileState::Verified, 10));
    assert_eq!(
        std::fs::read(w.dir.join("updates/qsmc/QSMC-V1R0M1-update-a.upd")).unwrap(),
        b"0123456789"
    );
    assert_eq!(w.posted("C3007110"), 1);

    // The server sends other bytes than the index names.
    w.publish("qsvs", 1, "QSVS-V1R0M1-update-b.upd", b"host");
    svc.check("qsadmin");
    w.fake.0.lock().unwrap().files.insert(
        format!("{SERVER}/qsvs/v1/file/QSVS-V1R0M1-update-b.upd"),
        b"evil".to_vec(),
    );
    svc.download(Stream::Qsvs, "QSVS-V1R0M1-update-b.upd", "qsadmin")
        .unwrap();
    let f = World::wait(&svc, "QSVS-V1R0M1-update-b.upd");
    assert_eq!(f.error.as_ref().unwrap().id, "QMC7205");
    assert_eq!(w.posted("A3007110"), 1);
    assert!(!w.dir.join("updates/qsvs/QSVS-V1R0M1-update-b.upd").exists());
    assert!(
        !w.dir
            .join("updates/.partial/qsvs/QSVS-V1R0M1-update-b.upd")
            .exists()
    );

    // A file the index does not name, or past the limit.
    assert_eq!(
        svc.download(Stream::Qsvs, "other.upd", "qsadmin")
            .unwrap_err()[0]
            .id,
        "QMC7209"
    );
    w.publish("qsvs", 2, "QSVS-V1R0M2-update-c.upd", &vec![0u8; 20_000]);
    svc.check("qsadmin");
    assert_eq!(
        svc.download(Stream::Qsvs, "QSVS-V1R0M2-update-c.upd", "qsadmin")
            .unwrap_err()[0]
            .id,
        "QMC7206"
    );
    svc.remove(Stream::Qsmc, "QSMC-V1R0M1-update-a.upd", "qsadmin")
        .unwrap();
    assert_eq!(w.posted("C3007115"), 1);
    // The one that failed its check stays listed, with why, until removed.
    let left = svc.status().files;
    assert!(left.iter().all(|f| f.state != FileState::Verified));
    svc.remove(Stream::Qsvs, "QSVS-V1R0M1-update-b.upd", "qsadmin")
        .unwrap();
    assert!(svc.status().files.is_empty());
}

#[test]
fn the_settings_are_kept_and_checked() {
    let w = World::new("settings");
    let svc = w.service();
    let bad = fetchd::proto::Config {
        server: "ftp://x".into(),
        check: Check::Daily,
        download: true,
    };
    assert_eq!(svc.configure(&bad, "qsadmin").unwrap_err()[0].id, "QMC7208");
    let good = fetchd::proto::Config {
        server: "https://updates.example/".into(),
        check: Check::Daily,
        download: true,
    };
    svc.configure(&good, "qsadmin").unwrap();
    drop(svc);
    let svc = w.service_keeping();
    let c = svc.status().config;
    assert_eq!(c.server, "https://updates.example");
    assert_eq!((c.check, c.download), (Check::Daily, true));
    assert_eq!(svc.backup()[0].path, "fetch/updates.toml");
}

impl World {
    /// The service as it starts, its settings untouched.
    fn service_keeping(&self) -> Service {
        Service::start(Config {
            dir: self.dir.join("fetch"),
            files: self.dir.join("updates"),
            keys: self.dir.join("keys"),
            limit: 10_000,
            running: "V1R0M0".parse().unwrap(),
            clock: Box::new(|| 1_800_000_000),
            offset: Box::new(|| 0),
            post: Box::new(|_, _| {}),
            log: Box::new(|_| {}),
            get: Box::new(self.fake.clone()),
        })
    }
}

#[test]
fn the_automatic_check_is_at_four_in_the_morning() {
    use fetchd::service::due;
    // 2027-01-03 is a Sunday; 04:00 UTC of it.
    let sunday = 1_798_948_800;
    assert!(!due(Check::Never, 0, sunday, 0));
    assert!(due(Check::Daily, 0, sunday, 0));
    assert!(!due(Check::Daily, sunday, sunday + 60, 0));
    assert!(due(Check::Daily, sunday, sunday + 86_400, 0));
    assert!(!due(Check::Daily, sunday, sunday + 86_399, 0));
    assert!(due(Check::Weekly, sunday - 1, sunday, 0));
    assert!(!due(Check::Weekly, sunday, sunday + 6 * 86_400, 0));
    // An hour east of UTC: 04:00 there is 03:00 UTC.
    assert!(due(Check::Daily, sunday - 3600 - 1, sunday - 3600, 3600));
}
