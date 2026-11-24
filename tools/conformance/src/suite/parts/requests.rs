// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! Suite 2: each request kind the pages add, through the web interface,
//! checked at the host; a host's refusal shown in its own words. Suite
//! 3: a profile changed under a second session's form.

use std::time::Duration;

use qsi::family as fam;
use qsi::host::model::PartitionState;
use qsi::host::partition::WithHost;

use super::{Lab, VS, asked, expect, host, kept, lab, line_of, post_form, post_on};
use crate::suite::mgmt::{page, until};
use crate::suite::web::{ADMIN, PASSWORD, follow, message_of, shows, signed_on, submit};
use crate::web::form::OFF;

/// A host's virtual resources' page `p`.
fn v(p: &str) -> String {
    format!("/qsmc/server/{VS}/virtual/{p}")
}

/// A partition's page `p`.
fn part(id: u32, p: &str) -> String {
    format!("/qsmc/partition/{VS}/{id}/{p}")
}

/// What the scenario did: a line of evidence each request kind, and the
/// audit records it must have left.
pub struct Done {
    /// Each request kind and what the host shows of it.
    pub evidence: Vec<String>,
    /// The audit records' words, each with the Administrator's ID.
    pub audits: Vec<&'static str>,
    /// What the pages do other than the documents say.
    pub findings: Vec<String>,
}

/// How many tape devices partition `id`'s default profile has at the host.
fn tape_devices(fleet: &crate::suite::mgmt::Fleet, id: u32) -> usize {
    host(fleet)
        .part_ref(id)
        .and_then(|p| {
            p.kept(&p.definition.default_profile)
                .map(|k| k.profile.tape.len())
        })
        .unwrap_or(0)
}

fn state(lab: &Lab<'_>, id: u32) -> Option<PartitionState> {
    host(&lab.fleet).part_ref(id).map(|p| p.status.state)
}

/// Wait until partition `id` is (or is not, `not`) in `want`.
fn wait_state(lab: &Lab<'_>, id: u32, want: PartitionState, not: bool) -> Result<(), String> {
    until(
        &format!("partition {id} {}{want:?}", if not { "not " } else { "" }),
        Duration::from_secs(30),
        || Ok((state(lab, id) == Some(want)) != not),
    )
}

/// Activate, shut down or restart through the form and its
/// confirmation: the page after.
fn confirmed(
    lab: &mut Lab<'_>,
    path: &str,
    set: &[(&str, &str)],
    id: &str,
) -> Result<String, String> {
    let b = &mut lab.c.browser;
    let r = post_form(b, path, ("ok", "OK"), set)?;
    let to = r
        .header("location")
        .ok_or_else(|| format!("{path} did not lead to its confirmation: {}", r.status))?
        .to_string();
    let html = page(b, &to)?;
    let r = post_on(b, &html, &to, ("ok", "OK"), &[])?;
    expect(b, &r, id)
}

