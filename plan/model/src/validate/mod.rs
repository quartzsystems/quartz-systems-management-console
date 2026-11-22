// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! Every item of a plan against what a host reports now ([`validate`]):
//! OK, Warning or Error with a message naming the thing and the number,
//! by the rules table ([`rules`]). What each item comes to at a
//! deployment is decided here too ([`Decisions`]): what is made, how,
//! and what of each profile is left out, so a deployment does exactly
//! what its validation said.

pub mod rules;

use std::collections::{BTreeMap, BTreeSet};

use ipc::{wire_enum, wire_struct};
use qsi::host::host::Owner;

use crate::doc::{Mode, PartitionPlan, Plan, ProfilePlan};
use crate::view::{HostView, shown};

pub use rules::{Status, Why};

/// What an item is.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Kind {
    /// The managed system.
    #[default]
    System,
    /// A storage pool.
    Pool,
    /// A virtual switch.
    Switch,
    /// A shared processor pool.
    ProcPool,
    /// A virtual disk.
    Vdisk,
    /// A tape volume.
    Volume,
    /// An optical image.
    Image,
    /// A partition's physical device or tape drive.
    PhysicalIo,
    /// A partition.
    Partition,
}
wire_enum!(Kind {
    0 => System,
    1 => Pool,
    2 => Switch,
    3 => ProcPool,
    4 => Vdisk,
    5 => Volume,
    6 => Image,
    7 => PhysicalIo,
    8 => Partition,
});

/// One row of the validation.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Item {
    /// What it is.
    pub kind: Kind,
    /// Its key: a name, `POOL/NAME`, a partition's ID, `ID@location`.
    pub key: String,
    /// The row's Plan item: `Storage pool POOL1`.
    pub label: String,
    /// How it stands.
    pub status: Status,
    /// What it comes to, the thing and the number named.
    pub message: String,
    /// The rule it stands by.
    pub why: Why,
    /// A partition's: what it needs that has a Warning or an Error,
    /// as its message names them (`POOL2`, `C3`).
    pub depends: Vec<String>,
}
wire_struct!(Item {
    kind,
    key,
    label,
    status,
    message,
    why,
    depends
});

/// What a storage pool, switch, processor pool, disk or volume comes to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Make {
    /// It is there as the plan has it: used as it is.
    Exists,
    /// It is made.
    Made,
    /// It is not made: its item is in Error.
    Skipped,
}

/// What a deployment does, as its validation decided.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Decisions {
    /// The plan's host is the target (the same serial number): MAC
    /// addresses are put back.
    pub same_host: bool,
    /// Storage pools, by name: what, and the target's disks a new one
    /// is made on, in the plan's unit order.
    pub pools: BTreeMap<String, (Make, Vec<String>)>,
    /// Switches: what, and a new one's uplink: the target's interfaces
    /// at the plan's places; none to make it without.
    pub switches: BTreeMap<String, (Make, Vec<String>)>,
    /// Shared processor pools: what, and a new one's cores.
    pub proc_pools: BTreeMap<String, (Make, Vec<u32>)>,
    /// Virtual disks by `POOL/NAME`: what, and whether made thick.
    pub vdisks: BTreeMap<String, (Make, bool)>,
    /// Tape volumes.
    pub volumes: BTreeMap<String, Make>,
    /// Images in the target's library.
    pub images: BTreeMap<String, bool>,
    /// Each partition's physical devices and tape drives, by
    /// `ID@location`: the target's own location code there when it is
    /// assigned, none when it is left out.
    pub io: BTreeMap<String, Option<String>>,
    /// Partitions by ID.
    pub partitions: BTreeMap<u32, PartMake>,
}

/// What a partition comes to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PartMake {
    /// Made, with every profile.
    Made,
    /// There already: these of its profiles are made, the others are
    /// the host's already.
    Partial(Vec<String>),
    /// There as the plan has it.
    Exists,
    /// Not made: in Error.
    Skipped,
}

/// A validation: its rows, and what a deployment of it does.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Validation {
    /// The rows, in the page's order.
    pub items: Vec<Item>,
    /// What each comes to.
    pub decisions: Decisions,
}

impl Validation {
    /// The row of `kind` and `key`.
    pub fn item(&self, kind: Kind, key: &str) -> Option<&Item> {
        self.items.iter().find(|i| i.kind == kind && i.key == key)
    }

    /// Whether the managed system's row is in Error: nothing is deployed.
    pub fn refused(&self) -> bool {
        self.items
            .first()
            .is_some_and(|i| i.kind == Kind::System && i.status == Status::Error)
    }
}

const MIB: u64 = 1 << 20;

