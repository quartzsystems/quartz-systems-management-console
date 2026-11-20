// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! The restore step (`docs/BACKUP.md`), after `/data` is mounted and
//! before anything reads state. A payload the web server left is put
//! in place of `state/` and `web/`, the folders before it kept aside
//! with a trial mark; the mark and the old folders go when the web
//! server answers ([`confirm`]). An IPL that finds the mark takes the
//! restore back: the restored console never answered, or the restore
//! was cut short.

use std::fs;
use std::io::{self, Write};
use std::path::Path;

use qmcbackup::payload::TOPS;
use qmcbackup::{Backup, Manifest};
use release::restore::{DIR, PENDING, PREVIOUS, TRIAL, UPLOADED};

/// What the step did.
#[derive(Debug, PartialEq, Eq)]
pub enum Outcome {
    /// No restore was waiting.
    Nothing,
    /// The backup is in place, on trial.
    Applied(Manifest),
    /// The restore waiting could not be used; nothing changed.
    Refused(String),
    /// A restore not confirmed was taken back.
    TakenBack,
}

fn sync_dir(dir: &Path) {
    if let Ok(d) = fs::File::open(dir) {
        let _ = d.sync_all();
    }
}

fn gone(r: io::Result<()>) -> io::Result<()> {
    match r {
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
        other => other,
    }
}

/// Put the folders kept aside back, and forget the restore.
fn take_back(data: &Path) -> io::Result<()> {
    let r = data.join(DIR);
    for top in TOPS {
        let kept = r.join(PREVIOUS).join(top);
        if kept.exists() {
            gone(fs::remove_dir_all(data.join(top)))?;
            fs::rename(&kept, data.join(top))?;
        }
    }
    sync_dir(data);
    gone(fs::remove_dir_all(&r))?;
    sync_dir(data);
    Ok(())
}

fn write_file(path: &Path, data: &[u8], mode: u32) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(mode & 0o777);
    }
    let mut f = options.open(path)?;
    f.write_all(data)?;
    f.sync_all()
}

fn apply(data: &Path, backup: &Backup) -> io::Result<()> {
    let r = data.join(DIR);
    fs::create_dir_all(r.join(PREVIOUS))?;
    // The mark first: from here an IPL that finds it takes back.
    write_file(&r.join(TRIAL), b"applying\n", 0o600)?;
    sync_dir(&r);
    for top in TOPS {
        let now = data.join(top);
        if now.exists() {
            fs::rename(&now, r.join(PREVIOUS).join(top))?;
        }
        fs::create_dir(&now)?;
    }
    sync_dir(data);
    for e in &backup.entries {
        write_file(&data.join(&e.path), &e.data, e.mode)?;
    }
    for top in TOPS {
        sync_dir(&data.join(top));
    }
    fs::remove_file(r.join(PENDING))?;
    gone(fs::remove_file(r.join(UPLOADED)))?;
    sync_dir(&r);
    Ok(())
}

/// The step: under `data` (`/data`), for a console of release `ours`.
pub fn at_ipl(data: &Path, ours: release::Release) -> Outcome {
    let r = data.join(DIR);
    if r.join(TRIAL).exists() {
        return match take_back(data) {
            Ok(()) => Outcome::TakenBack,
            Err(e) => Outcome::Refused(format!("the state before it could not be put back: {e}")),
        };
    }
    let Ok(bytes) = fs::read(r.join(PENDING)) else {
        // An upload never asked for is not kept past a restart.
        let _ = gone(fs::remove_dir_all(&r));
        return Outcome::Nothing;
    };
    let refuse = |why: String| {
        let _ = gone(fs::remove_dir_all(&r));
        Outcome::Refused(why)
    };
    let backup = match Backup::decode(&bytes) {
        Ok(b) => b,
        Err(e) => return refuse(e.to_string()),
    };
    match backup.manifest.release.parse::<release::Release>() {
        Ok(theirs) if theirs <= ours => {}
        Ok(theirs) => return refuse(format!("the backup is of {theirs}, newer than {ours}")),
        Err(_) => return refuse(format!("`{}` is not a release", backup.manifest.release)),
    }
    match apply(data, &backup) {
        Ok(()) => Outcome::Applied(backup.manifest),
        Err(e) => {
            let back = take_back(data);
            Outcome::Refused(format!(
                "it was not written ({e}){}",
                if back.is_ok() {
                    ""
                } else {
                    "; the state before it may not be whole"
                }
            ))
        }
    }
}

