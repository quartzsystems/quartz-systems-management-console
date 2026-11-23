// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! Suites 3 to 8: a plan captured and read back, validated against the
//! prototype's target, deployed with its steps in order, nothing harmed,
//! a restart of the web server and of the console in the middle, and
//! the import's refusals.

use std::time::Duration;

use super::{
    Lab, STANDALONE, VS1, VS2, create, deploy as wizard, ended, export, import as import_file, lab,
    read, table, untouchable, with_status,
};
use crate::suite::base::Run;
use crate::suite::mgmt::{connected, page, until};
use crate::suite::web::{ADMIN, PASSWORD, boot, message_of, shows, signed_on};

/// How long a deployment against the simulator may take.
const DEPLOYS: Duration = Duration::from_secs(300);

/// Suite 3: captured from the simulator's host, every definition and
/// profile; captured twice, the same; the document read back is the
/// document written.
pub fn capture(run: &Run<'_>) -> Result<String, String> {
    let mut lab = lab(run, "plans-capture")?;
    let b = &mut lab.c.browser;
    let first = create(b, VS1)?;
    let second = create(b, VS1)?;
    let a = read(b, &first)?;
    let again = read(b, &second)?;
    let diff = plan::compare::compare(&a, &again);
    if !diff.is_empty() {
        return Err(format!("two captures differ: {}", diff.join("; ")));
    }
    // Read back: the bytes exported are a document that writes the same.
    let bytes = export(b, &first)?;
    let written = plan::doc::text::write(&a)?;
    if written.as_bytes() != bytes.as_slice() {
        return Err("the document read back does not write the same".into());
    }
    let (mut defs, mut profiles) = (0, 0);
    {
        let h = sim::lock(&lab.fleet.systems[0]);
        for p in &h.parts {
            let mine = a
                .partition(p.definition.id)
                .ok_or_else(|| format!("partition {} is not in the plan", p.definition.name))?;
            if mine.name != p.definition.name || mine.serial != p.definition.serial {
                return Err(format!("partition {}'s definition differs", mine.name));
            }
            defs += 1;
            for k in &p.profiles {
                let pr = mine
                    .profiles
                    .iter()
                    .find(|x| x.name == k.profile.name)
                    .ok_or_else(|| format!("profile {} is not in the plan", k.profile.name))?;
                if plan::capture::profile_to(pr).as_ref() != Some(&k.profile) {
                    return Err(format!(
                        "{}'s profile {} differs from the host's",
                        mine.name, pr.name
                    ));
                }
                profiles += 1;
            }
        }
    }
    // A standalone system has no plan.
    let token = b.csrf("/qsmc/plans")?;
    let r = b.upload(
        "/qsmc/plans",
        &[
            ("csrf", &token),
            ("ms", STANDALONE),
            ("create", "Create plan"),
        ],
        b"",
    )?;
    if !shows(&r.text(), qmcmsg::NO_PLAN_STANDALONE.id) {
        return Err(format!("a standalone system: {}", message_of(&r.text())));
    }
    // The view shows each partition's profiles.
    let view = page(b, &format!("/qsmc/plan/{first}"))?;
    for name in ["QSDEV01", "QSTEST01", "QSPROD01", "POOL1", "ETHSW0"] {
        if !view.contains(name) {
            return Err(format!("View system plan lacks {name}"));
        }
    }
    lab.c.guest.kill();
    Ok(format!(
        "{first} and {second} captured from {VS1}: {defs} definitions and {profiles} profiles, \
         each the host's own; the two the same configuration; the {} bytes exported read back \
         and write the same; {STANDALONE} answered QMC6105",
        bytes.len()
    ))
}

