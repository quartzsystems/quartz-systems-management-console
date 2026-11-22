// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! Every name and number of a plan held to its bounds: QSI's where it
//! has them (`qsi::host::model`), the console's own otherwise. A plan
//! that is read, imported or written passes these or is refused with
//! the first thing out of bounds, named.

use std::collections::BTreeSet;

use super::{EthernetPlan, Mode, PartitionPlan, Plan, ProcessorsPlan, ProfilePlan, words::Unknown};

/// The longest plan name.
pub const MAX_PLAN_NAME: usize = 32;
/// The longest description.
pub const MAX_DESCRIPTION: usize = 256;
/// The longest name of anything a host names (QSI's).
pub const MAX_NAME: usize = qsi::host::model::MAX_NAME;
/// The longest location code or backing (QSI's text).
pub const MAX_TEXT: usize = qsi::host::model::MAX_TEXT;
/// The most items of one list (QSI's).
pub const MAX_ITEMS: usize = qsi::host::model::MAX_ITEMS;
/// The most partitions: QSI's highest ID.
pub const MAX_PARTITIONS: usize = qsi::host::model::MAX_ID as usize;
/// The most cores a host is planned with.
pub const MAX_CORES: u32 = 4096;
/// The most memory, MiB: 64 TiB.
pub const MAX_MIB: u64 = 64 << 20;
/// The largest disk, pool or volume, MiB: 1 PiB.
pub const MAX_DISK_MIB: u64 = 1 << 30;
/// The highest slot of a profile's adapter.
pub const MAX_SLOT: u32 = 1023;
/// The highest weight (QSI's).
pub const MAX_WEIGHT: u32 = qsi::host::partition::MAX_WEIGHT;
/// The longest delay, seconds (QSI's).
pub const MAX_DELAY: u32 = qsi::host::partition::MAX_DELAY;

fn graphic(s: &str) -> bool {
    s.bytes().all(|b| b.is_ascii_graphic())
}

/// Whether `n` is a plan's name.
pub fn plan_name_ok(n: &str) -> bool {
    let mut b = n.bytes();
    let first = b.next();
    first.is_some_and(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
        && n.len() <= MAX_PLAN_NAME
        && b.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || b"-_.".contains(&c))
}

/// A thing's name: 1 to [`MAX_NAME`] printable characters, no space.
pub fn name(what: &str, n: &str) -> Result<(), String> {
    if n.is_empty() || n.len() > MAX_NAME || !graphic(n) {
        return Err(format!(
            "{what} \"{}\": a name is 1 to {MAX_NAME} printable characters without spaces",
            shown(n)
        ));
    }
    Ok(())
}

/// A location code or a backing: 1 to [`MAX_TEXT`] printable characters.
pub fn text(what: &str, n: &str) -> Result<(), String> {
    if n.is_empty() || n.len() > MAX_TEXT || !graphic(n) {
        return Err(format!(
            "{what} \"{}\": 1 to {MAX_TEXT} printable characters without spaces",
            shown(n)
        ));
    }
    Ok(())
}

/// A person's words: at most [`MAX_DESCRIPTION`] bytes, no control
/// characters.
pub fn words(what: &str, n: &str) -> Result<(), String> {
    if n.len() > MAX_DESCRIPTION || n.chars().any(char::is_control) {
        return Err(format!(
            "{what}: at most {MAX_DESCRIPTION} bytes and no control characters"
        ));
    }
    Ok(())
}

/// The first 40 characters of `s`, for a message.
fn shown(s: &str) -> String {
    s.chars()
        .take(40)
        .map(|c| if c.is_control() { '?' } else { c })
        .collect()
}

fn count<T>(what: &str, items: &[T], max: usize) -> Result<(), String> {
    if items.len() > max {
        return Err(format!("{} {what}: at most {max}", items.len()));
    }
    Ok(())
}

fn within<T: PartialOrd + std::fmt::Display + Copy>(
    what: &str,
    v: T,
    low: T,
    high: T,
) -> Result<(), String> {
    if v < low || v > high {
        return Err(format!("{what} {v}: outside {low} to {high}"));
    }
    Ok(())
}

fn unique<'a>(what: &str, names: impl Iterator<Item = &'a str>) -> Result<(), String> {
    let mut seen = BTreeSet::new();
    for n in names {
        if !seen.insert(n) {
            return Err(format!("{what} {n} is named twice"));
        }
    }
    Ok(())
}

