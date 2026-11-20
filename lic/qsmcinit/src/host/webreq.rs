// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! The local console's option 4 while the web server is down: its three
//! changes, made by PID 1 on the files (`webusers::control`). Refused
//! while the web server runs, which keeps them and would write over
//! them.

use std::path::Path;

use state::Store;
use webusers::control::{NOTE_CERTIFICATE, NOTE_LISTENING, NOTE_RESET};
use webusers::{WebSettings, WebUsers};

/// Whether `key` is one of the three.
pub fn is_request(key: &str) -> bool {
    [NOTE_RESET, NOTE_CERTIFICATE, NOTE_LISTENING].contains(&key)
}

fn gone(r: std::io::Result<()>) -> Result<(), String> {
    match r {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e.to_string()),
        _ => Ok(()),
    }
}

/// Make the change `key` asks, with `value`, under `data` (`/data`).
pub fn apply(key: &str, value: &str, data: &Path, now: u64) -> Result<(), String> {
    let store = Store::at(data.join("state"));
    match key {
        NOTE_RESET => {
            let (id, hash) = value
                .split_once(' ')
                .ok_or("the request has no user ID and hash")?;
            let mut list = store
                .load::<WebUsers>()
                .map_err(|e| e.to_string())?
                .unwrap_or_default();
            list.reset_admin(id, hash, now)?;
            store.save(&list).map_err(|e| e.to_string())
        }
        NOTE_CERTIFICATE => {
            // The web server makes a new one when it starts and finds none.
            let web = data.join("web");
            gone(std::fs::remove_file(web.join(release::WEB_CERT_FILE)))?;
            gone(std::fs::remove_file(web.join(release::WEB_KEY_FILE)))
        }
        NOTE_LISTENING => {
            let mut s = store
                .load::<WebSettings>()
                .map_err(|e| e.to_string())?
                .unwrap_or_default();
            s.default_listening();
            store.save(&s).map_err(|e| e.to_string())
        }
        _ => Err(format!("{key} is no request")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("qsmc-webreq-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(d.join("state")).unwrap();
        std::fs::create_dir_all(d.join("web")).unwrap();
        d
    }

    #[test]
    fn an_administrator_is_made_and_reset() {
        let d = scratch("reset");
        let hash = users::hash("Rescue-pass-77").unwrap();
        apply(NOTE_RESET, &format!("rescue {hash}"), &d, 5).unwrap();
        let list: WebUsers = Store::at(d.join("state")).load().unwrap().unwrap();
        assert!(
            list.get("rescue")
                .is_some_and(|u| u.enabled && u.hash == hash)
        );
        assert!(apply(NOTE_RESET, "rescue not-a-hash", &d, 6).is_err());
        let _ = std::fs::remove_dir_all(d);
    }

    #[test]
    fn the_certificate_goes_and_listening_is_the_default() {
        let d = scratch("cert");
        std::fs::write(d.join("web").join(release::WEB_CERT_FILE), "x").unwrap();
        std::fs::write(d.join("web").join(release::WEB_KEY_FILE), "y").unwrap();
        apply(NOTE_CERTIFICATE, "", &d, 1).unwrap();
        assert!(!d.join("web").join(release::WEB_KEY_FILE).exists());
        // Twice is no error: there is nothing to delete.
        apply(NOTE_CERTIFICATE, "", &d, 1).unwrap();
        let store = Store::at(d.join("state"));
        let s = WebSettings {
            allow: vec!["10.0.0.0/8".into()],
            ..WebSettings::default()
        };
        store.save(&s).unwrap();
        apply(NOTE_LISTENING, "", &d, 1).unwrap();
        let s: WebSettings = store.load().unwrap().unwrap();
        assert!(s.allow.is_empty() && !s.listen_private);
        let _ = std::fs::remove_dir_all(d);
    }
}