/// The prototype's step 2 rows against `QSHQVS2`, and the rows the
/// served table adds (`docs/PLANS.md`, "Against the prototype").
const PROTOTYPE: &[(&str, &str, &str)] = &[
    (
        "Managed system QSHQVS2",
        "OK",
        "Type-Model 9Q10-V1 matches; 8 processors, 131,072 MB available",
    ),
    ("Storage pool POOL1", "OK", "Pool exists, 5,400 GB"),
    (
        "Storage pool POOL2",
        "Error",
        "No backing device /dev/sdb on target; VD05 (2,000 GB) cannot be created",
    ),
    (
        "Virtual switch ETHSW0",
        "OK",
        "Will be created with VLANs 10, 20, 30 on C1 port 1",
    ),
    (
        "Partition QSPROD01 physical I/O C3",
        "Warning",
        "Slot C3 on target is empty; adapter assignment will be skipped",
    ),
    (
        "Partition QSDEV01",
        "Warning",
        "Its MAC addresses and serial number will be new",
    ),
    (
        "Partition QSTEST01",
        "Warning",
        "Its MAC addresses and serial number will be new",
    ),
    (
        "Partition QSPROD01",
        "Warning",
        "Depends on POOL2 and C3 (see above); its MAC addresses and serial number will be new",
    ),
];

/// Suite 4: the fixture's deployment to `QSHQVS2` gives the prototype's
/// table; another kind of target and the plan's own host give theirs.
pub fn validate(run: &Run<'_>) -> Result<String, String> {
    let mut lab = lab(run, "plans-validate")?;
    let b = &mut lab.c.browser;
    let name = create(b, VS1)?;
    let step2 = |b: &mut crate::web::Browser, t: &str| -> Result<Vec<Vec<String>>, String> {
        let html = page(b, &format!("/qsmc/plan/{name}/deploy/step/2?t={t}"))?;
        Ok(table(&html, "Plan item"))
    };
    let rows = step2(b, VS2)?;
    let mut at = 0;
    for (item, status, message) in PROTOTYPE {
        let i = rows[at..]
            .iter()
            .position(|r| r.first().is_some_and(|x| x == item))
            .ok_or_else(|| format!("{item} is not in the table, or out of order"))?;
        let r = &rows[at + i];
        at += i + 1;
        if r.get(1).map(String::as_str) != Some(status)
            || r.get(2).map(String::as_str) != Some(message)
        {
            return Err(format!("{item}: {r:?}, not {status} `{message}`"));
        }
    }
    let added = rows.len() - PROTOTYPE.len();
    // A standalone system: Error, nothing else checked.
    let standalone = step2(b, STANDALONE)?;
    if standalone.len() != 1 || standalone[0].get(1).map(String::as_str) != Some("Error") {
        return Err(format!("{STANDALONE}: {standalone:?}"));
    }
    // The plan's own host: everything there, used as it is.
    let own = step2(b, VS1)?;
    let not_ok: Vec<String> = own
        .iter()
        .filter(|r| r.get(1).is_some_and(|s| s != "OK"))
        .map(|r| r.join(" "))
        .collect();
    if !not_ok.is_empty() {
        return Err(format!("on its own host: {}", not_ok.join("; ")));
    }
    // Step 3's defaults: QSPROD01 not ticked, the others ticked.
    let html = page(b, &format!("/qsmc/plan/{name}/deploy/step/3?t={VS2}"))?;
    let ticked = |id: u32| html.contains(&format!("name=\"p{id}\" checked"));
    let as_validated = ticked(1) && ticked(2) && !ticked(3);
    if !as_validated || !html.contains("Warning (POOL2, C3)") {
        return Err("step 3's partitions are not ticked as their validation says".into());
    }
    lab.c.guest.kill();
    Ok(format!(
        "against {VS2} the {} rows of the prototype's step 2 with its statuses and messages, the \
         partitions' new identity added, and {added} rows of items it leaves out; {STANDALONE} \
         Error alone; on {VS1} its {} rows OK; step 3 ticked QSDEV01 and QSTEST01, not QSPROD01 \
         (Warning (POOL2, C3))",
        PROTOTYPE.len(),
        own.len()
    ))
}

fn tell(lab: &Lab<'_>, i: usize, line: &str) {
    let _ = lab.fleet.tell(i, line);
}

