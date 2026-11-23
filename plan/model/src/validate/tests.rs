// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! The fixture's plan of `QSHQVS1` against `QSHQVS2`, and each rule.

use super::*;
use crate::capture::{Meta, capture};
use crate::view::read;

fn meta(name: &str) -> Meta {
    Meta {
        name: name.into(),
        description: "QSHQVS1 as built, 3 partitions".into(),
        created: 1_791_454_320,
        by: "qsadmin".into(),
        automatic: false,
    }
}

fn hosts() -> sim::direct::Direct {
    sim::direct::Direct::new(vec![sim::fixture::qshqvs1(), sim::fixture::qshqvs2()])
}

fn baseline(d: &mut sim::direct::Direct) -> Plan {
    let v = read::host(d, "QSHQVS1", "qsadmin").expect("QSHQVS1 read");
    capture(&v, meta("qshqvs1-baseline")).expect("captured")
}

/// The rows the prototype's step 2 has, as (item, status, message).
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
    ("Partition QSDEV01", "OK", ""),
    ("Partition QSTEST01", "OK", ""),
    (
        "Partition QSPROD01",
        "Warning",
        "Depends on POOL2 and C3 (see above)",
    ),
];

/// What the served table has against the prototype's (`docs/PLANS.md`,
/// "Against the prototype"): its rows for the items the prototype does
/// not list, and the partitions' new identity on another host.
const ADDED: &[(&str, &str, &str)] = &[
    (
        "Virtual switch ETHSW1",
        "OK",
        "Will be created with VLAN 99 and no uplink",
    ),
    (
        "Shared processor pool TESTPOOL",
        "OK",
        "Will be created with cores 6, 7",
    ),
    (
        "Virtual disk VD01",
        "OK",
        "Will be created, 400 GB thick in POOL1",
    ),
    (
        "Virtual disk VD02",
        "OK",
        "Will be created, 400 GB thick in POOL1",
    ),
    (
        "Virtual disk VD03",
        "OK",
        "Will be created, 120 GB thin in POOL1",
    ),
    (
        "Virtual disk VD04",
        "OK",
        "Will be created, 120 GB thin in POOL1",
    ),
    ("Virtual disk VD05", "Error", "Its pool POOL2 is in Error"),
    ("Tape volume VTAP01", "OK", "Will be created, 200 GB"),
    ("Tape volume VTAP02", "OK", "Will be created, 200 GB"),
    (
        "Optical image QS400_V1R0M0_INSTALL.iso",
        "OK",
        "In the target's media library",
    ),
    (
        "Partition QSPROD01 tape drive C3-T1-L0",
        "Warning",
        "Tape drive C3-T1-L0 on target is absent; the device is made empty",
    ),
];

fn rows(v: &Validation) -> Vec<(String, String, String)> {
    v.items
        .iter()
        .map(|i| {
            (
                i.label.clone(),
                i.status.word().to_string(),
                i.message.clone(),
            )
        })
        .collect()
}

#[test]
fn the_fixture_s_deployment_gives_the_prototype_s_table() {
    let mut d = hosts();
    let plan = baseline(&mut d);
    let target = read::host(&mut d, "QSHQVS2", "qsadmin").unwrap();
    let v = validate(&plan, &target, &[], None);
    let got = rows(&v);
    let own = |s: &str| s.to_string();
    // Every row of the prototype's, in its order, the partitions'
    // identity on another host added to their messages.
    let mut at = 0;
    for (label, status, message) in PROTOTYPE {
        let i = got[at..]
            .iter()
            .position(|r| r.0 == *label)
            .unwrap_or_else(|| panic!("{label} is not in the table: {got:#?}"));
        let row = &got[at + i];
        at += i + 1;
        let identity = label.starts_with("Partition QS") && !label.contains("I/O");
        if identity {
            let (st, msg) = if message.is_empty() {
                (
                    "Warning",
                    own("Its MAC addresses and serial number will be new"),
                )
            } else {
                (
                    *status,
                    format!("{message}; its MAC addresses and serial number will be new"),
                )
            };
            assert_eq!(
                (row.1.as_str(), row.2.as_str()),
                (st, msg.as_str()),
                "{label}"
            );
        } else {
            assert_eq!(
                (row.1.as_str(), row.2.as_str()),
                (*status, *message),
                "{label}"
            );
        }
    }
    // And nothing else but the rows the prototype leaves out.
    for r in &got {
        let in_proto = PROTOTYPE.iter().any(|p| p.0 == r.0);
        let added = ADDED.iter().find(|a| a.0 == r.0);
        match added {
            Some(a) => assert_eq!((r.1.as_str(), r.2.as_str()), (a.1, a.2), "{}", r.0),
            None => assert!(in_proto, "{} is neither the prototype's nor listed", r.0),
        }
    }
    assert_eq!(got.len(), PROTOTYPE.len() + ADDED.len());
}

