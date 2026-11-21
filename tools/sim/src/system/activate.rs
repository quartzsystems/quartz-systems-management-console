// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! An activation as a QS/VS host does it (`docs/RESOURCES.md`): the
//! profile's resources checked, the processors and memory given (the
//! desired amount when there is that much, else the most there is down
//! to the minimum), the virtual disks, media, drives and devices held,
//! then the IPL from the profile's source; and all of it given back at
//! the partition's end. Holders are worked out again from what each
//! partition holds, and each change is an event.

use qsi::any::AnyEvent;
use qsi::base::{Keylock, PowerState};
use qsi::host::host::{self, DeviceHeld, DeviceHolder};
use qsi::host::model::{
    Current, Holder, IplSource, PartitionState, ProcessorMode, Profile, Sharing, TapeSource,
};
use qsi::host::partition::Activate;
use qsi::host::partitions::{self, Link};
use qsi::host::vres::{self, Drive, DriveKind};

use super::codes::{self, Ipl};
use super::refuse::{Refusal, activated, no, no_partition, no_such, units};
use super::{Part, System, now_s};

/// The virtual Ethernet adapters of a partition as its status lists
/// them: its profile in force's, with link while it is activated.
pub fn links(p: &Part) -> Vec<Link> {
    let link = p.active() && p.holds.is_some();
    p.in_force()
        .map(|pr| {
            pr.ethernet
                .iter()
                .map(|e| Link {
                    slot: e.slot,
                    switch: e.switch.clone().unwrap_or_default(),
                    mac: e.mac.clone().unwrap_or_default(),
                    link,
                })
                .collect()
        })
        .unwrap_or_default()
}

/// A profile's devices as a running partition's drives.
pub fn drives_of(pr: &Profile) -> Vec<Drive> {
    let mut v: Vec<Drive> = pr
        .optical
        .iter()
        .map(|d| Drive {
            slot: d.slot,
            kind: DriveKind::Optical,
            name: d.media.clone().unwrap_or_default(),
            prevented: false,
        })
        .collect();
    v.extend(pr.tape.iter().map(|d| match d.tape_source() {
        TapeSource::Drive(l) => Drive {
            slot: d.slot,
            kind: DriveKind::PhysicalTape,
            name: l.into(),
            prevented: false,
        },
        TapeSource::Volume(n) => Drive {
            slot: d.slot,
            kind: DriveKind::Tape,
            name: n.into(),
            prevented: false,
        },
        TapeSource::Empty => Drive {
            slot: d.slot,
            kind: DriveKind::Tape,
            name: String::new(),
            prevented: false,
        },
    }));
    v.sort_by_key(|d| d.slot);
    v
}

impl System {
    /// Partition `id`'s IPL source (its profile in force's) and the
    /// image in its first optical device with the image's volume
    /// identifier.
    pub fn boot_of(&self, id: u32) -> (IplSource, Option<(String, String)>) {
        let Some(p) = self.part_ref(id) else {
            return (IplSource::A, None);
        };
        let source = p.in_force().map_or(IplSource::A, |pr| pr.boot.source);
        let media = p
            .drives
            .iter()
            .filter(|d| d.kind == DriveKind::Optical)
            .min_by_key(|d| d.slot)
            .filter(|d| !d.name.is_empty())
            .map(|d| {
                let label = self
                    .vres
                    .images
                    .iter()
                    .find(|i| i.name == d.name)
                    .map(|i| i.label.clone())
                    .unwrap_or_default();
                (d.name.clone(), label)
            });
        (source, media)
    }