/// `131,072`.
pub fn thousands(n: u64) -> String {
    let s = n.to_string();
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// MiB as the pages say a size: `120 GB`, `2,000 GB`, `512 MB`.
pub fn size(mib: u64) -> String {
    if (mib >= 1024 && mib.is_multiple_of(1024)) || mib >= 10 * 1024 {
        format!("{} GB", thousands(mib.div_ceil(1024)))
    } else {
        format!("{} MB", thousands(mib))
    }
}

/// Hundredths of a processor: `8`, `0.50`.
pub fn units(u: u64) -> String {
    if u.is_multiple_of(100) {
        (u / 100).to_string()
    } else {
        format!("{}.{:02}", u / 100, u % 100)
    }
}

fn list<T: std::fmt::Display>(v: &[T]) -> String {
    v.iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(", ")
}

fn vlans(v: &[u16]) -> String {
    if v.len() == 1 {
        format!("VLAN {}", v[0])
    } else {
        format!("VLANs {}", list(v))
    }
}

/// `A`, `A and B`, `A, B and C`.
fn and(names: &[String]) -> String {
    match names {
        [] => String::new(),
        [a] => a.clone(),
        [rest @ .., last] => format!("{} and {last}", rest.join(", ")),
    }
}

/// What a partition's default profile needs at least: hundredths of a
/// processor and MiB.
fn minimum(p: &PartitionPlan) -> (u64, u64) {
    p.profiles
        .iter()
        .find(|x| x.name == p.default_profile)
        .map_or((0, 0), |pr| {
            let cpu = match pr.processors.mode {
                Mode::Shared => u64::from(pr.processors.minimum_units.unwrap_or(0)),
                _ => u64::from(pr.processors.minimum) * 100,
            };
            (cpu, pr.memory.minimum_mib)
        })
}

/// A thing a partition's profiles use.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Need {
    kind: Kind,
    key: String,
    required: bool,
}

fn needs(part: &PartitionPlan, default_pool: &str) -> Vec<Need> {
    let mut out = BTreeSet::new();
    let mut add = |kind, key: String, required| {
        out.insert(Need {
            kind,
            key,
            required,
        });
    };
    for pr in &part.profiles {
        if let Some(pool) = &pr.processors.pool
            && pr.processors.mode == Mode::Shared
            && pool != default_pool
        {
            add(Kind::ProcPool, pool.clone(), true);
        }
        let load: Vec<String> = pr
            .load_source()
            .into_iter()
            .map(|(p, n)| format!("{p}/{n}"))
            .collect();
        for (pool, name) in pr.disks() {
            let key = format!("{pool}/{name}");
            let req = load.contains(&key);
            add(Kind::Vdisk, key, req);
        }
        for e in &pr.ethernet {
            if let Some(s) = &e.switch {
                add(Kind::Switch, s.clone(), e.required);
            }
        }
        for v in pr.volumes() {
            add(Kind::Volume, v.to_string(), false);
        }
        for i in pr.images() {
            add(Kind::Image, i.to_string(), false);
        }
        for io in &pr.physical_io {
            add(
                Kind::PhysicalIo,
                format!("{}@{}", part.id, io.location),
                false,
            );
        }
        for d in pr.drives() {
            add(Kind::PhysicalIo, format!("{}@{}", part.id, d), false);
        }
    }
    // A thing both required and not is required.
    let mut v: Vec<Need> = out.into_iter().collect();
    let required: BTreeSet<(Kind, String)> = v
        .iter()
        .filter(|n| n.required)
        .map(|n| (n.kind, n.key.clone()))
        .collect();
    v.retain(|n| n.required || !required.contains(&(n.kind, n.key.clone())));
    v
}

struct Ctx<'a> {
    plan: &'a Plan,
    v: &'a HostView,
    items: Vec<Item>,
    d: Decisions,
}

impl Ctx<'_> {
    fn push(&mut self, kind: Kind, key: &str, label: String, why: Why, message: String) {
        self.items.push(Item {
            kind,
            key: key.to_string(),
            label,
            status: why.status(),
            message,
            why,
            depends: Vec::new(),
        });
    }

    fn status(&self, kind: Kind, key: &str) -> Option<(Status, Why)> {
        self.items
            .iter()
            .find(|i| i.kind == kind && i.key == key)
            .map(|i| (i.status, i.why))
    }
}

/// `plan` against host `v`. `made` are the partitions earlier
/// deployments made on the host (`ID NAME`); `chosen` the partitions to
/// create, every one when `None`.
pub fn validate(plan: &Plan, v: &HostView, made: &[String], chosen: Option<&[u32]>) -> Validation {
    let mut c = Ctx {
        plan,
        v,
        items: Vec::new(),
        d: Decisions {
            same_host: !plan.host.serial.is_empty() && plan.host.serial == v.serial,
            ..Decisions::default()
        },
    };
    let parts: Vec<&PartitionPlan> = plan
        .partitions
        .iter()
        .filter(|p| chosen.is_none_or(|ch| ch.contains(&p.id)))
        .collect();
    if !system(&mut c, &parts) {
        return Validation {
            items: c.items,
            decisions: c.d,
        };
    }
    let default_pool = v
        .host
        .as_ref()
        .and_then(|h| h.pools.first())
        .map_or_else(String::new, |p| p.name.clone());
    let wanted: Vec<Need> = parts.iter().flat_map(|p| needs(p, &default_pool)).collect();
    let want = |kind: Kind, key: &str| wanted.iter().any(|n| n.kind == kind && n.key == key);
    let thin = pools(&mut c, &want);
    switches(&mut c);
    proc_pools(&mut c, &default_pool);
    vdisks(&mut c, &want, &thin);
    volumes(&mut c, &want);
    images(&mut c, &want);
    for p in &parts {
        io(&mut c, p);
    }
    for p in &parts {
        partition(&mut c, p, &default_pool, made);
    }
    Validation {
        items: c.items,
        decisions: c.d,
    }
}