/// Every request kind of this phase, in an order the fixture allows.
#[allow(clippy::too_many_lines, reason = "one request kind a step")]
pub fn scenario(lab: &mut Lab<'_>) -> Result<Done, String> {
    let mut ev = Vec::new();
    let mut audits = Vec::new();
    let mut findings = Vec::new();
    let vres_before = asked(&lab.fleet, fam::VRES, ADMIN);
    {
        let (fleet, b) = (&lab.fleet, &mut lab.c.browser);
        // Switches: made, changed and renamed with an uplink.
        let r = post_form(
            b,
            &v("ethernet/switch"),
            ("ok", "OK"),
            &[("to", "TESTSW"), ("v", "50"), ("mtu", "1500")],
        )?;
        expect(b, &r, "QMC2425")?;
        if !line_of(&kept(fleet, "switches"), "TESTSW").is_some_and(|l| l.contains("vlans 50 ")) {
            return Err(format!(
                "TESTSW not at the host: {}",
                kept(fleet, "switches")
            ));
        }
        let r = post_form(
            b,
            &v("ethernet/switch?n=TESTSW"),
            ("ok", "OK"),
            &[
                ("to", "TESTSW2"),
                ("v", "50, 51"),
                ("u", "U9Q10.V1.9F11A2-P1-C1-T2"),
            ],
        )?;
        expect(b, &r, "QMC2426")?;
        let sw = kept(fleet, "switches");
        let Some(l) = line_of(&sw, "TESTSW2").filter(|l| l.contains("vlans 50,51 ")) else {
            return Err(format!("TESTSW2 not changed at the host: {sw}"));
        };
        ev.push(format!(
            "switch made (QMC2425), changed, renamed, uplinked (QMC2426): `{l}`"
        ));
        audits.extend([
            "create switch TESTSW on QSHQVS1",
            "change switch TESTSW on QSHQVS1",
            "rename switch TESTSW to TESTSW2 on QSHQVS1",
            "set the uplink of switch TESTSW2 on QSHQVS1",
        ]);
        // A storage pool of a disk with another system's label: named on
        // a confirmation first.
        let path = v("storage/pool");
        let r = post_form(
            b,
            &path,
            ("ok", "Create"),
            &[("to", "POOL3"), ("pr", "None"), ("d_sdj", "on")],
        )?;
        let t = r.text();
        let r = if r.status == 200 && t.contains("Confirm Use of Disks") && t.contains("sdj") {
            if line_of(&kept(fleet, "pools"), "POOL3").is_some() {
                return Err("POOL3 was made before its confirmation".into());
            }
            post_on(b, &t, &path, ("ok", "OK"), &[])?
        } else {
            findings.push(format!(
                "the pool form does not offer sdj, the disk with another system's label, so its \
                 confirmation is never reached: {}",
                super::plain(&message_of(&t))
            ));
            post_form(
                b,
                &path,
                ("ok", "Create"),
                &[("to", "POOL3"), ("pr", "None"), ("d_sdi", "on")],
            )?
        };
        expect(b, &r, "QMC2420")?;
        let pools = kept(fleet, "pools");
        let Some(l) = line_of(&pools, "POOL3").map(str::to_string) else {
            return Err(format!("POOL3 not at the host: {pools}"));
        };
        let r = post_form(
            b,
            &v("storage/pool?n=POOL3"),
            ("ok", "OK"),
            &[("to", "POOL4")],
        )?;
        expect(b, &r, "QMC2421")?;
        if line_of(&kept(fleet, "pools"), "POOL4").is_none() {
            return Err("POOL3 not renamed POOL4 at the host".into());
        }
        // The media repository moved and a pool extended: the host's
        // refusals, in its words.
        let r = post_form(
            b,
            &v("storage/pool?n=POOL4"),
            ("ok", "OK"),
            &[("repo", "on")],
        )?;
        let des = follow(b, &r)?.text();
        let r = post_form(b, &v("storage/pool?n=POOL4"), ("extend", "x"), &[])?;
        let ext = follow(b, &r)?.text();
        if !shows(&ext, "QVS3019") || !message_of(&des).contains("QVS") {
            return Err(format!(
                "a designate and an extend the host refuses: {} / {}",
                message_of(&des),
                message_of(&ext)
            ));
        }
        ev.push(format!(
            "pool made (QMC2420): `{l}`, renamed POOL4 (QMC2421); \
             designate refused `{}`, extend refused with QVS3019",
            message_of(&des)
        ));
        audits.extend([
            "create storage pool POOL3 on QSHQVS1",
            "rename storage pool POOL3 to POOL4 on QSHQVS1",
            "media repository of POOL4 on QSHQVS1",
            "extend storage pool POOL4 on QSHQVS1",
        ]);
        // A virtual disk made, given to QSTEST01's profile, and grown.
        let r = post_form(
            b,
            &v("storage/create"),
            ("ok", "OK"),
            &[
                ("n", "VD07"),
                ("s", "10"),
                ("p", "POOL1"),
                ("t", "thin"),
                ("a", "QSTEST01"),
                ("c", ""),
            ],
        )?;
        let t = expect(b, &r, "QMC2410")?;
        if !shows(&t, "QMC2411") {
            return Err(format!("VD07 not assigned: {}", message_of(&t)));
        }
        let given = host(fleet).part_ref(2).and_then(|p| {
            p.kept(&p.definition.default_profile).map(|k| {
                k.profile
                    .scsi
                    .iter()
                    .any(|a| a.luns.iter().any(|l| l.backing == "POOL1/VD07"))
            })
        });
        if given != Some(true) {
            return Err("QSTEST01's profile has no LUN of POOL1/VD07".into());
        }
        let r = post_form(
            b,
            &v("storage/resize?p=POOL1&n=VD07"),
            ("ok", "Resize"),
            &[("s", "20")],
        )?;
        expect(b, &r, "QMC2413")?;
        let vd = kept(fleet, "vdisks");
        let Some(l) = line_of(&vd, "POOL1/VD07").filter(|l| l.contains("bytes 19999")) else {
            return Err(format!("VD07 not grown at the host: {vd}"));
        };
        ev.push(format!(
            "virtual disk made (QMC2410), in QSTEST01's profile (QMC2411), grown (QMC2413): `{l}`"
        ));
        audits.extend([
            "create virtual disk POOL1/VD07 on QSHQVS1",
            "assign POOL1/VD07 to QSTEST01",
            "resize virtual disk POOL1/VD07 on QSHQVS1",
        ]);
        // An image unloaded and another loaded in QSDEV01's drive.
        let r = post_form(
            b,
            &v("optical/load?n=HLINV_SAVF_20261028.iso&unload=1"),
            ("ok", "OK"),
            &[],
        )?;
        expect(b, &r, "QMC2407")?;
        let r = post_form(
            b,
            &v("optical/load?n=QS400_CUM_26290.iso"),
            ("ok", "OK"),
            &[("d", "QSDEV01 OPT01")],
        )?;
        expect(b, &r, "QMC2406")?;
        let drives = kept(fleet, "drives 1");
        if !drives.contains("QS400_CUM_26290.iso") || drives.contains("HLINV") {
            return Err(format!("QSDEV01's drives: {drives}"));
        }
        ev.push(format!(
            "image unloaded (QMC2407) and loaded (QMC2406): QSDEV01 holds `{}`",
            drives.lines().find(|l| l.contains("CUM")).unwrap_or("")
        ));
        audits.extend([
            "unload OPT01 from QSDEV01",
            "load QS400_CUM_26290.iso into OPT01 of QSDEV01",
        ]);
        // A tape volume made, protected and given to a partition.
        let r = post_form(
            b,
            &v("tape/volume"),
            ("create", "Create volume"),
            &[("vn", "VTAP09"), ("vs", "10")],
        )?;
        expect(b, &r, "QMC2430")?;
        let r = post_form(b, &v("tape/volume"), ("protect", "VTAP09"), &[])?;
        expect(b, &r, "QMC2431")?;
        let r = post_form(
            b,
            &v("tape/volume?p=QSTEST01&s=6"),
            ("give", "Give it the volume"),
            &[("v", "VTAP09")],
        )?;
        expect(b, &r, "QMC2433")?;
        let vols = kept(fleet, "volumes");
        let Some(l) = line_of(&vols, "VTAP09").filter(|l| l.contains("protected true")) else {
            return Err(format!("VTAP09 at the host: {vols}"));
        };
        let tape = host(fleet).part_ref(2).and_then(|p| {
            p.kept(&p.definition.default_profile).map(|k| {
                k.profile
                    .tape
                    .iter()
                    .any(|t| t.media.as_deref() == Some("VTAP09"))
            })
        });
        if tape != Some(true) {
            return Err("QSTEST01's profile was not given VTAP09".into());
        }
        ev.push(format!(
            "tape volume made (QMC2430), protected (QMC2431), given to QSTEST01 (QMC2433): `{l}`"
        ));
        audits.extend([
            "create tape volume VTAP09 on QSHQVS1",
            "protect tape volume VTAP09 on QSHQVS1",
            "give QSTEST01's tape device 6 volume VTAP09",
        ]);
        // A tape drive made for a partition: a new device in its profile.
        let before = tape_devices(fleet, 2);
        let r = post_form(
            b,
            &v("tape/volume"),
            ("add_drive", "Create drive"),
            &[("dp", "QSTEST01"), ("dv", "VTAP09")],
        )?;
        let added = expect(b, &r, "QMC2436")?;
        if tape_devices(fleet, 2) != before + 1 {
            return Err(format!(
                "QSTEST01's profile has no new tape device: {}",
                message_of(&added)
            ));
        }
        if !page(b, &v("tape"))?.contains("QSTEST01") {
            return Err("Virtual Tape does not list QSTEST01's new drive".into());
        }
        ev.push(format!(
            "a tape drive made for QSTEST01 (QMC2436): {}",
            message_of(&added)
        ));
        audits.push("add a tape device in slot");
        // A shared processor pool made and changed.
        let r = post_form(
            b,
            &v("pools/change"),
            ("ok", "OK"),
            &[("to", "NEWPOOL"), ("c", "5"), ("m", "1.00")],
        )?;
        expect(b, &r, "QMC2440")?;
        let r = post_form(b, &v("pools/change?id=2"), ("ok", "OK"), &[("m", "0.50")])?;
        expect(b, &r, "QMC2440")?;
        let pp = kept(fleet, "procpools");
        let Some(l) = line_of(&pp, "NEWPOOL").filter(|l| l.contains("maximum 0.50")) else {
            return Err(format!("NEWPOOL at the host: {pp}"));
        };
        ev.push(format!("processor pool made and changed (QMC2440): `{l}`"));
        audits.extend([
            "create processor pool NEWPOOL on QSHQVS1",
            "change processor pool NEWPOOL on QSHQVS1",
        ]);
    }
    let vres = asked(&lab.fleet, fam::VRES, ADMIN) - vres_before;
    ev.push(format!(
        "{vres} requests of the vres family from user admin"
    ));
    {
        let (fleet, b) = (&lab.fleet, &mut lab.c.browser);
        // QSTEST01's definition and settings, then its profile.
        let r = post_form(
            b,
            &part(2, "general"),
            ("apply", "Apply"),
            &[("n", "QSTEST09"), ("d", "Test partition, changed")],
        )?;
        expect(b, &r, "QMC3120")?;
        {
            let h = host(fleet);
            let p = h.part_ref(2).ok_or("no partition 2")?;
            if p.definition.name != "QSTEST09"
                || p.settings.description != "Test partition, changed"
            {
                return Err(format!(
                    "partition 2 at the host: {} `{}`",
                    p.definition.name, p.settings.description
                ));
            }
        }
        let r = post_form(
            b,
            &part(2, "settings"),
            ("apply", "Apply"),
            &[("cp", OFF), ("sd", "Immediate")],
        )?;
        expect(b, &r, "QMC3120")?;
        let (panel, with) = {
            let h = host(fleet);
            let p = h.part_ref(2).ok_or("no partition 2")?;
            (p.settings.panel, p.settings.with_host)
        };
        if panel || with != WithHost::Immediate {
            return Err(format!("partition 2's settings: panel {panel}, {with:?}"));
        }
        let mem = page(b, &part(2, "memory"))?;
        let (min, des) = (
            super::field(&mem, &part(2, "memory"), "min"),
            super::field(&mem, &part(2, "memory"), "des"),
        );
        let max = super::field(&mem, &part(2, "memory"), "max");
        let want = if des == max { min } else { max };
        let r = post_on(
            b,
            &mem,
            &part(2, "memory"),
            ("apply", "Apply"),
            &[("des", &want)],
        )?;
        expect(b, &r, "QMC3120")?;
        let got = host(fleet).part_ref(2).and_then(|p| {
            p.kept(&p.definition.default_profile)
                .map(|k| k.profile.memory.desired_mib)
        });
        if got.map(|m| m.to_string()) != Some(want.replace(',', "")) {
            return Err(format!("QSTEST09's profile has {got:?} MB, not {want}"));
        }
        ev.push(format!(
            "partition renamed QSTEST09 and described (QMC3120), its panel functions off and its end \
             with the host immediate, its profile's memory {want} MB"
        ));
        audits.extend([
            "change partition QSTEST01 of QSHQVS1",
            "change settings of QSTEST09 of QSHQVS1",
            "change profile install of QSHQVS1 partition 2",
        ]);
        // QSDEV01's units changed now as well as in its profile.
        let r = post_form(
            b,
            &part(1, "processors"),
            ("apply", "Apply"),
            &[("des", "0.50")],
        )?;
        let t = expect(b, &r, "QMC3122")?;
        let units = host(fleet)
            .part_ref(1)
            .and_then(|p| p.holds.as_ref().map(|c| c.units));
        if !shows(&t, "QMC3121") || units != Some(50) {
            return Err(format!(
                "QSDEV01's entitlement: {units:?}, {}",
                message_of(&t)
            ));
        }
        ev.push(
            "QSDEV01's profile changed (QMC3121) and its entitlement now 0.50 (QMC3122)".into(),
        );
        audits.push("change the entitlement of QSDEV01 on QSHQVS1");
        // Profiles: copied, made default, the running one saved, deleted.
        let r = post_form(
            b,
            &part(1, "profiles?copy=1&prof=install"),
            ("ok", "OK"),
            &[("n", "install2")],
        )?;
        expect(b, &r, "QMC3204")?;
        let r = post_form(b, &part(1, "profiles"), ("default", "Set default..."), &[])?;
        let html = r.text();
        let r = post_on(
            b,
            &html,
            &part(1, "profiles"),
            ("ok", "OK"),
            &[("n", "install2")],
        )?;
        expect(b, &r, "QMC3203")?;
        let r = post_form(
            b,
            &part(1, "profiles?save=1"),
            ("ok", "OK"),
            &[("n", "saved1")],
        )?;
        expect(b, &r, "QMC3202")?;
        let r = submit(
            b,
            "/qsmc/confirm/profile-delete?ms=QSHQVS1&p=1&n=install2",
            &[("ms", VS), ("p", "1"), ("n", "install2"), ("ok", "OK")],
        )?;
        let refused = follow(b, &r)?.text();
        if !shows(&refused, "QVS2007") {
            return Err(format!(
                "the default profile's delete: {}",
                message_of(&refused)
            ));
        }
        let r = submit(
            b,
            "/qsmc/confirm/profile-delete?ms=QSHQVS1&p=1&n=saved1",
            &[("ms", VS), ("p", "1"), ("n", "saved1"), ("ok", "OK")],
        )?;
        expect(b, &r, "QMC3205")?;
        let profs = kept(fleet, "profiles 1");
        if !line_of(&profs, "install2").is_some_and(|l| l.ends_with(" default"))
            || line_of(&profs, "saved1").is_some()
        {
            return Err(format!("QSDEV01's profiles: {profs}"));
        }
        ev.push(format!(
            "profile copied (QMC3204), made default (QMC3203), saved from the running one (QMC3202), \
             deleted (QMC3205), the default's delete refused with QVS2007: `{}`",
            profs.replace('\n', "; ")
        ));
        audits.extend([
            "copy profile install of partition 1 as install2 on QSHQVS1",
            "set profile install2 default for partition 1 on QSHQVS1",
            "save the running configuration of partition 1 as saved1 on QSHQVS1",
            "delete profile saved1 of QSDEV01 on QSHQVS1",
        ]);
    }
    // Activate, shut down, restart, immediate shut down, delete.
    confirmed(
        lab,
        &part(2, "activate"),
        &[
            ("prof", "default"),
            ("key", "Normal"),
            ("src", "A"),
            ("type", "Unattended"),
            ("tw", OFF),
        ],
        "QMC3101",
    )?;
    wait_state(lab, 2, PartitionState::NotActivated, true)?;
    confirmed(
        lab,
        &part(3, "shutdown"),
        &[("how", "delayed"), ("delay", "300")],
        "QMC3102",
    )?;
    wait_state(lab, 3, PartitionState::NotActivated, false)?;
    confirmed(
        lab,
        &part(1, "shutdown?restart=1"),
        &[("how", "delayed"), ("delay", "60"), ("restart", "on")],
        "QMC3103",
    )?;
    wait_state(lab, 1, PartitionState::NotActivated, true)?;
    confirmed(lab, &part(2, "shutdown"), &[("how", "imm")], "QMC3102")?;
    wait_state(lab, 2, PartitionState::NotActivated, false)?;
    ev.push(
        "QSTEST09 activated (QMC3101), QSPROD01 shut down delayed (QMC3102) and Not activated, \
         QSDEV01 restarted (QMC3103) and active, QSTEST09 ended immediately (QMC3102)"
            .into(),
    );
    audits.extend([
        "on QSHQVS1 with profile default",
        "shut down QSPROD01 on QSHQVS1: Delayed, 300 s",
        "shut down QSDEV01 on QSHQVS1: Delayed, 60 s, then restart",
        "on QSHQVS1: Immediate",
    ]);
    {
        let (fleet, b) = (&lab.fleet, &mut lab.c.browser);
        let r = submit(
            b,
            "/qsmc/confirm/partition-delete?ms=QSHQVS1&p=2",
            &[("ms", VS), ("p", "2"), ("ok", "OK")],
        )?;
        expect(b, &r, "QMC3133")?;
        if host(fleet).part_ref(2).is_some()
            || line_of(&kept(fleet, "vdisks"), "POOL1/VD04").is_none()
        {
            return Err("partition 2 not deleted with its disks kept".into());
        }
        ev.push("QSTEST09 deleted (QMC3133), its VD04 kept".into());
        audits.push("delete partition QSTEST0");
        // The host's power-on settings.
        let r = post_form(
            b,
            &format!("/qsmc/server/{VS}/power"),
            ("apply", "Apply"),
            &[("auto", "none")],
        )?;
        let t = follow(b, &r)?.text();
        if host(fleet).power_on.start_partitions {
            return Err(format!(
                "the power-on settings did not change: {}",
                message_of(&t)
            ));
        }
        ev.push("power-on settings: start no partitions, at the host".into());
        audits.push("power-on settings of QSHQVS1");
    }
    Ok(Done {
        evidence: ev,
        audits,
        findings,
    })
}

