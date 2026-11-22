// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! A deployment's steps ([`steps`]): in order the switches, storage
//! pools, shared processor pools, virtual disks and tape volumes a
//! validation says are made, then each chosen partition with every one
//! of its profiles, then the activations asked for. An item in Error is
//! a step skipped with its message; one that is there already is no
//! step. Each step's [`Action`] is what is asked of the host, and only
//! ever makes: nothing a deployment does deletes or changes what it did
//! not make, and no disk with a label is ever named in a pool's request.

use serde::{Deserialize, Serialize};

use qsi::any::AnyRequest;
use qsi::host::model::{AutoStart, Definition};
use qsi::host::partition::{self, Settings};
use qsi::host::{profiles, vres};

use crate::doc::{BondMode, PartitionPlan, Plan, Protection};
use crate::validate::{Kind, Make, PartMake, Validation, adjusted, size};

/// What a step asks of the host.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "do", rename_all = "kebab-case")]
pub enum Action {
    /// A virtual switch made.
    Switch {
        /// Its name.
        name: String,
        /// Its VLANs.
        vlans: Vec<u16>,
        /// Its MTU.
        mtu: u32,
        /// Its uplink's interfaces; none to make it without.
        #[serde(default)]
        uplink: Vec<String>,
        /// Their bond.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        bond: Option<BondMode>,
        /// The uplink's native VLAN.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        native_vlan: Option<u16>,
    },
    /// A storage pool made on the target's free disks.
    Pool {
        /// Its name.
        name: String,
        /// Its protection.
        protection: Protection,
        /// The target's disks, by name.
        disks: Vec<String>,
    },
    /// A shared processor pool made.
    ProcPool {
        /// Its name.
        name: String,
        /// Its cores.
        cores: Vec<u32>,
        /// Its maximum, hundredths of a processor.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        maximum_units: Option<u32>,
    },
    /// A virtual disk made.
    Vdisk {
        /// Its pool.
        pool: String,
        /// Its name.
        name: String,
        /// Its size, MiB.
        mib: u64,
        /// Written whole.
        thick: bool,
    },
    /// A tape volume made.
    Volume {
        /// Its name.
        name: String,
        /// Its identifier.
        identifier: String,
        /// Its capacity, MiB.
        mib: u64,
    },
    /// A partition made, or completed: its profiles as deployed.
    Partition {
        /// It is made; it is there already otherwise and only the
        /// profiles named are added.
        create: bool,
        /// The partition, its profiles as they are put.
        partition: PartitionPlan,
        /// The profiles to put.
        put: Vec<String>,
    },
    /// A partition activated with its default profile.
    Activate {
        /// Its ID.
        id: u32,
        /// Its name.
        name: String,
        /// The profile.
        profile: String,
    },
}

/// One step: what it is in words, and what it does; a step with no
/// action is skipped, and says why.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Planned {
    /// `Create virtual switch ETHSW0`.
    pub what: String,
    /// What it asks; `None` when it is skipped.
    pub action: Option<Action>,
    /// Why it is skipped.
    pub skipped: String,
}

fn skipped(what: String, why: &str) -> Planned {
    Planned {
        what,
        action: None,
        skipped: format!("Error at validation: {why}"),
    }
}

fn done(what: String, action: Action) -> Planned {
    Planned {
        what,
        action: Some(action),
        skipped: String::new(),
    }
}

