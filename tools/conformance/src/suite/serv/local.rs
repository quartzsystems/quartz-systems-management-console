// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! Suite 11: the local console's option 8 at the serial port: its
//! screens against their golden files, a problem closed, a dump taken,
//! copied to a removable volume and deleted, and the next level installed
//! from the volume and restarted into.

use std::time::{Duration, Instant};

use super::console::{file_name, volume, with_disks};
use super::{CONSOLE_ROW, VS, post_code, qsmc_here, qsmc_next, row};
use crate::qemu::term::key;
use crate::suite::base::{Run, power_off};
use crate::suite::install::{
    Kind, MAIN_MENU, USER, golden, help, option, said, sign_on, titled, titled_with,
};
use crate::suite::mgmt::{page, until};
use crate::suite::web::{ADMIN, Console, PASSWORD, boot, signed_on};

const MENU: &str = "Service Functions";

/// Suite 11.
#[allow(clippy::too_many_lines, reason = "each option of the menu in turn")]
pub fn local(run: &Run<'_>) -> Result<String, String> {
    let next = run
        .image
        .next
        .as_ref()
        .ok_or("the next variant is not built (`build --test-image --variant next all`)")?;
    let name = file_name(next);
    let bytes = std::fs::read(next).map_err(|e| e.to_string())?;
    let vol = volume(run, "local-volume", &[(name.clone(), bytes)])?;
    let vol_path = vol.path.clone();
    let m = with_disks(run, "serv-local", Kind::Single, vec![vol])?;
    let mut c = boot(run, m)?;
    signed_on(&mut c.browser, ADMIN, PASSWORD)?;
    post_code(
        &mut c.guest,
        "A3005030",
        &format!("system:{VS}"),
        &format!("{VS} was not reached for 31 minutes"),
    )?;
    let d = || Instant::now() + Duration::from_secs(90);
    let mut held = 0;
    sign_on(&mut c.guest, USER, PASSWORD, d())?;
    let menu = option(&mut c.guest, 8, MENU, d())?;
    golden("console-service", &menu)?;
    help(&mut c.guest, "console-service", MENU, d())?;
    held += 2;
    // 1: the problem, displayed and closed.
    let list = option(&mut c.guest, 1, "Work with Problems", d())?;
    if !list.contains("A3005030") {
        return Err(format!("the console's problem is not listed:\n{list}"));
    }
    golden("console-problems", &list)?;
    help(&mut c.guest, "console-problems", "Work with Problems", d())?;
    c.guest.type_keys(&format!("5{}", key::ENTER))?;
    let shown = titled(&mut c.guest, "Display Problem", d())?;
    if !shown.contains(VS) {
        return Err(format!("the problem does not name {VS}:\n{shown}"));
    }
    golden("console-problem", &shown)?;
    c.guest.type_keys(key::F12)?;
    titled(&mut c.guest, "Work with Problems", d())?;
    c.guest.type_keys(&format!("4{}", key::ENTER))?;
    let close = titled(&mut c.guest, "Confirm Close Problem", d())?;
    golden("console-problem-close", &close)?;
    c.guest.type_keys(key::F10)?;
    let closed = said(&mut c.guest, "closed.", d())?;
    let n: u64 = closed
        .lines()
        .find_map(|l| l.split("Problem ").nth(1)?.split(' ').next()?.parse().ok())
        .ok_or("the message names no problem")?;
    until(
        "the problem closed in the web interface",
        Duration::from_secs(30),
        || {
            let t = page(&mut c.browser, &format!("/qsmc/event/console/{n}"))?;
            Ok(t.contains("Closed") && t.contains(&format!("service tools user {USER}")))
        },
    )?;
    held += 4;
    c.guest.type_keys(key::F12)?;
    titled(&mut c.guest, MENU, d())?;
    // 2: a dump taken.
    let take = option(&mut c.guest, 2, "Take a Console Dump", d())?;
    golden("console-dump-take", &take)?;
    help(
        &mut c.guest,
        "console-dump-take",
        "Take a Console Dump",
        d(),
    )?;
    held += 2;
    c.guest.type_keys(key::F10)?;
    let begun = said(&mut c.guest, "begun.", d())?;
    let dump = begun
        .split(consdump::KIND.prefix)
        .nth(1)
        .and_then(|r| r.split(".zip").next())
        .map(|r| format!("{}{r}.zip", consdump::KIND.prefix))
        .ok_or("the message names no dump")?;
    until("the dump written", Duration::from_secs(60), || {
        Ok(page(&mut c.browser, "/qsmc/dumps")?.contains(&dump))
    })?;
    // 3: copied to the volume, then deleted.
    let dumps = option(&mut c.guest, 3, "Work with Dumps", d())?;
    if !dumps.contains(&dump) {
        return Err(format!("{dump} is not listed:\n{dumps}"));
    }
    help(&mut c.guest, "console-dumps", "Work with Dumps", d())?;
    c.guest.type_keys(&format!("3{}", key::ENTER))?;
    let copy = titled(&mut c.guest, "Copy Dump to a Removable Volume", d())?;
    if !copy.contains(&dump) {
        return Err(format!(
            "the copy panel does not name {dump}:
{copy}"
        ));
    }
    c.guest
        .type_keys(&format!("vdb{}{}", key::FIELD_EXIT, key::ENTER))?;
    said(&mut c.guest, "copied to vdb", d())?;
    c.guest.type_keys(&format!("4{}", key::ENTER))?;
    titled(&mut c.guest, "Confirm Delete Dump", d())?;
    c.guest.type_keys(key::F10)?;
    said(&mut c.guest, "deleted.", d())?;
    if page(&mut c.browser, "/qsmc/dumps")?.contains(&dump) {
        return Err(format!("{dump} is still listed after its delete"));
    }
    held += 1;
    c.guest.type_keys(key::F12)?;
    titled(&mut c.guest, MENU, d())?;
    // 5: the levels before.
    let levels = option(&mut c.guest, 5, "Display Code Levels", d())?;
    if !levels.contains(&qsmc_here()) {
        return Err(format!(
            "the code levels do not show {}:\n{levels}",
            qsmc_here()
        ));
    }
    help(&mut c.guest, "console-levels", "Display Code Levels", d())?;
    held += 1;
    c.guest.type_keys(key::F12)?;
    titled(&mut c.guest, MENU, d())?;
    // 4: the next level from the volume, then a restart into it.
    let form = option(
        &mut c.guest,
        4,
        "Install an Update from a Removable Volume",
        d(),
    )?;
    golden("console-update-volume", &form)?;
    help(
        &mut c.guest,
        "console-update-volume",
        "Install an Update from a Removable Volume",
        d(),
    )?;
    held += 2;
    c.guest
        .type_keys(&format!("vdb{}{}", key::FIELD_EXIT, key::ENTER))?;
    titled(&mut c.guest, "Choose an Update Image", d())?;
    c.guest.type_keys(&format!("1{}", key::ENTER))?;
    let chosen = titled_with(
        &mut c.guest,
        "Confirm Install Update",
        "Runs now",
        Instant::now() + Duration::from_secs(180),
    )?;
    if !chosen.contains(&qsmc_next()) || !chosen.contains(&qsmc_here()) {
        return Err(format!(
            "the confirmation does not show both levels:\n{chosen}"
        ));
    }
    c.guest.type_keys(key::F10)?;
    titled_with(
        &mut c.guest,
        "Installing Update",
        "is installed and pending",
        Instant::now() + Duration::from_secs(300),
    )?;
    c.guest.type_keys(key::F10)?;
    c.guest
        .wait_exit(Instant::now() + Duration::from_secs(300))?;
    let Console { guest, machine, .. } = c;
    drop(guest);
    let mut c = boot(run, machine)?;
    signed_on(&mut c.browser, ADMIN, PASSWORD)?;
    until(
        "the console confirmed on the next level",
        Duration::from_secs(120),
        || {
            let t = page(&mut c.browser, "/qsmc/updates/code")?;
            Ok(row(&t, CONSOLE_ROW).contains(&format!("{} build", qsmc_next())))
        },
    )?;
    sign_on(&mut c.guest, USER, PASSWORD, d())?;
    option(&mut c.guest, 8, MENU, d())?;
    let levels = option(&mut c.guest, 5, "Display Code Levels", d())?;
    if !levels.contains(&qsmc_next()) || !levels.contains("confirmed") {
        return Err(format!("after the restart the levels say:\n{levels}"));
    }
    c.guest.type_keys(key::F3)?;
    titled(&mut c.guest, MAIN_MENU, d())?;
    power_off(&mut c.guest)?;
    drop(c);
    let image = std::fs::read(&vol_path).map_err(|e| e.to_string())?;
    let copied =
        diskimg::fat::read_file(&image, &dump).map_err(|e| format!("{dump} on the volume: {e}"))?;
    if !copied.starts_with(b"PK") {
        return Err(format!("{dump} on the volume is not an archive"));
    }

    Ok(format!(
        "{held} screens and helps of option 8 held to their golden files; problem {n} displayed \
         and closed at the serial port, closed by service tools user {USER} in the web \
         interface; {dump} taken, copied to the removable volume ({} bytes there) and deleted; \
         {} staged from the volume, shown beside {}, installed and restarted into, then \
         confirmed and shown by option 5",
        copied.len(),
        qsmc_next(),
        qsmc_here()
    ))
}
