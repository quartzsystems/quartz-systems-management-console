// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! Suites 5, 6 and 7: an installed mirrored load source with either
//! disk removed; the missing unit replaced by a blank disk from option
//! 2 of the local console and rebuilt; mirrored protection started on a
//! single-disk installation from option 2.

use std::time::{Duration, Instant};

use super::{
    DISK_BYTES, HOST, Kind, MAIN_MENU, PASSWORD, USER, blank_disks, boot_installed, golden,
    installed, installed_machine, is_installed, option, sign_on, titled, titled_with,
};
use crate::image::disks;
use crate::qemu::session::Disk;
use crate::qemu::term::key;
use crate::suite::base::{Guest, Run, channel, posted, power_off};

/// The title of option 2.
const DISKS: &str = "Work with Disk Units";
/// How long a rebuild or resynchronization of a whole unit may take.
const REBUILD_WITHIN: Duration = Duration::from_secs(600);

/// The location codes the installation wrote into the set's label, by
/// unit.
fn locations(disk: &Disk) -> Result<Vec<String>, String> {
    let label = disks::label_of(&disk.path)?.ok_or("the disk has no set label")?;
    Ok(label.members.into_iter().map(|m| m.location).collect())
}

/// Wait until both units are Active and nothing is being copied.
fn whole(guest: &mut Guest<'_>, within: Duration) -> Result<stgd::Status, String> {
    let deadline = Instant::now() + within;
    channel(guest)?.storage_until("both units in step", deadline, |s| {
        s.protection == stgd::MIRRORED && s.whole() && s.work.is_empty()
    })
}

/// Suite 5: either disk removed, both ways.
pub fn degraded(run: &Run<'_>) -> Result<String, String> {
    let pair = installed(run, Kind::Mirrored, "degraded")?;
    // Brought in step once, as an installed console is.
    {
        let mut guest =
            boot_installed(run, &installed_machine(run, &pair), run.launch.timeout * 2)?;
        whole(&mut guest, REBUILD_WITHIN)?;
        power_off(&mut guest)?;
    }
    let places = locations(&pair[0])?;
    let mut seen = Vec::new();
    for missing in [1usize, 0] {
        let alone = pair[1 - missing].clone();
        let copy = Disk {
            path: run.file(&format!("alone-{missing}.img")),
            ..alone.clone()
        };
        driver::disk::sparse_copy(&alone.path, &copy.path)?;
        let mut guest = boot_installed(
            run,
            &installed_machine(run, &[copy]),
            run.launch.timeout * 2,
        )?;
        is_installed(&mut guest, HOST)?;
        let place = &places[missing];
        let line = guest
            .log()
            .lines()
            .find(|l| l.contains(&posted(&refcode::UNIT_FAILED, "")))
            .unwrap_or_default()
            .to_string();
        if !line.contains(place.as_str()) {
            return Err(format!(
                "the missing unit {} is not named by its location code {place}: `{line}`",
                missing + 1
            ));
        }
        let storage = channel(&mut guest)?.storage()?;
        let state = storage
            .units
            .iter()
            .find(|u| u.number == u32::try_from(missing + 1).unwrap_or(0))
            .map(|u| u.state.clone())
            .unwrap_or_default();
        if state != "Missing" {
            return Err(format!("unit {} is {state}, not Missing", missing + 1));
        }
        power_off(&mut guest)?;
        seen.push(format!("`{}`", line.trim()));
    }
    Ok(format!(
        "the installed mirror IPLs and runs on either disk alone, naming the other: {}",
        seen.join("; ")
    ))
}

/// Option 2's display of the units, after signing on at the serial port.
fn disk_units(guest: &mut Guest<'_>, deadline: Instant) -> Result<String, String> {
    sign_on(guest, USER, PASSWORD, deadline)?;
    option(guest, 2, DISKS, deadline)
}