fn set<T: Copy>(what: &str, v: Option<T>) -> Result<T, String> {
    v.ok_or_else(|| format!("{what} is missing"))
}

fn vlan(what: &str, v: u32) -> Result<(), String> {
    within(what, v, 1, 4094)
}

fn mac_ok(m: &str) -> bool {
    let parts: Vec<&str> = m.split(':').collect();
    parts.len() == 6
        && parts
            .iter()
            .all(|p| p.len() == 2 && p.bytes().all(|b| b.is_ascii_hexdigit()))
}

/// The whole plan.
#[allow(
    clippy::too_many_lines,
    reason = "every list of the document, in its order"
)]
pub fn plan(p: &Plan) -> Result<(), String> {
    if !plan_name_ok(&p.name) {
        return Err(format!(
            "plan name \"{}\": 1 to {MAX_PLAN_NAME} of a-z, 0-9, -, _ and ., a letter or digit first",
            shown(&p.name)
        ));
    }
    words("the description", &p.description)?;
    name("the user", &p.by)?;
    if !p.system.is_empty() {
        name("managed system", &p.system)?;
    }
    if p.automatic && p.system.is_empty() {
        return Err("an automatic plan names its managed system".into());
    }
    let h = &p.host;
    for (what, v) in [
        ("the host's name", &h.name),
        ("the host's type and model", &h.model),
        ("the host's serial number", &h.serial),
    ] {
        if !v.is_empty() {
            name(what, v)?;
        }
    }
    within("the host's cores", h.cores, 1, MAX_CORES)?;
    within("the host's memory (MiB)", h.memory_mib, 1, MAX_MIB)?;
    count("storage pools", &p.pools, 64)?;
    unique("storage pool", p.pools.iter().map(|x| x.name.as_str()))?;
    for pool in &p.pools {
        name("storage pool", &pool.name)?;
        set(
            &format!("storage pool {}'s protection", pool.name),
            pool.protection.qsi(),
        )?;
        within(
            &format!("storage pool {}'s size (bytes)", pool.name),
            pool.bytes,
            0,
            MAX_DISK_MIB << 20,
        )?;
        count(
            &format!("units of storage pool {}", pool.name),
            &pool.units,
            MAX_ITEMS,
        )?;
        unique(
            &format!("a unit of storage pool {}: location", pool.name),
            pool.units.iter().map(|u| u.location.as_str()),
        )?;
        for u in &pool.units {
            text(
                &format!("a unit of storage pool {}", pool.name),
                &u.location,
            )?;
            if !u.disk.is_empty() {
                name("a unit's disk", &u.disk)?;
            }
            if !u.serial.is_empty() {
                name("a unit's serial number", &u.serial)?;
            }
            within("a unit's size (bytes)", u.bytes, 0, MAX_DISK_MIB << 20)?;
        }
    }
    count("virtual switches", &p.switches, MAX_ITEMS)?;
    unique("virtual switch", p.switches.iter().map(|x| x.name.as_str()))?;
    for s in &p.switches {
        name("virtual switch", &s.name)?;
        count(&format!("VLANs of switch {}", s.name), &s.vlans, 4094)?;
        for v in &s.vlans {
            vlan(&format!("switch {}'s VLAN", s.name), u32::from(*v))?;
        }
        within(&format!("switch {}'s MTU", s.name), s.mtu, 576, 9216)?;
        count(
            &format!("uplink interfaces of switch {}", s.name),
            &s.uplink,
            8,
        )?;
        for i in &s.uplink {
            text(&format!("switch {}'s uplink", s.name), i)?;
        }
        if let Some(b) = s.bond {
            set(&format!("switch {}'s bond", s.name), b.qsi())?;
        }
        if let Some(v) = s.native_vlan {
            vlan(&format!("switch {}'s native VLAN", s.name), u32::from(v))?;
        }
    }
    count("shared processor pools", &p.proc_pools, 64)?;
    unique(
        "shared processor pool",
        p.proc_pools.iter().map(|x| x.name.as_str()),
    )?;
    for pp in &p.proc_pools {
        name("shared processor pool", &pp.name)?;
        count(
            &format!("cores of pool {}", pp.name),
            &pp.cores,
            MAX_CORES as usize,
        )?;
        for c in &pp.cores {
            within(&format!("pool {}'s core", pp.name), *c, 0, MAX_CORES - 1)?;
        }
        if let Some(u) = pp.maximum_units {
            within(
                &format!("pool {}'s maximum", pp.name),
                u,
                1,
                MAX_CORES * 100,
            )?;
        }
    }
    count("virtual disks", &p.vdisks, MAX_CORES as usize)?;
    let keys: Vec<String> = p
        .vdisks
        .iter()
        .map(|v| format!("{}/{}", v.pool, v.name))
        .collect();
    unique("virtual disk", keys.iter().map(String::as_str))?;
    for v in &p.vdisks {
        name("virtual disk", &v.name)?;
        name(&format!("virtual disk {}'s pool", v.name), &v.pool)?;
        within(
            &format!("virtual disk {}'s size (MiB)", v.name),
            v.mib,
            1,
            MAX_DISK_MIB,
        )?;
    }
    count("tape volumes", &p.volumes, MAX_CORES as usize)?;
    unique("tape volume", p.volumes.iter().map(|v| v.name.as_str()))?;
    for v in &p.volumes {
        name("tape volume", &v.name)?;
        if !v.identifier.is_empty() {
            name(
                &format!("tape volume {}'s identifier", v.name),
                &v.identifier,
            )?;
        }
        within(
            &format!("tape volume {}'s capacity (MiB)", v.name),
            v.mib,
            1,
            MAX_DISK_MIB,
        )?;
    }
    count("images", &p.images, MAX_CORES as usize)?;
    unique("image", p.images.iter().map(|v| v.name.as_str()))?;
    for i in &p.images {
        name("image", &i.name)?;
    }
    count("partitions", &p.partitions, MAX_PARTITIONS)?;
    unique("partition", p.partitions.iter().map(|x| x.name.as_str()))?;
    let mut ids = BTreeSet::new();
    for part in &p.partitions {
        if !ids.insert(part.id) {
            return Err(format!("partition ID {} is given twice", part.id));
        }
        partition(part)?;
    }
    Ok(())
}

