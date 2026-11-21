// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! The partition family as a QS/VS host answers it: create (at a chosen
//! ID too), change and delete a definition, its settings, activate,
//! shut down each way, restart, and a running partition's entitlement.

use qsi::Empty;
use qsi::any::AnyEvent;
use qsi::host::model::{
    AutoStart, Channel, Current, Definition, Environment, PartitionState, ProcessorMode, Sharing,
};
use qsi::host::partition::{
    self, Change, CreateWith, Delete, Entitlement, EntitlementChanged, Reply, Request, Settings,
    ShutDown, ShutdownKind, WithHost,
};
use qsi::host::partitions::Status;
use qsi::host::profiles;

use super::refuse::{Refusal, activated, name_ok, no, no_partition, no_such, not_activated, units};
use super::{Part, System, codes, now_s};

/// What a partition request comes to.
pub fn answer(s: &mut System, r: &Request, user: &str) -> Result<Reply, Refusal> {
    Ok(match r {
        Request::Create(c) => Reply::Create(s.create(None, &c.name, None)?),
        Request::CreateWith(c) => Reply::CreateWith(create_with(s, c)?),
        Request::Change(c) => {
            change(s, c)?;
            Reply::Change(Empty)
        }
        Request::Delete(d) => {
            delete(s, *d)?;
            Reply::Delete(Empty)
        }
        Request::Activate(a) => Reply::Activate(s.activate(a)?),
        Request::ShutDown(d) => {
            shut_down(s, d)?;
            Reply::ShutDown(Empty)
        }
        Request::Restart(q) => {
            let p = s.part_ref(q.id).ok_or_else(|| no_partition(q.id))?;
            if !p.active() {
                return Err(not_activated(&p.status.name));
            }
            codes::restart(s, q.id);
            Reply::Restart(Empty)
        }
        Request::Entitlement(e) => Reply::Entitlement(entitlement(s, e, user)?),
        Request::GetSettings(g) => Reply::GetSettings(
            s.part_ref(g.id)
                .ok_or_else(|| no_partition(g.id))?
                .settings
                .clone(),
        ),
        Request::SetSettings(q) => {
            if matches!(q.settings.with_host, WithHost::Other(_)) {
                return Err(no_such("shut down with the host"));
            }
            let p = s.part(q.id).ok_or_else(|| no_partition(q.id))?;
            p.settings = q.settings.clone();
            let name = p.status.name.clone();
            s.defined(q.id, &name, true);
            Reply::SetSettings(Empty)
        }
    })
}

impl System {
    /// Create a partition named `name`, at `id` or the lowest free one:
    /// its definition.
    pub fn create(
        &mut self,
        id: Option<u32>,
        name: &str,
        settings: Option<Settings>,
    ) -> Result<Definition, Refusal> {
        let max = self.settings.max_partitions;
        if !name_ok(name) {
            return Err(no(
                "QVS1001",
                format!("Partition name {name} is not valid."),
            ));
        }
        if self.parts.iter().any(|p| p.status.name == name) {
            return Err(no(
                "QVS1005",
                format!("Partition name {name} is already in use."),
            ));
        }
        if self.parts.len() >= max as usize {
            return Err(no(
                "QVS2006",
                format!("The host already has {max} partitions, the most it is set to have."),
            ));
        }
        let id = match id {
            Some(id) if self.part_ref(id).is_some() => {
                return Err(no("QVS2016", format!("Partition ID {id} is in use.")));
            }
            Some(id) if id > max => {
                return Err(no(
                    "QVS2017",
                    format!("Partition ID {id} is past this host's limit of {max} partitions."),
                ));
            }
            Some(id) => id,
            None => (1..=max)
                .find(|i| self.part_ref(*i).is_none())
                .unwrap_or(max),
        };
        let serial = format!("K2M41{id:02}");
        let definition = Definition {
            id,
            name: name.into(),
            environment: Environment::Standalone,
            serial,
            keylock: qsi::base::Keylock::Normal,
            default_profile: String::new(),
            auto_start: AutoStart::default(),
            created: now_s(),
        };
        let mut p = Part::new(
            definition.clone(),
            Status {
                id,
                name: name.into(),
                state: PartitionState::NotActivated,
                code: String::new(),
                text: String::new(),
                current: None,
                profile: String::new(),
                last_end: String::new(),
                message: None,
                ethernet: Vec::new(),
                channel: Channel::default(),
            },
        );
        if let Some(st) = settings {
            p.settings = st;
        }
        self.parts.push(p);
        self.parts.sort_by_key(|p| p.definition.id);
        if let Some(h) = &mut self.host {
            h.partitions += 1;
        }
        self.defined(id, name, true);
        Ok(definition)
    }
}