/// Suite 6: the missing disk replaced by a blank one from option 2; the
/// rebuild finishes; with the original disk removed the console IPLs.
pub fn replace(run: &Run<'_>) -> Result<String, String> {
    let pair = installed(run, Kind::Mirrored, "replace")?;
    {
        let mut guest =
            boot_installed(run, &installed_machine(run, &pair), run.launch.timeout * 2)?;
        whole(&mut guest, REBUILD_WITHIN)?;
        power_off(&mut guest)?;
    }
    let mut blank = blank_disks(run, "replacement", 1, DISK_BYTES)?.remove(0);
    blank.serial = disks::serial(2);
    let machine = installed_machine(run, &[pair[0].clone(), blank.clone()]);
    let mut guest = boot_installed(run, &machine, run.launch.timeout * 2)?;
    let deadline = run.deadline();
    let units = disk_units(&mut guest, deadline)?;
    if !units.contains("Missing") {
        return Err(format!("option 2 shows no missing unit:\n{units}"));
    }
    golden("console-disks-missing", &units)?;
    guest.type_keys(&format!("2{}", key::ENTER))?;
    let select = titled_with(&mut guest, DISKS, "in the place of unit 2", deadline)?;
    if !select.contains(&blank.serial) {
        return Err(format!("the blank disk is not offered:\n{select}"));
    }
    golden("console-disk-select", &select)?;
    guest.type_keys(&format!("1{}", key::ENTER))?;
    let confirm = titled(&mut guest, "Confirm Erase of Disk", deadline)?;
    golden("console-disk-confirm", &confirm)?;
    guest.type_keys(key::F10)?;
    titled_with(&mut guest, DISKS, "Enter=Continue", deadline)?;
    let storage = whole(&mut guest, REBUILD_WITHIN)?;
    // The rebuild took its time: the screens are given theirs anew.
    let deadline = run.deadline();
    let done = titled_with(&mut guest, DISKS, "Both units are in step.", deadline)?;
    golden("console-disk-rebuilt", &done)?;
    guest.type_keys(key::ENTER)?;
    titled(&mut guest, MAIN_MENU, deadline)?;
    power_off(&mut guest)?;
    drop(guest);

    // The original disk removed: the rebuilt one alone carries the console.
    let mut guest = boot_installed(
        run,
        &installed_machine(run, &[blank.clone()]),
        run.launch.timeout * 2,
    )?;
    is_installed(&mut guest, HOST)?;
    power_off(&mut guest)?;
    let units: Vec<String> = storage
        .units
        .iter()
        .map(|u| format!("unit {} {} {}", u.number, u.serial, u.state))
        .collect();
    Ok(format!(
        "unit 2 missing, {} chosen in option 2 and rebuilt: {}; the console IPLs from it \
         with the original disk removed",
        blank.serial,
        units.join(", ")
    ))
}

/// Suite 7: mirrored protection started on a single-disk installation
/// from option 2, in effect at the next IPL.
pub fn protect(run: &Run<'_>) -> Result<String, String> {
    let one = installed(run, Kind::Single, "protect")?;
    let mut second = blank_disks(run, "protect-second", 1, DISK_BYTES)?.remove(0);
    second.serial = disks::serial(1);
    let both = [one[0].clone(), second];
    let mut guest = boot_installed(run, &installed_machine(run, &both), run.launch.timeout)?;
    let deadline = run.deadline();
    let units = disk_units(&mut guest, deadline)?;
    if !units.contains("None: the load source is one disk") {
        return Err(format!(
            "option 2 does not say the load source is one disk:\n{units}"
        ));
    }
    golden("console-disks-unprotected", &units)?;
    guest.type_keys(&format!("1{}", key::ENTER))?;
    titled_with(
        &mut guest,
        DISKS,
        "to mirror the load source with",
        deadline,
    )?;
    guest.type_keys(&format!("1{}", key::ENTER))?;
    titled(&mut guest, "Confirm Erase of Disk", deadline)?;
    guest.type_keys(key::F10)?;
    titled_with(
        &mut guest,
        DISKS,
        "Mirrored protection starts at the next IPL",
        deadline,
    )?;
    guest.type_keys(key::F3)?;
    // The work is accepted at once and done after: the disk prepared.
    let by = Instant::now() + Duration::from_secs(120);
    let s = channel(&mut guest)?.storage_until("the second disk prepared", by, |s| {
        s.restart_needed || !s.failure.is_empty()
    })?;
    if !s.restart_needed {
        return Err(format!("starting protection failed: {}", s.failure));
    }
    power_off(&mut guest)?;
    drop(guest);

    let mut guest = boot_installed(run, &installed_machine(run, &both), run.launch.timeout * 2)?;
    let storage = whole(&mut guest, REBUILD_WITHIN)?;
    let data = channel(&mut guest)?.status()?;
    let data = data
        .fact(release::fact::DATA)
        .unwrap_or_default()
        .to_string();
    if !data.starts_with("/dev/ublkb") {
        return Err(format!("/data is on {data}, not the protected set"));
    }
    power_off(&mut guest)?;
    Ok(format!(
        "option 2 started protection with QSMCHD1; at the next IPL /data is on {data} and \
         both units are {}",
        storage
            .units
            .iter()
            .map(|u| u.state.as_str())
            .collect::<Vec<_>>()
            .join(" and ")
    ))
}
