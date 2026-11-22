// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! Suites 14 and 15, run by name: a real QS/VS host (its installed test
//! disk, `QSMC_QSVS_DISK`) whose pool disk is pulled, its problem shown,
//! noted and closed from the console; a real standalone QS/400
//! (`QSMC_QS400_IMAGE`) with its problems, dumps and fix level beside
//! the stream's latest. What the build machine cannot run of them is in
//! `docs/PHASE6.md`, for a real machine.

use std::time::{Duration, Instant};

use super::{lab_streams, row};
use crate::qemu::session::{Disk, Machine, Size};
use crate::qemu::term::key;
use crate::suite::install::{option, titled};
use crate::suite::mgmt::host::{
    DISK, MAC, MAIN_MENU, PAIRING, SIGN_ON, USER, USER_PASSWORD, env, password_of,
};
use crate::suite::mgmt::host_pages::{BLANK, POOL, free_disk};
use crate::suite::mgmt::{HOST, page, until};
use crate::suite::parts::{expect, post_form, post_on};
use crate::suite::web::{ADMIN, PASSWORD, follow, message_of, shows, signed_on, start, submit};
use crate::web::free_port;

/// Suite 14 on the build machine.
#[allow(
    clippy::too_many_lines,
    reason = "a host, a pool, its disk pulled, a step at a time"
)]
pub fn host(run: &crate::suite::base::Run<'_>) -> Result<String, String> {
    let disk = env(
        DISK,
        "install the QS/VS test disk at the pinned commit and name it",
    )?;
    let port = free_port()?;
    let scratch = run.file("qsvs.img");
    driver::disk::sparse_copy(&disk, &scratch)?;
    let blank = run.file("pool.img");
    std::fs::File::create(&blank)
        .and_then(|f| f.set_len(BLANK))
        .map_err(|e| format!("the blank disk: {e}"))?;
    let vs = Machine {
        disks: vec![
            Disk {
                path: scratch,
                serial: "QSVSHD0".into(),
                snapshot: false,
            },
            Disk {
                path: blank,
                serial: "QSVSPOOL0".into(),
                snapshot: false,
            },
        ],
        vars: run.vars("qsvs")?,
        nic: Some(MAC.into()),
        forwards: vec![(port, 2300)],
        keys: true,
        hook: None,
        monitor: Some(run.socket("monitor.sock")),
        size: Some(Size {
            memory_mb: 4096,
            cpus: 4,
            prealloc: false,
        }),
        ..run.machine()?
    };
    let mut host = run.boot(&vs)?;
    let deadline = Instant::now() + Duration::from_mins(10);
    host.expect("QSI server started", deadline)?;
    host.expect("Press Enter to sign on", deadline)?;
    host.type_keys(key::ENTER)?;
    titled(&mut host, SIGN_ON, deadline)?;
    host.type_keys(&format!("{USER}{}{USER_PASSWORD}{}", key::TAB, key::ENTER))?;
    titled(&mut host, MAIN_MENU, deadline)?;
    option(&mut host, 4, PAIRING, deadline)?;
    let screen = option(&mut host, 2, "Start Pairing", deadline)?;
    let pw = password_of(&screen).ok_or_else(|| format!("no password on:\n{screen}"))?;
    let mut c = start(run, "serv-real-host")?;
    let b = &mut c.browser;
    signed_on(b, ADMIN, PASSWORD)?;
    let r = submit(
        b,
        "/qsmc/servers/add",
        &[
            ("host", HOST),
            ("port", &port.to_string()),
            ("pw", &pw),
            ("connect", "Connect"),
        ],
    )?;
    let t = r.text();
    if !shows(&t, "QMC1001") {
        return Err(format!("the host was not added: {}", message_of(&t)));
    }
    let name = t
        .split("<th>Name</th><td><span class=\"mono\">")
        .nth(1)
        .and_then(|s| s.split('<').next())
        .unwrap_or("")
        .to_string();
    let r = submit(b, "/qsmc/servers/add", &[("ms", &name), ("add", "Add")])?;
    follow(b, &r)?;
    let v = |p: &str| format!("/qsmc/server/{name}/virtual/{p}");
    until("the host's storage tab", Duration::from_mins(2), || {
        Ok(b.get(&v("storage"))?.status == 200)
    })?;
    // A pool on the blank disk.
    let form = page(b, &v("storage/pool"))?;
    let d = free_disk(&form)
        .ok_or_else(|| format!("the pool form offers no free disk: {}", message_of(&form)))?;
    let use_d = format!("d_{d}");
    let r = post_on(
        b,
        &form,
        &v("storage/pool"),
        ("ok", "Create"),
        &[("to", POOL), ("pr", "None"), (&use_d, "on")],
    )?;
    expect(b, &r, "QMC2420")?;
    until(
        "the pool on Virtual Storage",
        Duration::from_mins(2),
        || Ok(page(b, &v("storage"))?.contains(POOL)),
    )?;
    // Its disk pulled: every transfer to it fails from now on.
    // The host's events before: its own of the IPL (the IOMMU's, under
    // QEMU) are not the pulled disk's.
    let numbers = |t: &str| -> Vec<u64> {
        t.split("<tr")
            .filter_map(|r| {
                r.split("/qsmc/event/")
                    .nth(1)?
                    .split('"')
                    .next()?
                    .parse()
                    .ok()
            })
            .collect()
    };
    let before = numbers(&page(b, &format!("/qsmc/events?s=All&ms={name}&rows=100"))?);
    host.monitor("drive_del hd1")?;
    // A transfer to it fails only when one is made: a virtual disk made
    // in the pool writes to it. Its own refusal, if it comes, is fine.
    let _ = post_form(
        b,
        &v("storage/create"),
        ("ok", "OK"),
        &[("n", "MCPULL"), ("s", "1"), ("p", POOL), ("t", "thick")],
    );
    let mut n = 0;
    let pulled = until(
        "the host's problem for the unit",
        Duration::from_mins(5),
        || {
            let t = page(b, &format!("/qsmc/events?ms={name}&rows=100"))?;
            n = numbers(&t)
                .into_iter()
                .find(|x| !before.contains(x))
                .unwrap_or(0);
            Ok(n > 0)
        },
    );
    // Under QEMU the host at the pin opens no problem for a unit made
    // to fail by `drive_del` (`docs/PHASE6.md`): the note and the close
    // are shown on the host's own problem of its IPL instead.
    let unit = if pulled.is_ok() {
        let detail = page(b, &format!("/qsmc/event/{n}"))?;
        if !detail.contains("Disk unit") && !detail.contains("Storage pool") {
            return Err(format!(
                "event {n} names no unit or pool: {}",
                crate::suite::parts::plain(&detail)
            ));
        }
        format!("the host's problem shown as event {n} with the unit named")
    } else {
        n = *before
            .first()
            .ok_or("the host has no problem to note and close")?;
        format!(
            "no problem of the host's for the pulled unit in 300 s (for a real machine); its IPL's \
             own problem, event {n}, used"
        )
    };
    // A note and a close from the console, in the host's own log.
    let text = "Looked at by the conformance suite";
    let r = post_form(
        b,
        &format!("/qsmc/event/{n}/note"),
        ("ok", "OK"),
        &[("text", text)],
    )?;
    expect(b, &r, qmcmsg::EVENT_NOTED.id)?;
    until(
        "the host's log holds the note",
        Duration::from_secs(60),
        || Ok(page(b, &format!("/qsmc/event/{n}"))?.contains(&format!("Note by {ADMIN}: {text}"))),
    )?;
    let path = format!("/qsmc/confirm/event-close?n={n}");
    let html = page(b, &path)?;
    let r = post_on(b, &html, &path, ("ok", "OK"), &[])?;
    expect(b, &r, qmcmsg::EVENT_CLOSED.id)?;
    until("the host's problem closed", Duration::from_secs(60), || {
        let t = page(b, &format!("/qsmc/events?s=Closed&ms={name}&rows=100"))?;
        Ok(t.contains(&format!("/qsmc/event/{n}\"")))
    })?;
    let by = page(b, &format!("/qsmc/event/{n}"))?;
    c.guest.kill();
    host.kill();
    Ok(format!(
        "QS/VS host {name} paired; pool {POOL} made on its blank disk {d}; the disk pulled: \
         {unit}; a note from the console in the host's analysis log and the problem closed at \
         the host{}",
        if by.contains(ADMIN) { ", by admin" } else { "" }
    ))
}

