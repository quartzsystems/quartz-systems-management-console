// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! The power pulled while the writers outside `base/state` write
//! (Phase 6's `power-pull` churns its files): `mgmtd`'s history and
//! problems while a host posts codes and opens problems, the web
//! certificate while one is imported after another, and a restore just
//! confirmed. After each IPL each is whole: the old or the new.

use std::collections::BTreeSet;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crate::suite::base::{Run, channel};
use crate::suite::mgmt::{connected, page, until};
use crate::suite::serv::{Lab, VS, lab};
use crate::suite::web::{
    ADMIN, CERT, Console, PASSWORD, boot, follow, message_of, served, shows, signed_on, submit,
};

/// Cuts while the host's codes and problems come.
const CHURN_CUTS: u32 = 8;
/// Cuts while certificates are imported.
const CERT_CUTS: u32 = 5;
/// Cuts just after a restore is confirmed.
const RESTORE_CUTS: u32 = 3;
/// The longest the work runs before the power goes, in milliseconds.
const RUN_ON_MS: u64 = 3000;
/// The host's history file and the problems' file.
const HISTORY: &str = "/data/mgmt/refcodes/QSHQVS1.codes";
const PROBLEMS: &str = "/data/mgmt/problems.bin";
const PASS: &str = "sec-power backup passphrase";

/// Whether file `name` ends in `.ext`.
fn is(name: &str, ext: &str) -> bool {
    std::path::Path::new(name)
        .extension()
        .is_some_and(|e| e == ext)
}

/// A small generator: the moments need to vary, not to be secret.
struct Moments(u64);

impl Moments {
    fn next_ms(&mut self, most: u64) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0 % most
    }
}

/// The power pulled, and the console booted again and signed on, its
/// host connected.
fn cut<'a>(run: &Run<'a>, c: Console<'a>) -> Result<Console<'a>, String> {
    let Console {
        mut guest, machine, ..
    } = c;
    guest.kill();
    drop(guest);
    let mut c = boot(run, machine)?;
    signed_on(&mut c.browser, ADMIN, PASSWORD)?;
    Ok(c)
}

/// The console's numbers of the host's serviceable events listed.
fn numbers(c: &mut Console<'_>) -> Result<BTreeSet<u64>, String> {
    let t = page(&mut c.browser, "/qsmc/events?s=All&rows=100")?;
    Ok(t.split("/qsmc/event/")
        .skip(1)
        .filter_map(|r| r.split('"').next()?.parse().ok())
        .collect())
}

/// The console's number of the host's event whose row holds `text`.
fn number_of(c: &mut Console<'_>, text: &str) -> Result<Option<u64>, String> {
    let t = page(&mut c.browser, "/qsmc/events?s=All&rows=100")?;
    Ok(t.split("<tr").find(|r| r.contains(text)).and_then(|r| {
        r.split("/qsmc/event/")
            .nth(1)?
            .split('"')
            .next()?
            .parse()
            .ok()
    }))
}

/// `mgmtd`'s files whole: the problems kept and not set aside, each
/// line of the history whole and once.
fn mgmt_whole(c: &mut Console<'_>) -> Result<usize, String> {
    let ch = channel(&mut c.guest)?;
    let mgmt = ch.ls("/data/mgmt")?;
    if mgmt.iter().any(|n| is(n, "bad")) {
        return Err(format!("mgmtd set a file aside: {mgmt:?}"));
    }
    if ch.cat(PROBLEMS)?.is_empty() {
        return Err("problems.bin is empty".into());
    }
    let text = ch.text(HISTORY)?;
    if !text.is_empty() && !text.ends_with('\n') {
        return Err("the history's last line is cut".into());
    }
    let mut seen = BTreeSet::new();
    for l in text.lines() {
        let f: Vec<&str> = l.splitn(8, '\t').collect();
        let whole = f.len() == 8
            && f[0].parse::<u64>().is_ok()
            && f[1].parse::<u32>().is_ok()
            && f[3].parse::<u64>().is_ok();
        if !whole {
            return Err(format!("a history line is not whole: {l:?}"));
        }
        if !seen.insert((f[1].to_string(), f[0].to_string())) {
            return Err(format!("a history line is there twice: {l:?}"));
        }
    }
    Ok(seen.len())
}