/// Suites 5 and 6: Cancel before the next step; a step the host
/// refuses; the whole deployment with activation; again, skipping what
/// exists; nothing harmed.
#[allow(clippy::too_many_lines, reason = "one host's deployments in turn")]
pub fn deploy(run: &Run<'_>) -> Result<String, String> {
    let mut lab = lab(run, "plans-deploy")?;
    let name = create(&mut lab.c.browser, VS1)?;
    let before = (untouchable(&lab.fleet, 0), untouchable(&lab.fleet, 1));
    // Cancel while the first switch is made: stopped before the next.
    tell(&lab, 1, "slow vres/0x0045 6000");
    wizard(&mut lab.c.browser, &name, VS2, &[1], false)?;
    let b = &mut lab.c.browser;
    let path = super::progress_path(&name, VS2);
    until("the first step under way", Duration::from_secs(60), || {
        Ok(page(b, &path)?.contains("In progress"))
    })?;
    let token = b.csrf(&path)?;
    let r = b.post(&path, &[("csrf", &token), ("stop", "Cancel deployment")])?;
    crate::suite::parts::expect(b, &r, qmcmsg::CANCEL_ASKED.id)?;
    let (id, steps) = ended(b, &name, VS2, DEPLOYS)?;
    if id != qmcmsg::DEPLOY_CANCELLED.id {
        return Err(format!("Cancel ended the job with {id}"));
    }
    let made = with_status(&steps, "Complete");
    if made != ["Create virtual switch ETHSW0"] || with_status(&steps, "Pending").is_empty() {
        return Err(format!("after Cancel: {steps:?}"));
    }
    tell(&lab, 1, "allow");
    // A step the host refuses stops the job with its message.
    tell(
        &lab,
        1,
        "refuse-family partition/0x0008 QVS2006 The host already has 64 partitions, the most it is set to have.",
    );
    wizard(&mut lab.c.browser, &name, VS2, &[1, 2, 3], false)?;
    let (id, steps) = ended(&mut lab.c.browser, &name, VS2, DEPLOYS)?;
    let failed: Vec<&Vec<String>> = steps.iter().filter(|r| r[1] == "Failed").collect();
    if id != qmcmsg::DEPLOY_FAILED.id
        || failed.len() != 1
        || !failed[0][0].starts_with("Create partition QSDEV01")
        || !failed[0][4].contains("QVS2006")
    {
        return Err(format!("a refused step: {id} {steps:?}"));
    }
    let refused_made = with_status(&steps, "Complete").len();
    tell(&lab, 1, "allow");
    // The whole deployment, activated: Errors skipped, the rest made.
    wizard(&mut lab.c.browser, &name, VS2, &[1, 2, 3], true)?;
    let (id, steps) = ended(&mut lab.c.browser, &name, VS2, DEPLOYS)?;
    if id != qmcmsg::DEPLOYED.id {
        return Err(format!("the deployment ended {id}: {steps:?}"));
    }
    let skipped = with_status(&steps, "Skipped");
    if skipped
        != [
            "Create storage pool POOL2",
            "Create virtual disk VD05 (2,000 GB, POOL2)",
        ]
    {
        return Err(format!("skipped: {skipped:?}"));
    }
    let order: Vec<&str> = steps.iter().map(|r| r[0].as_str()).collect();
    // Each kind after the one before; a kind made by the refused run is
    // no step now.
    let positions: Vec<usize> = [
        "Create virtual switch",
        "Create storage pool",
        "Create shared processor pool",
        "Create virtual disk",
        "Create virtual tape",
        "Create partition",
        "Activate partition",
    ]
    .iter()
    .filter_map(|p| order.iter().position(|s| s.starts_with(p)))
    .collect();
    let in_order = positions.windows(2).all(|w| w[0] < w[1]) && positions.len() >= 3;
    if !in_order {
        return Err(format!("the steps are not in order: {order:?}"));
    }
    {
        let h = sim::lock(&lab.fleet.systems[1]);
        if h.parts.len() != 3 || !h.parts.iter().all(sim::system::Part::active) {
            return Err("not every partition was made and activated".into());
        }
        let prod = h
            .parts
            .iter()
            .find(|p| p.definition.name == "QSPROD01")
            .ok_or("no QSPROD01")?;
        let pr = &prod.profiles[0].profile;
        if !pr.physical_io.is_empty()
            || pr
                .scsi
                .iter()
                .any(|a| a.luns.iter().any(|l| l.backing == "POOL2/VD05"))
        {
            return Err("QSPROD01's profile keeps what its Warning left out".into());
        }
        if !h
            .vres
            .switches
            .iter()
            .any(|s| s.name == "ETHSW0" && s.uplink.is_some())
        {
            return Err("ETHSW0 was not made with its uplink".into());
        }
    }
    // Again: what exists skipped, nothing to make.
    wizard(&mut lab.c.browser, &name, VS2, &[1, 2, 3], false)?;
    let (id, again) = ended(&mut lab.c.browser, &name, VS2, DEPLOYS)?;
    if id != qmcmsg::DEPLOYED.id || !with_status(&again, "Complete").is_empty() {
        return Err(format!("deployed again: {id} {again:?}"));
    }
    // Nothing harmed: QSHQVS1 as it was; QSHQVS2's pool, disks and
    // library as they were, its labelled disk in no pool.
    let after = (untouchable(&lab.fleet, 0), untouchable(&lab.fleet, 1));
    if after != before {
        return Err(format!(
            "a deployment changed what it did not make: {before:?} then {after:?}"
        ));
    }
    let foreign_kept = sim::lock(&lab.fleet.systems[1])
        .vres
        .disks
        .iter()
        .any(|d| d.name == "sdj" && d.foreign && d.pool.is_empty());
    lab.c.guest.kill();
    if !foreign_kept {
        return Err("the disk with another system's label was taken".into());
    }
    Ok(format!(
        "Cancel during the first step stopped the job (QMC6003) with ETHSW0 made and the rest \
         pending; a partition's creation refused (QVS2006) stopped it (QMC6004) after {refused_made} \
         steps; then {} steps in order, POOL2 and VD05 skipped, QSPROD01 made without VD05 and C3, \
         the three partitions activated (QMC6002); again, every step skipped; QSHQVS1 and \
         QSHQVS2's pool, disks and library as before, sdj in no pool",
        steps.len()
    ))
}