/// The managed system's row: false when it is in Error and nothing
/// else is checked.
fn system(c: &mut Ctx<'_>, parts: &[&PartitionPlan]) -> bool {
    let v = c.v;
    let label = format!("Managed system {}", v.name);
    if !v.is_host {
        c.push(
            Kind::System,
            &v.name,
            label,
            Why::SystemNotHost,
            format!("Not a host: {} is a standalone QS/400 system", v.name),
        );
        return false;
    }
    if !v.connected {
        let why = if v.reason.is_empty() {
            "the console has not reached it".to_string()
        } else {
            v.reason.clone()
        };
        c.push(
            Kind::System,
            &v.name,
            label,
            Why::SystemNotConnected,
            format!("Not connected: {why}"),
        );
        return false;
    }
    let Some(h) = v.host.as_ref() else {
        c.push(
            Kind::System,
            &v.name,
            label,
            Why::SystemNotHost,
            format!("Not a host: {} does not answer the host family", v.name),
        );
        return false;
    };
    let (cpu, mem) = (h.processors.available, h.memory.available);
    let have = format!("{} processors, {} MB available", units(cpu), thousands(mem));
    let fits = |p: &&PartitionPlan| {
        let (c, m) = minimum(p);
        c <= cpu && m <= mem
    };
    if !parts.is_empty() && !parts.iter().any(fits) {
        c.push(
            Kind::System,
            &v.name,
            label,
            Why::SystemTooSmall,
            format!("Not enough for any partition's minimum: {have}"),
        );
        return true;
    }
    let p = &c.plan.host;
    let mut warn = Vec::new();
    let mut why = Why::SystemOk;
    let model = if p.model.is_empty() || p.model == v.model {
        if p.model.is_empty() {
            format!("Type-Model {}", v.model)
        } else {
            format!("Type-Model {} matches", v.model)
        }
    } else {
        why = Why::SystemModel;
        warn.push(format!(
            "Type-Model {}, the plan's host is {}",
            v.model, p.model
        ));
        String::new()
    };
    if h.cores < p.cores || h.memory.installed < p.memory_mib {
        if why == Why::SystemOk {
            why = Why::SystemSmaller;
        }
        warn.push(format!(
            "{} processors and {} MB, the plan's host has {} and {} MB; enough for the partitions' minimums",
            h.cores,
            thousands(h.memory.installed),
            p.cores,
            thousands(p.memory_mib)
        ));
    }
    let message = if warn.is_empty() {
        format!("{model}; {have}")
    } else {
        warn.join("; ")
    };
    c.push(Kind::System, &v.name, label, why, message);
    true
}

