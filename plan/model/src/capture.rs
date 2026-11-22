// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! A plan of a host as it is read now ([`capture`]): its identity and
//! size, every storage pool, switch and shared processor pool but the
//! default, the virtual disks, tape volumes and images its profiles
//! name, and every partition's definition, settings and profiles.
//! Everything in a stable order, so a host captured twice gives the
//! same plan; a profile is the host's own, its MAC addresses and serial
//! numbers included.

use std::collections::BTreeSet;

use qsi::host::model::{EthernetAdapter, Lun, MediaDevice, PhysicalIo, Profile, ScsiAdapter};

use crate::doc::{
    AutoStartPlan, BondMode, BootPlan, Console, EthernetPlan, HostPlan, ImagePlan, IplKind,
    IplSource, Keylock, LunPlan, MediaPlan, MemoryPlan, Mode, PartitionPlan, PhysicalIoPlan, Plan,
    PoolPlan, ProcPoolPlan, ProcessorsPlan, ProfilePlan, Protection, ScsiPlan, SettingsPlan,
    Sharing, SwitchPlan, UnitPlan, VdiskPlan, VolumePlan, WithHost,
};
use crate::view::HostView;

/// What a capture is named and by whom.
#[derive(Clone, Debug, Default)]
pub struct Meta {
    /// The plan's name.
    pub name: String,
    /// Its description.
    pub description: String,
    /// When, Unix seconds.
    pub created: u64,
    /// Who.
    pub by: String,
    /// Saved by the console before an update.
    pub automatic: bool,
}

const MIB: u64 = 1 << 20;

/// `QSI`'s profile as the document has it.
pub fn profile_of(p: &Profile) -> Result<ProfilePlan, String> {
    let e = |u: crate::doc::words::Unknown| u.to_string();
    let pr = &p.processors;
    Ok(ProfilePlan {
        name: p.name.clone(),
        processors: ProcessorsPlan {
            mode: Mode::of(pr.mode).map_err(e)?,
            minimum: pr.minimum,
            desired: pr.desired,
            maximum: pr.maximum,
            minimum_units: pr.minimum_units,
            desired_units: pr.desired_units,
            maximum_units: pr.maximum_units,
            sharing: pr.sharing.map(Sharing::of).transpose().map_err(e)?,
            weight: pr.weight,
            pool: pr.pool.clone(),
        },
        memory: MemoryPlan {
            minimum_mib: p.memory.minimum_mib,
            desired_mib: p.memory.desired_mib,
            maximum_mib: p.memory.maximum_mib,
        },
        boot: BootPlan {
            source: IplSource::of(p.boot.source).map_err(e)?,
            ipl_type: IplKind::of(p.boot.ipl_type).map_err(e)?,
            console: Console::of(p.boot.console).map_err(e)?,
        },
        scsi: p
            .scsi
            .iter()
            .map(|a| ScsiPlan {
                slot: a.slot,
                luns: a
                    .luns
                    .iter()
                    .map(|l| LunPlan {
                        lun: l.lun,
                        backing: l.backing.clone(),
                        serial: l.serial.clone(),
                    })
                    .collect(),
            })
            .collect(),
        ethernet: p
            .ethernet
            .iter()
            .map(|a| EthernetPlan {
                slot: a.slot,
                switch: a.switch.clone(),
                vlan: a.vlan,
                tagged: a.tagged.clone(),
                mac: a.mac.clone(),
                required: a.required,
            })
            .collect(),
        optical: p.optical.iter().map(media_of).collect(),
        tape: p.tape.iter().map(media_of).collect(),
        physical_io: p
            .physical_io
            .iter()
            .map(|io| PhysicalIoPlan {
                location: io.location.clone(),
                required: io.required,
            })
            .collect(),
    })
}

fn media_of(m: &MediaDevice) -> MediaPlan {
    MediaPlan {
        slot: m.slot,
        media: m.media.clone(),
    }
}

