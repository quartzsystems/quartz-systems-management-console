// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! The local console of an installed console: suites 9, 10, 11, 14, 15
//! and 16. The panel for the unit to keep; the open network from option
//! 3, on trial; the sign-on rule, idle sign-off and users; every menu
//! panel and its help against its golden file; the base suites on an
//! installed console; nothing listens and no password is in clear.

use std::time::{Duration, Instant};

use release::fact;

use super::{
    HOST, Kind, MAIN_MENU, PASSWORD, SIGN_ON, USER, attended, boot_installed, display_keys, golden,
    help, installed, installed_machine, is_installed, logged, message, option, said, sign_on,
    titled, titled_with,
};
use crate::qemu::session::Disk;
use crate::qemu::term::key;
use crate::suite::base::{self, DHCP_ADDRESS, Guest, Run, TestImage, channel, power_off};

/// Write `text` to `path` on the console through the channel.
fn put(guest: &mut Guest<'_>, path: &str, text: &str) -> Result<(), String> {
    let hex = hex::encode(text.as_bytes());
    channel(guest)?.ask(&format!("{} {path} {hex}", qsmchook::proto::command::PUT))?;
    Ok(())
}

/// Suite 9: both disks of an installed mirror each run alone, then
/// together: the panel for the unit to keep on the display and the
/// serial port; unit 2 chosen at the display's keyboard is kept.
pub fn apart(run: &Run<'_>) -> Result<String, String> {
    let pair = installed(run, Kind::Mirrored, "apart")?;
    {
        let mut guest =
            boot_installed(run, &installed_machine(run, &pair), run.launch.timeout * 2)?;
        let deadline = Instant::now() + Duration::from_secs(600);
        channel(&mut guest)?.storage_until("both units in step", deadline, |s| {
            s.protection == stgd::MIRRORED && s.whole() && s.work.is_empty()
        })?;
        power_off(&mut guest)?;
    }
    // Each alone, leaving a mark only it holds.
    for (unit, disk) in pair.iter().enumerate() {
        let mut guest = boot_installed(
            run,
            &installed_machine(run, std::slice::from_ref(disk)),
            run.launch.timeout * 2,
        )?;
        put(
            &mut guest,
            &format!("/data/alone-{}", unit + 1),
            "ran alone\n",
        )?;
        power_off(&mut guest)?;
    }
    let mut guest = run.boot(&installed_machine(run, &pair))?;
    let deadline = run.deadline_degraded();
    guest.expect(&refcode::UNITS_APART.tag(), deadline)?;
    let panel = titled(&mut guest, panels::keep::TITLE, deadline)?;
    golden("keep-unit", &panel)?;
    let asked = channel(&mut guest)?.text("/run/keep-unit.log")?;
    if !(asked.contains("serial terminal") && asked.contains("asked for on")) {
        return Err(format!(
            "the panel was not on the display and the serial port: {asked}"
        ));
    }
    // Unit 2, at the display's keyboard: its field is the second.
    guest.send_keys(&display_keys(&format!("{}1{}", key::TAB, key::ENTER))?)?;
    let confirm = titled(&mut guest, "Confirm Unit to Keep", deadline)?;
    if !confirm.contains("Unit 2") {
        return Err(format!("the confirmation is not of unit 2:\n{confirm}"));
    }
    golden("keep-unit-confirm", &confirm)?;
    guest.send_keys(&["f10".to_string()])?;
    base::base_ready(&mut guest, deadline)?;
    let names = channel(&mut guest)?.ls("/data")?;
    if !names.iter().any(|n| n == "alone-2") || names.iter().any(|n| n == "alone-1") {
        return Err(format!("/data holds {names:?}, not unit 2's alone"));
    }
    power_off(&mut guest)?;
    Ok(format!(
        "`{}`; the panel on the display and the serial port ({}); unit 2 chosen at the \
         display's keyboard: /data holds what only unit 2 had",
        refcode::UNITS_APART.line(),
        asked
            .lines()
            .find(|l| l.contains("asked for on"))
            .unwrap_or("")
            .trim()
    ))
}

/// The lengths of the network form's fields: host name, interface,
/// address mode, address, prefix, gateway, VLAN, name servers.
const NETWORK_FIELDS: [usize; 8] = [40, 2, 1, 15, 2, 15, 4, 46];