/// The steps of deploying `plan` as `val` decided, the partitions
/// `chosen` created and, with `activate`, activated after.
#[allow(
    clippy::too_many_lines,
    reason = "a block a kind of step, in their order"
)]
pub fn steps(plan: &Plan, val: &Validation, chosen: &[u32], activate: bool) -> Vec<Planned> {
    let d = &val.decisions;
    let msg = |kind: Kind, key: &str| val.item(kind, key).map_or("", |i| i.message.as_str());
    let mut out = Vec::new();
    for s in &plan.switches {
        let what = format!("Create virtual switch {}", s.name);
        match d.switches.get(&s.name) {
            Some((Make::Made, with_uplink)) => out.push(done(
                what,
                Action::Switch {
                    name: s.name.clone(),
                    vlans: s.vlans.clone(),
                    mtu: s.mtu,
                    uplink: if *with_uplink {
                        s.uplink.clone()
                    } else {
                        Vec::new()
                    },
                    bond: s.bond.filter(|_| *with_uplink),
                    native_vlan: s.native_vlan.filter(|_| *with_uplink),
                },
            )),
            Some((Make::Skipped, _)) => out.push(skipped(what, msg(Kind::Switch, &s.name))),
            _ => {}
        }
    }
    for p in &plan.pools {
        match d.pools.get(&p.name) {
            Some((Make::Made, disks)) => out.push(done(
                format!(
                    "Create storage pool {} ({}, {} disk{})",
                    p.name,
                    p.protection.word(),
                    disks.len(),
                    if disks.len() == 1 { "" } else { "s" }
                ),
                Action::Pool {
                    name: p.name.clone(),
                    protection: p.protection,
                    disks: disks.clone(),
                },
            )),
            Some((Make::Skipped, _)) => out.push(skipped(
                format!("Create storage pool {}", p.name),
                msg(Kind::Pool, &p.name),
            )),
            _ => {}
        }
    }
    for pp in &plan.proc_pools {
        let what = format!("Create shared processor pool {}", pp.name);
        match d.proc_pools.get(&pp.name) {
            Some((Make::Made, cores)) => out.push(done(
                what,
                Action::ProcPool {
                    name: pp.name.clone(),
                    cores: cores.clone(),
                    maximum_units: pp
                        .maximum_units
                        .map(|m| m.min(u32::try_from(cores.len()).unwrap_or(u32::MAX) * 100)),
                },
            )),
            Some((Make::Skipped, _)) => out.push(skipped(what, msg(Kind::ProcPool, &pp.name))),
            _ => {}
        }
    }
    for v in &plan.vdisks {
        let key = format!("{}/{}", v.pool, v.name);
        let what = format!(
            "Create virtual disk {} ({}, {})",
            v.name,
            size(v.mib),
            v.pool
        );
        match d.vdisks.get(&key) {
            Some((Make::Made, thick)) => out.push(done(
                what,
                Action::Vdisk {
                    pool: v.pool.clone(),
                    name: v.name.clone(),
                    mib: v.mib,
                    thick: *thick,
                },
            )),
            Some((Make::Skipped, _)) => out.push(skipped(what, msg(Kind::Vdisk, &key))),
            _ => {}
        }
    }
    for t in &plan.volumes {
        let what = format!("Create virtual tape {} ({})", t.name, size(t.mib));
        match d.volumes.get(&t.name) {
            Some(Make::Made) => out.push(done(
                what,
                Action::Volume {
                    name: t.name.clone(),
                    identifier: if t.identifier.is_empty() {
                        t.name.clone()
                    } else {
                        t.identifier.clone()
                    },
                    mib: t.mib,
                },
            )),
            Some(Make::Skipped) => out.push(skipped(what, msg(Kind::Volume, &t.name))),
            _ => {}
        }
    }
    let mut made = Vec::new();
    for p in plan.partitions.iter().filter(|p| chosen.contains(&p.id)) {
        let mut deployed = p.clone();
        deployed.profiles = p.profiles.iter().map(|pr| adjusted(pr, p.id, d)).collect();
        if !d.same_host {
            deployed.serial.clear();
        }
        let names: Vec<String> = deployed.profiles.iter().map(|x| x.name.clone()).collect();
        let key = p.id.to_string();
        match d.partitions.get(&p.id) {
            Some(PartMake::Made) => {
                out.push(done(
                    format!("Create partition {}, {}", p.name, profiles_named(&names)),
                    Action::Partition {
                        create: true,
                        partition: deployed,
                        put: names,
                    },
                ));
                made.push(p);
            }
            Some(PartMake::Partial(missing)) => {
                out.push(done(
                    format!(
                        "Complete partition {}{}",
                        p.name,
                        if missing.is_empty() {
                            String::new()
                        } else {
                            format!(", {}", profiles_named(missing))
                        }
                    ),
                    Action::Partition {
                        create: false,
                        partition: deployed,
                        put: missing.clone(),
                    },
                ));
                made.push(p);
            }
            Some(PartMake::Skipped) => out.push(skipped(
                format!("Create partition {}", p.name),
                msg(Kind::Partition, &key),
            )),
            _ => {}
        }
    }
    if activate {
        for p in made {
            out.push(done(
                format!(
                    "Activate partition {}, profile {}",
                    p.name, p.default_profile
                ),
                Action::Activate {
                    id: p.id,
                    name: p.name.clone(),
                    profile: p.default_profile.clone(),
                },
            ));
        }
    }
    out
}

fn profiles_named(names: &[String]) -> String {
    if names.len() == 1 {
        format!("profile {}", names[0])
    } else {
        format!("profiles {}", names.join(", "))
    }
}