/// Storage pools; the virtual disks made thin for want of room.
#[allow(clippy::too_many_lines, reason = "a pool there, then one to make")]
fn pools(c: &mut Ctx<'_>, want: &dyn Fn(Kind, &str) -> bool) -> BTreeSet<String> {
    let mut thin = BTreeSet::new();
    let (plan, v) = (c.plan, c.v);
    for p in &plan.pools {
        let label = format!("Storage pool {}", p.name);
        // Its virtual disks a deployment makes, largest first.
        let mut new: Vec<&crate::doc::VdiskPlan> = plan
            .vdisks
            .iter()
            .filter(|d| d.pool == p.name && want(Kind::Vdisk, &format!("{}/{}", d.pool, d.name)))
            .filter(|d| {
                !v.vdisks
                    .iter()
                    .any(|x| x.pool == d.pool && x.name == d.name)
            })
            .collect();
        new.sort_by_key(|d| std::cmp::Reverse(d.mib));
        let thick: u64 = new.iter().filter(|d| d.thick).map(|d| d.mib).sum();
        let named = |ds: &[&crate::doc::VdiskPlan]| {
            and(&ds
                .iter()
                .map(|d| format!("{} ({})", d.name, size(d.mib)))
                .collect::<Vec<_>>())
        };
        // Thick disks that do not fit made thin, largest first.
        let fit = |free_mib: u64, thin: &mut BTreeSet<String>| -> bool {
            let mut need = thick;
            for d in new.iter().filter(|d| d.thick) {
                if need <= free_mib {
                    break;
                }
                thin.insert(format!("{}/{}", d.pool, d.name));
                need -= d.mib;
            }
            need < thick
        };
        if let Some(t) = v.pools.iter().find(|x| x.name == p.name) {
            let total = t.bytes / MIB;
            let free = t.bytes.saturating_sub(t.allocated) / MIB;
            if let Some(big) = new.iter().find(|d| d.mib > total) {
                c.push(
                    Kind::Pool,
                    &p.name,
                    label,
                    Why::PoolNoRoom,
                    format!(
                        "Not enough room: {} is larger than the pool's {}",
                        named(&[big]),
                        size(total)
                    ),
                );
                c.d.pools.insert(p.name.clone(), (Make::Exists, Vec::new()));
                continue;
            }
            let made_thin = fit(free, &mut thin);
            let protection = crate::doc::Protection::of_words(&t.protection).ok();
            let mut msg = format!("Pool exists, {}", size(total));
            let mut why = Why::PoolExists;
            if protection != Some(p.protection) {
                why = Why::PoolProtection;
                msg = format!(
                    "Pool exists, {}, with protection {}; the plan's is {}",
                    size(total),
                    t.protection,
                    p.protection.word()
                );
            }
            if made_thin {
                msg.push_str("; thick disks that do not fit are made thin");
            }
            c.push(Kind::Pool, &p.name, label, why, msg);
            c.d.pools.insert(p.name.clone(), (Make::Exists, Vec::new()));
            continue;
        }
        // Made on the disks at the plan's location codes.
        let mut disks = Vec::new();
        let mut found_bytes = 0u64;
        let mut refused = None;
        for u in &p.units {
            let dev = if u.disk.is_empty() {
                shown(&u.location)
            } else {
                format!("/dev/{}", u.disk)
            };
            match v.disk_at(&u.location) {
                None => {
                    let what = if new.is_empty() {
                        String::new()
                    } else {
                        format!("; {} cannot be created", named(&new))
                    };
                    refused = Some((
                        Why::PoolNoDisk,
                        format!("No backing device {dev} on target{what}"),
                    ));
                    break;
                }
                Some(d) if d.foreign => {
                    refused = Some((
                        Why::PoolLabelled,
                        format!(
                            "The disk at {} on target ({}) carries another system's label",
                            shown(&u.location),
                            d.name
                        ),
                    ));
                    break;
                }
                Some(d) if !d.free || !d.pool.is_empty() => {
                    let whose = if d.pool.is_empty() {
                        d.what.clone()
                    } else {
                        format!("storage pool {}", d.pool)
                    };
                    refused = Some((
                        Why::PoolLabelled,
                        format!(
                            "The disk at {} on target ({}) carries a label: it is {whose}'s",
                            shown(&u.location),
                            d.name
                        ),
                    ));
                    break;
                }
                Some(d) => {
                    disks.push(d.name.clone());
                    found_bytes += d.bytes.min(u.bytes);
                }
            }
        }
        if let Some((why, msg)) = refused {
            c.push(Kind::Pool, &p.name, label, why, msg);
            c.d.pools
                .insert(p.name.clone(), (Make::Skipped, Vec::new()));
            continue;
        }
        let planned: u64 = p.units.iter().map(|u| u.bytes).sum();
        let estimate = if planned == 0 {
            p.bytes
        } else {
            u64::try_from(
                u128::from(p.bytes) * u128::from(found_bytes.min(planned)) / u128::from(planned),
            )
            .unwrap_or(p.bytes)
        } / MIB;
        if let Some(big) = new.iter().find(|d| d.mib > estimate) {
            c.push(
                Kind::Pool,
                &p.name,
                label,
                Why::PoolNoRoom,
                format!(
                    "Not enough room: {} is larger than the {} the pool would be",
                    named(&[big]),
                    size(estimate)
                ),
            );
            c.d.pools
                .insert(p.name.clone(), (Make::Skipped, Vec::new()));
            continue;
        }
        let made_thin = fit(estimate, &mut thin);
        let n = disks.len();
        let s = if n == 1 { "" } else { "s" };
        let (why, mut msg) = if estimate < p.bytes / MIB {
            (
                Why::PoolSmaller,
                format!(
                    "Will be created on {n} disk{s}, {}, smaller than the plan's {} but large enough",
                    size(estimate),
                    size(p.bytes / MIB)
                ),
            )
        } else {
            (
                Why::PoolMade,
                format!(
                    "Will be created with protection {} on {n} disk{s}, {}",
                    p.protection.word(),
                    size(estimate)
                ),
            )
        };
        if made_thin {
            msg.push_str("; thick disks that do not fit are made thin");
        }
        c.push(Kind::Pool, &p.name, label, why, msg);
        c.d.pools.insert(p.name.clone(), (Make::Made, disks));
    }
    thin
}

fn switches(c: &mut Ctx<'_>) {
    let (plan, v) = (c.plan, c.v);
    for s in &plan.switches {
        let label = format!("Virtual switch {}", s.name);
        if let Some(t) = v.switches.iter().find(|x| x.name == s.name) {
            let mut have = t.vlans.clone();
            have.sort_unstable();
            if have == s.vlans {
                c.push(
                    Kind::Switch,
                    &s.name,
                    label,
                    Why::SwitchExists,
                    format!("Switch exists with {}", vlans(&s.vlans)),
                );
                c.d.switches
                    .insert(s.name.clone(), (Make::Exists, Vec::new()));
            } else {
                c.push(
                    Kind::Switch,
                    &s.name,
                    label,
                    Why::SwitchOtherVlans,
                    format!(
                        "Switch exists with {}; the plan's has {}",
                        vlans(&have),
                        vlans(&s.vlans)
                    ),
                );
                c.d.switches
                    .insert(s.name.clone(), (Make::Skipped, Vec::new()));
            }
            continue;
        }
        if s.uplink.is_empty() {
            c.push(
                Kind::Switch,
                &s.name,
                label,
                Why::SwitchMade,
                format!("Will be created with {} and no uplink", vlans(&s.vlans)),
            );
            c.d.switches
                .insert(s.name.clone(), (Make::Made, Vec::new()));
            continue;
        }
        let at = s.uplink.iter().map(|l| shown(l)).collect::<Vec<_>>();
        let mut absent = None;
        let mut management = None;
        let mut targets = Vec::new();
        for l in &s.uplink {
            match v.interface_at(l) {
                None => absent = Some(format!("there is no interface at {} on target", shown(l))),
                Some((Owner::Host, _)) => management = Some(shown(l)),
                Some((Owner::None, at)) => targets.push(at.to_string()),
                Some((_, _)) => absent = Some(format!("{} on target is in use", shown(l))),
            }
        }
        if let Some(m) = management {
            c.push(
                Kind::Switch,
                &s.name,
                label,
                Why::SwitchManagement,
                format!("The uplink {m} is the target's management interface"),
            );
            c.d.switches
                .insert(s.name.clone(), (Make::Skipped, Vec::new()));
        } else if let Some(why) = absent {
            c.push(
                Kind::Switch,
                &s.name,
                label,
                Why::SwitchNoUplink,
                format!(
                    "Will be created with {} without its uplink: {why}",
                    vlans(&s.vlans)
                ),
            );
            c.d.switches
                .insert(s.name.clone(), (Make::Made, Vec::new()));
        } else {
            c.push(
                Kind::Switch,
                &s.name,
                label,
                Why::SwitchMade,
                format!("Will be created with {} on {}", vlans(&s.vlans), and(&at)),
            );
            c.d.switches.insert(s.name.clone(), (Make::Made, targets));
        }
    }
}

