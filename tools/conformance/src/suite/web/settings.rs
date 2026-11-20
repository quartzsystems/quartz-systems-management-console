// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! Suite 7: Network settings. A change of the `open` interface the
//! browser came in on (DHCP to the same address, static: QEMU forwards
//! to one address) is kept when the browser signs on again within the
//! trial, and taken back when it does not; `private` is added and
//! removed with `open` answering throughout. Suite 8: Certificates. A
//! new self-signed one is served to the next connection; a PEM chain
//! and key imported and served; a key not the certificate's refused.
//! Suite 11: the listeners on `open` only, nothing on `private`, and
//! following a change of the `open` address and its taking back.

use std::time::{Duration, Instant};

use release::fact;

use super::{ADMIN, PASSWORD, follow, message_of, served, shows, signed_on, start, submit};
use crate::suite::base::network::{listening, only_web};
use crate::suite::base::{channel, fact_of};
use crate::web::{Answer, Browser};

/// The second NIC's MAC: the `private` interface.
const PRIVATE_MAC: &str = "52:54:00:12:34:57";
/// What the page gives `private`.
const PRIVATE: &str = "192.168.250.1";
/// How long a change of the browser's address waits to be kept, and a
/// little more.
const TRIAL: Duration = Duration::from_secs(135);

/// Network settings' form with `ip`, `mask`, `gw` for `open` and
/// `pip`, `pmask` for `private`, OK'd once or, with `confirmed`, after
/// its confirmation.
fn form(
    b: &mut Browser,
    open: (&str, &str, &str),
    private: (&str, &str),
    confirmed: bool,
) -> Result<Answer, String> {
    let mut fields = vec![
        ("ip", open.0),
        ("mask", open.1),
        ("gw", open.2),
        ("dns", ""),
        ("h", "mc-01"),
        ("d", ""),
        ("pip", private.0),
        ("pmask", private.1),
        ("allow", ""),
        ("apply", "Apply"),
    ];
    if confirmed {
        fields.push(("confirmed", "yes"));
    }
    submit(b, "/qsmc/console/network", &fields)
}

/// Sign on again while the interface comes back from its change: a
/// connection refused or cut is tried again for half a minute.
fn reached(b: &mut Browser) -> Result<(), String> {
    let end = Instant::now() + Duration::from_secs(30);
    loop {
        match signed_on(b, ADMIN, PASSWORD) {
            Ok(()) => return Ok(()),
            Err(e) if Instant::now() < end && !e.contains("was not signed on") => {
                std::thread::sleep(Duration::from_secs(2));
            }
            Err(e) => return Err(e),
        }
    }
}

fn line(guest: &mut super::Guest<'_>, key: &str) -> Result<String, String> {
    let s = channel(guest)?.status()?;
    Ok(fact_of(&s, key).unwrap_or("").to_string())
}

/// Wait until fact `key` satisfies `ok`.
fn until(
    guest: &mut super::Guest<'_>,
    key: &str,
    within: Duration,
    ok: impl Fn(&str) -> bool,
) -> Result<String, String> {
    let end = Instant::now() + within;
    loop {
        let now = line(guest, key)?;
        if ok(&now) {
            return Ok(now);
        }
        if Instant::now() > end {
            return Err(format!("{key} stayed `{now}`"));
        }
        std::thread::sleep(Duration::from_secs(1));
    }
}

/// Whether the guest's kernel has the local address `a`.
fn has_address(guest: &mut super::Guest<'_>, a: &str) -> Result<bool, String> {
    let trie = channel(guest)?.text("/proc/net/fib_trie")?;
    let lines: Vec<&str> = trie.lines().collect();
    Ok(lines.windows(2).any(|w| {
        w[0].trim_start().trim_start_matches("|-- ") == a && w[1].contains("/32 host LOCAL")
    }))
}

fn mode(guest: &mut super::Guest<'_>) -> Result<String, String> {
    let text = channel(guest)?.text("/data/state/network.toml")?;
    Ok(if text.contains("mode = \"static\"") {
        "static".into()
    } else {
        "dhcp".into()
    })
}