/// Part 1: the host's codes and problems while the power goes.
fn churn<'a>(
    run: &Run<'a>,
    lab: Lab<'a>,
    moments: &mut Moments,
) -> Result<(Lab<'a>, String), String> {
    let Lab {
        fleet,
        mut c,
        streams,
    } = lab;
    let mut opened = 0u32;
    let mut lines = 0;
    for n in 0..CHURN_CUTS {
        let before = numbers(&mut c)?;
        let end = Instant::now() + Duration::from_millis(moments.next_ms(RUN_ON_MS));
        let mut i = 0u32;
        while Instant::now() < end {
            fleet.tell(0, &format!("pcode 1 C6004031 churn {n} {i}"));
            fleet.tell(0, &format!("code C2001000 churn {n} {i}"));
            if i.is_multiple_of(4) {
                fleet.tell(0, &format!("problem open 1 B6000600 churn {n} {i}"));
                opened += 1;
            }
            i += 1;
        }
        c = cut(run, c)?;
        until("QSHQVS1 connected again", Duration::from_secs(120), || {
            Ok(connected(&page(&mut c.browser, "/qsmc/connectivity")?, VS))
        })?;
        lines = mgmt_whole(&mut c)?;
        // A number given is never given again.
        fleet.tell(0, &format!("problem open 1 B6000601 after cut {n}"));
        let said = format!("after cut {n}");
        let mut new = None;
        until(
            "the problem opened after the cut",
            Duration::from_secs(60),
            || {
                new = number_of(&mut c, &said)?;
                Ok(new.is_some())
            },
        )?;
        let new = new.unwrap_or(0);
        if before.last().is_some_and(|m| new <= *m) {
            return Err(format!(
                "after cut {n} a problem was numbered {new}, not past {before:?}"
            ));
        }
        // Each seen before is kept: the list shows the newest 100
        // alone, so each is asked for by its number.
        for m in &before {
            let t = page(&mut c.browser, &format!("/qsmc/event/{m}"))?;
            if !t.contains("B600060") {
                return Err(format!(
                    "after cut {n} problem {m} is gone: {}",
                    message_of(&t)
                ));
            }
        }
        opened += 1;
    }
    let said = format!(
        "{CHURN_CUTS} cuts while QSHQVS1 posted codes and opened {opened} problems: \
         problems.bin whole each time, no number given twice or lost, {lines} history lines \
         each whole and once"
    );
    Ok((Lab { fleet, c, streams }, said))
}

/// A certificate named `name` and its key, as an import's PEM: its
/// fingerprint as the pages show it.
fn imported(name: &str) -> Result<(String, String), String> {
    let now = u64::try_from(appliance::time::now_secs()).unwrap_or(0);
    let id = x509kit::server::self_signed(name, &["10.0.2.15"], now.saturating_sub(60), 30)
        .map_err(|e| e.to_string())?;
    let key = x509kit::pem::encode(
        "PRIVATE KEY",
        &id.key.to_pkcs8_der().map_err(|e| e.to_string())?,
    );
    let pem = format!(
        "{}{}",
        x509kit::pem::encode("CERTIFICATE", &id.cert),
        key.as_str()
    );
    Ok((pem, webd::tls::colons(&x509kit::crypto::sha256(&id.cert))))
}

fn import(c: &mut Console<'_>, name: &str) -> Result<(), String> {
    let (pem, sha) = imported(name)?;
    let token = c.browser.csrf("/qsmc/console/certs")?;
    let r = c.browser.upload(
        "/qsmc/console/certs",
        &[("csrf", &token), ("pw", "")],
        pem.as_bytes(),
    )?;
    c.browser.pin(&sha)?;
    if shows(&follow(&mut c.browser, &r)?.text(), "QMC8301") {
        Ok(())
    } else {
        Err(format!("{name} was not imported"))
    }
}

/// The certificate the console serves names `prefix`, its pair and its
/// mark whole.
fn cert_whole(c: &mut Console<'_>, prefix: &str) -> Result<String, String> {
    let ch = channel(&mut c.guest)?;
    let web = ch.ls("/data/web")?;
    if let Some(left) = web.iter().find(|n| is(n, "new")) {
        return Err(format!("{left} was left in /data/web"));
    }
    if web.iter().any(|n| n == webd::tls::MADE_FILE) {
        return Err("an imported certificate is marked as the console's own".into());
    }
    let pem = String::from_utf8(ch.cat(CERT)?).map_err(|e| e.to_string())?;
    let b = x509kit::bundle::read_pem(&pem, None).map_err(|e| e.to_string())?;
    let name = b
        .chain
        .first()
        .and_then(|c| c.info.common_name.clone())
        .unwrap_or_default();
    if !name.starts_with(prefix) {
        return Err(format!(
            "the certificate served is {name}, not one imported: a pair was made again"
        ));
    }
    Ok(name)
}