fn proc_pools(c: &mut Ctx<'_>, default_pool: &str) {
    let (plan, v) = (c.plan, c.v);
    let Some(h) = v.host.as_ref() else {
        return;
    };
    let taken: BTreeSet<u32> = h
        .pools
        .iter()
        .skip(1)
        .flat_map(|p| p.cores.iter().copied())
        .chain(h.reserved_cores.iter().copied())
        .collect();
    for pp in &plan.proc_pools {
        let label = format!("Shared processor pool {}", pp.name);
        let mut want = pp.cores.clone();
        want.sort_unstable();
        if pp.name == default_pool {
            c.push(
                Kind::ProcPool,
                &pp.name,
                label,
                Why::ProcPoolCannot,
                format!("Cannot be made: {} is the target's default pool", pp.name),
            );
            c.d.proc_pools
                .insert(pp.name.clone(), (Make::Skipped, Vec::new()));
            continue;
        }
        if let Some(t) = h.pools.iter().skip(1).find(|x| x.name == pp.name) {
            let mut have = t.cores.clone();
            have.sort_unstable();
            if have == want && pp.maximum_units.is_none_or(|m| m == t.maximum) {
                c.push(
                    Kind::ProcPool,
                    &pp.name,
                    label,
                    Why::ProcPoolExists,
                    format!("Exists with cores {}", list(&have)),
                );
                c.d.proc_pools
                    .insert(pp.name.clone(), (Make::Exists, Vec::new()));
            } else {
                c.push(
                    Kind::ProcPool,
                    &pp.name,
                    label,
                    Why::ProcPoolCannot,
                    format!(
                        "Cannot be made: a pool of that name exists with cores {}; the plan's are {}",
                        list(&have),
                        list(&want)
                    ),
                );
                c.d.proc_pools
                    .insert(pp.name.clone(), (Make::Skipped, Vec::new()));
            }
            continue;
        }
        let free: Vec<u32> = want
            .iter()
            .copied()
            .filter(|c| *c < h.cores && !taken.contains(c))
            .collect();
        if free.is_empty() {
            c.push(
                Kind::ProcPool,
                &pp.name,
                label,
                Why::ProcPoolCannot,
                format!(
                    "Cannot be made: the target has none of cores {} free",
                    list(&want)
                ),
            );
            c.d.proc_pools
                .insert(pp.name.clone(), (Make::Skipped, Vec::new()));
        } else if free.len() < want.len() {
            c.push(
                Kind::ProcPool,
                &pp.name,
                label,
                Why::ProcPoolFewer,
                format!(
                    "Will be created with cores {}; the plan's are {}",
                    list(&free),
                    list(&want)
                ),
            );
            c.d.proc_pools.insert(pp.name.clone(), (Make::Made, free));
        } else {
            c.push(
                Kind::ProcPool,
                &pp.name,
                label,
                Why::ProcPoolMade,
                format!("Will be created with cores {}", list(&free)),
            );
            c.d.proc_pools.insert(pp.name.clone(), (Make::Made, free));
        }
    }
}

fn kind_of(thick: bool) -> &'static str {
    if thick { "thick" } else { "thin" }
}

fn vdisks(c: &mut Ctx<'_>, want: &dyn Fn(Kind, &str) -> bool, thin: &BTreeSet<String>) {
    let (plan, v) = (c.plan, c.v);
    for d in &plan.vdisks {
        let key = format!("{}/{}", d.pool, d.name);
        if !want(Kind::Vdisk, &key) {
            continue;
        }
        let label = format!("Virtual disk {}", d.name);
        if let Some(t) = v
            .vdisks
            .iter()
            .find(|x| x.pool == d.pool && x.name == d.name)
        {
            let mib = t.bytes.div_ceil(MIB);
            if mib == d.mib && t.thick == d.thick {
                c.push(
                    Kind::Vdisk,
                    &key,
                    label,
                    Why::DiskExists,
                    format!("Exists, {} {}", size(mib), kind_of(t.thick)),
                );
                c.d.vdisks.insert(key, (Make::Exists, t.thick));
            } else {
                c.push(
                    Kind::Vdisk,
                    &key,
                    label,
                    Why::DiskOther,
                    format!(
                        "Exists as {} {}; the plan's is {} {}",
                        size(mib),
                        kind_of(t.thick),
                        size(d.mib),
                        kind_of(d.thick)
                    ),
                );
                c.d.vdisks.insert(key, (Make::Skipped, false));
            }
            continue;
        }
        let pool_ok = match c.status(Kind::Pool, &d.pool) {
            Some((s, _)) => s != Status::Error,
            None => v.pools.iter().any(|p| p.name == d.pool),
        };
        if !pool_ok {
            let why = if c.status(Kind::Pool, &d.pool).is_some() {
                format!("Its pool {} is in Error", d.pool)
            } else {
                format!(
                    "Its pool {} is neither in the plan nor on the target",
                    d.pool
                )
            };
            c.push(Kind::Vdisk, &key, label, Why::DiskPoolError, why);
            c.d.vdisks.insert(key, (Make::Skipped, false));
        } else if thin.contains(&key) {
            c.push(
                Kind::Vdisk,
                &key,
                label,
                Why::DiskThin,
                format!(
                    "Will be created thin, {}: only thin fits in {}",
                    size(d.mib),
                    d.pool
                ),
            );
            c.d.vdisks.insert(key, (Make::Made, false));
        } else {
            c.push(
                Kind::Vdisk,
                &key,
                label,
                Why::DiskMade,
                format!(
                    "Will be created, {} {} in {}",
                    size(d.mib),
                    kind_of(d.thick),
                    d.pool
                ),
            );
            c.d.vdisks.insert(key, (Make::Made, d.thick));
        }
    }
}

