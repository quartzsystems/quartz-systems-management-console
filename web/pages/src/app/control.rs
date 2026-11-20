// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! What the web server answers on its own socket: the local console's
//! option 4 (`webusers::control`). Each change is audited as the local
//! console's, with who asked there.

use std::sync::Arc;

use webusers::control::{Web, WebStatus};

use super::App;

/// The pages' App, answering the local console.
pub struct Control(pub Arc<App>);

/// Where the audit says a local console request came from.
const FROM: &str = "the local console";

impl Control {
    /// The `open` address, as the certificate and the listeners have it.
    fn open(&self) -> Option<String> {
        let line = self.0.console.fact(release::fact::MANAGEMENT);
        crate::pages::address(&line)
            .split_once('/')
            .map(|(a, _)| a.to_string())
    }
}

fn done<T>(r: &Result<T, String>) -> &str {
    match r {
        Ok(_) => "done",
        Err(e) => e,
    }
}

impl Web for Control {
    fn status(&mut self) -> WebStatus {
        let app = &self.0;
        let s = app.settings();
        let mut w = WebStatus {
            listening: self
                .open()
                .map(|a| {
                    vec![
                        format!("{a}:{}", release::HTTPS_PORT),
                        format!("{a}:{}", release::HTTP_PORT),
                    ]
                })
                .unwrap_or_default(),
            allow: s.allow,
            listen_private: s.listen_private,
            administrators: app
                .users()
                .users
                .iter()
                .filter(|u| u.enabled && u.role == webusers::Role::Administrator)
                .map(|u| u.id.clone())
                .collect(),
            ..WebStatus::default()
        };
        if let Some(c) = app.certs.get() {
            let shown = c.shown();
            w.subject = shown.info.subject;
            w.sha256 = shown.sha256;
            w.sha1 = shown.sha1;
            w.not_before = shown.info.not_before;
            w.not_after = shown.info.not_after;
        }
        w
    }

    fn reset_admin(&mut self, id: String, hash: String, by: String) -> Result<(), String> {
        let app = &self.0;
        let now = app.console.now();
        let r = app.change_users(|u| u.reset_admin(&id, &hash, now));
        if r.is_ok() {
            app.clear_failures(&id);
        }
        app.audit(
            &by,
            FROM,
            &format!("reset or make the Administrator {id}"),
            done(&r),
        );
        r
    }

    fn new_certificate(&mut self, by: String) -> Result<String, String> {
        let app = &self.0;
        let r = app
            .certs
            .get()
            .ok_or_else(|| "the web server has not started".to_string())
            .and_then(|c| {
                c.renew(
                    &app.console.name(),
                    &self.open().into_iter().collect::<Vec<_>>(),
                    app.console.now(),
                )
            })
            .map(|s| s.sha256);
        app.audit(
            &by,
            FROM,
            "make a new self-signed web certificate",
            done(&r),
        );
        r
    }

    fn default_listening(&mut self, by: String) -> Result<(), String> {
        let app = &self.0;
        let mut s = app.settings();
        s.default_listening();
        let r = app.change_settings(s);
        app.audit(&by, FROM, "restore the default listening rule", done(&r));
        r
    }
}

/// Answer the local console on [`release::WEBD_SOCKET`], on a thread of
/// its own.
#[cfg(unix)]
pub fn serve(app: Arc<App>) -> std::io::Result<()> {
    let path = release::WEBD_SOCKET;
    let _ = std::fs::remove_file(path);
    let listener = std::os::unix::net::UnixListener::bind(path)?;
    std::thread::spawn(move || webusers::control::serve(&listener, &mut Control(app)));
    Ok(())
}