/// The host refuses a family: the page shows its message after the
/// console's.
fn refusals(lab: &mut Lab<'_>) -> Result<Vec<String>, String> {
    let mut out = Vec::new();
    let tries: [(&str, &str, &str); 4] = [
        ("vres", "QVS9001", "switch"),
        ("partition", "QVS9002", "shut down"),
        ("profiles", "QVS9003", "copy"),
        ("host", "QVS9004", "power-on settings"),
    ];
    for (family, id, what) in tries {
        lab.fleet.tell(
            0,
            &format!("refuse-family {family} {id} Refused by the harness."),
        );
        let b = &mut lab.c.browser;
        let t = match what {
            "switch" => {
                let r = post_form(
                    b,
                    &v("ethernet/switch"),
                    ("ok", "OK"),
                    &[("to", "TESTSW3"), ("v", "60")],
                )?;
                follow(b, &r)?.text()
            }
            "shut down" => {
                let to = format!("/qsmc/partition/{VS}/1/shutdown?confirm=1&how=imm&delay=0");
                let html = page(b, &to)?;
                let r = post_on(b, &html, &to, ("ok", "OK"), &[])?;
                follow(b, &r)?.text()
            }
            "copy" => {
                let r = post_form(
                    b,
                    &part(1, "profiles?copy=1&prof=default"),
                    ("ok", "OK"),
                    &[("n", "refused1")],
                )?;
                follow(b, &r)?.text()
            }
            _ => {
                let r = post_form(
                    b,
                    &format!("/qsmc/server/{VS}/power"),
                    ("apply", "Apply"),
                    &[("auto", "auto")],
                )?;
                follow(b, &r)?.text()
            }
        };
        lab.fleet.tell(0, "allow");
        if !shows(&t, id) {
            return Err(format!(
                "the {family} family refused: {what} shows {}",
                message_of(&t)
            ));
        }
        out.push(format!("{what}: {}", message_of(&t)));
    }
    Ok(out)
}

