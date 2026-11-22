// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! Suite 12, run by name: a real QS/VS host (its installed test disk,
//! `QSMC_QSVS_DISK`, under QEMU) with three partitions, a switch and
//! virtual disks made from the console's own pages; a plan captured;
//! the partitions, their disks and the switch deleted on their own
//! pages; the plan deployed to the same host; every definition and
//! profile read back against the plan, MAC addresses and serial numbers
//! included; the partitions activated. What differs is reported, not
//! hidden: `docs/PHASE7.md` records it.

use std::time::{Duration, Instant};

use super::{create, deploy as wizard, ended, read, with_status};
use crate::qemu::session::{Disk, Machine, Size};
use crate::qemu::term::key;
use crate::suite::install::{option, titled};
use crate::suite::mgmt::host::{
    DISK, MAC, MAIN_MENU, PAIRING, SIGN_ON, USER, USER_PASSWORD, env, password_of,
};
use crate::suite::mgmt::host_pages::{BLANK, POOL, free_disk};
use crate::suite::mgmt::{HOST, page, until};
use crate::suite::parts::{expect, field, post_form, post_on};
use crate::suite::web::{ADMIN, PASSWORD, follow, message_of, shows, signed_on, start, submit};
use crate::web::{Browser, free_port};

const SWITCH: &str = "PLANSW";
const PARTS: [&str; 3] = ["PLANP1", "PLANP2", "PLANP3"];

fn step(n: usize) -> String {
    format!("/qsmc/partitions/create/step/{n}")
}

fn next(b: &mut Browser, n: usize, set: &[(&str, &str)]) -> Result<(), String> {
    let r = post_form(b, &step(n), ("next", "Next >"), set)?;
    let to = r.header("location").unwrap_or("");
    if r.status == 303 && to.ends_with(&step(n + 1)) {
        Ok(())
    } else {
        Err(format!(
            "step {n}'s Next: {} {}",
            r.status,
            message_of(&r.text())
        ))
    }
}

/// A partition `name` by the wizard on host `host`: shared, 1 GiB, an
/// Ethernet adapter on the switch, its virtual disk: its ID.
fn partition(b: &mut Browser, host: &str, name: &str, disk: &str) -> Result<String, String> {
    let first = page(b, &format!("{}?ms={host}", step(1)))?;
    let id = field(&first, &step(1), "id");
    next(b, 1, &[("id", &id), ("n", name), ("d", "Made by suite 12")])?;
    next(b, 2, &[("p", "NORMAL"), ("dft", "on")])?;
    next(
        b,
        3,
        &[
            ("m", "shared"),
            ("min", "0.10"),
            ("des", "0.20"),
            ("max", "1.00"),
            ("vmin", "1"),
            ("vdes", "1"),
            ("vmax", "1"),
        ],
    )?;
    next(b, 4, &[("min", "1024"), ("des", "1024"), ("max", "2048")])?;
    next(b, 5, &[])?;
    let s6 = page(b, &step(6))?;
    let add = post_on(b, &s6, &step(6), ("add", "Add..."), &[("t", "Ethernet")])?;
    let r = post_on(
        b,
        &add.text(),
        &step(6),
        ("keep", "OK"),
        &[("sw", SWITCH), ("vlan", "1")],
    )?;
    if r.status != 303 {
        return Err(format!("the Ethernet adapter: {}", message_of(&r.text())));
    }
    let s6 = page(b, &step(6))?;
    let add = post_on(
        b,
        &s6,
        &step(6),
        ("add", "Add..."),
        &[("t", "SCSI client (virtual disk)")],
    )?;
    let backing = format!("{POOL}/{disk}");
    let r = post_on(
        b,
        &add.text(),
        &step(6),
        ("keep", "OK"),
        &[("disk", &backing), ("req", "on")],
    )?;
    if r.status != 303 {
        return Err(format!("the SCSI adapter: {}", message_of(&r.text())));
    }
    next(b, 6, &[])?;
    next(b, 7, &[])?;
    next(b, 8, &[])?;
    let r = post_form(b, &step(9), ("finish", "Finish"), &[])?;
    expect(b, &r, "QMC3130")?;
    Ok(id)
}

/// Delete through the confirmation at `path`, OK with `set`.
fn confirmed(b: &mut Browser, path: &str, set: &[(&str, &str)], id: &str) -> Result<(), String> {
    let html = page(b, path)?;
    let r = post_on(b, &html, path, ("ok", "OK"), set)?;
    expect(b, &r, id).map(|_| ())
}