#[test]
fn a_soak_host_can_be_captured() {
    let mut d = sim::direct::Direct::new(vec![sim::fixture::fleet(1, 4)]);
    let v = read::host(&mut d, "QSSK001VS1", "qsadmin").expect("QSSK001VS1 read");
    let plan = capture(&v, meta("qssk001vs1")).expect("captured");
    assert_eq!(plan.partitions.len(), 4);
}

#[test]
fn a_plan_read_back_is_the_plan_written_and_two_captures_are_the_same() {
    let mut fleet = hosts();
    let a = baseline(&mut fleet);
    let text = crate::doc::text::write(&a).unwrap();
    let back = crate::doc::text::read(text.as_bytes()).unwrap();
    assert_eq!(back, a);
    let mut b = baseline(&mut fleet);
    b.created += 3600;
    b.by = "operator1".into();
    assert!(crate::compare::compare(&a, &b).is_empty());
    // Every definition and profile of the host.
    let h = sim::lock(&fleet.systems[0]);
    assert_eq!(a.partitions.len(), h.parts.len());
    for p in &h.parts {
        let mine = a.partition(p.definition.id).unwrap();
        assert_eq!(mine.serial, p.definition.serial);
        assert_eq!(mine.profiles.len(), p.profiles.len());
        for k in &p.profiles {
            let pr = mine
                .profiles
                .iter()
                .find(|x| x.name == k.profile.name)
                .unwrap();
            assert_eq!(crate::capture::profile_to(pr).as_ref(), Some(&k.profile));
        }
    }
    drop(h);
    // A change at the host is a difference.
    let mut c = a.clone();
    c.partitions[0].profiles[0].memory.desired_mib += 256;
    let diff = crate::compare::compare(&a, &c);
    assert_eq!(diff.len(), 1, "{diff:?}");
    assert!(diff[0].contains("memory"), "{diff:?}");
}

#[test]
fn a_damaged_large_newer_or_out_of_bounds_file_is_refused() {
    use crate::doc::text::{MAX_BYTES, Refused, read};
    let mut d = hosts();
    let text = crate::doc::text::write(&baseline(&mut d)).unwrap();
    assert!(matches!(
        read(&text.as_bytes()[..text.len() / 2]),
        Err(Refused::Invalid(_))
    ));
    assert_eq!(
        read(&vec![b' '; MAX_BYTES + 1]),
        Err(Refused::TooLarge(MAX_BYTES + 1))
    );
    assert_eq!(read(&[0xff, 0xfe, 0]), Err(Refused::NotText));
    let newer = text.replace("schema_version = 1", "schema_version = 2");
    assert_eq!(read(newer.as_bytes()), Err(Refused::Newer(2)));
    for (from, to) in [
        ("id = 1\n", "id = 1000\n"),
        ("name = \"QSDEV01\"", "name = \"QS DEV 01\""),
        ("mtu = 1500", "mtu = 99999"),
        ("name = \"qshqvs1-baseline\"", "name = \"../etc\""),
        ("thick = true", "thick = \"yes\""),
        ("cores = 8\n", "cores = 8\nkey = \"\"\n"),
    ] {
        assert!(text.contains(from), "{from}");
        let bad = text.replacen(from, to, 1);
        assert!(
            matches!(read(bad.as_bytes()), Err(Refused::Invalid(_))),
            "{to}"
        );
    }
}