/// The keys of the open interface's form: the host name and the
/// interface left, then the address mode, address, prefix, gateway,
/// VLAN and name servers, each field cleared past what is typed.
fn form(mode: &str, address: &str, prefix: &str, gateway: &str, vlan: &str) -> String {
    let values = [
        None,
        None,
        Some(mode),
        Some(address),
        Some(prefix),
        Some(gateway),
        Some(vlan),
        Some(""),
    ];
    format!("{}{}", super::fill(&values, &NETWORK_FIELDS), key::ENTER)
}

/// Wait until the open line starts with `address`.
fn open_is(guest: &mut Guest<'_>, address: &str, within: Duration) -> Result<String, String> {
    let deadline = Instant::now() + within;
    let status = channel(guest)?.until(&format!("open at {address}"), deadline, |s| {
        s.fact(fact::MANAGEMENT)
            .is_some_and(|m| m.starts_with(address))
    })?;
    Ok(status
        .fact(fact::MANAGEMENT)
        .unwrap_or_default()
        .to_string())
}

/// Suite 10: static to DHCP and back from option 3, without a restart;
/// a change that loses the network is put back after its trial.
pub fn network(run: &Run<'_>) -> Result<String, String> {
    let disk = installed(run, Kind::Single, "network")?;
    let mut guest = boot_installed(run, &installed_machine(run, &disk), run.launch.timeout)?;
    open_is(&mut guest, DHCP_ADDRESS, Duration::from_secs(30))?;
    let deadline = run.deadline();
    sign_on(&mut guest, USER, PASSWORD, deadline)?;
    let menu = option(&mut guest, 3, "Configure Network", deadline)?;
    golden("console-network", &menu)?;
    help(&mut guest, "console-network", "Configure Network", deadline)?;
    let shown = option(&mut guest, 1, "Configure Management Network", deadline)?;
    golden("console-network-open", &shown)?;

    // Static.
    guest.type_keys(&form("2", "10.0.2.21", "24", "10.0.2.2", ""))?;
    let kept = titled_with(
        &mut guest,
        "Configure Management Network",
        "is kept",
        deadline,
    )?;
    golden("console-network-kept", &kept)?;
    guest.type_keys(key::ENTER)?;
    said(
        &mut guest,
        "Management network configuration changed.",
        deadline,
    )?;
    let fixed = open_is(&mut guest, "10.0.2.21/24", Duration::from_secs(30))?;

    // DHCP again: a kept change is back on the network menu.
    option(&mut guest, 1, "Configure Management Network", deadline)?;
    guest.type_keys(&form("1", "", "", "", ""))?;
    titled_with(
        &mut guest,
        "Configure Management Network",
        "is kept",
        deadline,
    )?;
    guest.type_keys(key::ENTER)?;
    said(
        &mut guest,
        "Management network configuration changed.",
        deadline,
    )?;
    let dhcp = open_is(&mut guest, DHCP_ADDRESS, Duration::from_secs(30))?;

    // DHCP on a VLAN nobody answers on: no address, put back.
    option(&mut guest, 1, "Configure Management Network", deadline)?;
    guest.type_keys(&form("1", "", "", "", "999"))?;
    let trying = titled_with(
        &mut guest,
        "Configure Management Network",
        "restored in",
        deadline,
    )?;
    golden("console-network-trial", &trying)?;
    let back = Instant::now() + Duration::from_secs(60);
    let restored = titled_with(
        &mut guest,
        "Configure Management Network",
        "was not kept",
        back,
    )?;
    golden("console-network-restored", &restored)?;
    guest.type_keys(key::ENTER)?;
    let after = open_is(&mut guest, DHCP_ADDRESS, Duration::from_secs(30))?;
    let network = channel(&mut guest)?.text("/data/state/network.toml")?;
    if network.contains("999") {
        return Err("the change that was put back is in network.toml".into());
    }
    guest.type_keys(key::F3)?;
    power_off(&mut guest)?;
    Ok(format!(
        "from option 3 without a restart: static `{fixed}`, DHCP `{dhcp}`; DHCP on VLAN 999 \
         got no address and was put back after its trial: `{after}`, network.toml untouched"
    ))
}