fn volumes(c: &mut Ctx<'_>, want: &dyn Fn(Kind, &str) -> bool) {
    let (plan, v) = (c.plan, c.v);
    // The pool the library would be in: the target's, else the plan's.
    let repository = v.repository().map(|p| p.name.clone()).or_else(|| {
        plan.pools
            .iter()
            .find(|p| p.repository)
            .map(|p| p.name.clone())
    });
    for t in &plan.volumes {
        if !want(Kind::Volume, &t.name) {
            continue;
        }
        let label = format!("Tape volume {}", t.name);
        if let Some(x) = v.volumes.iter().find(|x| x.name == t.name) {
            if x.capacity_mib == t.mib {
                c.push(
                    Kind::Volume,
                    &t.name,
                    label,
                    Why::DiskExists,
                    format!("Exists, {}", size(t.mib)),
                );
                c.d.volumes.insert(t.name.clone(), Make::Exists);
            } else {
                c.push(
                    Kind::Volume,
                    &t.name,
                    label,
                    Why::DiskOther,
                    format!(
                        "Exists as {}; the plan's is {}",
                        size(x.capacity_mib),
                        size(t.mib)
                    ),
                );
                c.d.volumes.insert(t.name.clone(), Make::Skipped);
            }
            continue;
        }
        let ok = repository.as_ref().is_some_and(|r| {
            v.repository().is_some()
                || c.status(Kind::Pool, r)
                    .is_some_and(|(s, _)| s != Status::Error)
        });
        if ok {
            c.push(
                Kind::Volume,
                &t.name,
                label,
                Why::DiskMade,
                format!("Will be created, {}", size(t.mib)),
            );
            c.d.volumes.insert(t.name.clone(), Make::Made);
        } else {
            let why = repository.as_ref().map_or_else(
                || "There is no media repository on the target or in the plan".to_string(),
                |r| format!("Its pool {r}, the media repository, is in Error"),
            );
            c.push(Kind::Volume, &t.name, label, Why::DiskPoolError, why);
            c.d.volumes.insert(t.name.clone(), Make::Skipped);
        }
    }
}

fn images(c: &mut Ctx<'_>, want: &dyn Fn(Kind, &str) -> bool) {
    let (plan, v) = (c.plan, c.v);
    for i in &plan.images {
        if !want(Kind::Image, &i.name) {
            continue;
        }
        let label = format!("Optical image {}", i.name);
        let there = v.images.iter().any(|x| x.name == i.name);
        if there {
            c.push(
                Kind::Image,
                &i.name,
                label,
                Why::ImageThere,
                "In the target's media library".into(),
            );
        } else {
            c.push(
                Kind::Image,
                &i.name,
                label,
                Why::ImageAbsent,
                "Not in the target's media library: the device is made empty".into(),
            );
        }
        c.d.images.insert(i.name.clone(), there);
    }
}

/// `Slot C3`, `Device SPCIE-SLOT-3`.
fn slot(location: &str) -> String {
    let s = shown(location);
    if s.contains(['-', ' ']) {
        format!("Device {s}")
    } else {
        format!("Slot {s}")
    }
}