    /// Activate a partition: what it was given. Its IPL's codes and
    /// states follow as events.
    pub fn activate(&mut self, a: &Activate) -> Result<Current, Refusal> {
        let Some(p) = self.part_ref(a.id) else {
            return Err(no_partition(a.id));
        };
        let name = p.status.name.clone();
        if p.active() {
            return Err(activated(&name));
        }
        if self.power != PowerState::Running {
            return Err(no(
                "QVS2010",
                format!("Partition {name} cannot be activated: the host is shutting down."),
            ));
        }
        if matches!(a.source, Some(IplSource::Other(_))) {
            return Err(no_such("IPL source"));
        }
        if matches!(a.keylock, Some(Keylock::Other(_))) {
            return Err(no_such("keylock position"));
        }
        let pname = if a.profile.is_empty() {
            p.definition.default_profile.clone()
        } else {
            a.profile.clone()
        };
        if pname.is_empty() {
            return Err(no(
                "QVS2005",
                format!("Partition {name} has no default profile."),
            ));
        }
        let Some(kept) = p.kept(&pname) else {
            return Err(no(
                "QVS2004",
                format!("Partition {name} has no profile {pname}."),
            ));
        };
        let profile = kept.profile.clone();
        self.check_backing(a.id, &name, &profile)?;
        // What it held from an IPL that stopped is given back first.
        if self.part_ref(a.id).is_some_and(|p| p.holds.is_some()) {
            self.release(a.id);
        }
        let current = self.take(&name, &profile)?;
        let now = now_s();
        let Some(p) = self.part(a.id) else {
            return Err(no_partition(a.id));
        };
        p.holds = Some(current.clone());
        p.status.current = Some(current.clone());
        p.status.profile.clone_from(&pname);
        p.status.message = None;
        p.since = Some(now);
        p.keylock_now = a.keylock;
        p.drives = drives_of(&profile);
        if let Some(k) = p.profiles.iter_mut().find(|k| k.profile.name == pname) {
            k.last = Some(now);
        }
        self.rehold();
        self.resources_changed(a.id);
        let source = match a.source.unwrap_or(profile.boot.source) {
            IplSource::Recorded => IplSource::A,
            s => s,
        };
        let (_, media) = self.boot_of(a.id);
        let media = media.as_ref().map(|(i, l)| (i.as_str(), l.as_str()));
        if codes::ipl(self, a.id, source, media) == Ipl::NoMedia {
            self.release(a.id);
            self.set_state(a.id, PartitionState::NotActivated);
        } else {
            self.ports_changed(a.id, true);
        }
        Ok(current)
    }

    /// What an activation would hold, each checked: QS/VS's message for
    /// the first that is not there or is another partition's.
    #[allow(clippy::too_many_lines, reason = "one check a kind of resource")]
    fn check_backing(&self, id: u32, name: &str, pr: &Profile) -> Result<(), Refusal> {
        let missing = |what: &str| self.faults.missing.iter().any(|m| m == what);
        let other = |h: Holder| h.partition != 0 && h.partition != id;
        for a in &pr.scsi {
            for l in &a.luns {
                let found = l.backing.split_once('/').and_then(|(pool, disk)| {
                    self.vres
                        .vdisks
                        .iter()
                        .find(|v| v.pool == pool && v.name == disk)
                });
                let Some(v) = found.filter(|_| !missing(&l.backing)) else {
                    return Err(no(
                        "QVS2110",
                        format!(
                            "Partition {name} cannot be activated: the backing store of unit {} in slot {} is not there: {}",
                            l.lun, a.slot, l.backing
                        ),
                    ));
                };
                if other(v.holder) {
                    return Err(no(
                        "QVS3105",
                        format!(
                            "Virtual disk {} is held by partition {}.",
                            v.name,
                            self.part_name(v.holder.partition)
                        ),
                    ));
                }
                if let Some(pool) = self.vres.pools.iter().find(|p| p.name == v.pool)
                    && pool.state == "Failed"
                {
                    return Err(no(
                        "QVS2111",
                        format!(
                            "Partition {name} cannot be activated: storage pool {} is failed.",
                            pool.name
                        ),
                    ));
                }
            }
        }
        for e in pr.ethernet.iter().filter(|e| e.required) {
            let sw = e.switch.clone().unwrap_or_default();
            if missing(&sw) || !self.vres.switches.iter().any(|s| s.name == sw) {
                return Err(no(
                    "QVS2117",
                    format!(
                        "Partition {name} cannot be activated: the virtual Ethernet adapter in slot {} has no port: there is no switch named {sw}.",
                        e.slot
                    ),
                ));
            }
        }
        for d in pr.physical_io.iter().filter(|d| d.required) {
            let Some(dev) = self
                .devices
                .iter()
                .find(|x| x.location == d.location)
                .filter(|_| !missing(&d.location))
            else {
                return Err(no(
                    "QVS2401",
                    format!("No device of the host is at location {}.", d.location),
                ));
            };
            if !dev.released {
                return Err(no(
                    "QVS2402",
                    format!("Device {} is not released for partitions.", d.location),
                ));
            }
            if let Some(h) = &dev.holder
                && h.partition != id
            {
                return Err(no(
                    "QVS2403",
                    format!("Device {} is held by partition {}.", d.location, h.name),
                ));
            }
        }
        for o in &pr.optical {
            if let Some(m) = &o.media
                && (missing(m) || !self.vres.images.iter().any(|i| &i.name == m))
            {
                return Err(no(
                    "QVS3204",
                    format!("There is no image {m} in the media library."),
                ));
            }
        }
        for t in &pr.tape {
            match t.tape_source() {
                TapeSource::Empty => {}
                TapeSource::Volume(v) => {
                    let Some(vol) = self
                        .vres
                        .volumes
                        .iter()
                        .find(|x| x.name == v)
                        .filter(|_| !missing(v))
                    else {
                        return Err(no(
                            "QVS3204",
                            format!("There is no tape volume {v} in the media library."),
                        ));
                    };
                    if other(vol.holder) {
                        return Err(no(
                            "QVS3205",
                            format!(
                                "Tape volume {v} is in use by partition {}.",
                                self.part_name(vol.holder.partition)
                            ),
                        ));
                    }
                }
                TapeSource::Drive(l) => {
                    let Some(drive) = self
                        .vres
                        .tape_drives
                        .iter()
                        .find(|x| x.location == l)
                        .filter(|_| !missing(l))
                    else {
                        return Err(no(
                            "QVS2420",
                            format!("Location {l} is not a tape drive of the host."),
                        ));
                    };
                    if other(drive.holder) {
                        return Err(no(
                            "QVS2403",
                            format!(
                                "Device {l} is held by partition {}.",
                                self.part_name(drive.holder.partition)
                            ),
                        ));
                    }
                }
            }
        }
        Ok(())
    }

