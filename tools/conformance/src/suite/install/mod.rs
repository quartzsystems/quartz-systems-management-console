// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! Phase 2's suites: the installer from the test image's ISO, and the
//! local console of what it installed. Over the serial port each screen
//! is read as a 24x80 terminal and held against its golden file; on the
//! display keys are typed through QEMU's monitor and the result judged
//! by the state it leaves.

pub mod attended;
pub mod console;
pub mod mirror;
pub mod pull;
pub mod refused;
pub mod unattended;

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use super::base::SuiteFn;

use release::fact;

use super::base::{Guest, MAC, Run, channel};
use crate::image::disks;
use crate::qemu::session::{Disk, Machine};
use crate::qemu::term::{Grid, key};

/// Phase 2's suites, numbered as the phase numbers them; suite 1 is
/// every suite of Phases 0 and 1 on the new pins, run before these.
pub const SUITES: &[(&str, SuiteFn)] = &[
    ("install-serial", attended::serial),
    ("install-display", attended::display),
    ("install-unattended", unattended::mirrored),
    ("installed-degraded", mirror::degraded),
    ("replace-unit", mirror::replace),
    ("start-protection", mirror::protect),
    ("install-power-pull", pull::power_pull),
    ("apart-panel", console::apart),
    ("console-network", console::network),
    ("sign-on", console::sign_on_rules),
    ("refused-media", refused::media),
    ("refused-answers", unattended::invalid),
    ("small-disk", attended::small_disk),
    ("console-panels", console::panels),
    ("installed-base", console::installed_base),
    ("no-secrets", console::no_secrets),
];

/// The host name the suites install.
pub const HOST: &str = "mc-01";
/// The service tools user they make, and its password.
pub const USER: &str = "ADMIN";
/// The password; no log, screen or state file may hold it in clear.
pub const PASSWORD: &str = "quartz pass 9";
/// How long an installation may take.
pub const INSTALL_WITHIN: Duration = Duration::from_secs(600);
/// A load-source disk of the suites: the smallest QS/MC takes.
pub const DISK_BYTES: u64 = release::MIN_DISK_BYTES;
/// What PID 1 says when the installer has ended.
pub const INSTALLER_ENDED: &str = "The installer has ended: restarting";
/// The installer's title.
pub const TITLE: &str = "Install QS/MC";
/// The sign-on display's title.
pub const SIGN_ON: &str = "Service Tools Sign On";
/// The main menu's title.
pub const MAIN_MENU: &str = release::CONSOLE_TITLE;

/// The test image's ISO.
pub fn iso<'a>(run: &Run<'a>) -> Result<&'a Path, String> {
    run.image
        .iso
        .as_deref()
        .ok_or_else(|| "the test image has no ISO (`--test-image all`)".into())
}

/// `count` blank disks of `bytes` for this suite, `tag` naming them.
pub fn blank_disks(
    run: &Run<'_>,
    tag: &str,
    count: usize,
    bytes: u64,
) -> Result<Vec<Disk>, String> {
    (0..count)
        .map(|i| {
            let path = run.file(&format!("{tag}-{i}.img"));
            driver::disk::blank(&path, bytes)?;
            Ok(Disk {
                path,
                serial: disks::serial(i),
                snapshot: false,
            })
        })
        .collect()
}

/// Where an installation's media is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Media {
    /// A virtual CD-ROM.
    Cdrom,
    /// The same ISO written to a USB stick.
    Usb,
}

/// A machine that installs onto `disks` from the test image's ISO: a
/// NIC, the serial port typed at, the monitor, the media first.
pub fn installing(run: &Run<'_>, disks: &[Disk], media: Media) -> Result<Machine, String> {
    let iso = iso(run)?.to_path_buf();
    Ok(Machine {
        disks: disks.to_vec(),
        vars: run.vars("installing")?,
        nic: Some(MAC.into()),
        monitor: Some(run.socket("monitor.sock")),
        cdrom: (media == Media::Cdrom).then(|| iso.clone()),
        usb: (media == Media::Usb).then_some(iso),
        keys: true,
        ..Machine::default()
    })
}