fn partition(part: &PartitionPlan) -> Result<(), String> {
    within("partition ID", part.id, 1, qsi::host::model::MAX_ID)?;
    name(&format!("partition {}", part.id), &part.name)?;
    let who = format!("partition {}", part.name);
    if !part.serial.is_empty() {
        name(&format!("{who}'s serial number"), &part.serial)?;
    }
    set(&format!("{who}'s keylock"), part.keylock.qsi())?;
    let a = &part.auto_start;
    within(&format!("{who}'s start order"), a.order, 0, 999)?;
    within(&format!("{who}'s start delay"), a.delay_secs, 0, 3600)?;
    within(&format!("{who}'s start wait"), a.wait_secs, 0, 3600)?;
    if let Some(s) = &part.settings {
        words(&format!("{who}'s description"), &s.description)?;
        set(
            &format!("{who}'s shut down with the host"),
            s.with_host.qsi(),
        )?;
        within(&format!("{who}'s shut down delay"), s.delay, 0, MAX_DELAY)?;
    }
    if part.profiles.is_empty() {
        return Err(format!("{who} has no profile"));
    }
    count(&format!("profiles of {who}"), &part.profiles, MAX_ITEMS)?;
    unique(
        &format!("{who}'s profile"),
        part.profiles.iter().map(|x| x.name.as_str()),
    )?;
    if !part.profiles.iter().any(|x| x.name == part.default_profile) {
        return Err(format!(
            "{who}'s default profile {} is not one of its profiles",
            part.default_profile
        ));
    }
    for pr in &part.profiles {
        profile(&format!("{who}, profile {}", pr.name), pr)?;
    }
    Ok(())
}

fn processors(who: &str, p: &ProcessorsPlan) -> Result<(), String> {
    set(&format!("{who}: processor mode"), p.mode.qsi())?;
    let cap = MAX_CORES;
    within(&format!("{who}: minimum processors"), p.minimum, 1, cap)?;
    within(
        &format!("{who}: desired processors"),
        p.desired,
        p.minimum,
        cap,
    )?;
    within(
        &format!("{who}: maximum processors"),
        p.maximum,
        p.desired,
        cap,
    )?;
    if p.mode == Mode::Shared {
        let min = set(&format!("{who}: minimum units"), p.minimum_units)?;
        let des = set(&format!("{who}: desired units"), p.desired_units)?;
        let max = set(&format!("{who}: maximum units"), p.maximum_units)?;
        within(&format!("{who}: minimum units"), min, 1, cap * 100)?;
        within(&format!("{who}: desired units"), des, min, cap * 100)?;
        within(&format!("{who}: maximum units"), max, des, cap * 100)?;
        if let Some(s) = p.sharing {
            set(&format!("{who}: sharing"), s.qsi())?;
        }
        if let Some(pool) = &p.pool {
            name(&format!("{who}: processor pool"), pool)?;
        }
    } else if p.minimum_units.is_some() || p.pool.is_some() || p.sharing.is_some() {
        return Err(format!(
            "{who}: dedicated processors have no units, sharing or pool"
        ));
    }
    if let Some(w) = p.weight {
        within(&format!("{who}: weight"), w, 0, MAX_WEIGHT)?;
    }
    Ok(())
}