    /// The cores the default pool can give as dedicated processors,
    /// highest first: not in a named pool, and not ones its running
    /// partitions need.
    fn free_cores(&self) -> Vec<u32> {
        let Some(h) = &self.host else {
            return Vec::new();
        };
        let Some(d) = h.pools.first() else {
            return Vec::new();
        };
        let named: Vec<u32> = h.pools[1..].iter().flat_map(|p| p.cores.clone()).collect();
        let mut free: Vec<u32> = d
            .cores
            .iter()
            .copied()
            .filter(|c| !named.contains(c))
            .collect();
        free.sort_unstable_by(|a, b| b.cmp(a));
        let most_virtuals = self
            .parts
            .iter()
            .filter_map(|p| p.holds.as_ref())
            .filter(|c| c.mode == ProcessorMode::Shared && c.pool == d.name)
            .map(|c| c.virtuals)
            .max()
            .unwrap_or(0);
        let need = (d.entitled.div_ceil(100)).max(most_virtuals) as usize;
        free.truncate(d.cores.len().saturating_sub(need));
        free
    }

    /// Give `pr`'s processors and memory: what was given.
    #[allow(clippy::too_many_lines, reason = "memory, then each processor mode")]
    fn take(&mut self, name: &str, pr: &Profile) -> Result<Current, Refusal> {
        let Some(h) = &self.host else {
            return Err(no("QSI0301", "The partition manager did not answer."));
        };
        let block = self.settings.memory_block_mib.max(1);
        let m = &pr.memory;
        let available = h.memory.available / block * block;
        let memory_mib = if m.desired_mib <= available {
            m.desired_mib
        } else if m.minimum_mib <= available {
            available
        } else {
            return Err(no(
                "QVS2104",
                format!(
                    "Partition {name} needs {} MB of memory; {available} MB are available.",
                    m.minimum_mib
                ),
            ));
        };
        let p = &pr.processors;
        let current = match p.mode {
            ProcessorMode::Dedicated => {
                let free = self.free_cores();
                let n = (p.desired as usize).min(free.len());
                if n < p.minimum as usize {
                    return Err(no(
                        "QVS2101",
                        format!(
                            "Partition {name} needs {} dedicated processors; {} are available.",
                            p.minimum,
                            free.len()
                        ),
                    ));
                }
                let mut cores = free[..n].to_vec();
                cores.sort_unstable();
                let n = u32::try_from(n).unwrap_or(0);
                Current {
                    mode: ProcessorMode::Dedicated,
                    cores,
                    pool: String::new(),
                    units: n * 100,
                    virtuals: n,
                    capped: false,
                    weight: 0,
                    memory_mib,
                }
            }
            ProcessorMode::Shared => {
                let default = h.pools.first().map(|d| d.name.clone()).unwrap_or_default();
                let pool_name = p.pool.clone().unwrap_or(default);
                let Some(pool) = h.pools.iter().find(|x| x.name == pool_name) else {
                    return Err(no(
                        "QVS2105",
                        format!("Shared processor pool {pool_name} does not exist."),
                    ));
                };
                let cores = u32::try_from(pool.cores.len()).unwrap_or(0);
                let mut virtuals = p.desired.min(cores);
                if virtuals < p.minimum {
                    return Err(no(
                        "QVS2103",
                        format!(
                            "Partition {name} needs {} virtual processors; pool {pool_name} has {cores} cores.",
                            p.minimum
                        ),
                    ));
                }
                let left = u64::from(pool.available).min(h.processors.available);
                let left = u32::try_from(left).unwrap_or(u32::MAX);
                let units_ = p.desired_units.unwrap_or(100).min(left).min(100 * virtuals);
                let least = p.minimum_units.unwrap_or(5);
                if units_ < least {
                    return Err(no(
                        "QVS2102",
                        format!(
                            "Partition {name} needs {} processing units in pool {pool_name}; {} are available.",
                            units(least),
                            units(left)
                        ),
                    ));
                }
                virtuals = virtuals.min(units_ / 5).max(1);
                let capped = p.sharing != Some(Sharing::Uncapped);
                Current {
                    mode: ProcessorMode::Shared,
                    cores: Vec::new(),
                    pool: pool_name,
                    units: units_,
                    virtuals,
                    capped,
                    weight: if capped { 0 } else { p.weight.unwrap_or(128) },
                    memory_mib,
                }
            }
            ProcessorMode::Other(_) => return Err(no_such("processor mode")),
        };
        self.account(&current, true);
        Ok(current)
    }