/// Suite 7: the web server restarted while a step runs still shows the
/// deployment; the console restarted marks the step interrupted, and
/// deploying again finishes.
pub fn restart(run: &Run<'_>) -> Result<String, String> {
    let mut lab = lab(run, "plans-restart")?;
    let name = create(&mut lab.c.browser, VS1)?;
    tell(&lab, 1, "slow vres/0x0011 20000");
    wizard(&mut lab.c.browser, &name, VS2, &[1, 2], false)?;
    let path = super::progress_path(&name, VS2);
    let b = &mut lab.c.browser;
    until(
        "a virtual disk being made",
        Duration::from_secs(120),
        || {
            let t = page(b, &path)?;
            Ok(table(&t, "Step")
                .iter()
                .any(|r| r[0].starts_with("Create virtual disk") && r[1] == "In progress"))
        },
    )?;
    let seen = table(&page(b, &path)?, "Step");
    crate::suite::serv::kill(&mut lab.c.guest, "webd")?;
    crate::suite::web::webd_ready(&mut lab.c.guest, Duration::from_secs(90))?;
    let b = &mut lab.c.browser;
    signed_on(b, ADMIN, PASSWORD)?;
    let after_web = table(&page(b, &path)?, "Step");
    let firsts = |t: &[Vec<String>]| t.iter().map(|r| r[0].clone()).collect::<Vec<_>>();
    if firsts(&after_web) != firsts(&seen) {
        return Err("the web server's restart lost the deployment's steps".into());
    }
    // The console restarted during the step.
    until("the next disk being made", Duration::from_secs(120), || {
        let t = page(b, &path)?;
        Ok(table(&t, "Step")
            .iter()
            .any(|r| r[0].starts_with("Create virtual disk") && r[1] == "In progress"))
    })?;
    lab.c.guest.kill();
    let machine = lab.c.machine.clone();
    let mut c = boot(run, machine)?;
    signed_on(&mut c.browser, ADMIN, PASSWORD)?;
    let t = page(&mut c.browser, &path)?;
    let steps = table(&t, "Step");
    if !shows(&t, qmcmsg::DEPLOY_STOPPED.id) || with_status(&steps, "Interrupted").len() != 1 {
        return Err(format!(
            "after the console's restart: {} {steps:?}",
            message_of(&t)
        ));
    }
    tell(&lab, 1, "allow");
    // The web server may answer before mgmtd has the hosts again.
    until(
        "both hosts connected again",
        Duration::from_secs(120),
        || {
            let conn = page(&mut c.browser, "/qsmc/connectivity")?;
            Ok(connected(&conn, VS1) && connected(&conn, VS2))
        },
    )?;
    wizard(&mut c.browser, &name, VS2, &[1, 2], false)?;
    let (id, again) = ended(&mut c.browser, &name, VS2, DEPLOYS)?;
    c.guest.kill();
    if id != qmcmsg::DEPLOYED.id {
        return Err(format!("deployed again: {id} {again:?}"));
    }
    if sim::lock(&lab.fleet.systems[1]).parts.len() != 2 {
        return Err("deploying again did not make both partitions".into());
    }
    Ok(format!(
        "the web server killed while a virtual disk was made: its {} steps shown again after \
         it restarted; the console restarted during the next: the step Interrupted and the job \
         stopped (QMC6005); deployed again, it finished (QMC6002) with {} steps made",
        seen.len(),
        with_status(&again, "Complete").len()
    ))
}