/// The suite of Network settings.
pub fn network(run: &super::Run<'_>) -> Result<String, String> {
    let mut m = super::machine(run, "web-network")?;
    m.second_nic = Some(PRIVATE_MAC.into());
    let mut c = super::boot(run, m)?;
    let b = &mut c.browser;
    signed_on(b, ADMIN, PASSWORD)?;
    let open = ("10.0.2.15", "255.255.255.0", "10.0.2.2");
    // Said first, then on trial; kept when reached again in time.
    let r = form(b, open, ("", ""), false)?;
    if !r.text().contains("Confirm Network Change") {
        return Err(format!(
            "the change was not said first: {}",
            message_of(&r.text())
        ));
    }
    let r = form(b, open, ("", ""), true)?;
    if !shows(&r.text(), "QMC8100") {
        return Err(format!(
            "the change was not tried: {}",
            message_of(&r.text())
        ));
    }
    std::thread::sleep(Duration::from_secs(5));
    let mut again = b.fresh();
    reached(&mut again)?;
    let welcome = again.get("/qsmc/welcome")?.text();
    if !shows(&welcome, "QMC8101") {
        return Err(format!(
            "reached again, the change was not kept: {}",
            message_of(&welcome)
        ));
    }
    std::thread::sleep(Duration::from_secs(3));
    if mode(&mut c.guest)? != "static" {
        return Err("the kept change is not in network.toml".into());
    }
    // Back to DHCP and never reached: taken back after the trial.
    let r = form(&mut again, ("", "", ""), ("", ""), true)?;
    if !shows(&r.text(), "QMC8100") {
        return Err(format!(
            "the change back was not tried: {}",
            message_of(&r.text())
        ));
    }
    std::thread::sleep(TRIAL);
    if mode(&mut c.guest)? != "static" {
        return Err("a change not reached again was kept".into());
    }
    let mut after = again.fresh();
    reached(&mut after)?;
    // `private` added and removed; `open` answers throughout.
    let r = form(&mut after, open, (PRIVATE, "255.255.255.0"), false)?;
    let page = follow(&mut after, &r)?.text();
    if !shows(&page, "QMC8100") {
        return Err(format!("private was not tried: {}", message_of(&page)));
    }
    let up = until(&mut c.guest, fact::PRIVATE, Duration::from_secs(40), |l| {
        l.starts_with(&format!("{PRIVATE}/24"))
    })?;
    if after.get("/qsmc/welcome")?.status != 200 {
        return Err("open stopped answering when private was added".into());
    }
    form(&mut after, open, ("", ""), false)?;
    // The network service notes no line for a role it no longer has
    // (`docs/OPEN_QUESTIONS.md`): the kernel's addresses say it went.
    let end = Instant::now() + Duration::from_secs(40);
    while has_address(&mut c.guest, PRIVATE)? {
        if Instant::now() > end {
            return Err(format!("{PRIVATE} stayed on the private interface"));
        }
        std::thread::sleep(Duration::from_secs(1));
    }
    if channel(&mut c.guest)?
        .text("/data/state/network.toml")?
        .contains("role = \"private\"")
    {
        return Err("network.toml still has a private interface".into());
    }
    if after.get("/qsmc/welcome")?.status != 200 {
        return Err("open stopped answering when private was removed".into());
    }
    let _ = crate::suite::base::power_off(&mut c.guest);
    Ok(format!(
        "DHCP to static 10.0.2.15/24 said first, tried, and kept when signed on again (QMC8101); \
         back to DHCP not reached again and taken back after {} s; private `{up}` added and \
         removed with open answering",
        TRIAL.as_secs()
    ))
}

