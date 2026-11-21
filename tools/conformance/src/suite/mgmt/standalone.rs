// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! Suite 14: a real standalone QS/400. Its disk image (built at the
//! pinned commit, `QSMC_QS400_IMAGE`) runs in a second machine whose QSI
//! port the build machine forwards, so the console reaches it at
//! `10.0.2.2`. QS/400's service console on its serial port drives its
//! display as QS/400's own suite does: its address, QSECOFR signed on,
//! `STRQSIPAIR` for the pairing password. The system is added from the
//! Add page; Servers, its properties, its codes in the history, its
//! console in the terminal window and its control panel follow.

use std::time::{Duration, Instant};

use qsi::host::model::SYSTEM_ID;

use super::{page, until};
use crate::qemu::session::{Disk, Machine, Session, Size};
use crate::suite::web::{ADMIN, PASSWORD, follow, message_of, shows, signed_on, start, submit};
use crate::web::{Browser, free_port};

/// QS/400's service console prompt.
const PROMPT: &str = "QS400-SVC> ";
/// The guest's address on QEMU's user network, where forwards go.
const GUEST: &str = "10.0.2.15";
/// The kernel link TCP/IP takes as a line.
const LINE: &str = "ETH0";
/// QSECOFR's password as QS/400's suites set it.
const SECOFR: &str = "Quartz2026";
/// The last code of QS/400's IPL.
const IPL_COMPLETE: &str = "C6001090";
/// The functions QS/400 lists at ID 0 (`qsisvr::families::panel`).
const FUNCTIONS: [u8; 15] = [1, 3, 8, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22];

/// A service console command: what it printed before the next prompt.
fn svc(s: &mut Session, line: &str, within: Duration) -> Result<String, String> {
    let from = s.log().len();
    s.type_keys(&format!("{line}\n"))?;
    s.wait_for(
        &format!("the prompt after `{line}`"),
        Instant::now() + within,
        |log| log.get(from..).is_some_and(|l| l.contains(PROMPT)),
    )?;
    let out = s.log().get(from..).unwrap_or("");
    Ok(out[..out.find(PROMPT).unwrap_or(out.len())].to_string())
}

fn cmd(s: &mut Session, line: &str) -> Result<String, String> {
    svc(s, line, Duration::from_secs(60))
}

/// The display once it shows one of `wants`: which, and the screen.
fn display(s: &mut Session, wants: &[&str], within: Duration) -> Result<(usize, String), String> {
    let end = Instant::now() + within;
    let mut last = String::new();
    while Instant::now() < end {
        last = cmd(s, "screen")?;
        if let Some(i) = wants.iter().position(|w| last.contains(w)) {
            return Ok((i, last));
        }
        std::thread::sleep(Duration::from_millis(500));
    }
    Err(format!(
        "the display never showed any of {wants:?}:\n{last}"
    ))
}

/// Type at the display, then a key.
fn enter(s: &mut Session, text: &str) -> Result<(), String> {
    cmd(s, &format!("type {text}"))?;
    cmd(s, "key enter").map(|_| ())
}

/// Wait until `svc.tcp what` shows `want`.
fn tcp_until(s: &mut Session, what: &str, want: &str, within: Duration) -> Result<String, String> {
    let end = Instant::now() + within;
    loop {
        let out = cmd(s, &format!("svc.tcp {what}"))?;
        if out.contains(want) {
            return Ok(out);
        }
        if Instant::now() > end {
            return Err(format!("svc.tcp {what}: no {want} in:\n{out}"));
        }
        std::thread::sleep(Duration::from_millis(500));
    }
}

/// The guest's address on the NIC, as QS/400's suites configure it.
fn address(s: &mut Session) -> Result<(), String> {
    tcp_until(s, "ifc", "TCP/IP Active", Duration::from_mins(3))?;
    let row = format!("ifc {GUEST} ");
    if !cmd(s, "svc.tcp ifc")?.lines().any(|l| l.starts_with(&row)) {
        let out = cmd(
            s,
            &format!("ADDTCPIFC INTNETADR('{GUEST}') LIND({LINE}) SUBNETMASK('255.255.255.0')"),
        )?;
        if !out.contains("TCP1D03") {
            return Err(format!("ADDTCPIFC: {out}"));
        }
    }
    let ifc = cmd(s, "svc.tcp ifc")?;
    if !ifc
        .lines()
        .any(|l| l.starts_with(&row) && (l.contains(" Active ") || l.contains(" RCYPND ")))
    {
        let out = cmd(s, &format!("STRTCPIFC INTNETADR('{GUEST}')"))?;
        if !out.contains("TCP1A0B") {
            return Err(format!("STRTCPIFC: {out}"));
        }
    }
    tcp_until(
        s,
        "ifc",
        &format!("kernel addr eth0 {GUEST}/24"),
        Duration::from_secs(30),
    )
    .map(|_| ())
}