#[test]
#[allow(clippy::too_many_lines, reason = "a case a rule")]
fn each_rule_has_its_outcome() {
    let mut d = hosts();
    let plan = baseline(&mut d);
    let base = read::host(&mut d, "QSHQVS2", "qsadmin").unwrap();
    let mut seen = BTreeSet::new();
    let mut run = |plan: &Plan, v: &HostView, made: &[String]| {
        for i in validate(plan, v, made, None).items {
            seen.insert(i.why);
        }
    };
    run(&plan, &base, &[]);
    // The plan's own host: everything there, used as it is.
    let vs1 = read::host(&mut d, "QSHQVS1", "qsadmin").unwrap();
    run(&plan, &vs1, &[]);
    // Not a host; not connected.
    run(
        &plan,
        &HostView {
            name: "QSHQ400".into(),
            ..HostView::default()
        },
        &[],
    );
    run(
        &plan,
        &HostView {
            name: "X".into(),
            is_host: true,
            ..HostView::default()
        },
        &[],
    );
    // Smaller, another model, too small.
    let mut small = base.clone();
    small.model = "9Q20-V1".into();
    if let Some(h) = &mut small.host {
        h.cores = 4;
        h.memory.installed = 65_536;
    }
    run(&plan, &small, &[]);
    small.model.clone_from(&base.model);
    run(&plan, &small, &[]);
    let mut tiny = base.clone();
    if let Some(h) = &mut tiny.host {
        h.memory.available = 1024;
    }
    run(&plan, &tiny, &[]);
    // A pool there with another protection and no room; a labelled disk.
    let mut other = base.clone();
    other.pools[0].protection = "raid5".into();
    other.pools[0].allocated = other.pools[0].bytes - (500 << 30);
    run(&plan, &other, &[]);
    let mut full = base.clone();
    full.pools[0].bytes = 300 << 30;
    run(&plan, &full, &[]);
    let mut labelled = base.clone();
    labelled.disks.push(qsi::host::vres::PoolDisk {
        name: "sdx".into(),
        location: sim::fixture::loc("T2-L1").replace("9F11A2", "7C22B4"),
        foreign: true,
        ..labelled.disks[0].clone()
    });
    run(&plan, &labelled, &[]);
    // A smaller disk where POOL2's was: made smaller; the disk too small.
    let mut smaller = base.clone();
    smaller.disks.push(qsi::host::vres::PoolDisk {
        name: "sdb".into(),
        location: sim::fixture::loc("T2-L1").replace("9F11A2", "7C22B4"),
        bytes: 4 << 40,
        free: true,
        foreign: false,
        pool: String::new(),
        ..smaller.disks[0].clone()
    });
    run(&plan, &smaller, &[]);
    if let Some(d) = smaller.disks.last_mut() {
        d.bytes = 8_001_563_222_016;
    }
    run(&plan, &smaller, &[]);
    // A disk there and different.
    let mut differs = base.clone();
    differs.vdisks = vs1.vdisks.clone();
    differs.vdisks[2].bytes /= 2;
    run(&plan, &differs, &[]);
    // The plan's own host without its partitions: each free, and OK.
    let mut empty = vs1.clone();
    empty.partitions.clear();
    run(&plan, &empty, &[]);
    // Switches: other VLANs, the management interface, an uplink in use.
    let mut sw = base.clone();
    sw.switches = vs1.switches.clone();
    sw.switches[1].vlans = vec![98];
    run(&plan, &sw, &[]);
    let mut mgmt = plan.clone();
    mgmt.switches[0].uplink = vec![sim::fixture::loc("C4-T1")];
    run(&mgmt, &base, &[]);
    let mut gone = plan.clone();
    gone.switches[0].uplink = vec![sim::fixture::loc("C9-T1")];
    run(&gone, &base, &[]);
    // Processor pools: fewer cores, none, one there and different.
    let mut pp = plan.clone();
    pp.proc_pools[0].cores = vec![7, 8, 9];
    run(&pp, &base, &[]);
    pp.proc_pools[0].cores = vec![9];
    run(&pp, &base, &[]);
    // Disks: thick where only thin fits; one there and different.
    let mut thin = base.clone();
    thin.pools[0].allocated = thin.pools[0].bytes - (600 << 30);
    run(&plan, &thin, &[]);
    // An image absent; a device free; a partition's ID taken.
    let mut library = base.clone();
    library.images.clear();
    library.devices = vs1.devices.clone();
    library.devices[2].holder = None;
    library.devices[2].location = sim::fixture::loc("C3").replace("9F11A2", "7C22B4");
    library.partitions = vs1.partitions[..1].to_vec();
    library.partitions[0].definition.name = "OTHER".into();
    run(&plan, &library, &[]);
    // A partition its pool's Error leaves without its load source.
    let mut req = plan.clone();
    req.partitions[2].profiles[0].scsi.retain(|a| a.slot == 4);
    run(&req, &base, &[]);
    // As an earlier deployment left it: its other profiles to make.
    let mut part = base.clone();
    let mut p1 = vs1.partitions[0].clone();
    p1.profiles.truncate(1);
    part.partitions = vec![p1];
    run(&plan, &part, &["1 QSDEV01".to_string()]);
    // A partition too big for what the host has left.
    let mut big = base.clone();
    if let Some(h) = &mut big.host {
        h.memory.available = 10_000;
    }
    run(&plan, &big, &[]);
    let missing: Vec<&Why> = rules::ALL.iter().filter(|w| !seen.contains(w)).collect();
    assert!(missing.is_empty(), "rules never met: {missing:?}");
}