fn media_to(m: &MediaPlan) -> MediaDevice {
    MediaDevice {
        slot: m.slot,
        media: m.media.clone(),
    }
}

/// The document's profile as QSI carries it; `None` when a value is
/// unset (a plan that passed its checks has none).
pub fn profile_to(p: &ProfilePlan) -> Option<Profile> {
    let pr = &p.processors;
    Some(Profile {
        name: p.name.clone(),
        processors: qsi::host::model::Processors {
            mode: pr.mode.qsi()?,
            minimum: pr.minimum,
            desired: pr.desired,
            maximum: pr.maximum,
            minimum_units: pr.minimum_units,
            desired_units: pr.desired_units,
            maximum_units: pr.maximum_units,
            sharing: match pr.sharing {
                Some(s) => Some(s.qsi()?),
                None => None,
            },
            weight: pr.weight,
            pool: pr.pool.clone(),
        },
        memory: qsi::host::model::Memory {
            minimum_mib: p.memory.minimum_mib,
            desired_mib: p.memory.desired_mib,
            maximum_mib: p.memory.maximum_mib,
        },
        boot: qsi::host::model::Boot {
            source: p.boot.source.qsi()?,
            ipl_type: p.boot.ipl_type.qsi()?,
            console: p.boot.console.qsi()?,
        },
        scsi: p
            .scsi
            .iter()
            .map(|a| ScsiAdapter {
                slot: a.slot,
                luns: a
                    .luns
                    .iter()
                    .map(|l| Lun {
                        lun: l.lun,
                        backing: l.backing.clone(),
                        serial: l.serial.clone(),
                    })
                    .collect(),
            })
            .collect(),
        ethernet: p
            .ethernet
            .iter()
            .map(|a| EthernetAdapter {
                slot: a.slot,
                switch: a.switch.clone(),
                vlan: a.vlan,
                tagged: a.tagged.clone(),
                mac: a.mac.clone(),
                required: a.required,
            })
            .collect(),
        optical: p.optical.iter().map(media_to).collect(),
        tape: p.tape.iter().map(media_to).collect(),
        physical_io: p
            .physical_io
            .iter()
            .map(|io| PhysicalIo {
                location: io.location.clone(),
                required: io.required,
            })
            .collect(),
    })
}