/// Type a user and password at the sign-on display: its message.
fn attempt(
    guest: &mut Guest<'_>,
    user: &str,
    password: &str,
    deadline: Instant,
) -> Result<String, String> {
    titled(guest, SIGN_ON, deadline)?;
    guest.type_keys(&format!(
        "{user}{}{password}{}{}",
        key::FIELD_EXIT,
        key::FIELD_EXIT,
        key::ENTER
    ))?;
    std::thread::sleep(Duration::from_millis(400));
    let shown = guest.screen("the sign-on's answer", deadline, |g| {
        !g.message().trim().is_empty() || g.row(1).trim() == MAIN_MENU
    })?;
    Ok(shown.lines().nth(23).unwrap_or("").trim().to_string())
}

/// Suite 11: a wrong password refused, the ID disabled after five, idle
/// sign-off, users added and removed, the last one kept.
pub fn sign_on_rules(run: &Run<'_>) -> Result<String, String> {
    let disk = installed(run, Kind::Single, "signon")?;
    let mut machine = installed_machine(run, &disk);
    machine.oem = vec![qsmchook::oem_string("short-idle")];
    let mut guest = boot_installed(run, &machine, run.launch.timeout)?;
    let deadline = run.deadline();
    // Users: add, remove, the last one kept.
    sign_on(&mut guest, USER, PASSWORD, deadline)?;
    let users = option(&mut guest, 5, "Work with Service Tools User IDs", deadline)?;
    golden("console-users", &users)?;
    guest.type_keys(key::F6)?;
    let add = titled(&mut guest, "Add Service Tools User ID", deadline)?;
    golden("console-user-add", &add)?;
    // A password may not hold its user ID.
    let second = "other pw 44";
    guest.type_keys(&format!(
        "second{}{second}{}{second}{}",
        key::TAB,
        key::TAB,
        key::ENTER
    ))?;
    said(&mut guest, "User ID SECOND added.", deadline)?;
    guest.type_keys(&format!("{}4{}", key::TAB, key::ENTER))?;
    titled(&mut guest, "Confirm Remove of User ID", deadline)?;
    guest.type_keys(key::F10)?;
    said(&mut guest, "User ID SECOND removed.", deadline)?;
    guest.type_keys(&format!("4{}", key::ENTER))?;
    titled(&mut guest, "Confirm Remove of User ID", deadline)?;
    guest.type_keys(key::F10)?;
    said(&mut guest, "The last user ID cannot be removed.", deadline)?;
    guest.type_keys(key::F6)?;
    titled(&mut guest, "Add Service Tools User ID", deadline)?;
    guest.type_keys(&format!(
        "second{}{second}{}{second}{}",
        key::TAB,
        key::TAB,
        key::ENTER
    ))?;
    said(&mut guest, "User ID SECOND added.", deadline)?;
    guest.type_keys(key::F3)?;
    guest.type_keys(key::F3)?;
    std::thread::sleep(Duration::from_secs(1));

    // A wrong password, then five: disabled even for the right one.
    guest.type_keys(key::ENTER)?;
    let wrong = attempt(&mut guest, USER, "not the one 1", deadline)?;
    if wrong != "User ID or password not correct." {
        return Err(format!("a wrong password said `{wrong}`"));
    }
    for _ in 0..4 {
        attempt(&mut guest, USER, "not the one 1", deadline)?;
    }
    let disabled = attempt(&mut guest, USER, PASSWORD, deadline)?;
    if !disabled.starts_with("User ID disabled after 5 wrong passwords.") {
        return Err(format!("the sixth attempt said `{disabled}`"));
    }
    // Another ID signs on and goes quiet: signed off.
    attempt(&mut guest, "second", second, deadline)?;
    titled(&mut guest, MAIN_MENU, deadline)
        .map_err(|e| format!("SECOND did not sign on: {e}; `{}`", message(&mut guest)))?;
    logged(
        &mut guest,
        "service tools user SECOND signed off after no key for the idle time at the serial port",
        Duration::from_secs(60),
    )?;
    let refused = channel(&mut guest)?.text("/data/log/system.ndjson")?;
    let count = refused
        .matches("sign-on refused at the serial port")
        .count();
    power_off(&mut guest)?;
    Ok(format!(
        "a wrong password: `{wrong}`; after five: `{disabled}`; {count} refusals logged; \
         SECOND added, removed, added again; ADMIN, the last, not removed; SECOND signed off \
         after the idle time (15 s in a test image's short-idle hook)"
    ))
}

