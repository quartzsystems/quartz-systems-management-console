// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! Suite 10: a real QS/VS host. Its installed test disk (built at the
//! pinned commit, `QSMC_QSVS_DISK`) runs in a second machine, with a
//! copy of QS/400's disk image (`QSMC_QS400_IMAGE`) as its first
//! further disk; the build machine forwards its QSI port, so the
//! console reaches it at `10.0.2.2`. The pairing passwords are read
//! from the host's Service Tools at its serial port. The host is added
//! from the Add page and shown with its properties; a partition is made
//! and activated by a second console beside this one (`beside`), since
//! making partitions is Phase 5's; its codes come into the history as
//! it IPLs, its console is signed on in the terminal window, function
//! 21 is refused in normal and brings dedicated service tools in
//! manual; then the host is restarted from the console.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use qsi::base::Keylock;
use qsi::host::model::PartitionState;

use super::beside::Beside;
use super::{connected, page, until};
use crate::qemu::session::{Disk, Machine, Size};
use crate::qemu::term::key;
use crate::suite::install::{option, titled};
use crate::suite::web::{ADMIN, PASSWORD, follow, message_of, shows, signed_on, start, submit};
use crate::web::{Browser, free_port};

/// The installed QS/VS test disk.
pub const DISK: &str = "QSMC_QSVS_DISK";
/// QS/400's disk image, built at the commit QS/VS pins.
pub const QS400: &str = "QSMC_QS400_IMAGE";

/// QS/VS's Service Tools, as its own suites name them.
pub(crate) const SIGN_ON: &str = "Service Tools Sign On";
pub(crate) const MAIN_MENU: &str = "QS/VS Service Tools";
pub(crate) const PAIRING: &str = "Management Console Pairing";
/// The Service Tools user QS/VS's test installation makes.
pub(crate) const USER: &str = "ADMIN";
pub(crate) const USER_PASSWORD: &str = "service tools 1";
/// QS/VS's own harness's NIC.
pub(crate) const MAC: &str = "52:54:00:12:34:56";
/// The partition's name.
const PART: &str = "MCTEST";
/// QSECOFR's password in QS/400's image, and the one it ships with.
const SECOFR: [&str; 2] = ["Quartz2026", "QSECOFR"];
/// The last code of QS/400's IPL.
const IPL_COMPLETE: &str = "C6001090";
/// How long an IPL may take, three levels deep.
const IPL: Duration = Duration::from_mins(40);