    /// Count `c` as held (`give`) or given back in the host's and its
    /// pools' amounts.
    pub fn account(&mut self, c: &Current, give: bool) {
        let Some(h) = &mut self.host else {
            return;
        };
        let units_ = u64::from(c.units);
        if give {
            h.processors.available = h.processors.available.saturating_sub(units_);
            h.memory.available = h.memory.available.saturating_sub(c.memory_mib);
        } else {
            h.processors.available =
                (h.processors.available + units_).min(h.processors.configurable);
            h.memory.available = (h.memory.available + c.memory_mib).min(h.memory.configurable);
        }
        if c.mode == ProcessorMode::Dedicated {
            if let Some(d) = h.pools.first_mut() {
                if give {
                    d.cores.retain(|x| !c.cores.contains(x));
                    d.maximum = d.maximum.saturating_sub(c.units);
                } else {
                    d.cores.extend(c.cores.iter().copied());
                    d.cores.sort_unstable();
                    d.cores.dedup();
                    d.maximum += c.units;
                }
                d.available = d.maximum.saturating_sub(d.entitled);
            }
        } else if let Some(pool) = h.pools.iter_mut().find(|x| x.name == c.pool) {
            if give {
                pool.entitled += c.units;
                pool.partitions += 1;
            } else {
                pool.entitled = pool.entitled.saturating_sub(c.units);
                pool.partitions = pool.partitions.saturating_sub(1);
            }
            pool.available = pool.maximum.saturating_sub(pool.entitled);
        }
    }

    /// Partition `id` gives back everything it holds.
    pub fn release(&mut self, id: u32) {
        let Some(p) = self.part(id) else {
            return;
        };
        let Some(held) = p.holds.take() else {
            return;
        };
        let was_up = p.active();
        p.status.current = None;
        p.since = None;
        p.keylock_now = None;
        p.drives.clear();
        self.account(&held, false);
        if was_up {
            self.ports_changed(id, false);
        }
        self.rehold();
        self.resources_changed(id);
    }

    /// Say what partition `id` holds now, and the host's resources.
    pub fn resources_changed(&mut self, id: u32) {
        let Some(p) = self.part(id) else {
            return;
        };
        p.status.ethernet = links(p);
        let (name, current) = (p.status.name.clone(), p.status.current.clone());
        self.push(&AnyEvent::Partitions(partitions::Event::Resources(
            partitions::ResourcesChanged {
                id,
                name,
                current,
                time: now_s(),
            },
        )));
        if let Some(h) = self.host.clone() {
            self.push(&AnyEvent::Host(host::Event::Resources(h)));
        }
    }

    /// Say partition `id`'s ports opened or closed.
    pub fn ports_changed(&mut self, id: u32, open: bool) {
        let Some(p) = self.part(id) else {
            return;
        };
        p.status.ethernet = links(p);
        let ports: Vec<_> = p
            .status
            .ethernet
            .iter()
            .filter(|l| !l.switch.is_empty())
            .map(|l| (l.slot, l.switch.clone()))
            .collect();
        for (slot, switch) in ports {
            self.push(&AnyEvent::Vres(vres::Event::Port(vres::PortChanged {
                partition: id,
                slot,
                switch,
                open,
                time: now_s(),
            })));
        }
    }

