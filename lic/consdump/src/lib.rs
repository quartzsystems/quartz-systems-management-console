// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! The console's own service dump (`docs/SERVICE.md`), over
//! `base/servicedump`: one ZIP archive of the logs, the problem log, the
//! state, each service's status and the console's own folders, with
//! every file that keeps a secret left out by name and by what it holds.
//! PID 1 takes it on request, when a service stops after its retries,
//! when a pending slot did not confirm, and after a kernel panic.

#![forbid(unsafe_code)]

use std::path::{Path, PathBuf};

use servicedump::{Kind, Paths, Sources, Store};

/// The console's own files that keep a secret, beside the base's
/// (`servicedump::LEFT_OUT`: the QSI identity's key, the service tools
/// users), each with why.
pub const LEFT_OUT: &[(&str, &str)] = &[
    ("webusers.toml", "the web users and their password hashes"),
    (release::WEB_KEY_FILE, "the web certificate's private key"),
    (
        "systems.toml",
        "the managed systems as paired, with their pins",
    ),
];

/// What a console dump is: `QSMCDUMP-YYYYMMDD-HHMMSS.zip`, at most 4
/// kept and 1 GiB together.
pub static KIND: Kind = Kind {
    prefix: "QSMCDUMP-",
    title: "Console dump",
    max_dumps: 4,
    max_total: 1 << 30,
    left_out: LEFT_OUT,
};

/// Where the console's dumps are kept.
pub fn store() -> Store {
    Store::at(release::DUMPS_DIR, &KIND)
}

/// What every dump takes, under the data file system `data`.
pub fn paths(data: &Path) -> Paths {
    Paths {
        log: data.join("log"),
        state: data.join("state"),
        problems: data.join("problems"),
        proc: PathBuf::from("/proc"),
    }
}

/// The console's own folders beside the state: the managed-systems
/// service's settings, the download service's, what the update service
/// keeps of each slot, the web certificate (its key left out). A file
/// is taken whole; a folder's files one level down.
const OWN: &[(&str, &str)] = &[
    ("mgmt", "mgmt"),
    ("fetch", "fetch"),
    ("update", "update"),
    ("web", "web"),
];

/// The console's own entries, read from the data file system `data`;
/// `base/servicedump` leaves out each that keeps a secret.
pub struct Own {
    /// The data file system.
    pub data: PathBuf,
}