/// The machine that boots what was installed on `disks`, with the
/// variable store the installation left: the channel, the monitor, the
/// serial port typed at, a NIC.
pub fn installed_machine(run: &Run<'_>, disks: &[Disk]) -> Machine {
    Machine {
        disks: disks.to_vec(),
        vars: run.file("install-vars.fd"),
        nic: Some(MAC.into()),
        hook: Some(run.socket("hook.sock")),
        monitor: Some(run.socket("monitor.sock")),
        keys: true,
        ..Machine::default()
    }
}

/// A seed volume of `text` beside `machine`'s disks.
pub fn with_seed(run: &Run<'_>, machine: &mut Machine, text: &str) -> Result<(), String> {
    machine.disks.push(super::base::seed_disk(run, text)?);
    Ok(())
}

/// The hash of [`PASSWORD`], made as the installer makes it.
pub fn password_hash() -> Result<String, String> {
    users::hash(PASSWORD).map_err(|e| e.to_string())
}

/// `seed.toml` of an unattended installation onto the disks with the
/// serial numbers `disks`, with the `[host]` lines and `[network]`
/// tables given.
pub fn answers(disks: &[&str], host: &str, network: &str) -> Result<String, String> {
    let disks: Vec<String> = disks.iter().map(|d| format!("\"{d}\"")).collect();
    Ok(format!(
        "schema_version = 2\n\n[host]\n{host}\n{network}\n[install]\ndisks = [{}]\n\
         user_id = \"{}\"\npassword_hash = \"{}\"\n",
        disks.join(", "),
        USER.to_lowercase(),
        password_hash()?
    ))
}

/// Install unattended onto `disks` with `seed`, from `media`: the
/// installer's log as the serial port showed it.
pub fn install_unattended(
    run: &Run<'_>,
    disks: &[Disk],
    seed: &str,
    media: Media,
) -> Result<String, String> {
    let mut machine = installing(run, disks, media)?;
    with_seed(run, &mut machine, seed)?;
    let mut guest = run.boot(&machine)?;
    let deadline = Instant::now() + INSTALL_WITHIN;
    guest.seen(INSTALLER_ENDED, deadline)?;
    guest.wait_exit(deadline)?;
    keep_vars(run, &machine)?;
    Ok(guest.log().to_string())
}

/// Keep the variable store an installation wrote for the boots of what
/// it installed.
pub fn keep_vars(run: &Run<'_>, machine: &Machine) -> Result<(), String> {
    std::fs::copy(&machine.vars, run.file("install-vars.fd"))
        .map(|_| ())
        .map_err(|e| format!("the installation's variable store: {e}"))
}

/// Boot what was installed and wait for base ready and the channel.
pub fn boot_installed<'a>(
    run: &Run<'a>,
    machine: &Machine,
    within: Duration,
) -> Result<Guest<'a>, String> {
    let mut guest = run.boot(machine)?;
    let deadline = Instant::now() + within;
    super::base::base_ready(&mut guest, deadline)?;
    guest.channel(deadline)?;
    Ok(guest)
}

/// Wait for a screen titled `title` on the serial terminal: its text.
pub fn titled(guest: &mut Guest<'_>, title: &str, deadline: Instant) -> Result<String, String> {
    guest.screen(&format!("the screen `{title}`"), deadline, |grid: &Grid| {
        grid.row(1).trim() == title
    })
}

/// Wait for a screen titled `title` with `mark` on it: its text.
pub fn titled_with(
    guest: &mut Guest<'_>,
    title: &str,
    mark: &str,
    deadline: Instant,
) -> Result<String, String> {
    guest.screen(
        &format!("the screen `{title}` with `{mark}`"),
        deadline,
        |grid: &Grid| grid.row(1).trim() == title && grid.contains(mark),
    )
}

/// The message line of the serial terminal now.
pub fn message(guest: &mut Guest<'_>) -> String {
    guest.grid().message().trim().to_string()
}