/// A plan of host `v`, named and made by `m`.
#[allow(clippy::too_many_lines, reason = "the document top to bottom")]
pub fn capture(v: &HostView, meta: Meta) -> Result<Plan, String> {
    if !v.is_host {
        return Err(format!(
            "{} is a standalone QS/400 system: it has no partitions to plan",
            v.name
        ));
    }
    if !v.connected {
        return Err(format!("{} is not connected", v.name));
    }
    let word = |u: crate::doc::words::Unknown| u.to_string();
    let h = v
        .host
        .as_ref()
        .ok_or_else(|| format!("{} did not say its processors and memory", v.name))?;
    let mut plan = Plan {
        name: meta.name,
        description: meta.description,
        created: meta.created,
        by: meta.by,
        system: v.name.clone(),
        automatic: meta.automatic,
        host: HostPlan {
            name: v.name.clone(),
            model: v.model.clone(),
            serial: v.serial.clone(),
            cores: h.cores,
            memory_mib: h.memory.installed,
        },
        ..Plan::default()
    };
    for p in &v.pools {
        let mut units: Vec<UnitPlan> = p
            .units
            .iter()
            .map(|u| UnitPlan {
                location: u.location.clone(),
                disk: u.disk.clone(),
                bytes: u.bytes,
                serial: u.serial.clone(),
            })
            .collect();
        units.sort_by(|a, b| a.location.cmp(&b.location));
        plan.pools.push(PoolPlan {
            name: p.name.clone(),
            protection: Protection::of_words(&p.protection).map_err(word)?,
            bytes: p.bytes,
            repository: p.repository,
            units,
        });
    }
    for s in &v.switches {
        let (uplink, bond, native_vlan) = match &s.uplink {
            Some(u) => (
                u.interfaces.clone(),
                u.bond.map(BondMode::of).transpose().map_err(word)?,
                u.native_vlan,
            ),
            None => (Vec::new(), None, None),
        };
        let mut vlans = s.vlans.clone();
        vlans.sort_unstable();
        plan.switches.push(SwitchPlan {
            name: s.name.clone(),
            vlans,
            mtu: s.mtu,
            uplink,
            bond,
            native_vlan,
        });
    }
    // The first is the default pool, which every host has.
    for p in h.pools.iter().skip(1) {
        plan.proc_pools.push(ProcPoolPlan {
            name: p.name.clone(),
            cores: p.cores.clone(),
            maximum_units: Some(p.maximum),
        });
    }
    for part in &v.partitions {
        if part.profiles.is_empty() {
            continue;
        }
        let d = &part.definition;
        let mut profiles = part
            .profiles
            .iter()
            .map(profile_of)
            .collect::<Result<Vec<_>, _>>()?;
        profiles.sort_by(|a, b| a.name.cmp(&b.name));
        let default_profile = if profiles.iter().any(|p| p.name == d.default_profile) {
            d.default_profile.clone()
        } else {
            profiles[0].name.clone()
        };
        plan.partitions.push(PartitionPlan {
            id: d.id,
            name: d.name.clone(),
            serial: d.serial.clone(),
            keylock: Keylock::of(d.keylock).map_err(word)?,
            default_profile,
            auto_start: AutoStartPlan {
                enabled: d.auto_start.enabled,
                order: d.auto_start.order,
                delay_secs: d.auto_start.delay_secs,
                wait: d.auto_start.wait,
                wait_secs: d.auto_start.wait_secs,
            },
            settings: part
                .settings
                .as_ref()
                .map(|s| {
                    Ok::<_, String>(SettingsPlan {
                        description: s.description.clone(),
                        panel: s.panel,
                        with_host: WithHost::of(s.with_host).map_err(word)?,
                        delay: s.delay,
                    })
                })
                .transpose()?,
            profiles,
        });
    }
    plan.partitions.sort_by_key(|p| p.id);
    named(&mut plan, v);
    plan.pools.sort_by(|a, b| a.name.cmp(&b.name));
    plan.switches.sort_by(|a, b| a.name.cmp(&b.name));
    plan.proc_pools.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(plan)
}

/// The virtual disks, tape volumes and images the profiles name, as
/// the host has them.
fn named(plan: &mut Plan, v: &HostView) {
    let mut disks = BTreeSet::new();
    let mut volumes = BTreeSet::new();
    let mut images = BTreeSet::new();
    for pr in plan.partitions.iter().flat_map(|p| &p.profiles) {
        for (pool, name) in pr.disks() {
            disks.insert((pool.to_string(), name.to_string()));
        }
        volumes.extend(pr.volumes().map(str::to_string));
        images.extend(pr.images().map(str::to_string));
    }
    for (pool, name) in disks {
        if let Some(d) = v.vdisks.iter().find(|d| d.pool == pool && d.name == name) {
            plan.vdisks.push(VdiskPlan {
                pool,
                name,
                mib: d.bytes.div_ceil(MIB),
                thick: d.thick,
            });
        }
    }
    for name in volumes {
        if let Some(t) = v.volumes.iter().find(|t| t.name == name) {
            plan.volumes.push(VolumePlan {
                name,
                identifier: t.identifier.clone(),
                mib: t.capacity_mib,
            });
        }
    }
    for name in images {
        let bytes = v
            .images
            .iter()
            .find(|i| i.name == name)
            .map_or(0, |i| i.bytes);
        plan.images.push(ImagePlan { name, bytes });
    }
}