/// Suite 12.
#[allow(
    clippy::too_many_lines,
    reason = "a host made, captured, emptied and made again"
)]
pub fn real_host(run: &crate::suite::base::Run<'_>) -> Result<String, String> {
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
        size: Some(Size {
            memory_mb: 6144,
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
    let mut c = start(run, "plans-real-host")?;
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
    // A pool on the blank disk, three virtual disks, a switch.
    let form = page(b, &v("storage/pool"))?;
    let d = free_disk(&form).ok_or("the pool form offers no free disk")?;
    let use_d = format!("d_{d}");
    let r = post_on(
        b,
        &form,
        &v("storage/pool"),
        ("ok", "Create"),
        &[("to", POOL), ("pr", "None"), (&use_d, "on")],
    )?;
    expect(b, &r, "QMC2420")?;
    until("the pool", Duration::from_mins(2), || {
        Ok(page(b, &v("storage"))?.contains(POOL))
    })?;
    let disks: Vec<String> = (1..=3).map(|i| format!("PLAND{i}")).collect();
    for (i, d) in disks.iter().enumerate() {
        let kind = if i == 0 { "thick" } else { "thin" };
        let r = post_form(
            b,
            &v("storage/create"),
            ("ok", "OK"),
            &[
                ("n", d),
                ("s", "1"),
                ("p", POOL),
                ("t", kind),
                ("a", "(none)"),
                ("c", ""),
            ],
        )?;
        expect(b, &r, "QMC2410")?;
    }
    until("the virtual disks", Duration::from_mins(2), || {
        let t = page(b, &v("storage"))?;
        Ok(disks.iter().all(|d| t.contains(d.as_str())))
    })?;
    let r = post_form(
        b,
        &v("ethernet/switch"),
        ("ok", "OK"),
        &[("to", SWITCH), ("v", "1,20"), ("mtu", "1500")],
    )?;
    expect(b, &r, "QMC2425")?;
    until("the switch", Duration::from_mins(1), || {
        Ok(page(b, &v("ethernet"))?.contains(SWITCH))
    })?;
    // Three partitions by the wizard.
    let mut ids = Vec::new();
    for (p, d) in PARTS.iter().zip(&disks) {
        ids.push(partition(b, &name, p, d)?);
    }
    until("the partitions listed", Duration::from_mins(2), || {
        let t = page(b, &format!("/qsmc/partitions?ms={name}"))?;
        Ok(PARTS.iter().all(|p| t.contains(p)))
    })?;
    // The plan.
    let plan_name = create(b, &name)?;
    let before = read(b, &plan_name)?;
    let made: Vec<&str> = before.partitions.iter().map(|p| p.name.as_str()).collect();
    if !PARTS.iter().all(|p| made.contains(p)) {
        return Err(format!("the plan holds {made:?}"));
    }
    // Deleted from their own pages: the partitions with their disks,
    // then the switch.
    for id in &ids {
        confirmed(
            b,
            &format!("/qsmc/confirm/partition-delete?ms={name}&p={id}"),
            &[("disks", "on")],
            "QMC3133",
        )?;
    }
    until("the partitions gone", Duration::from_mins(2), || {
        let t = page(b, &format!("/qsmc/partitions?ms={name}"))?;
        Ok(!PARTS.iter().any(|p| t.contains(p)))
    })?;
    confirmed(
        b,
        &format!("/qsmc/confirm/switch-delete?ms={name}&n={SWITCH}"),
        &[],
        qmcmsg::SWITCH_DELETED.id,
    )?;
    until("the switch and disks gone", Duration::from_mins(2), || {
        let e = page(b, &v("ethernet"))?;
        let s = page(b, &v("storage"))?;
        Ok(!e.contains(SWITCH) && !disks.iter().any(|d| s.contains(d.as_str())))
    })?;
    // Deployed to the same host, activated.
    let chosen: Vec<u32> = before.partitions.iter().map(|p| p.id).collect();
    wizard(b, &plan_name, &name, &chosen, true)?;
    let (id, steps) = ended(b, &plan_name, &name, Duration::from_mins(20))?;
    let complete = with_status(&steps, "Complete").len();
    let failed: Vec<String> = steps
        .iter()
        .filter(|r| r[1] == "Failed")
        .map(|r| format!("{}: {}", r[0], r[4]))
        .collect();
    // Read back: a capture of the host now, against the plan.
    let after_name = create(b, &name)?;
    let after = read(b, &after_name)?;
    let diff = plan::compare::compare(&before, &after);
    let states = page(b, &format!("/qsmc/partitions?ms={name}"))?;
    let activated = PARTS
        .iter()
        .filter(|p| {
            states
                .split("<tr")
                .any(|r| r.contains(*p) && !r.contains("Not activated"))
        })
        .count();
    c.guest.kill();
    host.kill();
    if id != qmcmsg::DEPLOYED.id {
        return Err(format!(
            "the deployment ended {id} after {complete} steps: {}",
            failed.join("; ")
        ));
    }
    let differs = if diff.is_empty() {
        "every definition and profile read back equal to the plan, MAC addresses and serial \
         numbers included"
            .to_string()
    } else {
        format!("what was not put back the same: {}", diff.join("; "))
    };
    Ok(format!(
        "{name}: pool {POOL}, switch {SWITCH}, three virtual disks and partitions {} made from \
         the pages, captured as {plan_name}, deleted from their own pages; deployed back in \
         {complete} steps (QMC6002); {differs}; {activated} of 3 partitions activated",
        PARTS.join(", ")
    ))
}