/// Wait until the message line says `text`.
pub fn said(guest: &mut Guest<'_>, text: &str, deadline: Instant) -> Result<String, String> {
    guest.screen(&format!("the message `{text}`"), deadline, |grid: &Grid| {
        grid.message().contains(text)
    })
}

/// Type the number of a menu option and Enter: the screen titled
/// `title` that follows.
pub fn option(
    guest: &mut Guest<'_>,
    number: u8,
    title: &str,
    deadline: Instant,
) -> Result<String, String> {
    guest.type_keys(&format!("{number}{}", key::ENTER))?;
    titled(guest, title, deadline)
}

/// The keys that fill a form's fields of the lengths `lens`, from the
/// first: `None` leaves a field as it is; a value shorter than its
/// field clears the rest of it.
pub fn fill(values: &[Option<&str>], lens: &[usize]) -> String {
    let mut keys = String::new();
    for (value, len) in values.iter().zip(lens) {
        match value {
            None => keys.push_str(key::TAB),
            Some(value) => {
                keys.push_str(value);
                if value.len() < *len {
                    keys.push_str(key::FIELD_EXIT);
                }
            }
        }
    }
    keys
}

/// Hold a screen against its golden file.
pub fn golden(name: &str, screen: &str) -> Result<(), String> {
    crate::qemu::golden::check(name, screen, &release::RELEASE.to_string())
}

/// F1 on the screen shown: hold its help against `name`'s golden file
/// and leave it with Enter, back on `title`.
pub fn help(
    guest: &mut Guest<'_>,
    name: &str,
    title: &str,
    deadline: Instant,
) -> Result<(), String> {
    guest.type_keys(key::F1)?;
    let shown = guest.screen("a help display", deadline, |grid: &Grid| {
        grid.row(1).trim().starts_with("Help: ")
    })?;
    golden(&format!("{name}-help"), &shown)?;
    guest.type_keys(key::ENTER)?;
    titled(guest, title, deadline).map(|_| ())
}

/// Press Enter at the serial port and sign on: the main menu's text.
pub fn sign_on(
    guest: &mut Guest<'_>,
    user: &str,
    password: &str,
    deadline: Instant,
) -> Result<String, String> {
    guest.type_keys(key::ENTER)?;
    titled(guest, SIGN_ON, deadline)?;
    guest.type_keys(&format!("{user}{}{password}{}", key::TAB, key::ENTER))?;
    titled(guest, MAIN_MENU, deadline)
}

/// The `sendkey` names that type `typed` at the display's keyboard: the
/// serial port's keys as the display has them.
pub fn display_keys(typed: &str) -> Result<Vec<String>, String> {
    let named = [
        (key::ENTER, "ret"),
        (key::TAB, "tab"),
        // Field Exit is the keypad's plus.
        (key::FIELD_EXIT, "kp_add"),
        (key::F10, "f10"),
        (key::F12, "f12"),
        (key::F1, "f1"),
        (key::F3, "f3"),
    ];
    let mut keys = Vec::new();
    let mut rest = typed;
    while !rest.is_empty() {
        if let Some((bytes, name)) = named.iter().find(|(b, _)| rest.starts_with(b)) {
            keys.push((*name).to_string());
            rest = &rest[bytes.len()..];
            continue;
        }
        let c = rest.chars().next().unwrap_or(' ');
        keys.push(match c {
            'a'..='z' | '0'..='9' => c.to_string(),
            'A'..='Z' => format!("shift-{}", c.to_ascii_lowercase()),
            ' ' => "spc".into(),
            '-' => "minus".into(),
            '.' => "dot".into(),
            other => return Err(format!("no key is known for `{other}`")),
        });
        rest = &rest[c.len_utf8()..];
    }
    Ok(keys)
}

