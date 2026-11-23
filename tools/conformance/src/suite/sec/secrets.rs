// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! No secret anywhere (`docs/SECURITY.md`), once more on the final
//! image: a system paired with its one-time password, a user made and
//! its password changed, a backup under a passphrase, a console dump
//! taken; then no password, passphrase or pairing password in any file
//! under `/data` (the backup and the dump among them), and no password
//! hash or private key outside its own file, nor on any page.

use std::collections::{BTreeSet, VecDeque};

use crate::suite::base::{Run, channel};
use crate::suite::mgmt::{Fleet, HOST};
use crate::suite::parts::pages::{key, links};
use crate::suite::web::{ADMIN, PASSWORD, follow, shows, signed_on, start, submit};

/// The plain secrets this suite uses.
const USER_PASSWORD: &str = "Hidden-pass-91";
const USER_PASSWORD_2: &str = "Hidden-pass-92";
const PASSPHRASE: &str = "Backup-phrase-77-qsmc";

/// The files a hash or a key may be in: their own.
const HOMES: [&str; 4] = [
    "/data/state/webusers.toml",
    "/data/state/users.db",
    "/data/state/identity.key",
    "/data/web/key.pem",
];

/// The largest file read whole over the channel.
const READ_MAX: u64 = 16 << 20;

/// Every file under `dir`, with its length.
fn files(ch: &mut crate::qemu::channel::Channel, dir: &str) -> Vec<(String, u64)> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_string()];
    while let Some(d) = stack.pop() {
        for name in ch.ls(&d).unwrap_or_default() {
            let path = format!("{d}/{name}");
            let Ok((mode, _, _)) = ch.stat(&path) else {
                continue;
            };
            if mode.starts_with("40") {
                stack.push(path);
            } else if let Ok((len, _)) = ch.sum(&path) {
                out.push((path, len));
            }
        }
    }
    out
}

/// The suite.
#[allow(
    clippy::too_many_lines,
    reason = "every kind of secret, then every place"
)]
pub fn secrets(run: &Run<'_>) -> Result<String, String> {
    let fleet = Fleet::start(vec![sim::fixture::qshqvs1()])?;
    let mut c = start(run, "sec-secrets")?;
    let b = &mut c.browser;
    signed_on(b, ADMIN, PASSWORD)?;
    // A system paired with its one-time password, and kept.
    let pairing = fleet.tell(0, "pair");
    let port = fleet.ports[0].to_string();
    let r = submit(
        b,
        "/qsmc/servers/add",
        &[
            ("host", HOST),
            ("port", &port),
            ("pw", &pairing),
            ("connect", "Connect"),
        ],
    )?;
    if !shows(&r.text(), "QMC1001") {
        return Err("the system was not added".into());
    }
    let r = submit(b, "/qsmc/servers/add", &[("ms", "QSHQVS1"), ("add", "Add")])?;
    if !shows(&follow(b, &r)?.text(), "QMC1005") {
        return Err("the system was not kept".into());
    }
    // A user and its password changed; a backup under a passphrase.
    for fields in [
        vec![
            ("id", "keeper"),
            ("role", webusers::Role::Viewer.label()),
            ("password", USER_PASSWORD),
            ("again", USER_PASSWORD),
            ("create", "OK"),
        ],
        vec![
            ("id", "keeper"),
            ("password", USER_PASSWORD_2),
            ("again", USER_PASSWORD_2),
            ("new-password", "OK"),
        ],
    ] {
        submit(b, "/qsmc/console/users", &fields)?;
    }
    submit(
        b,
        "/qsmc/console/backup",
        &[
            ("d", "on"),
            ("w", "file"),
            ("pp", PASSPHRASE),
            ("back_up_now", "x"),
        ],
    )?;
    let ch = channel(&mut c.guest)?;
    let dumped = ch.ask(qsmchook::proto::command::DUMP)?.join(" ");
    // What may be only in its own file.
    let users = ch.text("/data/state/webusers.toml")?;
    let hashes: Vec<String> = users
        .split('"')
        .filter(|s| s.starts_with("$argon2id$"))
        .map(str::to_string)
        .collect();
    if hashes.is_empty() {
        return Err("no password hash in webusers.toml".into());
    }
    let mut keyed = Vec::new();
    for home in ["/data/web/key.pem", "/data/state/identity.key"] {
        if let Ok(t) = ch.text(home) {
            keyed.extend(
                t.lines()
                    .filter(|l| l.len() > 40)
                    .take(2)
                    .map(str::to_string),
            );
        }
    }
    let plain = [
        PASSWORD,
        USER_PASSWORD,
        USER_PASSWORD_2,
        PASSPHRASE,
        pairing.as_str(),
    ];
    let mut read = 0;
    let mut skipped = Vec::new();
    for (path, len) in files(ch, "/data") {
        if len > READ_MAX {
            skipped.push(path);
            continue;
        }
        let text = String::from_utf8_lossy(&ch.cat(&path)?).to_string();
        read += 1;
        if let Some(s) = plain.iter().find(|s| text.contains(**s)) {
            return Err(format!("{path} holds the secret `{}...`", &s[..4]));
        }
        if !HOMES.contains(&path.as_str())
            && let Some(s) = hashes
                .iter()
                .chain(&keyed)
                .find(|s| text.contains(s.as_str()))
        {
            return Err(format!("{path} holds a hash or key `{}...`", &s[..12]));
        }
    }
    // Every page from the tree.
    let mut queue: VecDeque<String> = webd::product::NAV
        .iter()
        .flat_map(|g| g.items.iter().map(|(_, r)| format!("/qsmc{r}")))
        .chain([
            "/qsmc/welcome".to_string(),
            "/qsmc/console/audit".to_string(),
        ])
        .collect();
    let mut seen = BTreeSet::new();
    while let Some(p) = queue.pop_front() {
        let bare = p.split('?').next().unwrap_or("");
        if bare.ends_with("/signoff") || bare.ends_with("/ws") || !seen.insert(key(&p)) {
            continue;
        }
        let t = b.get(&p)?.text();
        if let Some(s) = plain
            .iter()
            .copied()
            .chain(hashes.iter().map(String::as_str))
            .chain(keyed.iter().map(String::as_str))
            .find(|s| t.contains(*s))
        {
            return Err(format!("{p} shows a secret `{}...`", &s[..4]));
        }
        queue.extend(links(&t));
        if seen.len() > 2000 {
            break;
        }
    }
    c.guest.kill();
    Ok(format!(
        "after a pairing, a user's two passwords, a backup's passphrase and a console dump \
         ({dumped}): none of them in the {read} files under /data, no hash or key outside its \
         own file, none on the {} pages{}",
        seen.len(),
        if skipped.is_empty() {
            String::new()
        } else {
            format!("; larger than 16 MiB, not read: {}", skipped.join(", "))
        }
    ))
}
