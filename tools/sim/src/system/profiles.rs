// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! The profiles family as a QS/VS host answers it: each profile with a
//! revision that changes whenever it does, a put checked against the
//! revision it was read at, the default, a running partition's
//! configuration saved; and a profile's validation, every message at
//! once (QS/VS's `docs/PROFILE.md`).

use qsi::Empty;
use qsi::any::AnyEvent;
use qsi::host::model::{IplSource, ProcessorMode, Profile, Sharing, TapeSource};
use qsi::host::profiles::{
    self, Changed, Entry, ProfileUse, Reply, Request, Revised, SaveCurrent, Saved,
};

use super::refuse::{
    Refusal, every, media_name_ok, no, no_partition, no_such, not_activated, profile_name_ok, units,
};
use super::{Kept, System, now_s};

/// The most profiles a partition has.
pub const MAX_PROFILES: usize = 16;

/// What a profiles request comes to.
#[allow(
    clippy::many_single_char_names,
    reason = "the request, the system, the partition"
)]
pub fn answer(s: &mut System, r: &Request) -> Result<Reply, Refusal> {
    Ok(match r {
        Request::List(q) => {
            let p = s.part_ref(q.id).ok_or_else(|| no_partition(q.id))?;
            let mut v: Vec<Entry> = p
                .profiles
                .iter()
                .map(|k| Entry {
                    name: k.profile.name.clone(),
                    revision: k.revision,
                    default: k.profile.name == p.definition.default_profile,
                })
                .collect();
            v.sort_by(|a, b| a.name.cmp(&b.name));
            Reply::List(v)
        }
        Request::Get(g) => {
            let p = s.part_ref(g.id).ok_or_else(|| no_partition(g.id))?;
            let k = p
                .kept(&g.name)
                .ok_or_else(|| no_profile(&p.status.name, &g.name))?;
            Reply::Get(Revised {
                profile: k.profile.clone(),
                revision: k.revision,
            })
        }
        Request::Put(q) => Reply::Put(put(s, q.id, q.profile.clone(), q.revision)?),
        Request::Delete(d) => {
            let p = s.part_ref(d.id).ok_or_else(|| no_partition(d.id))?;
            let name = p.status.name.clone();
            if p.kept(&d.name).is_none() {
                return Err(no_profile(&name, &d.name));
            }
            if p.definition.default_profile == d.name {
                return Err(no(
                    "QVS2007",
                    format!(
                        "Profile {} is the default profile of partition {name}.",
                        d.name
                    ),
                ));
            }
            if let Some(p) = s.part(d.id) {
                p.profiles.retain(|k| k.profile.name != d.name);
            }
            changed(s, d.id, &d.name, None, false);
            Reply::Delete(Empty)
        }
        Request::SetDefault(d) => {
            let p = s.part(d.id).ok_or_else(|| no_partition(d.id))?;
            let Some(rev) = p.kept(&d.name).map(|k| k.revision) else {
                let name = p.status.name.clone();
                return Err(no_profile(&name, &d.name));
            };
            let was = std::mem::replace(&mut p.definition.default_profile, d.name.clone());
            let name = p.status.name.clone();
            let was_rev = p.kept(&was).map(|k| k.revision);
            if was != d.name && was_rev.is_some() {
                changed(s, d.id, &was, was_rev, false);
            }
            changed(s, d.id, &d.name, Some(rev), true);
            s.defined(d.id, &name, true);
            Reply::SetDefault(Empty)
        }
        Request::SaveCurrent(q) => Reply::SaveCurrent(save_current(s, q)?),
        Request::Activated(q) => {
            let p = s.part_ref(q.id).ok_or_else(|| no_partition(q.id))?;
            let mut v: Vec<ProfileUse> = p
                .profiles
                .iter()
                .map(|k| ProfileUse {
                    name: k.profile.name.clone(),
                    last: k.last,
                })
                .collect();
            v.sort_by(|a, b| a.name.cmp(&b.name));
            Reply::Activated(v)
        }
    })
}

fn no_profile(part: &str, name: &str) -> Refusal {
    no(
        "QVS2004",
        format!("Partition {part} has no profile {name}."),
    )
}

