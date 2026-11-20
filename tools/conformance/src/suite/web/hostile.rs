// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! Suite 13: hostile requests. Bytes that are not TLS; an oversized
//! head; a malformed request; a body past the upload limit; a slow
//! connection; more connections than the server takes: each is
//! refused or closed, and a signed-on session goes on throughout.
//! Suite 14: no password, passphrase or key in any log, and every kind
//! of change of the pages with its audit record.

use std::io::{Read, Write};
use std::time::{Duration, Instant};

use webusers::Role;

use super::{ADMIN, PASSWORD, follow, shows, signed_on, start, submit};
use crate::suite::base::channel;
use crate::web::Browser;

/// The session still works.
fn alive(b: &mut Browser, after: &str) -> Result<(), String> {
    let a = b.get("/qsmc/welcome")?;
    if a.status == 200 && a.text().contains("Welcome") {
        Ok(())
    } else {
        Err(format!("the session stopped after {after}: {}", a.status))
    }
}

/// The suite of hostile requests.
pub fn hostile(run: &super::Run<'_>) -> Result<String, String> {
    let mut c = start(run, "web-hostile")?;
    let b = &mut c.browser;
    signed_on(b, ADMIN, PASSWORD)?;
    let mut said = Vec::new();
    // Not TLS at all.
    let mut s = b.tcp()?;
    let _ = s.write_all(b"GET / HTTP/1.1\r\n\r\n");
    let mut back = Vec::new();
    let _ = s.read_to_end(&mut back);
    said.push(format!("plain HTTP on 443: {} bytes back", back.len()));
    alive(b, "plain HTTP on 443")?;
    // An oversized head, a malformed request line, a body past the limit.
    let big = format!(
        "GET /qsmc/welcome HTTP/1.1\r\nHost: x\r\nX-Big: {}\r\n\r\n",
        // Past the limit by more than one read: qsweb checks the head's
        // length while it waits for its end.
        "a".repeat(4 * qsweb::http::request::HEAD_MAX)
    );
    for (what, bytes) in [
        ("an oversized head", big.into_bytes()),
        ("a malformed request", b"NOT A REQUEST\r\n\r\n".to_vec()),
        (
            "a body past the upload limit",
            format!(
                "POST /qsmc/console/certs HTTP/1.1\r\nHost: x\r\nContent-Type: multipart/form-data; boundary=b\r\nContent-Length: {}\r\n\r\n",
                qsweb::http::request::BODY_MAX + 1
            )
            .into_bytes(),
        ),
    ] {
        let back = b.raw(&bytes)?;
        match Browser::status_of(&back) {
            Some(code) if (400..500).contains(&code) => said.push(format!("{what}: {code}")),
            None if back.is_empty() => said.push(format!("{what}: closed")),
            other => return Err(format!("{what} answered {other:?}")),
        }
        alive(b, what)?;
    }
    // A connection that says nothing is closed when it has been idle.
    let started = Instant::now();
    let mut slow = b.tcp()?;
    slow.set_read_timeout(Some(Duration::from_secs(90)))
        .map_err(|e| e.to_string())?;
    // The server may say a TLS alert before it closes: read to the end.
    let mut rest = Vec::new();
    let closed = slow.read_to_end(&mut rest).is_ok();
    if !closed || started.elapsed() > Duration::from_secs(80) {
        return Err(format!(
            "a connection that said nothing was not closed in {} s",
            started.elapsed().as_secs()
        ));
    }
    said.push(format!(
        "a silent connection closed after {} s",
        started.elapsed().as_secs()
    ));
    alive(b, "a silent connection")?;
    // More connections than the server takes.
    let many: Vec<_> = (0..webd::listen::MAX_CONNECTIONS + 16)
        .filter_map(|_| b.tcp().ok())
        .collect();
    let opened = many.len();
    std::thread::sleep(Duration::from_secs(2));
    drop(many);
    std::thread::sleep(Duration::from_secs(2));
    said.push(format!("{opened} connections at once"));
    alive(b, "too many connections")?;
    let _ = crate::suite::base::power_off(&mut c.guest);
    Ok(format!(
        "{}; the session went on after each",
        said.join("; ")
    ))
}