/// Suite 8: a plan exported and imported deploys the same; a damaged
/// file, one past the size limit, a newer version, and names or numbers
/// out of range each refused with a message.
pub fn import(run: &Run<'_>) -> Result<String, String> {
    let mut lab = lab(run, "plans-import")?;
    let b = &mut lab.c.browser;
    let name = create(b, VS1)?;
    let bytes = export(b, &name)?;
    let text = String::from_utf8(bytes.clone()).map_err(|e| e.to_string())?;
    let mut refused = Vec::new();
    let bad: Vec<(&str, Vec<u8>)> = vec![
        ("damaged", bytes[..bytes.len() / 2].to_vec()),
        (
            "past the size limit",
            vec![b'#'; plan::doc::text::MAX_BYTES + 1],
        ),
        (
            "a newer version",
            text.replace("schema_version = 1", "schema_version = 2")
                .into_bytes(),
        ),
        (
            "an ID out of range",
            text.replacen("id = 1\n", "id = 1000\n", 1).into_bytes(),
        ),
        (
            "a VLAN out of range",
            text.replacen("vlan = 20", "vlan = 5000", 1).into_bytes(),
        ),
        (
            "a name with a space",
            text.replacen("name = \"QSDEV01\"", "name = \"QS DEV01\"", 1)
                .into_bytes(),
        ),
        (
            "an unknown key",
            text.replacen("cores = ", "colours = 1\ncores = ", 1)
                .into_bytes(),
        ),
    ];
    for (what, file) in &bad {
        let t = import_file(b, file)?;
        if !shows(&t, qmcmsg::PLAN_REFUSED.id) {
            return Err(format!("{what} was not refused: {}", message_of(&t)));
        }
        refused.push(*what);
    }
    // Imported in place of the plan deleted, it deploys the same.
    let path = format!("/qsmc/confirm/plan-delete?p={name}");
    let html = page(b, &path)?;
    let r = crate::suite::parts::post_on(b, &html, &path, ("ok", "OK"), &[])?;
    crate::suite::parts::expect(b, &r, qmcmsg::PLAN_DELETED.id)?;
    let t = import_file(b, &bytes)?;
    if !shows(&t, qmcmsg::PLAN_IMPORTED.id) {
        return Err(format!("the export was not imported: {}", message_of(&t)));
    }
    if export(b, &name)? != bytes {
        return Err("the plan imported is not the one exported".into());
    }
    let listed = page(b, "/qsmc/plans")?;
    let rows = table(&listed, "Actions").len();
    wizard(b, &name, VS2, &[1, 2], false)?;
    let (id, steps) = ended(b, &name, VS2, DEPLOYS)?;
    lab.c.guest.kill();
    if id != qmcmsg::DEPLOYED.id {
        return Err(format!("the imported plan's deployment: {id} {steps:?}"));
    }
    Ok(format!(
        "refused with QMC6104 and its reason: {}; the plan exported, deleted through its \
         confirmation and imported again byte for byte ({rows} plan listed), deployed to {VS2}: \
         {} steps made",
        refused.join(", "),
        with_status(&steps, "Complete").len()
    ))
}