/// The value after the colon on the screen's line naming `label`.
fn field(screen: &str, label: &str) -> Option<String> {
    let line = screen.lines().find(|l| l.contains(label))?;
    let (_, v) = line.split_once(':')?;
    let v = v.trim().trim_start_matches('.').trim();
    (!v.is_empty()).then(|| v.to_string())
}

/// QSECOFR signed on at the display, `STRQSIPAIR`'s password read, the
/// password left out, and signed off to Sign On again.
fn pairing_password(s: &mut Session) -> Result<String, String> {
    let set = cmd(
        s,
        &format!("CHGUSRPRF USRPRF(QSECOFR) PASSWORD('{SECOFR}') PWDEXP(*NO)"),
    )?;
    if set.trim_start().starts_with("CPF") {
        return Err(format!("CHGUSRPRF QSECOFR: {set}"));
    }
    // The identity is made as the server first starts.
    let end = Instant::now() + Duration::from_secs(120);
    loop {
        let out = cmd(s, "WRKQSICSL")?;
        if out.contains("System's QSI fingerprint:") && !out.contains("(none yet)") {
            break;
        }
        if Instant::now() > end {
            return Err(format!("the QSI server never made its identity:\n{out}"));
        }
        std::thread::sleep(Duration::from_secs(2));
    }
    display(s, &["Sign On"], Duration::from_mins(2))?;
    cmd(s, "type QSECOFR")?;
    cmd(s, "key tab")?;
    enter(s, SECOFR)?;
    let wants = ["Sign-on Information", "Main Menu", "===>", "not correct"];
    let (mut i, screen) = display(s, &wants, Duration::from_mins(1))?;
    if i == 3 {
        return Err(format!("QSECOFR's sign-on refused:\n{screen}"));
    }
    if i == 0 {
        cmd(s, "key enter")?;
        i = display(s, &wants[1..3], Duration::from_mins(1))?.0 + 1;
    }
    if i != 1 {
        enter(s, "go main")?;
        display(s, &["Main Menu"], Duration::from_mins(1))?;
    }
    enter(s, "STRQSIPAIR")?;
    let (_, screen) = display(s, &["Start QSI Pairing"], Duration::from_mins(1))?;
    let pw = field(&screen, "Pairing password").ok_or_else(|| screen.clone())?;
    // Enter leaves it out.
    cmd(s, "key enter")?;
    display(s, &["Main Menu"], Duration::from_mins(1))?;
    enter(s, "SIGNOFF")?;
    display(s, &["Sign On"], Duration::from_mins(2))?;
    // The server looks for a password out twice a second.
    std::thread::sleep(Duration::from_millis(1500));
    Ok(pw)
}

/// The screen a new window's socket is sent first.
fn shown(b: &Browser, window: &str) -> Result<String, String> {
    let mut ws = b.websocket(&format!("{window}/ws"))?;
    let end = Instant::now() + Duration::from_secs(15);
    while Instant::now() < end && !ws.closed {
        if let Some(s) = ws
            .recv(Duration::from_secs(1))
            .into_iter()
            .find(|m| m.contains("\"t\":\"screen\""))
        {
            return Ok(s);
        }
    }
    Ok(String::new())
}

/// Wait for `wanted` on the window's screen.
fn screen_of(b: &Browser, window: &str, wanted: &str, within: Duration) -> Result<String, String> {
    let end = Instant::now() + within;
    let mut last = String::new();
    while Instant::now() < end {
        match shown(b, window) {
            Ok(s) => last = s,
            Err(e) => last = format!("(the socket: {e})"),
        }
        if last.contains(wanted) {
            return Ok(last);
        }
        std::thread::sleep(Duration::from_secs(5));
    }
    Err(format!("no {wanted:?} on the screen:\n{last}"))
}

/// The codes on the first page of the history.
fn codes(page: &str) -> Vec<String> {
    page.split("/qsmc/refcode/")
        .skip(1)
        .filter_map(|s| s.get(..8))
        .map(str::to_string)
        .collect()
}

/// The control panel's options: each one's number and text.
fn options(page: &str) -> Vec<(u8, String)> {
    page.split("<option")
        .skip(1)
        .filter_map(|s| {
            let text = s.split_once('>')?.1.split('<').next()?;
            let (n, rest) = text.split_once(" - ")?;
            Some((n.trim().parse().ok()?, rest.to_string()))
        })
        .collect()
}