/// What the suite types that no log may hold.
const SECRETS: [&str; 3] = [
    "Unseen-pass-word-1",
    "Unlogged-pass-2",
    "a passphrase never logged",
];

/// The suite of the logs.
#[allow(clippy::too_many_lines, reason = "every kind of change, then the logs")]
pub fn secrets(run: &super::Run<'_>) -> Result<String, String> {
    let mut c = start(run, "web-secrets")?;
    let b = &mut c.browser;
    signed_on(b, ADMIN, PASSWORD)?;
    let [first, second, passphrase] = SECRETS;
    let steps: [(&str, Vec<(&str, &str)>); 5] = [
        (
            "/qsmc/console/users",
            vec![
                ("id", "secret"),
                ("role", Role::Viewer.label()),
                ("password", first),
                ("again", first),
                ("create", "OK"),
            ],
        ),
        (
            "/qsmc/console/users",
            vec![
                ("id", "secret"),
                ("password", second),
                ("again", second),
                ("new-password", "OK"),
            ],
        ),
        (
            "/qsmc/console/datetime",
            vec![
                ("tz", "UTC-05:00"),
                ("s", "ntp"),
                ("ntp", "10.0.2.2"),
                ("apply", "Apply"),
            ],
        ),
        (
            "/qsmc/console/locale",
            vec![
                ("l", "English (US)"),
                ("df", "YYYY-MM-DD"),
                ("tf", "24-hour"),
                ("nf", "1,234.56"),
                ("to", "30"),
                ("max", "12"),
                ("apply", "Apply"),
            ],
        ),
        (
            "/qsmc/console/backup",
            vec![
                ("d", "on"),
                ("w", "file"),
                ("pp", passphrase),
                ("back_up_now", "x"),
            ],
        ),
    ];
    for (path, fields) in &steps {
        submit(b, path, fields)?;
    }
    let r = submit(
        b,
        "/qsmc/confirm/user-delete?u=secret",
        &[("u", "secret"), ("ok", "OK")],
    )?;
    if !shows(&follow(b, &r)?.text(), "QMC8004") {
        return Err("the user was not deleted".into());
    }
    // The last change: its answer's redirect is not followed, as the
    // browser trusts the certificate it replaces.
    submit(b, "/qsmc/confirm/cert-new", &[("ok", "OK")])?;
    std::thread::sleep(Duration::from_secs(3));
    let key = channel(&mut c.guest)?.text("/data/web/key.pem")?;
    let key_line = key.lines().nth(1).ok_or("the key has no body")?.to_string();
    let ch = channel(&mut c.guest)?;
    let mut logs = String::new();
    for name in ch.ls("/data/log")? {
        // A folder is no log to read.
        if let Ok(bytes) = ch.cat(&format!("/data/log/{name}")) {
            logs.push_str(&String::from_utf8_lossy(&bytes));
        }
    }
    for secret in SECRETS.iter().copied().chain([PASSWORD, key_line.as_str()]) {
        if logs.contains(secret) {
            return Err(format!(
                "a log holds `{}...`",
                secret.get(..6).unwrap_or(secret)
            ));
        }
    }
    let audits = [
        "create user secret role Viewer",
        "change password of secret",
        "change the display offset to UTC-05:00",
        "change the locale and session",
        "make a backup",
        "delete user secret",
        "make a new self-signed web certificate",
        "sign on: signed on",
    ];
    for want in audits {
        if !logs.contains(&format!("audit: user {ADMIN} from ")) || !logs.contains(want) {
            return Err(format!("no audit record of `{want}`"));
        }
    }
    let _ = crate::suite::base::power_off(&mut c.guest);
    Ok(format!(
        "{} bytes of logs hold none of the passwords, the passphrase or the key; each of the \
         {} kinds of change has its audit record",
        logs.len(),
        audits.len()
    ))
}
