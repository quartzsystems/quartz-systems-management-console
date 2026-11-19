// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! Suites 2, 3 and 13: an attended installation on one blank disk. Over
//! the serial port every screen is held against its golden file; on the
//! display the same answers are typed through QEMU's monitor and the
//! result judged by what was installed. A disk smaller than 16 GiB is
//! shown and cannot be chosen.

use std::time::{Duration, Instant};

use release::fact;

use super::{
    DISK_BYTES, HOST, INSTALL_WITHIN, INSTALLER_ENDED, MAIN_MENU, Media, PASSWORD, SIGN_ON, TITLE,
    USER, blank_disks, boot_installed, display_keys, golden, help, installed_machine, installing,
    is_installed, keep_vars, logged, said, sign_on, titled, titled_with,
};
use crate::qemu::term::key;
use crate::suite::base::{DHCP_ADDRESS, Guest, Run, channel, power_off};

/// Each screen of the installer in order: its golden file, what marks
/// it, and the keys that answer it.
fn screens() -> Vec<(&'static str, &'static str, String)> {
    vec![
        (
            "install-welcome",
            "Press Enter to continue",
            key::ENTER.into(),
        ),
        (
            "install-disks",
            "Type 1 by the load-source disk",
            format!("1{}", key::ENTER),
        ),
        (
            "install-erase",
            "Type ERASE to confirm",
            format!("ERASE{}", key::ENTER),
        ),
        (
            "install-network",
            "Interfaces (blank takes",
            // The host name, the rest of its field cleared; DHCP on the
            // first interface with carrier, as the form stands.
            format!("{HOST}{}{}", key::FIELD_EXIT, key::ENTER),
        ),
        ("install-time", "UTC offset", key::ENTER.into()),
        (
            "install-user",
            "Password again",
            format!(
                "{}{}{PASSWORD}{}{PASSWORD}{}",
                USER.to_lowercase(),
                key::TAB,
                key::TAB,
                key::ENTER
            ),
        ),
        (
            "install-summary",
            "F10 erases the disks named",
            key::F10.into(),
        ),
    ]
}

/// Boot what was installed on `disk` and check it is what was asked.
fn check_installed<'a>(
    run: &Run<'a>,
    disks: &[crate::qemu::session::Disk],
) -> Result<Guest<'a>, String> {
    let machine = installed_machine(run, disks);
    let mut guest = boot_installed(run, &machine, run.launch.timeout)?;
    is_installed(&mut guest, HOST)?;
    let deadline = run.deadline();
    channel(&mut guest)?.until("the open address", deadline, |s| {
        s.fact(fact::MANAGEMENT)
            .is_some_and(|m| m.starts_with(DHCP_ADDRESS))
    })?;
    Ok(guest)
}

/// Suite 2: over the serial port, every screen against its golden file;
/// restart; base ready; sign on.
pub fn serial(run: &Run<'_>) -> Result<String, String> {
    let disks = blank_disks(run, "attended", 1, DISK_BYTES)?;
    let machine = installing(run, &disks, Media::Cdrom)?;
    let mut guest = run.boot(&machine)?;
    let deadline = Instant::now() + INSTALL_WITHIN;
    let mut held = 0;
    for (i, (name, mark, keys)) in screens().into_iter().enumerate() {
        let shown = titled_with(&mut guest, TITLE, mark, deadline)?;
        golden(name, &shown)?;
        held += 1;
        if i == 0 {
            help(&mut guest, name, TITLE, deadline)?;
            held += 1;
        }
        guest.type_keys(&keys)?;
    }
    // The progress display with every step done.
    let progress = titled_with(&mut guest, TITLE, "X  C3004080  Writing state", deadline)?;
    golden("install-progress", &progress)?;
    let complete = titled_with(
        &mut guest,
        TITLE,
        "Remove the installation media.",
        deadline,
    )?;
    golden("install-complete", &complete)?;
    held += 2;
    guest.type_keys(key::ENTER)?;
    guest.seen(INSTALLER_ENDED, deadline)?;
    guest.wait_exit(deadline)?;
    keep_vars(run, &machine)?;
    drop(guest);

    let mut guest = check_installed(run, &disks)?;
    let deadline = run.deadline();
    guest.type_keys(key::ENTER)?;
    let sign = titled(&mut guest, SIGN_ON, deadline)?;
    golden("console-sign-on", &sign)?;
    guest.type_keys(&format!("{USER}{}{PASSWORD}{}", key::TAB, key::ENTER))?;
    let main = titled(&mut guest, MAIN_MENU, deadline)?;
    golden("console-main", &main)?;
    held += 2;
    guest.type_keys(key::F3)?;
    power_off(&mut guest)?;
    Ok(format!(
        "{held} screens equal to their golden files (the installer's {} and its help, \
         progress and completion; sign-on and the main menu); installed {HOST} on one \
         blank disk, restarted, base ready at {DHCP_ADDRESS}, {USER} signed on",
        screens().len()
    ))
}

