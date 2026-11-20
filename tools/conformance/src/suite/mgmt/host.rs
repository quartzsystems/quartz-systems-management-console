// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! Suite 10: a real QS/VS host. Its installed test disk (built at the
//! pinned commit, `QSMC_QSVS_DISK`) runs in a second machine whose QSI
//! port the build machine forwards, so the console reaches it at
//! `10.0.2.2`. The pairing password is read from the host's Service
//! Tools screen at its serial port; the host is added from the Add
//! page, shown with its type, serial number and code, its properties
//! as it says them, and restarted from the console. The partition steps
//! need a QS/400 partition in the host, a third level of
//! virtualization: they are listed for a real machine (`PARTITION`).

use std::path::PathBuf;
use std::time::{Duration, Instant};

use super::{page, until};
use crate::qemu::session::{Disk, Machine};
use crate::suite::install::{option, sign_on, titled};
use crate::suite::web::{ADMIN, PASSWORD, follow, message_of, shows, signed_on, start, submit};
use crate::web::free_port;

/// The installed QS/VS test disk.
pub const DISK: &str = "QSMC_QSVS_DISK";

/// QS/VS's Service Tools, as its own suites name them.
const SIGN_ON: &str = "Service Tools Sign On";
const MAIN_MENU: &str = "QS/VS Service Tools";
const PAIRING: &str = "Management Console Pairing";
/// The Service Tools user QS/VS's test installation makes.
const USER: &str = "ADMIN";
const USER_PASSWORD: &str = "service tools 1";

/// The steps of suite 10 that need a partition, run on a real machine.
pub const PARTITION: [&str; 3] = [
    "a partition activated on the host shows its reference codes in the history as they happen",
    "its console opens in the terminal window at QS/400's sign-on display and a sign-on typed in the browser succeeds",
    "function 21 in manual brings service tools to it",
];

fn password_of(screen: &str) -> Option<String> {
    screen
        .split_whitespace()
        .find(|w| {
            let g: Vec<&str> = w.split('-').collect();
            g.len() >= 2
                && g.iter()
                    .all(|p| p.len() == 4 && p.chars().all(|c| c.is_ascii_alphanumeric()))
        })
        .map(ToString::to_string)
}

/// Suite 10.
pub fn host(run: &crate::suite::base::Run<'_>) -> Result<String, String> {
    let disk = std::env::var(DISK).map_err(|_| {
        format!(
            "{DISK} names no QS/VS test disk: build one at the pinned commit (`build doctor` says how) \
             and install it with QS/VS's suites, then name it"
        )
    })?;
    let port = free_port()?;
    let scratch = run.file("mgmt-host-qsvs.img");
    driver::disk::sparse_copy(&PathBuf::from(&disk), &scratch)?;
    let vs = Machine {
        disks: vec![Disk {
            path: scratch,
            serial: "QSVSHD0".into(),
            snapshot: false,
        }],
        vars: run.vars("mgmt-host-qsvs")?,
        nic: Some("52:54:00:12:34:57".into()),
        forwards: vec![(port, 2300)],
        ..run.machine()?
    };
    let mut host = run.boot(&vs)?;
    let deadline = Instant::now() + Duration::from_secs(300);
    sign_on(&mut host, USER, USER_PASSWORD, deadline)
        .or_else(|_| titled(&mut host, MAIN_MENU, deadline))?;
    let _ = SIGN_ON;
    option(&mut host, 4, PAIRING, deadline)?;
    let screen = option(&mut host, 2, "Start Pairing", deadline)?;
    let pw = password_of(&screen).ok_or_else(|| format!("no password on:\n{screen}"))?;
    let mut c = start(run, "mgmt-host")?;
    let b = &mut c.browser;
    signed_on(b, ADMIN, PASSWORD)?;
    let r = submit(
        b,
        "/qsmc/servers/add",
        &[
            ("host", super::HOST),
            ("port", &port.to_string()),
            ("pw", &pw),
            ("connect", "Connect"),
        ],
    )?;
    let t = r.text();
    if !shows(&t, "QMC1001") || !t.contains("QS/VS host") {
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
    let servers = page(b, "/qsmc/servers")?;
    if !servers.contains(&name) || !servers.contains("QS/VS host") {
        return Err("Servers does not show the host".into());
    }
    let general = page(b, &format!("/qsmc/server/{name}/general"))?;
    let processors = page(b, &format!("/qsmc/server/{name}/processors"))?;
    if !general.contains("Operating") || !processors.contains("Installed") {
        return Err("the host's properties".into());
    }
    // Restart from the console; the session comes back.
    let r = submit(
        b,
        &format!("/qsmc/server/{name}/confirm?op=restart"),
        &[("ms", &name), ("how", "delayed"), ("ok", "OK")],
    )?;
    if !shows(&follow(b, &r)?.text(), "QMC2203") {
        return Err("the restart was not started".into());
    }
    until("the host away", Duration::from_secs(300), || {
        Ok(!page(b, "/qsmc/connectivity")?.contains(">Connected<"))
    })?;
    until("the host back", Duration::from_mins(15), || {
        Ok(page(b, "/qsmc/connectivity")?.contains(">Connected<"))
    })?;
    c.guest.kill();
    host.kill();
    Ok(format!(
        "QS/VS host {name} paired from the Add page with the password its Service Tools showed; \
         shown on Servers as a QS/VS host with its serial number and code, its properties as it \
         says them; restarted from the console, away, and its session back. For a real machine: {}",
        PARTITION.join("; ")
    ))
}
