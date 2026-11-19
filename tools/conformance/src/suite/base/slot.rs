// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! Suite 9: slot confirmation. Slot B booted on trial confirms and
//! becomes the default in the record on the disk and in the firmware's
//! boot order; with the refuse hook it does not, and the next IPL is on
//! slot A with the `A3` code.

use std::fs::{self, File};
use std::path::Path;

use abrec::Slot;
use diskimg::efi::{self, EspLocation};
use diskimg::layout::ESP_TYPE_GUID;
use loadsource::layout::SLOT_A_PATH;
use loadsource::record::HostRecord;
use release::{NAME, fact, names::SLOT_B_PATH};

use super::{Run, base_ready, fact_of, posted, power_off};
use crate::image::disks::{record_of, set_record};
use crate::qemu::session::Machine;
use crate::qemu::vars;

/// The boot entries the harness writes, as an installer would.
const ENTRY_A: u16 = 0;
const ENTRY_B: u16 = 1;

/// Where the ESP is on the image, by its table.
fn esp(image: &Path) -> Result<EspLocation, String> {
    let mut file = File::open(image).map_err(|e| e.to_string())?;
    let entry = loadsource::gpt::entries(&mut file)
        .map_err(|e| e.to_string())?
        .into_iter()
        .find(|e| e.type_guid == ESP_TYPE_GUID)
        .ok_or("the image has no ESP")?;
    Ok(EspLocation {
        index: entry.index,
        first_lba: entry.first_lba,
        sectors: entry.last_lba + 1 - entry.first_lba,
        guid: entry.guid,
    })
}

/// Slot B on trial: pending in the record with one attempt, and the
/// firmware's `BootNext`, while `BootOrder` has slot A alone.
fn on_trial(run: &Run<'_>, name: &str) -> Result<Machine, String> {
    let disk = run.disk_copy(&format!("{name}-disk"))?;
    set_record(
        &disk.path,
        HostRecord {
            pending: Some(Slot::B),
            attempts: 1,
            ..HostRecord::INITIAL
        },
    )?;
    let esp = esp(&disk.path)?;
    let template = fs::read(&run.launch.vars).map_err(|e| e.to_string())?;
    let store = vars::with_slots(
        &template,
        &efi::load_option(&format!("{NAME} slot A"), &esp, SLOT_A_PATH),
        &efi::load_option(&format!("{NAME} slot B"), &esp, SLOT_B_PATH),
        &[ENTRY_A],
        Some(ENTRY_B),
    )?;
    let store_path = run.file(&format!("{name}-vars.fd"));
    fs::write(&store_path, store).map_err(|e| e.to_string())?;
    Ok(Machine {
        disks: vec![disk],
        vars: store_path,
        ..run.machine()?
    })
}

fn order_of(store: &Path) -> Result<Vec<u16>, String> {
    let bytes = fs::read(store).map_err(|e| e.to_string())?;
    vars::numbers(&bytes, "BootOrder")?.ok_or_else(|| "the firmware has no BootOrder".to_string())
}

/// The slot on trial confirms.
fn confirms(run: &Run<'_>) -> Result<String, String> {
    let machine = on_trial(run, "confirm")?;
    let mut guest = run.boot(&machine)?;
    let deadline = run.deadline();
    guest.expect("IPL slot B (qsmc.slot=B)", deadline)?;
    guest.expect(
        &posted(
            &refcode::RECORD_READ,
            "default A, pending B (0 attempts left); 1 load-source disk",
        ),
        deadline,
    )?;
    base_ready(&mut guest, deadline)?;
    let confirmed = posted(
        &refcode::SLOT_CONFIRMED,
        "slot B is the default; BootOrder 0001,0000",
    );
    // Confirmed when the web server is ready (Phase 3): before or after
    // "Base ready", by which service is ready last.
    guest.seen(&confirmed, deadline)?;
    let status = guest.channel(deadline)?.status()?;
    if fact_of(&status, fact::RECORD)? != "default B, nothing pending" {
        return Err(format!(
            "the status's record is `{}`",
            fact_of(&status, fact::RECORD)?
        ));
    }
    power_off(&mut guest)?;
    drop(guest);

    let (record, _) = record_of(&machine.disks[0].path)?;
    let wanted = HostRecord {
        default_slot: Slot::B,
        pending: None,
        attempts: 0,
        confirmed: [None, Some(release::RELEASE)],
    };
    if record != wanted {
        return Err(format!("the record on the disk is {record:?}"));
    }
    let order = order_of(&machine.vars)?;
    if order.get(..2) != Some(&[ENTRY_B, ENTRY_A]) {
        return Err(format!("the firmware's BootOrder is {order:04X?}"));
    }
    Ok(format!(
        "slot B on trial confirmed: `{confirmed}`; the record has default B and {} in it",
        release::RELEASE
    ))
}

/// The slot on trial does not confirm; the next IPL is back on slot A.
fn falls_back(run: &Run<'_>) -> Result<String, String> {
    let mut machine = on_trial(run, "refuse")?;
    machine.oem = vec![qsmchook::oem_string(qsmchook::REFUSE_CONFIRM)];
    let mut guest = run.boot(&machine)?;
    let deadline = run.deadline();
    guest.expect("IPL slot B (qsmc.slot=B)", deadline)?;
    base_ready(&mut guest, deadline)?;
    guest.seen("TEST HOOK: the slot is not confirmed", deadline)?;
    let status = guest.channel(deadline)?.status()?;
    if fact_of(&status, fact::RECORD)? != "default A, pending B (0 attempts left)" {
        return Err(format!(
            "the record after the refused confirmation is `{}`",
            fact_of(&status, fact::RECORD)?
        ));
    }
    if guest.said(&refcode::SLOT_CONFIRMED.tag()) {
        return Err("the slot was confirmed though the hook refused".into());
    }
    power_off(&mut guest)?;
    drop(guest);
    if order_of(&machine.vars)?.first() != Some(&ENTRY_A) {
        return Err(format!(
            "BootOrder changed without a confirmation: {:04X?}",
            order_of(&machine.vars)?
        ));
    }

    // The firmware used BootNext once: this IPL is on the default.
    machine.oem.clear();
    let mut guest = run.boot(&machine)?;
    let deadline = run.deadline();
    guest.expect("IPL slot A (qsmc.slot=A)", deadline)?;
    let fell_back = posted(&refcode::SLOT_FELL_BACK, "slot B; this IPL is on slot A");
    guest.expect(&fell_back, deadline)?;
    base_ready(&mut guest, deadline)?;
    let status = guest.channel(deadline)?.status()?;
    if fact_of(&status, fact::RECORD)? != "default A, nothing pending" {
        return Err(format!(
            "the record after the fall back is `{}`",
            fact_of(&status, fact::RECORD)?
        ));
    }
    power_off(&mut guest)?;
    drop(guest);
    let (record, _) = record_of(&machine.disks[0].path)?;
    if (record.default_slot, record.pending) != (Slot::A, None) {
        return Err(format!("the record on the disk is {record:?}"));
    }
    Ok(format!(
        "slot B refused to confirm; the next IPL was on slot A with `{fell_back}`"
    ))
}

/// The suite.
pub fn slot_confirm(run: &Run<'_>) -> Result<String, String> {
    Ok(format!("{}; {}", confirms(run)?, falls_back(run)?))
}