/// The suite of Certificates.
pub fn certs(run: &super::Run<'_>) -> Result<String, String> {
    let mut c = start(run, "web-certs")?;
    let first = served(&mut c.guest)?;
    signed_on(&mut c.browser, ADMIN, PASSWORD)?;
    // A new self-signed certificate, served to the next connection.
    let r = submit(&mut c.browser, "/qsmc/confirm/cert-new", &[("ok", "OK")])?;
    let made = served(&mut c.guest)?;
    if made == first {
        return Err("the certificate is the same".into());
    }
    // The next connection is served the new one: the old pin fails, and
    // the session goes on under the new.
    if c.browser.get("/qsmc/welcome").is_ok() {
        return Err("a connection pinned to the old certificate succeeded".into());
    }
    c.browser.pin(&made)?;
    if !shows(&follow(&mut c.browser, &r)?.text(), "QMC8300") {
        return Err("no new certificate was said".into());
    }
    // A PEM chain and its key, imported and served.
    let now = u64::try_from(appliance::time::now_secs()).unwrap_or(0);
    let mine = x509kit::server::self_signed("imported.example", &["10.0.2.15"], now, 30)
        .map_err(|e| e.to_string())?;
    let key = x509kit::pem::encode(
        "PRIVATE KEY",
        &mine.key.to_pkcs8_der().map_err(|e| e.to_string())?,
    );
    let pem = format!(
        "{}{}",
        x509kit::pem::encode("CERTIFICATE", &mine.cert),
        key.as_str()
    );
    let token = c.browser.csrf("/qsmc/console/certs")?;
    let r = c.browser.upload(
        "/qsmc/console/certs",
        &[("csrf", &token), ("pw", "")],
        pem.as_bytes(),
    )?;
    let imported = webd::tls::colons(&x509kit::crypto::sha256(&mine.cert));
    if served(&mut c.guest)? != imported {
        return Err("the imported certificate is not the one in use".into());
    }
    c.browser.pin(&imported)?;
    if !shows(&follow(&mut c.browser, &r)?.text(), "QMC8301") {
        return Err("the PEM chain and key were not said imported".into());
    }
    // A key that is not the certificate's: refused, nothing changes.
    let other = x509kit::server::self_signed("other.example", &["10.0.2.15"], now, 30)
        .map_err(|e| e.to_string())?;
    let wrong = format!(
        "{}{}",
        x509kit::pem::encode("CERTIFICATE", &other.cert),
        key.as_str()
    );
    let token = c.browser.csrf("/qsmc/console/certs")?;
    let r = c.browser.upload(
        "/qsmc/console/certs",
        &[("csrf", &token), ("pw", "")],
        wrong.as_bytes(),
    )?;
    if !shows(&r.text(), "QMC8302") {
        return Err(format!(
            "a key not the certificate's: {}",
            message_of(&r.text())
        ));
    }
    if served(&mut c.guest)? != imported {
        return Err("a refused import changed the certificate".into());
    }
    let _ = crate::suite::base::power_off(&mut c.guest);
    Ok(format!(
        "a new self-signed certificate ({}...) served to the next connection and the old one \
         refused; a PEM chain and key imported and served ({}...); a key not the \
         certificate's refused (QMC8302)",
        &made[..11],
        &imported[..11]
    ))
}

fn tcp(guest: &mut super::Guest<'_>) -> Result<(String, String), String> {
    let ch = channel(guest)?;
    Ok((ch.text("/proc/net/tcp")?, ch.text("/proc/net/tcp6")?))
}

/// The suite of the listeners.
pub fn listeners(run: &super::Run<'_>) -> Result<String, String> {
    let mut m = super::machine(run, "web-listeners")?;
    m.second_nic = Some(PRIVATE_MAC.into());
    let mut c = super::boot(run, m)?;
    signed_on(&mut c.browser, ADMIN, PASSWORD)?;
    // `open` as it is, DHCP: only `private` changes.
    form(
        &mut c.browser,
        ("", "", ""),
        (PRIVATE, "255.255.255.0"),
        false,
    )?;
    until(&mut c.guest, fact::PRIVATE, Duration::from_secs(40), |l| {
        l.starts_with(PRIVATE)
    })?;
    let (t, t6) = tcp(&mut c.guest)?;
    let before = only_web(&t, &t6, "10.0.2.15")?;
    // The open address moves, untried by the browser: the listeners
    // follow, and follow back when the trial takes it back.
    let moved = ("10.0.2.16", "255.255.255.0", "10.0.2.2");
    // Its answer may not come back: the address moves under it.
    let _ = form(&mut c.browser, moved, (PRIVATE, "255.255.255.0"), true);
    let end = Instant::now() + Duration::from_secs(30);
    loop {
        let (t, t6) = tcp(&mut c.guest)?;
        if only_web(&t, &t6, "10.0.2.16").is_ok() && !listening(&t, false).is_empty() {
            break;
        }
        if Instant::now() > end {
            return Err(format!(
                "the listeners did not follow to 10.0.2.16: {:?}",
                listening(&t, false)
            ));
        }
        std::thread::sleep(Duration::from_secs(1));
    }
    std::thread::sleep(TRIAL);
    let (t, t6) = tcp(&mut c.guest)?;
    let back = only_web(&t, &t6, "10.0.2.15")?;
    let _ = crate::suite::base::power_off(&mut c.guest);
    Ok(format!(
        "TCP listens on {before:?} with private at {PRIVATE}; moved to 10.0.2.16 and back to \
         {back:?} when the trial took the change back"
    ))
}