/// Say a profile changed.
fn changed(s: &mut System, id: u32, name: &str, revision: Option<u64>, default: bool) {
    s.push(&AnyEvent::Profiles(profiles::Event::Changed(Changed {
        id,
        name: name.into(),
        revision,
        default,
        time: now_s(),
    })));
}

/// Put `profile` on partition `id` at `revision`: its revision now.
#[allow(
    clippy::many_single_char_names,
    reason = "the system, the partition, an adapter"
)]
pub fn put(
    s: &mut System,
    id: u32,
    mut profile: Profile,
    revision: Option<u64>,
) -> Result<Saved, Refusal> {
    let p = s.part_ref(id).ok_or_else(|| no_partition(id))?;
    let part = p.status.name.clone();
    let there = p.kept(&profile.name).map(|k| k.revision);
    match (there, revision) {
        (Some(_), None) => {
            return Err(no(
                "QVS2012",
                format!("Partition {part} has a profile {}.", profile.name),
            ));
        }
        (Some(now), Some(r)) if now != r => {
            return Err(no(
                "QVS2013",
                format!(
                    "Partition {part}'s profile {} is not at revision {r}.",
                    profile.name
                ),
            ));
        }
        (None, Some(_)) => return Err(no_profile(&part, &profile.name)),
        (None, None) if p.profiles.len() >= MAX_PROFILES => {
            return Err(no(
                "QVS2011",
                format!(
                    "Partition {part} already has {MAX_PROFILES} profiles, the most a partition has."
                ),
            ));
        }
        _ => {}
    }
    for e in &mut profile.ethernet {
        if e.mac.is_none() {
            e.mac = Some(s.mac(id, e.slot));
        }
    }
    let problems = check(s, id, &profile)?;
    if !problems.is_empty() {
        return Err(every(problems));
    }
    let rev = s.revision();
    let Some(p) = s.part(id) else {
        return Err(no_partition(id));
    };
    let first = p.profiles.is_empty();
    let last = p.kept(&profile.name).and_then(|k| k.last);
    let name = profile.name.clone();
    p.profiles.retain(|k| k.profile.name != name);
    p.profiles.push(Kept {
        profile,
        revision: rev,
        last,
    });
    if first {
        p.definition.default_profile.clone_from(&name);
    }
    let default = p.definition.default_profile == name;
    let pname = p.status.name.clone();
    p.status.ethernet = super::activate::links(p);
    changed(s, id, &name, Some(rev), default);
    if first {
        s.defined(id, &pname, true);
    }
    Ok(Saved { revision: rev })
}

/// What partition `id` runs with now, saved as a profile.
#[allow(
    clippy::many_single_char_names,
    reason = "the system, the partition, its parts"
)]
fn save_current(s: &mut System, q: &SaveCurrent) -> Result<Saved, Refusal> {
    let p = s.part_ref(q.id).ok_or_else(|| no_partition(q.id))?;
    let name = p.status.name.clone();
    let (Some(cur), true) = (p.holds.clone(), p.active()) else {
        return Err(no(
            "QVS2015",
            format!("Partition {name} is not activated: it has no current configuration to save."),
        ));
    };
    let Some(running) = p.kept(&p.status.profile).map(|k| k.profile.clone()) else {
        return Err(not_activated(&name));
    };
    let mut pr = running;
    pr.name.clone_from(&q.name);
    let c = &mut pr.processors;
    c.desired = cur.virtuals;
    c.minimum = c.minimum.min(cur.virtuals);
    c.maximum = c.maximum.max(cur.virtuals);
    if cur.mode == ProcessorMode::Shared {
        c.desired_units = Some(cur.units);
        c.minimum_units = c.minimum_units.map(|m| m.min(cur.units));
        c.maximum_units = c.maximum_units.map(|m| m.max(cur.units));
        c.sharing = Some(if cur.capped {
            Sharing::Capped
        } else {
            Sharing::Uncapped
        });
        c.weight = (!cur.capped).then_some(cur.weight);
        c.pool = Some(cur.pool.clone());
    }
    let m = &mut pr.memory;
    m.desired_mib = cur.memory_mib;
    m.minimum_mib = m.minimum_mib.min(cur.memory_mib);
    m.maximum_mib = m.maximum_mib.max(cur.memory_mib);
    put(s, q.id, pr, q.revision)
}