impl Sources for Own {
    fn entries(&mut self) -> Vec<(String, Vec<u8>)> {
        let mut out = Vec::new();
        for (name, dir) in OWN {
            let Ok(rd) = std::fs::read_dir(self.data.join(dir)) else {
                continue;
            };
            let mut files: Vec<PathBuf> = rd
                .flatten()
                .map(|e| e.path())
                .filter(|p| p.is_file())
                .collect();
            files.sort();
            for p in files {
                // An update image staged is no service data.
                if p.extension().is_some_and(|x| x.eq_ignore_ascii_case("img")) {
                    continue;
                }
                let Some(file) = p.file_name().map(|f| f.to_string_lossy().to_string()) else {
                    continue;
                };
                if let Ok(bytes) = std::fs::read(&p)
                    && bytes.len() as u64 <= servicedump::take::MAX_FILE
                {
                    out.push((format!("{name}/{file}"), bytes));
                }
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use servicedump::{Asked, take, zip};

    use super::*;

    /// Every file the console keeps a secret in, by the name its crate
    /// gives it, is left out by name: a new one fails the build until it
    /// is on the list.
    #[test]
    fn every_secret_file_of_the_console_is_on_the_list() {
        use state::Document as _;
        let named: Vec<&str> = servicedump::LEFT_OUT
            .iter()
            .chain(LEFT_OUT)
            .map(|(n, _)| *n)
            .collect();
        for secret in [
            identity::KEY_FILE,
            users::FILE,
            webusers::WebUsers::FILE,
            release::WEB_KEY_FILE,
            mgmtd::keep::files::Systems::FILE,
        ] {
            assert!(named.contains(&secret), "{secret} is not on the list");
        }
    }

    /// A dump of a console with each secret holds what its manifest says
    /// and none of them, by name or by what it holds.
    #[test]
    fn a_console_dump_holds_no_secret() {
        let d = std::env::temp_dir().join(format!("consdump-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        let state = d.join("state");
        fs::create_dir_all(&state).unwrap();
        identity::ensure(&state, "QSHQMC1", "S1", 1_794_834_942).unwrap();
        fs::write(
            state.join(users::FILE),
            users::first("QSSERVICE", &users::hash("Passw0rd!x").unwrap(), 1).unwrap(),
        )
        .unwrap();
        let hash = users::hash("Passw0rd!y").unwrap();
        fs::write(
            state.join("webusers.toml"),
            format!("schema_version = 1\n[[user]]\nid = \"qsadmin\"\nhash = \"{hash}\"\n"),
        )
        .unwrap();
        fs::write(
            state.join("host.toml"),
            "schema_version = 1\nname = \"QSHQMC1\"\n",
        )
        .unwrap();
        for (dir, file, body) in [
            (
                "web",
                "certificate.pem",
                "-----BEGIN CERTIFICATE-----\n".to_string(),
            ),
            (
                "web",
                "key.pem",
                "-----BEGIN PRIVATE KEY-----\n".to_string(),
            ),
            (
                "mgmt",
                "systems.toml",
                "[[systems]]\nfingerprint = \"ab\"\n".to_string(),
            ),
            ("mgmt", "mgmt.toml", "schema_version = 1\n".to_string()),
            ("fetch", "updates.toml", "schema_version = 1\n".to_string()),
            ("update", "slots.json", "{}".to_string()),
            ("update", "staged.img", "QSMCUPDT".to_string()),
            ("log", "refcodes.ndjson", "{}\n".to_string()),
            ("problems", "problems.json", "{}".to_string()),
        ] {
            fs::create_dir_all(d.join(dir)).unwrap();
            fs::write(d.join(dir).join(file), body).unwrap();
        }
        let store = Store::at(d.join("dumps"), &KIND);
        let asked = Asked {
            by: "qsadmin".into(),
            release: "QS/MC V1R0M0".into(),
            host: "QSHQMC1".into(),
            status: "webd running".into(),
        };
        let (info, _) = take(
            &store,
            &paths(&d),
            &mut Own { data: d.clone() },
            &asked,
            1_794_834_942,
        )
        .unwrap();
        assert!(info.name.starts_with("QSMCDUMP-"));
        let bytes = fs::read(d.join("dumps").join(&info.name)).unwrap();
        let entries = zip::entries(&bytes).unwrap();
        let names: Vec<&str> = entries.iter().map(|(n, _)| n.as_str()).collect();
        for want in [
            "state/host.toml",
            "state/identity.crt",
            "web/certificate.pem",
            "mgmt/mgmt.toml",
            "fetch/updates.toml",
            "update/slots.json",
            "log/refcodes.ndjson",
            "problems/problems.json",
            "services/status.txt",
            "MANIFEST.txt",
        ] {
            assert!(names.contains(&want), "{want} not in {names:?}");
        }
        for gone in [
            "identity.key",
            "users.db",
            "webusers.toml",
            "key.pem",
            "systems.toml",
            "staged.img",
        ] {
            assert!(!names.iter().any(|n| n.ends_with(gone)), "{gone} is in it");
        }
        for (name, data) in &entries {
            assert!(!servicedump::take::secret(name, data, LEFT_OUT), "{name}");
        }
        let manifest = String::from_utf8_lossy(&entries.last().unwrap().1).into_owned();
        assert!(manifest.starts_with("Console dump QSMCDUMP-"), "{manifest}");
        assert!(manifest.contains("mgmt/systems.toml: the managed systems as paired"));
        fs::remove_dir_all(&d).unwrap();
    }
}