/// The main menu's options, each with its panel's title and golden file.
const PANELS: [(u8, &str, &str); 9] = [
    (1, "Display Console Status", "console-status"),
    (2, "Work with Disk Units", "console-disks"),
    (3, "Configure Network", "console-network"),
    (4, "Work with the Web Interface", "console-web"),
    (5, "Work with Service Tools User IDs", "console-users"),
    (6, "Date and Time", "console-time"),
    (7, "Display Logs", "console-logs"),
    (8, "Service Functions", "console-service"),
    (9, "Power Off or Restart", "console-power"),
];

/// Suite 14: every menu panel and its help against its golden file,
/// the two placeholders among them.
pub fn panels(run: &Run<'_>) -> Result<String, String> {
    let disk = installed(run, Kind::Single, "panels")?;
    let mut guest = boot_installed(run, &installed_machine(run, &disk), run.launch.timeout)?;
    let deadline = run.deadline();
    let mut held = 0;
    let main = sign_on(&mut guest, USER, PASSWORD, deadline)?;
    golden("console-main", &main)?;
    help(&mut guest, "console-main", MAIN_MENU, deadline)?;
    held += 2;
    for (number, title, name) in PANELS {
        let shown = option(&mut guest, number, title, deadline)?;
        // The status display's rows move; its frame is held.
        if number != 1 {
            golden(name, &shown)?;
            held += 1;
        }
        help(&mut guest, name, title, deadline)?;
        held += 1;
        match number {
            7 => {
                for (n, log, file) in [
                    (1, "Display Reference Code History", "console-log-history"),
                    (2, "Display System Log", "console-log-system"),
                ] {
                    let shown = option(&mut guest, n, log, deadline)?;
                    if !shown.contains("Show: 1=All") {
                        return Err(format!("{log} is not shown:\n{shown}"));
                    }
                    help(&mut guest, file, log, deadline)?;
                    held += 1;
                    guest.type_keys(key::F12)?;
                    titled(&mut guest, title, deadline)?;
                }
            }
            9 => {
                let confirm = option(&mut guest, 2, "Confirm Restart", deadline)?;
                golden("console-restart", &confirm)?;
                help(&mut guest, "console-restart", "Confirm Restart", deadline)?;
                held += 2;
                guest.type_keys(key::F12)?;
                titled(&mut guest, title, deadline)?;
            }
            _ => {}
        }
        guest.type_keys(key::F12)?;
        titled(&mut guest, MAIN_MENU, deadline)?;
    }
    guest.type_keys(key::F3)?;
    power_off(&mut guest)?;
    Ok(format!(
        "{held} screens and helps equal to their golden files: the main menu, its nine \
         panels (the web interface's and the service functions' placeholders naming Phases 3 \
         and 6), the logs, the restart's confirmation"
    ))
}

/// The base suites that also run on an installed console, and why one
/// does not.
const ON_INSTALLED: &[&str] = &[
    "status",
    "dhcp",
    "supervision",
    "power-pull",
    "mirror",
    "apart",
    "slot-confirm",
    "shutdown",
    "identity",
    "processor",
];

/// `time` on an installed console: its time server is the installer's
/// answer, since an installed console reads no seed. The responder is
/// started first and the console installed with it.
fn installed_time(run: &Run<'_>) -> Result<String, String> {
    let (server, responder) = base::time::responder_ahead()?;
    let disks = super::blank_disks(run, "timed", 1, super::DISK_BYTES)?;
    let seed = super::answers(
        &["QSMCHD0"],
        &format!("name = \"{HOST}\"\nntp_servers = [\"{server}\"]"),
        "",
    )?;
    super::install_unattended(run, &disks, &seed, super::Media::Cdrom)?;
    let mut guest = boot_installed(run, &installed_machine(run, &disks), run.launch.timeout)?;
    guest.seen(
        &crate::suite::base::posted(&refcode::TIME_SYNCHRONIZED, &server),
        run.deadline(),
    )?;
    let now: i64 = channel(&mut guest)?
        .status()?
        .fact(fact::TIME)
        .and_then(|t| t.parse().ok())
        .ok_or("the status has no time")?;
    responder.stop();
    power_off(&mut guest)?;
    let off = base::time::off_ahead(now);
    if off.abs() > 10 {
        return Err(format!("the clock is {off} s from the responder's"));
    }
    Ok(format!(
        "time from the installer's server {server}, {off} s off"
    ))
}