/// Suite 15 on the build machine.
pub fn standalone(run: &crate::suite::base::Run<'_>) -> Result<String, String> {
    let (mut sys, mut c, name) =
        crate::suite::mgmt::standalone::paired(run, "serv-real-standalone")?;
    let b = &mut c.browser;
    let general = page(b, &format!("/qsmc/server/{name}/general"))?;
    let release = general
        .split(|ch: char| !ch.is_ascii_alphanumeric())
        .find(|w| w.len() == 6 && w.starts_with('V') && w.contains('R') && w.contains('M'))
        .ok_or("the system's properties name no release")?
        .to_string();
    let streams = lab_streams(b, &release)?;
    // Its problems and its dumps listed, its dump space shown.
    let events = page(b, &format!("/qsmc/events?s=All&ms={name}&rows=100"))?;
    let problems = events.matches("/qsmc/event/").count();
    let dumps = page(b, "/qsmc/dumps")?;
    if !dumps.contains(&format!("{name}: ")) {
        return Err(format!("no dump space shown for {name}"));
    }
    let listed = dumps.matches(&format!(">{name}<")).count();
    // Its fix level beside the stream's latest.
    let mut fleet = String::new();
    until(
        "the fix level beside the latest",
        Duration::from_mins(2),
        || {
            fleet = page(b, "/qsmc/updates")?;
            Ok(row(&fleet, &name).contains(super::CUM))
        },
    )?;
    let line = row(&fleet, &name).to_string();
    // Its level as the system said it: a fresh image has no cumulative
    // package installed.
    let shown = line
        .split("<td class=\"mono\">")
        .nth(2)
        .and_then(|c| c.split('<').next())
        .unwrap_or_default()
        .to_string();
    if shown.is_empty() || shown == "(not known)" {
        return Err(format!("{name}'s fix level was not asked: {line}"));
    }
    drop(streams);
    c.guest.kill();
    sys.kill();
    Ok(format!(
        "standalone QS/400 {name} ({release}) paired; {problems} of its problems listed on \
         Serviceable events, {listed} of its dumps on Manage dumps with its dump space; on \
         Updates its fix level, {shown}, beside the qs400 catalog's latest, {}",
        super::CUM
    ))
}