/// Part 2: certificates imported one after another while the power goes.
fn certificates<'a>(
    run: &Run<'a>,
    mut c: Console<'a>,
    moments: &mut Moments,
) -> Result<(Console<'a>, String), String> {
    import(&mut c, "sec-power-first")?;
    let mut imports = 1;
    for n in 0..CERT_CUTS {
        let end = Instant::now() + Duration::from_millis(moments.next_ms(RUN_ON_MS));
        let mut i = 0;
        while Instant::now() < end {
            // The last may be cut short: its answer is not asked for.
            let _ = import(&mut c, &format!("sec-power-{n}-{i}"));
            i += 1;
            imports += 1;
        }
        c = cut(run, c)?;
        cert_whole(&mut c, "sec-power")?;
        let sha = served(&mut c.guest)?;
        c.browser.pin(&sha)?;
    }
    let said = format!(
        "{CERT_CUTS} cuts among {imports} certificate imports: an imported pair served after \
         each, none made again, no half pair or mark left"
    );
    Ok((c, said))
}

/// The web users' IDs, as `webusers.toml` has them.
fn ids(c: &mut Console<'_>) -> Result<BTreeSet<String>, String> {
    let text = channel(&mut c.guest)?.text("/data/state/webusers.toml")?;
    Ok(text
        .lines()
        .filter_map(|l| l.strip_prefix("id = \""))
        .map(|l| l.trim_end_matches('"').to_string())
        .collect())
}

/// Part 3: the power pulled just after a restore is confirmed.
fn restores<'a>(
    run: &Run<'a>,
    mut c: Console<'a>,
    moments: &mut Moments,
) -> Result<(Console<'a>, String), String> {
    let r = submit(
        &mut c.browser,
        "/qsmc/console/backup",
        &[
            ("d", "on"),
            ("w", "file"),
            ("pp", PASS),
            ("back_up_now", "x"),
        ],
    )?;
    if r.status != 200 {
        return Err(format!("no backup came: {}", message_of(&r.text())));
    }
    let file = r.body;
    let backed = ids(&mut c)?;
    let mut said = Vec::new();
    for n in 0..RESTORE_CUTS {
        // A user the backup does not have: gone when it is restored.
        let id = format!("later{n}");
        let pw = "Sec-power-pass-1";
        let r = submit(
            &mut c.browser,
            "/qsmc/console/users",
            &[
                ("id", &id),
                ("role", webusers::Role::Viewer.label()),
                ("password", pw),
                ("again", pw),
                ("create", "OK"),
            ],
        )?;
        if !shows(&follow(&mut c.browser, &r)?.text(), "QMC8001") {
            return Err(format!("{id} was not made"));
        }
        let token = c.browser.csrf("/qsmc/console/backup?tab=restore")?;
        let opened = c.browser.upload(
            "/qsmc/console/backup?tab=restore",
            &[("csrf", &token), ("pp", PASS), ("open", "Open")],
            &file,
        )?;
        if !opened.text().contains("The backup holds") {
            return Err(format!(
                "the backup did not open: {}",
                message_of(&opened.text())
            ));
        }
        // The console restarts at once: the answer may not come.
        let _ = submit(&mut c.browser, "/qsmc/confirm/restore", &[("ok", "OK")]);
        std::thread::sleep(Duration::from_millis(moments.next_ms(1500)));
        c = cut(run, c)?;
        let applied = c.guest.said(&refcode::RESTORE_APPLIED.tag());
        let back = c.guest.said(&refcode::RESTORE_TAKEN_BACK.tag());
        let users = ids(&mut c)?;
        let what = if applied {
            if users != backed {
                return Err(format!("restored, the users are {users:?}, not {backed:?}"));
            }
            "applied"
        } else if back {
            if !users.contains(&id) {
                return Err(format!("taken back, {id} is gone"));
            }
            "taken back"
        } else {
            return Err(format!("after cut {n} the confirmed restore was lost"));
        };
        said.push(what);
    }
    Ok((
        c,
        format!(
            "{RESTORE_CUTS} cuts just after a restore was confirmed: {}, the users as that says",
            said.join(", ")
        ),
    ))
}

/// The suite.
pub fn power(run: &Run<'_>) -> Result<String, String> {
    let seed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(1, |d| u64::from(d.subsec_nanos()) | 1);
    let mut moments = Moments(seed);
    let lab = lab(run, "sec-power", &[])?;
    let (lab, first) = churn(run, lab, &mut moments)?;
    let (c, second) = certificates(run, lab.c, &mut moments)?;
    let (mut c, third) = restores(run, c, &mut moments)?;
    let _ = crate::suite::base::power_off(&mut c.guest);
    Ok(format!("seed {seed}: {first}; {second}; {third}"))
}