fn ethernet(who: &str, e: &EthernetPlan) -> Result<(), String> {
    let at = format!("{who}: Ethernet slot {}", e.slot);
    if let Some(s) = &e.switch {
        name(&format!("{at}'s switch"), s)?;
    }
    vlan(&format!("{at}'s VLAN"), e.vlan)?;
    count(&format!("{at}'s tagged VLANs"), &e.tagged, 4094)?;
    for t in &e.tagged {
        vlan(&format!("{at}'s tagged VLAN"), *t)?;
    }
    if let Some(m) = &e.mac
        && !mac_ok(m)
    {
        return Err(format!("{at}: MAC address \"{}\" is not one", shown(m)));
    }
    Ok(())
}

fn profile(who: &str, pr: &ProfilePlan) -> Result<(), String> {
    name(who, &pr.name)?;
    processors(who, &pr.processors)?;
    let m = pr.memory;
    within(&format!("{who}: minimum memory"), m.minimum_mib, 1, MAX_MIB)?;
    within(
        &format!("{who}: desired memory"),
        m.desired_mib,
        m.minimum_mib,
        MAX_MIB,
    )?;
    within(
        &format!("{who}: maximum memory"),
        m.maximum_mib,
        m.desired_mib,
        MAX_MIB,
    )?;
    set(&format!("{who}: IPL source"), pr.boot.source.qsi())?;
    set(&format!("{who}: IPL type"), pr.boot.ipl_type.qsi())?;
    set(&format!("{who}: console"), pr.boot.console.qsi())?;
    for (what, n) in [
        ("SCSI adapters", pr.scsi.len()),
        ("Ethernet adapters", pr.ethernet.len()),
        ("optical devices", pr.optical.len()),
        ("tape devices", pr.tape.len()),
        ("physical devices", pr.physical_io.len()),
    ] {
        if n > MAX_ITEMS {
            return Err(format!("{who}: {n} {what}, at most {MAX_ITEMS}"));
        }
    }
    let mut slots = BTreeSet::new();
    let all = pr
        .scsi
        .iter()
        .map(|a| a.slot)
        .chain(pr.ethernet.iter().map(|a| a.slot))
        .chain(pr.optical.iter().map(|a| a.slot))
        .chain(pr.tape.iter().map(|a| a.slot));
    for s in all {
        within(&format!("{who}: slot"), s, 0, MAX_SLOT)?;
        if !slots.insert(s) {
            return Err(format!("{who}: slot {s} is given twice"));
        }
    }
    for a in &pr.scsi {
        count(
            &format!("{who}: units of SCSI slot {}", a.slot),
            &a.luns,
            MAX_ITEMS,
        )?;
        for l in &a.luns {
            let at = format!("{who}: SCSI slot {} unit {}", a.slot, l.lun);
            within(&at, l.lun, 0, 255)?;
            text(&at, &l.backing)?;
            let Some((pool, disk)) = Plan::backing(&l.backing) else {
                return Err(format!("{at}: a backing is POOL/NAME"));
            };
            name(&format!("{at}'s pool"), pool)?;
            name(&format!("{at}'s disk"), disk)?;
            if let Some(s) = &l.serial {
                name(&format!("{at}'s serial number"), s)?;
            }
        }
    }
    for e in &pr.ethernet {
        ethernet(who, e)?;
    }
    for m in pr.optical.iter().chain(&pr.tape) {
        if let Some(n) = &m.media {
            match n.strip_prefix(qsi::host::model::DRIVE_MARK) {
                Some(l) => text(&format!("{who}: slot {}'s drive", m.slot), l)?,
                None => name(&format!("{who}: slot {}'s media", m.slot), n)?,
            }
        }
    }
    for io in &pr.physical_io {
        text(&format!("{who}: physical I/O"), &io.location)?;
    }
    Ok(())
}

/// A value QSI has and the document does not, as a refusal's words.
pub fn unknown(e: &Unknown) -> String {
    e.to_string()
}