pub(crate) fn password_of(screen: &str) -> Option<String> {
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

pub(crate) fn env(name: &str, how: &str) -> Result<PathBuf, String> {
    std::env::var_os(name)
        .map(PathBuf::from)
        .ok_or_else(|| format!("{name} is not set: {how}"))
}

/// The screen a new window's socket is sent first: its rows' text.
pub(super) fn shown(b: &Browser, window: &str) -> Result<String, String> {
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

/// Wait for one of `wanted` on the partition's screen with its
/// keyboard open (a key while it says `X SYSTEM` is lost): its index.
fn screen_of(
    b: &Browser,
    window: &str,
    wanted: &[&str],
    within: Duration,
) -> Result<(usize, String), String> {
    let end = Instant::now() + within;
    let mut last = String::new();
    while Instant::now() < end {
        // A socket refused under load is tried again on the next turn.
        match shown(b, window) {
            Ok(s) => last = s,
            Err(e) => last = format!("(the socket: {e})"),
        }
        let open = last.contains("\"kb\":") && !last.contains("\"kb\":\"X SYSTEM\"");
        if let Some(i) = wanted.iter().position(|w| open && last.contains(w)) {
            return Ok((i, last));
        }
        std::thread::sleep(Duration::from_secs(5));
    }
    Err(format!("none of {wanted:?} on the screen:\n{last}"))
}

/// Keys typed in the window, as its script sends them; once more when
/// the socket is refused before any is sent.
fn typed(b: &Browser, window: &str, keys: &[(&str, &str)]) -> Result<(), String> {
    typed_once(b, window, keys).or_else(|e| {
        eprintln!("the window's socket: {e}; again");
        std::thread::sleep(Duration::from_secs(3));
        typed_once(b, window, keys)
    })
}

fn typed_once(b: &Browser, window: &str, keys: &[(&str, &str)]) -> Result<(), String> {
    let mut ws = b.websocket(&format!("{window}/ws"))?;
    let _ = ws.recv(Duration::from_secs(2));
    for (kind, what) in keys {
        let m = match *kind {
            "type" => format!("{{\"t\":\"type\",\"s\":\"{what}\"}}"),
            _ => format!("{{\"t\":\"key\",\"k\":\"{what}\"}}"),
        };
        ws.send(&m)?;
    }
    let _ = ws.recv(Duration::from_secs(2));
    Ok(())
}

/// QSECOFR signed on in the window, as QS/VS's suites do it at the
/// host: the screen after.
fn signed_on_400(b: &Browser, window: &str) -> Result<String, String> {
    signed_on_with(b, window, &SECOFR)
}

/// QSECOFR signed on in the window with the first of `passwords` that
/// it takes: the screen after.
pub(super) fn signed_on_with(
    b: &Browser,
    window: &str,
    passwords: &[&str],
) -> Result<String, String> {
    let wanted = [
        "===>",
        "Change Password",
        "not correct",
        "Sign-on Information",
    ];
    for &pw in passwords {
        typed(
            b,
            window,
            &[
                ("key", "Home"),
                ("type", "QSECOFR"),
                ("key", "FieldExit"),
                ("type", pw),
                ("key", "FieldExit"),
                ("key", "Enter"),
            ],
        )?;
        let (i, s) = screen_of(b, window, &wanted, Duration::from_mins(3))?;
        match i {
            0 => return Ok(s),
            1 => {
                typed(
                    b,
                    window,
                    &[
                        ("type", pw),
                        ("key", "Tab"),
                        ("type", SECOFR[0]),
                        ("key", "Tab"),
                        ("type", SECOFR[0]),
                        ("key", "Enter"),
                    ],
                )?;
                let (i, s) = screen_of(
                    b,
                    window,
                    &["===>", "Sign-on Information"],
                    Duration::from_mins(3),
                )?;
                if i == 0 {
                    return Ok(s);
                }
                typed(b, window, &[("key", "Enter")])?;
                return screen_of(b, window, &["===>"], Duration::from_mins(3)).map(|(_, s)| s);
            }
            3 => {
                typed(b, window, &[("key", "Enter")])?;
                return screen_of(b, window, &["===>"], Duration::from_mins(3)).map(|(_, s)| s);
            }
            _ => {}
        }
    }
    Err("QSECOFR could not sign on in the window".into())
}

/// Wait until the partition is `state` at the host.
fn reaches(h: &Beside, id: u32, state: PartitionState, within: Duration) -> Result<(), String> {
    let end = Instant::now() + within;
    let mut last = None;
    while Instant::now() < end {
        let s = h.state(id)?;
        if s == state {
            return Ok(());
        }
        last = Some(s);
        std::thread::sleep(Duration::from_secs(5));
    }
    Err(format!("the partition is {last:?}, not {state:?}"))
}

/// The codes on the first page of the partition's history.
pub(super) fn codes(page: &str) -> Vec<String> {
    page.split("/qsmc/refcode/")
        .skip(1)
        .filter_map(|s| s.get(..8))
        .map(str::to_string)
        .collect()
}

/// Suite 10.
#[allow(clippy::too_many_lines, clippy::many_single_char_names)] // a host and a partition, a step at a time
pub fn host(run: &crate::suite::base::Run<'_>) -> Result<String, String> {
    let how = "build both at the pinned commits (`build doctor` says how), install the QS/VS disk \
               with its suites, then name them";
    let disk = env(DISK, how)?;
    let image = env(QS400, how)?;
    let port = free_port()?;
    let scratch = run.file("qsvs.img");
    driver::disk::sparse_copy(&disk, &scratch)?;
    let unit = run.file("qs400.img");
    driver::disk::sparse_copy(&image, &unit)?;
    let vs = Machine {
        disks: vec![
            Disk {
                path: scratch,
                serial: "QSVSHD0".into(),
                snapshot: false,
            },
            Disk {
                path: unit,
                serial: "QSVSUNIT0".into(),
                snapshot: false,
            },
        ],
        vars: run.vars("qsvs")?,
        nic: Some(MAC.into()),
        forwards: vec![(port, 2300)],
        keys: true,
        hook: None,
        size: Some(Size {
            memory_mb: 5120,
            cpus: 6,
            prealloc: true,
        }),
        ..run.machine()?
    };
    let mut host = run.boot(&vs)?;
    let deadline = Instant::now() + Duration::from_mins(10);
    // QS/VS's Service Tools, as its own suites sign on.
    host.expect("QSI server started", deadline)?;
    host.expect("Press Enter to sign on", deadline)?;
    host.type_keys(key::ENTER)?;
    titled(&mut host, SIGN_ON, deadline)?;
    host.type_keys(&format!("{USER}{}{USER_PASSWORD}{}", key::TAB, key::ENTER))?;
    titled(&mut host, MAIN_MENU, deadline)?;
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
    // The harness's own console: a second password, then the partition.
    host.type_keys(key::F12)?;
    titled(&mut host, PAIRING, deadline)?;
    let screen = option(&mut host, 2, "Start Pairing", deadline)?;
    let pw = password_of(&screen).ok_or_else(|| format!("no second password on:\n{screen}"))?;
    let h = Beside::pair("127.0.0.1", port, &pw)?;
    let id = h.partition(PART)?;
    until(
        "the partition at the console",
        Duration::from_mins(1),
        || Ok(page(b, "/qsmc/terminal")?.contains(PART)),
    )?;
    // Its codes come into the history as it IPLs.
    let history = format!("/qsmc/refcodes?ms={name}&p={PART}");
    h.activate(id, None)?;
    let end = Instant::now() + IPL;
    let mut seen: Vec<String> = Vec::new();
    let mut growths = 0;
    let mut running: Option<Instant> = None;
    loop {
        let now = codes(&page(b, &history)?);
        let new = now.iter().filter(|c| !seen.contains(c)).count();
        let up = h.state(id)? == PartitionState::Running;
        if new > 0 && !up {
            growths += 1;
        }
        if up && running.is_none() {
            running = Some(Instant::now());
        }
        for code in now {
            if !seen.contains(&code) {
                seen.push(code);
            }
        }
        if seen.iter().any(|c| c == IPL_COMPLETE) {
            break;
        }
        let late = running.is_some_and(|t| t.elapsed() > Duration::from_secs(90));
        if Instant::now() > end || late {
            let all = qsweb::golden::tokens(&page(b, &format!("/qsmc/refcodes?ms={name}"))?)
                .into_iter()
                .filter_map(|t| match t {
                    qsweb::golden::Tok::Text(x) => Some(x),
                    _ => None,
                })
                .collect::<Vec<_>>()
                .join(" ");
            let l = super::logs(&mut c.guest, &["mgmtd"]);
            return Err(format!(
                "the IPL's codes at the console: {seen:?}; at the host: {:?}\nthe history: {all}\n{l}",
                h.history(id)
            ));
        }
        std::thread::sleep(Duration::from_secs(3));
    }
    if growths < 2 {
        return Err(format!(
            "the history grew {growths} times while the partition IPLed: {seen:?}"
        ));
    }
    reaches(&h, id, PartitionState::Running, Duration::from_mins(5))?;
    // Its console in the terminal window: the sign-on display, signed on.
    let window = format!("/qsmc/terminal/{name}/{id}");
    screen_of(b, &window, &["Sign On"], Duration::from_mins(10))?;
    let after = signed_on_400(b, &window)?;
    // 21: refused in normal, in the host's words.
    let panel = format!("/qsmc/partition/{name}/{id}/panel");
    let r = submit(b, &panel, &[("f", "21 Force DST")])?;
    let t = follow(b, &r)?.text();
    if !shows(&t, "QMC5010") || !shows(&t, "QVS2307") {
        return Err(format!("21 in normal: {}", message_of(&t)));
    }
    // In manual, dedicated service tools on the console.
    h.end(id)?;
    reaches(&h, id, PartitionState::NotActivated, Duration::from_mins(5))?;
    h.activate(id, Some(Keylock::Manual))?;
    let (i, _) = screen_of(b, &window, &["IPL or Install the System", "Sign On"], IPL)?;
    if i == 0 {
        typed(b, &window, &[("type", "1"), ("key", "Enter")])?;
        screen_of(b, &window, &["IPL Options"], Duration::from_mins(10))?;
        typed(b, &window, &[("key", "Enter")])?;
        screen_of(
            b,
            &window,
            &["Set Major System Options"],
            Duration::from_mins(10),
        )?;
        typed(b, &window, &[("key", "Enter")])?;
    }
    reaches(&h, id, PartitionState::Running, IPL)?;
    let r = submit(b, &panel, &[("f", "21 Force DST")])?;
    let t = follow(b, &r)?.text();
    if !shows(&t, "QMC5003") {
        return Err(format!("21 in manual: {}", message_of(&t)));
    }
    screen_of(
        b,
        &window,
        &["Dedicated Service Tools (DST) Sign On"],
        Duration::from_mins(3),
    )?;
    h.end(id)?;
    reaches(&h, id, PartitionState::NotActivated, Duration::from_mins(5))?;
    // Restart from the console; the session comes back.
    let r = submit(
        b,
        &format!("/qsmc/server/{name}/confirm?op=restart"),
        &[("ms", &name), ("how", "delayed"), ("ok", "OK")],
    )?;
    if !shows(&follow(b, &r)?.text(), "QMC2203") {
        return Err("the restart was not started".into());
    }
    // The harness's machine ends at a restart: the host goes, and is
    // started again on its disks, as its firmware would.
    // A delayed restart waits its delay (`power::DELAY`, 300 s) first.
    host.wait_exit(Instant::now() + Duration::from_mins(15))?;
    until("the host away", Duration::from_mins(2), || {
        Ok(!connected(&page(b, "/qsmc/connectivity")?, &name))
    })?;
    let mut host = run.boot(&vs)?;
    until("the host back", Duration::from_mins(15), || {
        Ok(connected(&page(b, "/qsmc/connectivity")?, &name))
    })?;
    c.guest.kill();
    host.kill();
    if !after.contains("===>") {
        return Err("no command line after the sign-on".into());
    }
    Ok(format!(
        "QS/VS host {name} paired from the Add page with the password its Service Tools showed; \
         shown on Servers as a QS/VS host, its properties as it says them; partition {id} made \
         and activated beside it: {} of its codes in the history, which grew {growths} times as \
         it IPLed, to {IPL_COMPLETE}; its console in the terminal window at Sign On, QSECOFR \
         signed on from the browser to a command line; 21 refused in \
         normal (QVS2307), in manual dedicated service tools' sign-on on the console; the host \
         restarted from the console, away, and its session back",
        seen.len()
    ))
}