fn create_with(s: &mut System, c: &CreateWith) -> Result<Definition, Refusal> {
    if matches!(c.settings.with_host, WithHost::Other(_)) {
        return Err(no_such("shut down with the host"));
    }
    s.create(c.id, &c.name, Some(c.settings.clone()))
}

fn change(s: &mut System, c: &Change) -> Result<(), Refusal> {
    let d = &c.definition;
    let p = s.part_ref(d.id).ok_or_else(|| no_partition(d.id))?;
    let name = p.status.name.clone();
    if p.active() {
        return Err(activated(&name));
    }
    let was = &p.definition;
    for (what, same) in [
        ("serial number", was.serial == d.serial),
        ("environment", was.environment == d.environment),
        ("creation time", was.created == d.created),
    ] {
        if !same {
            return Err(no(
                "QVS1006",
                format!("The {what} of partition {name} cannot be changed."),
            ));
        }
    }
    if !name_ok(&d.name) {
        return Err(no(
            "QVS1001",
            format!("Partition name {} is not valid.", d.name),
        ));
    }
    if d.name != name && s.parts.iter().any(|q| q.status.name == d.name) {
        return Err(no(
            "QVS1005",
            format!("Partition name {} is already in use.", d.name),
        ));
    }
    if !d.default_profile.is_empty() && p.kept(&d.default_profile).is_none() {
        return Err(no(
            "QVS1007",
            format!(
                "Partition {name} has no profile {} to be its default.",
                d.default_profile
            ),
        ));
    }
    if matches!(d.keylock, qsi::base::Keylock::Other(_)) {
        return Err(no_such("keylock position"));
    }
    let p = s.part(d.id).ok_or_else(|| no_partition(d.id))?;
    p.definition = d.clone();
    p.status.name.clone_from(&d.name);
    s.defined(d.id, &d.name, true);
    Ok(())
}

fn delete(s: &mut System, d: Delete) -> Result<(), Refusal> {
    let p = s.part_ref(d.id).ok_or_else(|| no_partition(d.id))?;
    let name = p.status.name.clone();
    if p.active() {
        return Err(activated(&name));
    }
    let backings: Vec<String> = p
        .profiles
        .iter()
        .flat_map(|k| k.profile.scsi.iter())
        .flat_map(|a| a.luns.iter().map(|l| l.backing.clone()))
        .collect();
    if d.disks {
        // Every one or none: a disk another partition's profile names
        // keeps them all.
        for b in &backings {
            if let Some((other, profile)) = s
                .parts
                .iter()
                .filter(|q| q.definition.id != d.id)
                .find_map(|q| {
                    q.profiles
                        .iter()
                        .find(|k| {
                            k.profile
                                .scsi
                                .iter()
                                .any(|a| a.luns.iter().any(|l| &l.backing == b))
                        })
                        .map(|k| (q.status.name.clone(), k.profile.name.clone()))
                })
            {
                return Err(no(
                    "QVS3108",
                    format!("Virtual disk {b} is named by partition {other}, profile {profile}."),
                ));
            }
        }
    }
    s.release(d.id);
    let gone: Vec<String> = s.part_ref(d.id).map_or_else(Vec::new, |p| {
        p.profiles.iter().map(|k| k.profile.name.clone()).collect()
    });
    s.parts.retain(|p| p.definition.id != d.id);
    if let Some(h) = &mut s.host {
        h.partitions = h.partitions.saturating_sub(1);
    }
    for profile in gone {
        s.push(&AnyEvent::Profiles(profiles::Event::Changed(
            profiles::Changed {
                id: d.id,
                name: profile,
                revision: None,
                default: false,
                time: now_s(),
            },
        )));
    }
    if d.disks {
        for b in backings {
            if let Some((pool, disk)) = b.split_once('/') {
                let _ = super::vres::storage::delete_vdisk(s, pool, disk, true);
            }
        }
    }
    s.defined(d.id, &name, false);
    Ok(())
}