    /// Work out again who holds each virtual disk, medium, tape drive
    /// and device from what each partition holds; say what changed.
    #[allow(clippy::too_many_lines, reason = "one pass a kind of holder")]
    pub fn rehold(&mut self) {
        let time = now_s();
        let held: Vec<&Part> = self.parts.iter().filter(|p| p.holds.is_some()).collect();
        let find = |f: &dyn Fn(&Part) -> Option<u32>| {
            held.iter()
                .find_map(|p| f(p).map(|slot| (p.definition.id, slot)))
                .map_or(Holder::default(), |(partition, slot)| Holder {
                    partition,
                    slot,
                })
        };
        let mut events = Vec::new();
        let vdisks: Vec<Holder> = self
            .vres
            .vdisks
            .iter()
            .map(|v| {
                let backing = format!("{}/{}", v.pool, v.name);
                find(&|p| {
                    p.in_force()?
                        .scsi
                        .iter()
                        .find(|a| a.luns.iter().any(|l| l.backing == backing))
                        .map(|a| a.slot)
                })
            })
            .collect();
        let drive = |kind: DriveKind, name: String| {
            find(&|p| {
                p.drives
                    .iter()
                    .find(|d| d.kind == kind && d.name == name)
                    .map(|d| d.slot)
            })
        };
        let images: Vec<Holder> = self
            .vres
            .images
            .iter()
            .map(|i| drive(DriveKind::Optical, i.name.clone()))
            .collect();
        let volumes: Vec<Holder> = self
            .vres
            .volumes
            .iter()
            .map(|v| drive(DriveKind::Tape, v.name.clone()))
            .collect();
        let tapes: Vec<Holder> = self
            .vres
            .tape_drives
            .iter()
            .map(|t| drive(DriveKind::PhysicalTape, t.location.clone()))
            .collect();
        let devices: Vec<Option<DeviceHolder>> = self
            .devices
            .iter()
            .map(|d| {
                held.iter().find_map(|p| {
                    let pr = p.in_force()?;
                    pr.physical_io
                        .iter()
                        .any(|x| x.location == d.location)
                        .then(|| DeviceHolder {
                            partition: p.definition.id,
                            name: p.status.name.clone(),
                            profile: pr.name.clone(),
                        })
                })
            })
            .collect();
        for (v, h) in self.vres.vdisks.iter_mut().zip(vdisks) {
            if v.holder != h {
                v.holder = h;
                events.push(AnyEvent::Vres(vres::Event::Vdisk(vres::VdiskChanged {
                    pool: v.pool.clone(),
                    name: v.name.clone(),
                    vdisk: Some(v.clone()),
                    time,
                })));
            }
        }
        for (i, h) in self.vres.images.iter_mut().zip(images) {
            if i.holder != h {
                i.holder = h;
                events.push(AnyEvent::Vres(vres::Event::Image(vres::ImageChanged {
                    name: i.name.clone(),
                    image: Some(i.clone()),
                    time,
                })));
            }
        }
        for (v, h) in self.vres.volumes.iter_mut().zip(volumes) {
            if v.holder != h {
                v.holder = h;
                events.push(AnyEvent::Vres(vres::Event::Volume(vres::VolumeChanged {
                    name: v.name.clone(),
                    volume: Some(v.clone()),
                    time,
                })));
            }
        }
        for (t, h) in self.vres.tape_drives.iter_mut().zip(tapes) {
            if t.holder != h {
                t.holder = h;
                events.push(AnyEvent::Vres(vres::Event::TapeDrive(
                    vres::TapeDriveChanged {
                        location: t.location.clone(),
                        drive: Some(t.clone()),
                        time,
                    },
                )));
            }
        }
        for (d, h) in self.devices.iter_mut().zip(devices) {
            if d.holder != h {
                let e = match (&d.holder, &h) {
                    (_, Some(now)) => host::Event::Assigned(DeviceHeld {
                        location: d.location.clone(),
                        holder: now.clone(),
                        time,
                    }),
                    (Some(was), None) => host::Event::Returned(DeviceHeld {
                        location: d.location.clone(),
                        holder: was.clone(),
                        time,
                    }),
                    (None, None) => continue,
                };
                d.holder = h;
                d.assignable = d.released && d.blocked.is_none() && d.holder.is_none();
                events.push(AnyEvent::Host(e));
            }
        }
        for e in &events {
            self.push(e);
        }
    }
}