fn io(c: &mut Ctx<'_>, p: &PartitionPlan) {
    let v = c.v;
    let mut devices = BTreeSet::new();
    let mut drives = BTreeSet::new();
    for pr in &p.profiles {
        devices.extend(pr.physical_io.iter().map(|i| i.location.clone()));
        drives.extend(pr.drives().map(str::to_string));
    }
    for l in devices {
        let key = format!("{}@{l}", p.id);
        let label = format!("Partition {} physical I/O {}", p.name, shown(&l));
        let at = slot(&l);
        let (why, msg) = match v.device_at(&l) {
            None => (
                Why::IoSkipped,
                format!("{at} on target is empty; adapter assignment will be skipped"),
            ),
            Some(d) if !d.released || (!d.assignable && d.blocked.is_some()) => (
                Why::IoSkipped,
                format!(
                    "{at} on target is not released for partitions; adapter assignment will be skipped"
                ),
            ),
            // Its own already: the partition is there as the plan has it.
            Some(d)
                if d.holder
                    .as_ref()
                    .is_some_and(|h| h.partition == p.id && h.name == p.name) =>
            {
                (
                    Why::IoFree,
                    format!("{at} on target is partition {}'s already", p.name),
                )
            }
            Some(d) if d.holder.is_some() => (
                Why::IoSkipped,
                format!(
                    "{at} on target is held by partition {}; adapter assignment will be skipped",
                    d.holder.as_ref().map_or("", |h| h.name.as_str())
                ),
            ),
            Some(_) => (Why::IoFree, format!("{at} on target is released and free")),
        };
        let ok = (why == Why::IoFree)
            .then(|| v.device_at(&l).map(|d| d.location.clone()))
            .flatten();
        c.push(Kind::PhysicalIo, &key, label, why, msg);
        c.d.io.insert(key, ok);
    }
    for l in drives {
        let key = format!("{}@{l}", p.id);
        let label = format!("Partition {} tape drive {}", p.name, shown(&l));
        let (why, msg) = match v.drive_at(&l) {
            None => (
                Why::IoSkipped,
                format!(
                    "Tape drive {} on target is absent; the device is made empty",
                    shown(&l)
                ),
            ),
            Some(d) if !d.released => (
                Why::IoSkipped,
                format!(
                    "Tape drive {} on target is not released for partitions; the device is made empty",
                    shown(&l)
                ),
            ),
            Some(d)
                if d.holder.partition == p.id
                    && v.partition(p.id)
                        .is_some_and(|x| x.definition.name == p.name) =>
            {
                (
                    Why::IoFree,
                    format!(
                        "Tape drive {} on target is partition {}'s already",
                        shown(&l),
                        p.name
                    ),
                )
            }
            Some(d) if d.holder.partition != 0 => (
                Why::IoSkipped,
                format!(
                    "Tape drive {} on target is held by partition {}; the device is made empty",
                    shown(&l),
                    d.holder.partition
                ),
            ),
            Some(_) => (
                Why::IoFree,
                format!("Tape drive {} on target is released and free", shown(&l)),
            ),
        };
        let ok = (why == Why::IoFree)
            .then(|| v.drive_at(&l).map(|d| d.location.clone()))
            .flatten();
        c.push(Kind::PhysicalIo, &key, label, why, msg);
        c.d.io.insert(key, ok);
    }
}

/// What a partition's dependency is called in its message: a disk in
/// Error by its pool's name, a device by its place.
fn dep_name(c: &Ctx<'_>, n: &Need) -> String {
    match n.kind {
        Kind::Vdisk => {
            let (pool, name) = n.key.split_once('/').unwrap_or(("", &n.key));
            match c.status(Kind::Vdisk, &n.key) {
                Some((_, Why::DiskPoolError)) => pool.to_string(),
                _ => name.to_string(),
            }
        }
        // A device by its place; a drive by its adapter's (`C3-T1-L0`
        // is `C3`'s).
        Kind::PhysicalIo => {
            let place = shown(n.key.split_once('@').map_or(&n.key[..], |(_, l)| l));
            match place.find("-T") {
                Some(i) => place[..i].to_string(),
                None => place,
            }
        }
        _ => n.key.clone(),
    }
}

/// A profile as a deployment puts it: MAC addresses left to the host
/// on another host, and what is in Error or skipped left out.
pub fn adjusted(pr: &ProfilePlan, id: u32, d: &Decisions) -> ProfilePlan {
    let mut p = pr.clone();
    let skipped = |m: Option<&(Make, bool)>| matches!(m, Some((Make::Skipped, _)));
    for a in &mut p.scsi {
        a.luns.retain(|l| !skipped(d.vdisks.get(&l.backing)));
    }
    p.scsi.retain(|a| !a.luns.is_empty());
    p.ethernet.retain(|e| {
        e.switch
            .as_ref()
            .is_none_or(|s| !matches!(d.switches.get(s), Some((Make::Skipped, _))))
    });
    if !d.same_host {
        for e in &mut p.ethernet {
            e.mac = None;
        }
    }
    for m in &mut p.optical {
        if let Some(n) = &m.media
            && d.images.get(n) == Some(&false)
        {
            m.media = None;
        }
    }
    // A drive and a device by the target's own location code there; one
    // left out, empty or gone.
    for m in &mut p.tape {
        let Some(x) = m.media.clone() else {
            continue;
        };
        match x.strip_prefix(qsi::host::model::DRIVE_MARK) {
            Some(l) => match d.io.get(&format!("{id}@{l}")) {
                Some(Some(at)) => m.media = Some(format!("{}{at}", qsi::host::model::DRIVE_MARK)),
                Some(None) => m.media = None,
                None => {}
            },
            None => {
                if matches!(d.volumes.get(&x), Some(Make::Skipped)) {
                    m.media = None;
                }
            }
        }
    }
    p.physical_io
        .retain_mut(|io| match d.io.get(&format!("{id}@{}", io.location)) {
            Some(Some(at)) => {
                io.location.clone_from(at);
                true
            }
            Some(None) => false,
            None => true,
        });
    p
}

/// Whether the host's profile `have` is the plan's `want` as deployed:
/// the same, a MAC address the plan leaves to the host aside.
fn same_profile(have: &ProfilePlan, want: &ProfilePlan) -> bool {
    let mut h = have.clone();
    for (e, w) in h.ethernet.iter_mut().zip(&want.ethernet) {
        if w.mac.is_none() {
            e.mac = None;
        }
    }
    h == *want
}