/// Suite 2.
pub fn requests(run: &crate::suite::base::Run<'_>) -> Result<String, String> {
    let mut lab = lab(run, "parts-requests")?;
    let done = scenario(&mut lab)?;
    let refused = refusals(&mut lab)?;
    let total = [fam::VRES, fam::PARTITION, fam::PROFILES, fam::HOST]
        .iter()
        .map(|f| asked(&lab.fleet, *f, ADMIN))
        .sum::<usize>();
    // Manage profiles' Edit opens the Processors tab of that profile.
    let edit = page(&mut lab.c.browser, &part(1, "processors?profile=big-test"))?;
    let des = super::field(&edit, &part(1, "processors"), "des");
    lab.c.guest.kill();
    let mut findings = done.findings;
    if des != "2.00" {
        findings.push(format!(
            "Manage profiles' Edit of big-test (2.00 units desired) shows `{des}`: the Processors \
             tab is the default profile's"
        ));
    }
    if !findings.is_empty() {
        return Err(format!(
            "{}; refusals: {}; but: {}",
            done.evidence.join("; "),
            refused.join("; "),
            findings.join("; ")
        ));
    }
    Ok(format!(
        "{}; {total} requests of the vres, partition, profiles and host families named user admin; \
         refused families shown in the host's words: {}",
        done.evidence.join("; "),
        refused.join("; ")
    ))
}

