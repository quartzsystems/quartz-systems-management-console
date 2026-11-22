// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! What differs between two plans' configurations: a line a thing,
//! named. Who made a plan, when, its name and description are not
//! configuration and are not compared; two captures of one host that
//! did not change give no line.

use crate::doc::{PartitionPlan, Plan, ProfilePlan};

fn by_name<'a, T>(
    out: &mut Vec<String>,
    what: &str,
    a: &'a [T],
    b: &'a [T],
    key: impl Fn(&T) -> String,
    mut same: impl FnMut(&mut Vec<String>, &str, &'a T, &'a T),
) {
    for x in a {
        let k = key(x);
        match b.iter().find(|y| key(y) == k) {
            Some(y) => same(out, &k, x, y),
            None => out.push(format!("{what} {k} is only in the first")),
        }
    }
    for y in b {
        let k = key(y);
        if !a.iter().any(|x| key(x) == k) {
            out.push(format!("{what} {k} is only in the second"));
        }
    }
}

fn profile(out: &mut Vec<String>, who: &str, a: &ProfilePlan, b: &ProfilePlan) {
    let parts: [(&str, bool); 9] = [
        ("processors", a.processors == b.processors),
        ("memory", a.memory == b.memory),
        ("boot", a.boot == b.boot),
        ("SCSI adapters", a.scsi == b.scsi),
        ("Ethernet adapters", a.ethernet == b.ethernet),
        ("optical devices", a.optical == b.optical),
        ("tape devices", a.tape == b.tape),
        ("physical I/O", a.physical_io == b.physical_io),
        ("name", a.name == b.name),
    ];
    for (what, same) in parts {
        if !same {
            out.push(format!("{who}, profile {}: its {what} differ", a.name));
        }
    }
}

fn partition(out: &mut Vec<String>, a: &PartitionPlan, b: &PartitionPlan) {
    let who = format!("partition {}", a.name);
    for (what, same) in [
        ("ID", a.id == b.id),
        ("serial number", a.serial == b.serial),
        ("keylock", a.keylock == b.keylock),
        ("default profile", a.default_profile == b.default_profile),
        ("start with the host", a.auto_start == b.auto_start),
        ("settings", a.settings == b.settings),
    ] {
        if !same {
            out.push(format!("{who}: its {what} differs"));
        }
    }
    by_name(
        out,
        &format!("{who}, profile"),
        &a.profiles,
        &b.profiles,
        |p| p.name.clone(),
        |out, _, x, y| profile(out, &who, x, y),
    );
}

/// Every difference of `a` and `b`; empty when they are the same
/// configuration.
pub fn compare(a: &Plan, b: &Plan) -> Vec<String> {
    let mut out = Vec::new();
    if a.host != b.host {
        out.push("the host's identity or size differs".into());
    }
    macro_rules! list {
        ($what:literal, $field:ident, $key:expr) => {
            by_name(&mut out, $what, &a.$field, &b.$field, $key, |o, k, x, y| {
                if x != y {
                    o.push(format!("{} {k} differs", $what));
                }
            });
        };
    }
    list!("storage pool", pools, |p| p.name.clone());
    list!("virtual switch", switches, |p| p.name.clone());
    list!("shared processor pool", proc_pools, |p| p.name.clone());
    list!("virtual disk", vdisks, |p| format!("{}/{}", p.pool, p.name));
    list!("tape volume", volumes, |p| p.name.clone());
    list!("image", images, |p| p.name.clone());
    by_name(
        &mut out,
        "partition",
        &a.partitions,
        &b.partitions,
        |p| p.name.clone(),
        |o, _, x, y| partition(o, x, y),
    );
    out
}