fn shut_down(s: &mut System, d: &ShutDown) -> Result<(), Refusal> {
    let p = s.part_ref(d.id).ok_or_else(|| no_partition(d.id))?;
    let name = p.status.name.clone();
    if matches!(d.kind, ShutdownKind::Other(_)) {
        return Err(no_such("shut down kind"));
    }
    if !p.active() {
        return Err(not_activated(&name));
    }
    if p.status.state == PartitionState::ShuttingDown {
        return Err(no(
            "QVS2009",
            format!("Partition {name} is already shutting down."),
        ));
    }
    if d.kind != ShutdownKind::Immediate && p.status.channel.none {
        let kind = if d.kind == ShutdownKind::Delayed {
            "delayed"
        } else {
            "operating system"
        };
        return Err(no(
            "QVS2301",
            format!(
                "Partition {name} has no service channel: a {kind} shut down cannot be asked of its system."
            ),
        ));
    }
    let profile = p.status.profile.clone();
    codes::shut_down(s, d.id, d.kind, d.delay);
    if d.restart {
        let a = partition::Activate {
            id: d.id,
            profile,
            source: None,
            keylock: None,
        };
        s.activate(&a)?;
    }
    Ok(())
}

/// A running shared partition's entitlement, changed as QS/VS changes
/// it (`docs/RESOURCES.md`): within its profile, its virtual
/// processors' worth and its pool.
#[allow(
    clippy::too_many_lines,
    reason = "each rule, then the change and its events"
)]
fn entitlement(s: &mut System, e: &Entitlement, user: &str) -> Result<Current, Refusal> {
    if matches!(e.sharing, Some(Sharing::Other(_))) {
        return Err(no_such("sharing mode"));
    }
    let p = s.part_ref(e.id).ok_or_else(|| no_partition(e.id))?;
    let name = p.status.name.clone();
    let (Some(before), PartitionState::Running) = (p.holds.clone(), p.status.state) else {
        return Err(no("QVS2501", format!("Partition {name} is not running.")));
    };
    if before.mode == ProcessorMode::Dedicated {
        return Err(no(
            "QVS2502",
            format!("Partition {name} has dedicated processors."),
        ));
    }
    let pr = p.in_force().map(|pr| pr.processors.clone());
    let want = e.units.unwrap_or(before.units);
    let (least, most) = pr.as_ref().map_or((5, 100 * before.virtuals), |pr| {
        (
            pr.minimum_units.unwrap_or(5),
            pr.maximum_units.unwrap_or(100 * before.virtuals),
        )
    });
    if !(least..=most).contains(&want) {
        return Err(no(
            "QVS2503",
            format!(
                "{} processing units are outside partition {name}'s profile: {} to {}.",
                units(want),
                units(least),
                units(most)
            ),
        ));
    }
    let v = before.virtuals;
    if !(5 * v..=100 * v).contains(&want) {
        return Err(no(
            "QVS2504",
            format!(
                "{} processing units do not suit partition {name}'s {v} virtual processors: {} to {}.",
                units(want),
                units(5 * v),
                units(100 * v)
            ),
        ));
    }
    let pool_left = s
        .host
        .as_ref()
        .and_then(|h| h.pools.iter().find(|x| x.name == before.pool))
        .map_or(0, |x| x.available);
    if want > pool_left + before.units {
        return Err(no(
            "QVS2505",
            format!(
                "Pool {} has not {} processing units for partition {name}: {} are available to it.",
                before.pool,
                units(want),
                units(pool_left + before.units)
            ),
        ));
    }
    let capped = match e.sharing {
        Some(Sharing::Capped) => true,
        Some(_) => false,
        None => before.capped,
    };
    let weight = if capped {
        0
    } else {
        e.weight
            .or((before.weight > 0).then_some(before.weight))
            .or(pr.and_then(|p| p.weight))
            .unwrap_or(128)
    };
    let mut current = before.clone();
    current.units = want;
    current.capped = capped;
    current.weight = weight;
    s.account(&before, false);
    s.account(&current, true);
    let Some(p) = s.part(e.id) else {
        return Err(no_partition(e.id));
    };
    p.holds = Some(current.clone());
    p.status.current = Some(current.clone());
    s.push(&AnyEvent::Partition(partition::Event::Entitlement(
        EntitlementChanged {
            id: e.id,
            name: name.clone(),
            before,
            current: current.clone(),
            by: user.into(),
            time: now_s(),
        },
    )));
    let name_ = name.clone();
    codes::post_part(
        s,
        e.id,
        "C2013040",
        &format!("Partition entitlement changed: {name_}"),
        qsi::host::refcodes::Source::Host,
        true,
    );
    s.resources_changed(e.id);
    Ok(current)
}