#[test]
fn a_deployment_s_steps_are_in_order_and_skip_what_is_in_error() {
    let mut d = hosts();
    let plan = baseline(&mut d);
    let target = read::host(&mut d, "QSHQVS2", "qsadmin").unwrap();
    let v = validate(&plan, &target, &[], Some(&[1, 2, 3]));
    let steps = crate::deploy::steps(&plan, &v, &[1, 2, 3], true);
    let what: Vec<&str> = steps.iter().map(|s| s.what.as_str()).collect();
    assert_eq!(
        what,
        [
            "Create virtual switch ETHSW0",
            "Create virtual switch ETHSW1",
            "Create storage pool POOL2",
            "Create shared processor pool TESTPOOL",
            "Create virtual disk VD01 (400 GB, POOL1)",
            "Create virtual disk VD02 (400 GB, POOL1)",
            "Create virtual disk VD03 (120 GB, POOL1)",
            "Create virtual disk VD04 (120 GB, POOL1)",
            "Create virtual disk VD05 (2,000 GB, POOL2)",
            "Create virtual tape VTAP01 (200 GB)",
            "Create virtual tape VTAP02 (200 GB)",
            "Create partition QSDEV01, profiles big-test, default, install",
            "Create partition QSTEST01, profiles default, install, test-small",
            "Create partition QSPROD01, profile prod",
            "Activate partition QSDEV01, profile default",
            "Activate partition QSTEST01, profile install",
            "Activate partition QSPROD01, profile prod",
        ]
    );
    // POOL2 and VD05 are in Error: skipped, with their messages.
    assert!(steps[2].action.is_none() && steps[2].skipped.contains("/dev/sdb"));
    assert!(steps[8].action.is_none());
    // QSPROD01 without VD05, the C3 adapter and its drive; no MACs on
    // another host; ETHSW0 with its uplink.
    let Some(crate::deploy::Action::Partition { partition, .. }) = &steps[13].action else {
        panic!("{:?}", steps[13]);
    };
    let prod = &partition.profiles[0];
    assert!(prod.disks().all(|(_, n)| n != "VD05"));
    assert!(prod.physical_io.is_empty());
    assert!(prod.drives().next().is_none());
    assert!(prod.ethernet.iter().all(|e| e.mac.is_none()));
    assert!(partition.serial.is_empty());
    assert!(crate::deploy::uplink(steps[0].action.as_ref().unwrap()).is_some());
    // Nothing of the plan's host is a step on itself.
    let vs1 = read::host(&mut d, "QSHQVS1", "qsadmin").unwrap();
    let again = validate(&plan, &vs1, &[], None);
    assert!(crate::deploy::steps(&plan, &again, &[1, 2, 3], false).is_empty());
    assert!(again.decisions.same_host);
}

#[test]
fn a_real_machine_s_model_and_serial_are_its_own_words() {
    let mut d = hosts();
    let mut p = baseline(&mut d);
    p.host.model = "Standard PC (Q35 + ICH9, 2009)".into();
    p.pools[0].units[0].serial = "QEMU HARDDISK 0001".into();
    let text = crate::doc::text::write(&p).unwrap();
    assert_eq!(crate::doc::text::read(text.as_bytes()).unwrap(), p);
    p.host.model = "a\u{7}b".into();
    assert!(crate::doc::text::write(&p).is_err());
}