/// The restore on trial is kept: the folders before it go. Says
/// whether there was one.
pub fn confirm(data: &Path) -> bool {
    let r = data.join(DIR);
    if !r.join(TRIAL).exists() {
        return false;
    }
    let _ = gone(fs::remove_dir_all(r.join(PREVIOUS)));
    let _ = gone(fs::remove_dir_all(&r));
    sync_dir(data);
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use qmcbackup::Entry;

    fn scratch(name: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("qsmc-restore-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(d.join("state")).unwrap();
        fs::create_dir_all(d.join("web")).unwrap();
        fs::write(d.join("state/host.toml"), "old").unwrap();
        fs::write(d.join("web/key.pem"), "old key").unwrap();
        d
    }

    fn backup(release: &str) -> Vec<u8> {
        Backup {
            manifest: Manifest {
                console: "QSHQMC1".into(),
                release: release.into(),
                build: "b".into(),
                made: 1,
            },
            entries: vec![Entry {
                path: "state/host.toml".into(),
                mode: 0o644,
                data: b"new".to_vec(),
            }],
        }
        .encode()
        .unwrap()
    }

    fn pending(d: &Path, bytes: &[u8]) {
        fs::create_dir_all(d.join(DIR)).unwrap();
        fs::write(d.join(DIR).join(PENDING), bytes).unwrap();
    }

    const OURS: release::Release = release::Release { v: 1, r: 0, m: 0 };

    #[test]
    fn applied_then_confirmed() {
        let d = scratch("confirm");
        pending(&d, &backup("V1R0M0"));
        assert!(matches!(at_ipl(&d, OURS), Outcome::Applied(m) if m.console == "QSHQMC1"));
        assert_eq!(
            fs::read_to_string(d.join("state/host.toml")).unwrap(),
            "new"
        );
        // Nothing of the old web folder is in the new one.
        assert!(!d.join("web/key.pem").exists());
        assert!(confirm(&d));
        assert!(!d.join(DIR).exists());
        assert!(!confirm(&d));
        let _ = fs::remove_dir_all(d);
    }

    #[test]
    fn not_confirmed_is_taken_back() {
        let d = scratch("back");
        pending(&d, &backup("V1R0M0"));
        assert!(matches!(at_ipl(&d, OURS), Outcome::Applied(_)));
        // The next IPL: never confirmed.
        assert_eq!(at_ipl(&d, OURS), Outcome::TakenBack);
        assert_eq!(
            fs::read_to_string(d.join("state/host.toml")).unwrap(),
            "old"
        );
        assert_eq!(
            fs::read_to_string(d.join("web/key.pem")).unwrap(),
            "old key"
        );
        assert_eq!(at_ipl(&d, OURS), Outcome::Nothing);
        let _ = fs::remove_dir_all(d);
    }

    #[test]
    fn cut_short_is_taken_back() {
        let d = scratch("cut");
        // The mark is down and state was moved aside; then power went.
        fs::create_dir_all(d.join(DIR).join(PREVIOUS)).unwrap();
        fs::write(d.join(DIR).join(TRIAL), "applying\n").unwrap();
        fs::rename(d.join("state"), d.join(DIR).join(PREVIOUS).join("state")).unwrap();
        fs::create_dir(d.join("state")).unwrap();
        pending(&d, &backup("V1R0M0"));
        assert_eq!(at_ipl(&d, OURS), Outcome::TakenBack);
        assert_eq!(
            fs::read_to_string(d.join("state/host.toml")).unwrap(),
            "old"
        );
        assert!(!d.join(DIR).exists());
        let _ = fs::remove_dir_all(d);
    }

    #[test]
    fn newer_or_damaged_is_refused() {
        let d = scratch("refused");
        pending(&d, &backup("V1R1M0"));
        assert!(matches!(at_ipl(&d, OURS), Outcome::Refused(w) if w.contains("newer")));
        pending(&d, b"QSMCPAY1 not a payload");
        assert!(matches!(at_ipl(&d, OURS), Outcome::Refused(_)));
        assert_eq!(
            fs::read_to_string(d.join("state/host.toml")).unwrap(),
            "old"
        );
        assert!(!d.join(DIR).exists());
        let _ = fs::remove_dir_all(d);
    }
}