/// Suite 14.
#[allow(clippy::too_many_lines, clippy::many_single_char_names)] // a system, a step at a time
pub fn standalone(run: &crate::suite::base::Run<'_>) -> Result<String, String> {
    let image = std::env::var_os(super::host::QS400)
        .map(std::path::PathBuf::from)
        .ok_or_else(|| {
            format!(
                "{} is not set: build QS/400 at the pinned commit and name its disk image",
                super::host::QS400
            )
        })?;
    let port = free_port()?;
    let unit = run.file("qs400.img");
    driver::disk::sparse_copy(&image, &unit)?;
    let m = Machine {
        disks: vec![Disk {
            path: unit,
            serial: "QS400HD0".into(),
            snapshot: false,
        }],
        vars: run.vars("qs400")?,
        nic: Some("52:54:00:12:34:56".into()),
        forwards: vec![(port, 2300)],
        keys: true,
        hook: None,
        size: Some(Size {
            memory_mb: 4096,
            cpus: 4,
            prealloc: false,
        }),
        ..run.machine()?
    };
    let mut sys = run.boot(&m)?;
    sys.expect(
        &format!("SRC {IPL_COMPLETE}"),
        Instant::now() + Duration::from_mins(20),
    )?;
    let ready = Instant::now() + Duration::from_mins(2);
    sys.expect("Service console ready", ready)?;
    sys.expect(PROMPT, ready)?;
    // What the console says as it attaches comes first.
    std::thread::sleep(Duration::from_secs(3));
    address(&mut sys)?;
    let pw = pairing_password(&mut sys)?;
    // The console, and the system added from the Add page.
    let mut c = start(run, "mgmt-standalone")?;
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
    if !shows(&t, "QMC1001") || !t.contains("Standalone QS/400") {
        let l = super::logs(&mut c.guest, &["mgmtd"]);
        return Err(format!("the system was not added: {}\n{l}", message_of(&t)));
    }
    let name = t
        .split("<th>Name</th><td><span class=\"mono\">")
        .nth(1)
        .and_then(|s| s.split('<').next())
        .unwrap_or("")
        .to_string();
    let r = submit(b, "/qsmc/servers/add", &[("ms", &name), ("add", "Add")])?;
    let t = follow(b, &r)?.text();
    if !shows(&t, "QMC1005") {
        return Err(format!("{name} was not kept: {}", message_of(&t)));
    }
    // Servers, and its properties.
    let servers = page(b, "/qsmc/servers")?;
    if !servers.contains(&name) || !servers.contains("Standalone QS/400") {
        return Err("Servers does not show the system as a standalone QS/400".into());
    }
    let general = page(b, &format!("/qsmc/server/{name}/general"))?;
    if !general.contains("QS/400 version") || !general.contains("Standalone QS/400") {
        return Err(format!("the system's properties:\n{general}"));
    }
    // Its codes in the history, to IPL complete.
    let history = format!("/qsmc/refcodes?ms={name}");
    let mut seen = Vec::new();
    until(
        "the IPL's codes in the history",
        Duration::from_mins(2),
        || {
            seen = codes(&page(b, &history)?);
            Ok(seen.iter().any(|c| c == IPL_COMPLETE))
        },
    )
    .map_err(|e| format!("{e}: {seen:?}"))?;
    // Its console in the terminal window, at Sign On.
    let window = format!("/qsmc/terminal/{name}/{SYSTEM_ID}");
    page(b, &window)?;
    screen_of(b, &window, "Sign On", Duration::from_mins(3))?;
    // The control panel: the system's own list.
    let panel = format!("/qsmc/partition/{name}/{SYSTEM_ID}/panel");
    let listed = options(&page(b, &panel)?);
    let numbers: Vec<u8> = listed.iter().map(|(n, _)| *n).collect();
    if numbers != FUNCTIONS
        || !listed
            .iter()
            .any(|(n, t)| *n == 21 && t.contains("dedicated service tools"))
    {
        return Err(format!("the control panel lists {listed:?}"));
    }
    // 21 at a normal keylock: refused in the system's words.
    let r = submit(
        b,
        &panel,
        &[
            ("func", "21 - Start dedicated service tools"),
            ("select", "Select"),
        ],
    )?;
    let mut t = follow(b, &r)?;
    if let Some(to) = r
        .header("location")
        .filter(|l| l.starts_with("/qsmc/confirm/panel"))
    {
        let to = to.to_string();
        let a = submit(
            b,
            &to,
            &[
                ("ms", &name),
                ("p", &SYSTEM_ID.to_string()),
                ("f", "21"),
                ("ok", "OK"),
            ],
        )?;
        t = follow(b, &a)?;
    }
    let t = t.text();
    if !shows(&t, "QMC5010") || !t.contains("QSI0452") {
        return Err(format!("21 at a normal keylock: {}", message_of(&t)));
    }
    c.guest.kill();
    sys.kill();
    Ok(format!(
        "standalone QS/400 {name} paired from the Add page with STRQSIPAIR's password; shown \
         on Servers as a standalone QS/400 with its properties; {} codes in the history to \
         {IPL_COMPLETE}; its console (ID {SYSTEM_ID}) in the terminal window at Sign On; the \
         control panel lists its {} functions; 21 at a normal keylock refused, QMC5010 with \
         QSI0452",
        seen.len(),
        listed.len()
    ))
}