/// Suite 15: Phase 1's suites on an installed console as on the built
/// image. `static-vlan` is left out: it shows a seed applied to a
/// console that has no configuration, and an installed console has one.
/// `time` gives its server the way an installed console takes it.
/// `sockets` names its time servers in a seed too; that nothing listens
/// on an installed console is suite 16's.
pub fn installed_base(run: &Run<'_>) -> Result<String, String> {
    let timed = installed_time(run).map_err(|e| format!("time on the installed console: {e}"))?;
    let disk: Vec<Disk> = installed(run, Kind::Single, "base-installed")?;
    let image = TestImage {
        disk: disk[0].path.clone(),
        ..run.image.clone()
    };
    let mut passed = Vec::new();
    for (name, suite) in base::SUITES
        .iter()
        .filter(|(n, _)| ON_INSTALLED.contains(n))
    {
        let inner = Run {
            name,
            image: &image,
            work: run.work,
            launch: run.launch,
            log: run.log,
        };
        suite(&inner).map_err(|e| format!("{name} on the installed console: {e}"))?;
        passed.push(*name);
    }
    Ok(format!(
        "on a console installed from the ISO: {} passed; time: {timed}; static-vlan and \
         sockets left out (each needs a console with no configuration; suite 16 shows \
         only the web server listens on an installed one)",
        passed.join(", ")
    ))
}

/// Suite 16: only the web server listens; no password is in any
/// log, screen or state file in clear.
pub fn no_secrets(run: &Run<'_>) -> Result<String, String> {
    // An attended installation types the password at the serial port.
    attended::serial(run)?;
    let disk = installed(run, Kind::Single, "secrets")?;
    let mut guest = boot_installed(run, &installed_machine(run, &disk), run.launch.timeout)?;
    is_installed(&mut guest, HOST)?;
    sign_on(&mut guest, USER, PASSWORD, run.deadline())?;
    // Only the web server listens, on the open address; UDP only
    // DHCP's port.
    let open = channel(&mut guest)?
        .status()?
        .fact(release::fact::MANAGEMENT)
        .and_then(|l| l.split_whitespace().next())
        .and_then(|a| a.split('/').next())
        .unwrap_or("")
        .to_string();
    let tcp = channel(&mut guest)?
        .text("/proc/net/tcp")
        .unwrap_or_default();
    let tcp6 = channel(&mut guest)?
        .text("/proc/net/tcp6")
        .unwrap_or_default();
    crate::suite::base::network::only_web(&tcp, &tcp6, &open)?;
    let mut ports = Vec::new();
    for table in ["/proc/net/udp", "/proc/net/udp6"] {
        let text = channel(&mut guest)?.text(table).unwrap_or_default();
        for line in text.lines().skip(1) {
            if let Some(port) = line
                .split_whitespace()
                .nth(1)
                .and_then(|local| local.rsplit(':').next())
                .and_then(|hex| u16::from_str_radix(hex, 16).ok())
            {
                ports.push(port);
            }
        }
    }
    if ports.iter().any(|p| *p != 68 && *p < 32768) {
        return Err(format!("UDP bound to {ports:?}"));
    }
    // No password in clear.
    let mut looked = 0;
    let mut files = Vec::new();
    for dir in ["/data/log", "/data/state", "/data/log/kernel"] {
        for name in channel(&mut guest)?.ls(dir).unwrap_or_default() {
            files.push(format!("{dir}/{name}"));
        }
    }
    for file in &files {
        let bytes = channel(&mut guest)?.cat(file).unwrap_or_default();
        looked += 1;
        if String::from_utf8_lossy(&bytes).contains(PASSWORD) {
            return Err(format!("{file} holds the password in clear"));
        }
    }
    guest.type_keys(key::F3)?;
    power_off(&mut guest)?;
    drop(guest);
    let serial = run.log.borrow();
    if serial.contains(PASSWORD) {
        return Err("a serial log holds the password in clear".into());
    }
    let mut goldens = 0;
    for entry in std::fs::read_dir(crate::qemu::golden::dir()).map_err(|e| e.to_string())? {
        let text =
            std::fs::read_to_string(entry.map_err(|e| e.to_string())?.path()).unwrap_or_default();
        goldens += 1;
        if text.contains(PASSWORD) {
            return Err("a golden screen holds the password".into());
        }
    }
    Ok(format!(
        "TCP listens only on 443 and 80 of the open address, UDP bound only to {ports:?}; \
         the password typed at the \
         installer and at sign-on is in none of {looked} log and state files, the serial \
         logs of this suite's {} bytes, or {goldens} golden screens",
        serial.len()
    ))
}
