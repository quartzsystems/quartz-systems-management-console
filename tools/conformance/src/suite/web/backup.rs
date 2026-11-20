// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! Suite 9: backup and restore. A console's backup, downloaded; a
//! second console installed from the same ISO refuses a wrong
//! passphrase, a damaged file and a backup of a newer release, then
//! restores the backup at a restart: its users, network, time settings,
//! QSI identity and web certificate are the first console's. Nothing of
//! the backup is readable without its passphrase.

use std::time::Duration;

use webusers::Role;

use super::{ADMIN, PASSWORD, boot, follow, machine, message_of, served, shows, signed_on, submit};
use crate::suite::base::channel;
use crate::web::Browser;

const PASS: &str = "suite backup passphrase";

/// What a restore must bring back, as the guest's files have it.
fn kept(guest: &mut super::Guest<'_>) -> Result<Vec<(&'static str, Vec<u8>)>, String> {
    let ch = channel(guest)?;
    [
        "/data/state/network.toml",
        "/data/state/host.toml",
        "/data/state/identity.crt",
        "/data/web/certificate.pem",
    ]
    .into_iter()
    .map(|p| Ok((p, ch.cat(p)?)))
    .collect()
}

fn ids(guest: &mut super::Guest<'_>) -> Result<String, String> {
    let text = channel(guest)?.text("/data/state/webusers.toml")?;
    let mut ids: Vec<&str> = text
        .lines()
        .filter_map(|l| l.strip_prefix("id = \""))
        .map(|l| l.trim_end_matches('"'))
        .collect();
    ids.sort_unstable();
    Ok(ids.join(", "))
}

fn open(b: &mut Browser, file: &[u8], pp: &str) -> Result<String, String> {
    let token = b.csrf("/qsmc/console/backup?tab=restore")?;
    Ok(b.upload(
        "/qsmc/console/backup?tab=restore",
        &[("csrf", &token), ("pp", pp), ("open", "Open")],
        file,
    )?
    .text())
}

/// The suite.
#[allow(
    clippy::too_many_lines,
    reason = "one backup's whole story, two consoles"
)]
pub fn backup(run: &super::Run<'_>) -> Result<String, String> {
    // The first console, changed where a fresh one differs.
    let mut a = boot(run, machine(run, "web-backup-a")?)?;
    signed_on(&mut a.browser, ADMIN, PASSWORD)?;
    let pw = "Backed-up-pass-1";
    let r = submit(
        &mut a.browser,
        "/qsmc/console/users",
        &[
            ("id", "keeper"),
            ("role", Role::Operator.label()),
            ("password", pw),
            ("again", pw),
            ("create", "OK"),
        ],
    )?;
    if !shows(&follow(&mut a.browser, &r)?.text(), "QMC8001") {
        return Err("the user was not made".into());
    }
    let r = submit(
        &mut a.browser,
        "/qsmc/console/datetime",
        &[
            ("tz", "UTC+02:00"),
            ("s", "ntp"),
            ("ntp", "10.0.2.2"),
            ("apply", "Apply"),
        ],
    )?;
    if !shows(&r.text(), "QMC8200") {
        return Err(format!("the time settings: {}", message_of(&r.text())));
    }
    let r = submit(
        &mut a.browser,
        "/qsmc/console/network",
        &[
            ("ip", ""),
            ("mask", ""),
            ("gw", ""),
            ("dns", "10.0.2.3"),
            ("h", "mc-01"),
            ("d", ""),
            ("pip", ""),
            ("pmask", ""),
            ("allow", ""),
            ("apply", "Apply"),
        ],
    )?;
    if !shows(&r.text(), "QMC8100") {
        return Err(format!("the name servers: {}", message_of(&r.text())));
    }
    std::thread::sleep(Duration::from_secs(35));
    let before = kept(&mut a.guest)?;
    let users = ids(&mut a.guest)?;
    let r = submit(
        &mut a.browser,
        "/qsmc/console/backup",
        &[
            ("d", "on"),
            ("w", "file"),
            ("pp", PASS),
            ("back_up_now", "x"),
        ],
    )?;
    if r.status != 200
        || !r
            .header("content-disposition")
            .is_some_and(|d| d.contains(".qmcb"))
    {
        return Err(format!("no backup came: {}", message_of(&r.text())));
    }
    let file = r.body;
    for secret in [b"keeper".as_slice(), b"BEGIN", b"argon2id", b"10.0.2.3"] {
        if file.windows(secret.len()).any(|w| w == secret) {
            return Err(format!(
                "the backup shows `{}` without its passphrase",
                String::from_utf8_lossy(secret)
            ));
        }
    }
    let _ = crate::suite::base::power_off(&mut a.guest);
    drop(a);

    // The second console: the same installation, never changed.
    let mut b = boot(run, machine(run, "web-backup-b")?)?;
    let fresh = kept(&mut b.guest)?;
    if fresh[2] == before[2] {
        return Err("the two consoles have the same identity before the restore".into());
    }
    signed_on(&mut b.browser, ADMIN, PASSWORD)?;
    if !shows(
        &open(&mut b.browser, &file, "not the passphrase at all")?,
        "QMC8402",
    ) {
        return Err("a wrong passphrase was not refused".into());
    }
    let mut damaged = file.clone();
    let at = damaged.len() / 2;
    damaged[at] ^= 0x40;
    if !shows(&open(&mut b.browser, &damaged, PASS)?, "QMC8402") {
        return Err("a damaged backup was not refused".into());
    }
    let mut newer = qmcbackup::seal::open(&file, PASS).map_err(|e| e.to_string())?;
    newer.manifest.release = "V9R0M0".into();
    let newer = qmcbackup::seal::seal(&newer, PASS).map_err(|e| e.to_string())?;
    if !shows(&open(&mut b.browser, &newer, PASS)?, "QMC8404") {
        return Err("a backup of a newer release was not refused".into());
    }
    let page = open(&mut b.browser, &file, PASS)?;
    if !page.contains("The backup holds") {
        return Err(format!("the backup did not open: {}", message_of(&page)));
    }
    // The console restarts at once: the answer may not come.
    let _ = submit(&mut b.browser, "/qsmc/confirm/restore", &[("ok", "OK")]);
    b.guest
        .wait_exit(std::time::Instant::now() + Duration::from_secs(60))
        .map_err(|e| format!("the console did not restart for the restore: {e}"))?;
    let m = b.machine.clone();
    drop(b);

    let mut r = boot(run, m)?;
    for code in [&refcode::RESTORE_APPLIED, &refcode::RESTORE_CONFIRMED] {
        r.guest.seen(&code.tag(), run.deadline())?;
    }
    let after = kept(&mut r.guest)?;
    for ((path, was), (_, now)) in before.iter().zip(&after) {
        if was != now {
            return Err(format!(
                "{path} is not the first console's after the restore"
            ));
        }
    }
    let restored = ids(&mut r.guest)?;
    if restored != users {
        return Err(format!("the users are {restored}, not {users}"));
    }
    let sha = served(&mut r.guest)?;
    r.browser.pin(&sha)?;
    signed_on(&mut r.browser, "keeper", pw)?;
    let _ = crate::suite::base::power_off(&mut r.guest);
    Ok(format!(
        "{} bytes of backup, none of it readable without the passphrase; a wrong passphrase and \
         a damaged file refused (QMC8402), a newer release refused (QMC8404); restored at the \
         restart (C3001045, C3001096): users {users}, network.toml, host.toml, the QSI identity \
         and the web certificate as the first console had them",
        file.len()
    ))
}