impl System {
    /// The MAC address the host makes for partition `id`'s adapter in
    /// `slot`: its prefix, the ID and the slot.
    pub fn mac(&self, id: u32, slot: u32) -> String {
        let prefix = self.settings.mac_prefix.as_deref().unwrap_or("02:51:53");
        format!(
            "{prefix}:{:02x}:{:02x}:{:02x}",
            (id >> 8) & 0xff,
            id & 0xff,
            slot & 0xff
        )
    }
}

fn msg(v: &mut Refusal, id: &str, text: String) {
    v.push((id.into(), text));
}

/// A profile for partition `id`, checked as QS/VS checks it: every
/// problem, none when it is good. A value of an enumeration the host
/// does not have is refused alone.
#[allow(
    clippy::too_many_lines,
    clippy::many_single_char_names,
    reason = "one rule after another, each on a part of the profile"
)]
pub fn check(s: &System, id: u32, pr: &Profile) -> Result<Refusal, Refusal> {
    let mut v = Refusal::new();
    if !profile_name_ok(&pr.name) {
        msg(
            &mut v,
            "QVS1101",
            format!("Profile name {} is not valid.", pr.name),
        );
    }
    let c = &pr.processors;
    if matches!(c.mode, ProcessorMode::Other(_)) {
        return Err(no_such("processor mode"));
    }
    if matches!(c.sharing, Some(Sharing::Other(_))) {
        return Err(no_such("sharing mode"));
    }
    if matches!(pr.boot.source, IplSource::Other(_)) {
        return Err(no_such("IPL source"));
    }
    let shared = c.mode == ProcessorMode::Shared;
    let what = if shared {
        "virtual processors"
    } else {
        "processors"
    };
    let cores = s.host.as_ref().map_or(64, |h| h.cores);
    let most = if shared { 64 } else { cores };
    if c.minimum < 1 || c.maximum > most {
        msg(
            &mut v,
            "QVS1102",
            format!("The number of {what} is out of range: minimum 1, maximum {most}."),
        );
    }
    if !(c.minimum <= c.desired && c.desired <= c.maximum) {
        msg(
            &mut v,
            "QVS1103",
            format!(
                "Minimum {}, desired {} and maximum {} {what} are not in order.",
                c.minimum, c.desired, c.maximum
            ),
        );
    }
    if shared {
        match (c.minimum_units, c.desired_units, c.maximum_units) {
            (Some(lo), Some(d), Some(hi)) => {
                if !(lo <= d && d <= hi) {
                    msg(
                        &mut v,
                        "QVS1106",
                        format!(
                            "Minimum {}, desired {} and maximum {} processing units are not in order.",
                            units(lo),
                            units(d),
                            units(hi)
                        ),
                    );
                }
                for (level, u, vp) in [
                    ("minimum", lo, c.minimum),
                    ("desired", d, c.desired),
                    ("maximum", hi, c.maximum),
                ] {
                    if u < 5 * vp {
                        msg(
                            &mut v,
                            "QVS1107",
                            format!(
                                "The {level} of {} processing units is too few for {vp} virtual processors: at least {}.",
                                units(u),
                                units(5 * vp)
                            ),
                        );
                    } else if u > 100 * vp {
                        msg(
                            &mut v,
                            "QVS1108",
                            format!(
                                "The {level} of {} processing units is too many for {vp} virtual processors: at most {}.",
                                units(u),
                                units(100 * vp)
                            ),
                        );
                    }
                }
            }
            _ => msg(
                &mut v,
                "QVS1105",
                "Shared processors need minimum, desired and maximum processing units.".into(),
            ),
        }
        if let Some(w) = c.weight
            && w > 255
        {
            msg(
                &mut v,
                "QVS1109",
                format!("Uncapped weight {w} is not valid."),
            );
        }
    } else {
        for (field, given) in [
            (
                "Processing units",
                c.minimum_units.is_some() || c.desired_units.is_some(),
            ),
            ("A sharing mode", c.sharing.is_some()),
            ("An uncapped weight", c.weight.is_some()),
            ("A shared processor pool", c.pool.is_some()),
        ] {
            if given {
                msg(
                    &mut v,
                    "QVS1104",
                    format!("{field} is for shared processors only."),
                );
            }
        }
    }
    let m = &pr.memory;
    if !(m.minimum_mib <= m.desired_mib && m.desired_mib <= m.maximum_mib) {
        msg(
            &mut v,
            "QVS1111",
            format!(
                "Minimum {} MB, desired {} MB and maximum {} MB of memory are not in order.",
                m.minimum_mib, m.desired_mib, m.maximum_mib
            ),
        );
    }
    let block = s.settings.memory_block_mib.max(1);
    for (level, mib) in [
        ("minimum", m.minimum_mib),
        ("desired", m.desired_mib),
        ("maximum", m.maximum_mib),
    ] {
        if mib == 0 {
            msg(
                &mut v,
                "QVS1113",
                format!("The {level} memory of {mib} MB is out of range."),
            );
        } else if mib % block != 0 {
            msg(
                &mut v,
                "QVS1112",
                format!(
                    "The {level} memory of {mib} MB is not a multiple of the memory block size, {block} MB."
                ),
            );
        }
    }
    let mut slots: Vec<(u32, &str)> = Vec::new();
    let kinds = pr
        .scsi
        .iter()
        .map(|a| (a.slot, "virtual SCSI"))
        .chain(pr.ethernet.iter().map(|a| (a.slot, "virtual Ethernet")))
        .chain(pr.optical.iter().map(|a| (a.slot, "virtual optical")))
        .chain(pr.tape.iter().map(|a| (a.slot, "virtual tape")));
    for (slot, kind) in kinds {
        if !(2..=255).contains(&slot) {
            msg(
                &mut v,
                "QVS1201",
                format!("Virtual slot {slot} of a {kind} adapter is not valid."),
            );
        }
        if let Some((_, other)) = slots.iter().find(|(s_, _)| *s_ == slot) {
            msg(
                &mut v,
                "QVS1202",
                format!(
                    "Virtual slot {slot} is used twice: by a {other} adapter and a {kind} adapter."
                ),
            );
        }
        slots.push((slot, kind));
    }
    if pr.scsi.len() > 8 {
        msg(
            &mut v,
            "QVS1203",
            format!(
                "The profile has {} virtual SCSI adapters; the most is 8.",
                pr.scsi.len()
            ),
        );
    }
    if pr.scsi.iter().all(|a| a.luns.is_empty()) {
        msg(&mut v, "QVS1204", "The profile has no load source.".into());
    }
    let mut backings: Vec<&str> = Vec::new();
    for a in &pr.scsi {
        if a.luns.len() > 64 {
            msg(
                &mut v,
                "QVS1205",
                format!(
                    "The virtual SCSI adapter in slot {} has {} units; the most is 64.",
                    a.slot,
                    a.luns.len()
                ),
            );
        }
        let mut numbers: Vec<u32> = a.luns.iter().map(|l| l.lun).collect();
        numbers.sort_unstable();
        if numbers.iter().enumerate().any(|(i, n)| *n as usize != i) {
            msg(
                &mut v,
                "QVS1217",
                format!(
                    "The units of the virtual SCSI adapter in slot {} are not numbered from 0 without a gap.",
                    a.slot
                ),
            );
        }
        for l in &a.luns {
            let vdisk = l
                .backing
                .split_once('/')
                .is_some_and(|(p, n)| !p.is_empty() && !n.is_empty() && !n.contains('/'));
            if !vdisk {
                msg(
                    &mut v,
                    "QVS1218",
                    format!(
                        "The backing store of unit {} in slot {} is not a virtual disk: {}.",
                        l.lun, a.slot, l.backing
                    ),
                );
            }
            if backings.contains(&l.backing.as_str()) {
                msg(
                    &mut v,
                    "QVS1209",
                    format!(
                        "The backing store of unit {} in slot {} is another unit's too: {}",
                        l.lun, a.slot, l.backing
                    ),
                );
            }
            backings.push(&l.backing);
        }
    }
    let limit = s.vres.limits.adapters_per_partition as usize;
    if pr.ethernet.len() > limit {
        msg(
            &mut v,
            "QVS1222",
            format!(
                "The profile has {} virtual Ethernet adapters; a partition may have {limit}.",
                pr.ethernet.len()
            ),
        );
    }
    for e in &pr.ethernet {
        let slot = e.slot;
        if !(1..=4094).contains(&e.vlan) {
            msg(
                &mut v,
                "QVS1212",
                format!(
                    "VLAN {} of the virtual Ethernet adapter in slot {slot} is not valid.",
                    e.vlan
                ),
            );
        }
        for t in &e.tagged {
            if !(1..=4094).contains(t) || *t == e.vlan {
                msg(
                    &mut v,
                    "QVS1220",
                    format!(
                        "Tagged VLAN {t} of the virtual Ethernet adapter in slot {slot} is not valid."
                    ),
                );
            }
        }
        if let Some(mac) = &e.mac {
            if !mac_ok(mac) {
                msg(
                    &mut v,
                    "QVS1213",
                    format!(
                        "MAC address {mac} of the virtual Ethernet adapter in slot {slot} is not valid."
                    ),
                );
            } else if let Some(other) = s.parts.iter().filter(|p| p.definition.id != id).find(|p| {
                p.profiles.iter().any(|k| {
                    k.profile
                        .ethernet
                        .iter()
                        .any(|x| x.mac.as_deref() == Some(mac))
                })
            }) {
                msg(
                    &mut v,
                    "QVS1225",
                    format!(
                        "MAC address {mac} of the virtual Ethernet adapter in slot {slot} is also {}'s.",
                        other.status.name
                    ),
                );
            }
        }
        let Some(sw) = &e.switch else {
            msg(
                &mut v,
                "QVS1226",
                format!("The virtual Ethernet adapter in slot {slot} names no switch."),
            );
            continue;
        };
        match s.vres.switches.iter().find(|x| &x.name == sw) {
            None => msg(
                &mut v,
                "QVS1223",
                format!(
                    "The virtual Ethernet adapter in slot {slot} names switch {sw}, which does not exist."
                ),
            ),
            Some(x) => {
                for vlan in std::iter::once(&e.vlan).chain(e.tagged.iter()) {
                    if u16::try_from(*vlan).is_ok_and(|v_| !x.vlans.contains(&v_)) {
                        msg(
                            &mut v,
                            "QVS1224",
                            format!(
                                "The virtual Ethernet adapter in slot {slot} uses VLAN {vlan}, which switch {sw} does not carry."
                            ),
                        );
                    }
                }
            }
        }
    }
    for (kind, list) in [("optical", &pr.optical), ("tape", &pr.tape)] {
        for d in list {
            let bad = match (kind, d.tape_source()) {
                ("tape", TapeSource::Drive(l)) => l.is_empty(),
                (_, TapeSource::Volume(n)) => !media_name_ok(n),
                _ => false,
            };
            if bad {
                msg(
                    &mut v,
                    "QVS1214",
                    format!(
                        "The media of the virtual {kind} device in slot {} is not valid: {}",
                        d.slot,
                        d.media.clone().unwrap_or_default()
                    ),
                );
            }
        }
    }
    if pr.boot.source == IplSource::D && pr.optical.is_empty() {
        msg(
            &mut v,
            "QVS1219",
            "The profile's IPL source is D and it has no optical device.".into(),
        );
    }
    let mut seen: Vec<&str> = Vec::new();
    for d in &pr.physical_io {
        if seen.contains(&d.location.as_str()) {
            msg(
                &mut v,
                "QVS1216",
                format!("Location code {} is listed twice.", d.location),
            );
        }
        seen.push(&d.location);
        if !s.devices.iter().any(|x| x.location == d.location) {
            msg(
                &mut v,
                "QVS2401",
                format!("No device of the host is at location {}.", d.location),
            );
        }
    }
    Ok(v)
}

/// `xx:xx:xx:xx:xx:xx`, not a group address.
fn mac_ok(mac: &str) -> bool {
    let parts: Vec<&str> = mac.split(':').collect();
    parts.len() == 6
        && parts
            .iter()
            .all(|p| p.len() == 2 && p.chars().all(|c| c.is_ascii_hexdigit()))
        && u8::from_str_radix(parts[0], 16).is_ok_and(|b| b & 1 == 0)
}