/// Suite 3: the same answers typed at the display's keyboard through
/// the monitor, judged by what was installed.
pub fn display(run: &Run<'_>) -> Result<String, String> {
    let disks = blank_disks(run, "display", 1, DISK_BYTES)?;
    let mut machine = installing(run, &disks, Media::Cdrom)?;
    // Nothing is typed at the serial port; it shows where the installer
    // is, since the same screens are drawn there.
    machine.keys = false;
    let mut guest = run.boot(&machine)?;
    let deadline = Instant::now() + INSTALL_WITHIN;
    let mut typed = 0;
    for (_, mark, keys) in screens() {
        titled_with(&mut guest, TITLE, mark, deadline)?;
        let keys = display_keys(&keys)?;
        typed += keys.len();
        guest.send_keys(&keys)?;
    }
    titled_with(
        &mut guest,
        TITLE,
        "Remove the installation media.",
        deadline,
    )?;
    guest.send_keys(&["ret".to_string()])?;
    typed += 1;
    guest.seen(INSTALLER_ENDED, deadline)?;
    guest.wait_exit(deadline)?;
    keep_vars(run, &machine)?;
    drop(guest);

    let mut guest = check_installed(run, &disks)?;
    // The display has a session of its own: Enter, the user and the
    // password at its keyboard; the log says who signed on where.
    guest.send_keys(&["ret".to_string()])?;
    std::thread::sleep(Duration::from_millis(700));
    guest.send_keys(&display_keys(&format!(
        "{}{}{PASSWORD}{}",
        USER.to_lowercase(),
        key::TAB,
        key::ENTER
    ))?)?;
    let within = Duration::from_secs(20);
    logged(
        &mut guest,
        &format!("service tools user {USER} signed on at the display"),
        within,
    )?;
    // The serial port has its own meanwhile.
    sign_on(&mut guest, USER, PASSWORD, run.deadline())?;
    guest.type_keys(key::F3)?;
    guest.send_keys(&["f3".to_string()])?;
    logged(
        &mut guest,
        &format!("service tools user {USER} signed off at the display"),
        within,
    )?;
    power_off(&mut guest)?;
    Ok(format!(
        "{typed} keys at the display's keyboard installed {HOST}: base ready at \
         {DHCP_ADDRESS}; {USER} signed on at the display and, at once, at the serial port"
    ))
}

/// Suite 13: a disk smaller than 16 GiB is shown, marked, and refused.
pub fn small_disk(run: &Run<'_>) -> Result<String, String> {
    let mut disks = blank_disks(run, "small", 1, DISK_BYTES - (1 << 30))?;
    disks.extend(blank_disks(run, "large", 1, DISK_BYTES)?);
    disks[1].serial = crate::image::disks::serial(1);
    let machine = installing(run, &disks, Media::Cdrom)?;
    let mut guest = run.boot(&machine)?;
    let deadline = Instant::now() + INSTALL_WITHIN;
    titled_with(&mut guest, TITLE, "Press Enter to continue", deadline)?;
    guest.type_keys(key::ENTER)?;
    let shown = titled_with(
        &mut guest,
        TITLE,
        "Type 1 by the load-source disk",
        deadline,
    )?;
    let row = shown
        .lines()
        .find(|l| l.contains(&disks[1].serial))
        .map(str::to_string);
    let small = shown
        .lines()
        .find(|l| l.contains(" *"))
        .ok_or_else(|| format!("no disk is marked too small:\n{shown}"))?
        .to_string();
    if !shown.contains("Smaller than a load source may be (17.1 GB); cannot be chosen.") {
        return Err(format!("the screen does not say why:\n{shown}"));
    }
    golden("install-disks-small", &shown)?;
    // The small one is first; choosing it is refused.
    guest.type_keys(&format!("1{}", key::ENTER))?;
    let refused = said(&mut guest, "too small to be a load source", deadline)?;
    if !refused.contains("Type 1 by the load-source disk") {
        return Err("the installer left the disks after a refusal".into());
    }
    guest.kill();
    Ok(format!(
        "the disk of 16.1 GB is listed and marked: `{}`; choosing it says `{}`; the disk of \
         17.1 GB is offered: `{}`",
        small.trim(),
        refused.lines().last().map_or("", str::trim),
        row.as_deref().map_or("", str::trim)
    ))
}