/// Suite 3.
pub fn stale(run: &crate::suite::base::Run<'_>) -> Result<String, String> {
    let mut lab = lab(run, "parts-stale")?;
    let tab = part(2, "processors");
    let mut second = lab.c.browser.fresh();
    signed_on(&mut second, ADMIN, PASSWORD)?;
    let first = &mut lab.c.browser;
    let a = page(first, &tab)?;
    let b2 = page(&mut second, &tab)?;
    let revision = |lab: &Lab<'_>| {
        host(&lab.fleet)
            .part_ref(2)
            .and_then(|p| p.kept(&p.definition.default_profile).map(|k| k.revision))
            .unwrap_or(0)
    };
    let units = |lab: &Lab<'_>| {
        host(&lab.fleet)
            .part_ref(2)
            .and_then(|p| {
                p.kept(&p.definition.default_profile)
                    .and_then(|k| k.profile.processors.desired_units)
            })
            .unwrap_or(0)
    };
    let r0 = revision(&lab);
    let r = post_on(
        &mut lab.c.browser,
        &a,
        &tab,
        ("apply", "Apply"),
        &[("des", "1.25")],
    )?;
    expect(&mut lab.c.browser, &r, "QMC3120")?;
    let r1 = revision(&lab);
    if units(&lab) != 125 || r1 == r0 {
        return Err(format!(
            "the first apply: {} units, revision {r0} to {r1}",
            units(&lab)
        ));
    }
    // The second session's form was read at the old revision.
    let r = post_on(
        &mut second,
        &b2,
        &tab,
        ("apply", "Apply"),
        &[("des", "1.75")],
    )?;
    let t = r.text();
    if r.status != 200 || !shows(&t, "QMC3201") || !shows(&t, "QSI0401") {
        return Err(format!("the stale apply: {} {}", r.status, message_of(&t)));
    }
    let kept_entry = super::field(&t, &tab, "des");
    if kept_entry != "1.75" || units(&lab) != 125 {
        return Err(format!(
            "after the stale apply the form shows `{kept_entry}` and the host has {}",
            units(&lab)
        ));
    }
    let r = post_on(&mut second, &t, &tab, ("apply", "Apply"), &[])?;
    expect(&mut second, &r, "QMC3120")?;
    let r2 = revision(&lab);
    if units(&lab) != 175 {
        return Err(format!("applied again, the host has {} units", units(&lab)));
    }
    lab.c.guest.kill();
    Ok(format!(
        "two sessions read QSTEST01's Processors tab at revision {r0}; the first applied 1.25 units \
         (revision {r1}); the second's 1.75 was refused with QMC3201 and the host's QSI0401, its form \
         shown again with 1.75 kept and the host unchanged; applied again it was taken (revision {r2}, \
         1.75 units)"
    ))
}