/// Wait until the system log has `wanted`.
pub fn logged(guest: &mut Guest<'_>, wanted: &str, within: Duration) -> Result<(), String> {
    let deadline = Instant::now() + within;
    loop {
        let log = channel(guest)?.text("/data/log/system.ndjson")?;
        if log.contains(wanted) {
            return Ok(());
        }
        if Instant::now() > deadline {
            return Err(format!("the system log lacks `{wanted}`"));
        }
        std::thread::sleep(Duration::from_millis(300));
    }
}

/// The installed console is the one asked for: base ready, the host
/// name, slot A, every service ready.
pub fn is_installed(
    guest: &mut Guest<'_>,
    host: &str,
) -> Result<supervise::control::HostStatus, String> {
    let status = channel(guest)?.status()?;
    super::base::all_ready(&status)?;
    let get = |k| super::base::fact_of(&status, k).map(str::to_string);
    if get(fact::HOST)? != host {
        return Err(format!("the host is {}", get(fact::HOST)?));
    }
    if get(fact::SLOT)? != "A" {
        return Err(format!("the IPL is from slot {}", get(fact::SLOT)?));
    }
    Ok(status)
}

/// What a load source a suite installed is: one disk, or a mirrored
/// pair, each with the network the suites give it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// One disk, DHCP.
    Single,
    /// Two disks, DHCP.
    Mirrored,
}

/// A load source of `kind` installed unattended once in this run and
/// copied for each suite that asks: its disks, for the suite `tag`.
pub fn installed(run: &Run<'_>, kind: Kind, tag: &str) -> Result<Vec<Disk>, String> {
    let (kind_name, count) = match kind {
        Kind::Single => ("single", 1),
        Kind::Mirrored => ("mirrored", 2),
    };
    // Kept for the ISO it was installed from: the ISO's name has its hash.
    let from = iso(run)?
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let name = format!("{kind_name}-{from}");
    let kept: Vec<PathBuf> = (0..count)
        .map(|i| run.work.join(format!("installed-{name}-{i}.img")))
        .collect();
    let vars = run.work.join(format!("installed-{name}-vars.fd"));
    if !kept.iter().all(|p| p.is_file()) || !vars.is_file() {
        let fresh = blank_disks(run, &format!("installing-{name}"), count, DISK_BYTES)?;
        let serials: Vec<&str> = fresh.iter().map(|d| d.serial.as_str()).collect();
        let seed = answers(&serials, &format!("name = \"{HOST}\""), "")?;
        install_unattended(run, &fresh, &seed, Media::Cdrom)?;
        for (disk, keep) in fresh.iter().zip(&kept) {
            std::fs::rename(&disk.path, keep).map_err(|e| e.to_string())?;
        }
        std::fs::copy(run.file("install-vars.fd"), &vars).map_err(|e| e.to_string())?;
    }
    std::fs::copy(&vars, run.file("install-vars.fd")).map_err(|e| e.to_string())?;
    kept.iter()
        .enumerate()
        .map(|(i, keep)| {
            let path = run.file(&format!("{tag}-{i}.img"));
            driver::disk::sparse_copy(keep, &path)?;
            Ok(Disk {
                path,
                serial: disks::serial(i),
                snapshot: false,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_form_is_filled_field_by_field() {
        let keys = fill(&[Some("mc-01"), None, Some("1")], &[63, 2, 1]);
        assert_eq!(keys, format!("mc-01{}{}1", key::FIELD_EXIT, key::TAB));
    }

    #[test]
    fn the_display_s_keys_are_the_serial_port_s() {
        let keys = display_keys(&format!("Ab-1.{}{}", key::FIELD_EXIT, key::F10)).unwrap();
        assert_eq!(keys, ["shift-a", "b", "minus", "1", "dot", "kp_add", "f10"]);
        assert!(display_keys("?").is_err());
    }

    #[test]
    fn the_answers_are_a_seed_with_an_install_table() {
        let seed =
            seed::format::parse(&answers(&["QSMCHD0"], "name = \"mc-01\"", "").unwrap()).unwrap();
        let install = seed.install.unwrap();
        assert_eq!(install.disks, ["QSMCHD0"]);
        assert!(users::valid_hash(&install.password_hash));
    }
}