/// The request of a step that makes a resource; `None` for a partition
/// or an activation, which [`create`], [`put`], [`define`] and
/// [`activation`] ask.
pub fn request(a: &Action) -> Option<AnyRequest> {
    Some(AnyRequest::Vres(match a {
        Action::Switch {
            name, vlans, mtu, ..
        } => vres::Request::SwitchCreate(vres::SwitchSpec {
            name: name.clone(),
            vlans: vlans.clone(),
            mtu: *mtu,
        }),
        Action::Pool {
            name,
            protection,
            disks,
        } => vres::Request::PoolCreate(vres::PoolCreate {
            name: name.clone(),
            protection: protection.qsi()?,
            disks: disks.clone(),
            spares: Vec::new(),
            // A deployment never confirms a disk with a label.
            confirm: Vec::new(),
        }),
        Action::ProcPool {
            name,
            cores,
            maximum_units,
        } => vres::Request::ProcPoolCreate(vres::ProcPoolSpec {
            name: name.clone(),
            cores: cores.clone(),
            maximum_units: *maximum_units,
        }),
        Action::Vdisk {
            pool,
            name,
            mib,
            thick,
        } => vres::Request::VdiskCreate(vres::VdiskCreate {
            pool: pool.clone(),
            name: name.clone(),
            mib: *mib,
            thick: *thick,
        }),
        Action::Volume {
            name,
            identifier,
            mib,
        } => vres::Request::VolumeCreate(vres::VolumeCreate {
            name: name.clone(),
            identifier: identifier.clone(),
            mib: *mib,
        }),
        Action::Partition { .. } | Action::Activate { .. } => return None,
    }))
}

/// A new switch's uplink, set after it is made; `None` for none.
pub fn uplink(a: &Action) -> Option<AnyRequest> {
    let Action::Switch {
        name,
        uplink,
        bond,
        native_vlan,
        ..
    } = a
    else {
        return None;
    };
    if uplink.is_empty() {
        return None;
    }
    Some(AnyRequest::Vres(vres::Request::UplinkSet(
        vres::UplinkSet {
            switch: name.clone(),
            uplink: vres::Uplink {
                interfaces: uplink.clone(),
                bond: bond.and_then(BondMode::qsi),
                native_vlan: *native_vlan,
            },
        },
    )))
}

/// A partition made at its plan's ID with its settings.
pub fn create(p: &PartitionPlan) -> AnyRequest {
    let settings = p.settings.as_ref().map_or_else(
        || Settings {
            description: String::new(),
            panel: true,
            with_host: partition::WithHost::Delayed,
            delay: 600,
        },
        |s| Settings {
            description: s.description.clone(),
            panel: s.panel,
            with_host: s.with_host.qsi().unwrap_or(partition::WithHost::Delayed),
            delay: s.delay,
        },
    );
    AnyRequest::Partition(partition::Request::CreateWith(partition::CreateWith {
        id: Some(p.id),
        name: p.name.clone(),
        settings,
    }))
}

/// A profile put as a new one.
pub fn put(p: &PartitionPlan, profile: &str) -> Option<AnyRequest> {
    let pr = p.profiles.iter().find(|x| x.name == profile)?;
    Some(AnyRequest::Profiles(profiles::Request::Put(
        profiles::Put {
            id: p.id,
            profile: crate::capture::profile_to(pr)?,
            revision: None,
        },
    )))
}

/// The definition the host made, with the plan's keylock, start and
/// default profile: what is the host's alone (its serial number and
/// creation time) as the host has it.
pub fn define(made: &Definition, p: &PartitionPlan) -> AnyRequest {
    let a = &p.auto_start;
    let mut d = made.clone();
    if let Some(k) = p.keylock.qsi() {
        d.keylock = k;
    }
    d.default_profile.clone_from(&p.default_profile);
    d.auto_start = AutoStart {
        enabled: a.enabled,
        order: a.order,
        delay_secs: a.delay_secs,
        wait: a.wait,
        wait_secs: a.wait_secs,
    };
    AnyRequest::Partition(partition::Request::Change(partition::Change {
        definition: d,
    }))
}

/// An activation with a profile.
pub fn activation(id: u32, profile: &str) -> AnyRequest {
    AnyRequest::Partition(partition::Request::Activate(partition::Activate {
        id,
        profile: profile.to_string(),
        source: None,
        keylock: None,
    }))
}