#[allow(
    clippy::too_many_lines,
    reason = "there already, what it needs, its size, its identity"
)]
fn partition(c: &mut Ctx<'_>, p: &PartitionPlan, default_pool: &str, made: &[String]) {
    let v = c.v;
    let key = p.id.to_string();
    let label = format!("Partition {}", p.name);
    let want: Vec<ProfilePlan> = p
        .profiles
        .iter()
        .map(|pr| adjusted(pr, p.id, &c.d))
        .collect();
    // Taken, there as the plan has it, or as an earlier deployment left it.
    let by_id = v.partition(p.id);
    let by_name = v.partition_named(&p.name);
    let mut outcome = PartMake::Made;
    if let Some(ex) = by_id {
        if ex.definition.name != p.name {
            c.push(
                Kind::Partition,
                &key,
                label,
                Why::PartTaken,
                format!(
                    "Its ID {} is taken by partition {}",
                    p.id, ex.definition.name
                ),
            );
            c.d.partitions.insert(p.id, PartMake::Skipped);
            return;
        }
        let have: Vec<ProfilePlan> = ex
            .profiles
            .iter()
            .filter_map(|x| crate::capture::profile_of(x).ok())
            .collect();
        let all_same = have.iter().all(|h| {
            want.iter()
                .find(|w| w.name == h.name)
                .is_some_and(|w| same_profile(h, w))
        });
        let mine = made.contains(&format!("{} {}", p.id, p.name));
        if all_same && have.len() == want.len() {
            c.push(
                Kind::Partition,
                &key,
                label,
                Why::PartExists,
                "Exists as the plan has it: used as it is".into(),
            );
            c.d.partitions.insert(p.id, PartMake::Exists);
            return;
        }
        if all_same && mine {
            let missing: Vec<String> = want
                .iter()
                .filter(|w| !have.iter().any(|h| h.name == w.name))
                .map(|w| w.name.clone())
                .collect();
            outcome = PartMake::Partial(missing);
        } else {
            c.push(
                Kind::Partition,
                &key,
                label,
                Why::PartTaken,
                format!(
                    "Its ID {} is taken by partition {}, which differs from the plan's",
                    p.id, p.name
                ),
            );
            c.d.partitions.insert(p.id, PartMake::Skipped);
            return;
        }
    } else if let Some(ex) = by_name {
        c.push(
            Kind::Partition,
            &key,
            label,
            Why::PartTaken,
            format!(
                "Its name {} is taken by partition {}",
                p.name, ex.definition.id
            ),
        );
        c.d.partitions.insert(p.id, PartMake::Skipped);
        return;
    }
    // What it needs.
    let mut required_error = None;
    let mut depends = Vec::new();
    for n in needs(p, default_pool) {
        let Some((st, _)) = c.status(n.kind, &n.key) else {
            continue;
        };
        if st == Status::Error && n.required && required_error.is_none() {
            required_error = Some(dep_name(c, &n));
        } else if st != Status::Ok {
            let name = dep_name(c, &n);
            if !depends.contains(&name) {
                depends.push(name);
            }
        }
    }
    if let Some(what) = required_error {
        c.push(
            Kind::Partition,
            &key,
            label,
            Why::PartRequires,
            format!("Requires {what}, which is in Error"),
        );
        c.d.partitions.insert(p.id, PartMake::Skipped);
        return;
    }
    if outcome == PartMake::Made {
        let (cpu, mem) = minimum(p);
        let h = v.host.as_ref();
        let (have_cpu, have_mem) =
            h.map_or((0, 0), |h| (h.processors.available, h.memory.available));
        if cpu > have_cpu || mem > have_mem {
            c.push(
                Kind::Partition,
                &key,
                label,
                Why::PartTooBig,
                format!(
                    "Needs {} processors and {} MB; the host has {} and {} MB left",
                    units(cpu),
                    thousands(mem),
                    units(have_cpu),
                    thousands(have_mem)
                ),
            );
            c.d.partitions.insert(p.id, PartMake::Skipped);
            return;
        }
    }
    let identity = !c.d.same_host
        && (!p.serial.is_empty()
            || p.profiles
                .iter()
                .any(|pr| pr.ethernet.iter().any(|e| e.mac.is_some())));
    let mut words = Vec::new();
    let mut why = Why::PartOk;
    if !depends.is_empty() {
        why = Why::PartNeeds;
        words.push(format!("Depends on {} (see above)", and(&depends)));
    }
    if identity {
        if why == Why::PartOk {
            why = Why::PartIdentity;
        }
        words.push(if words.is_empty() {
            "Its MAC addresses and serial number will be new".into()
        } else {
            "its MAC addresses and serial number will be new".into()
        });
    }
    if let PartMake::Partial(missing) = &outcome {
        if why == Why::PartOk {
            why = Why::PartExists;
        }
        words.insert(
            0,
            format!(
                "Made by an earlier deployment; {} to be made",
                if missing.is_empty() {
                    "its default profile".to_string()
                } else {
                    format!("profiles {}", list(missing))
                }
            ),
        );
    }
    c.push(Kind::Partition, &key, label, why, words.join("; "));
    if let Some(i) = c.items.last_mut() {
        i.depends = depends;
    }
    c.d.partitions.insert(p.id, outcome);
}

#[cfg(test)]
mod tests;
