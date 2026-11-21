// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! Storage pools, their disks and virtual disks, as QS/VS's pool
//! service keeps them (`docs/STORAGE.md`): a pool of whole disks with
//! its protection, fixed in size once made; virtual disks thin or
//! thick in it, held by one partition at a time.

use qsi::Empty;
use qsi::any::AnyEvent;
use qsi::host::host::{self, ItemKind, Owner};
use qsi::host::vres::{
    self, Naming, PoolDisk, PoolUnit, Protection, Reply, Request, StoragePool, Vdisk,
};
use sha2::{Digest as _, Sha256};

use super::pool_changed;
use crate::system::refuse::{Refusal, disk_name_ok, name_ok, no, no_such};
use crate::system::{System, now_s};

/// One MiB.
pub const MIB: u64 = 1 << 20;
/// The largest virtual disk, MiB: 64 TiB.
pub const MAX_VDISK_MIB: u64 = 64 << 20;
/// The largest pool, bytes: 64 TiB.
pub const MAX_POOL: u64 = 64 << 40;
/// The most hot spares a pool has.
pub const MAX_SPARES: usize = 8;

fn no_pool(name: &str) -> Refusal {
    no("QVS3003", format!("There is no storage pool {name}."))
}

/// A pool's state in which it can be used.
fn usable(state: &str) -> bool {
    matches!(
        state,
        "Active" | "Degraded" | "Rebuilding" | "Resynchronizing"
    )
}

/// A UUID made from `seed`, the same each run.
pub fn uuid(seed: &str) -> String {
    let h: [u8; 32] = Sha256::digest(seed.as_bytes()).into();
    let x = crate::system::hex(&h[..16]);
    format!(
        "{}-{}-{}-{}-{}",
        &x[..8],
        &x[8..12],
        &x[12..16],
        &x[16..20],
        &x[20..32]
    )
}

/// The storage pools' requests.
pub fn pools(s: &mut System, r: &Request) -> Result<Reply, Refusal> {
    Ok(match r {
        Request::Pools(_) => Reply::Pools(s.vres.pools.clone()),
        Request::Disks(_) => Reply::Disks(s.vres.disks.clone()),
        Request::PoolCreate(c) => Reply::PoolCreate(create(s, c)?),
        Request::PoolDelete(d) => {
            delete(s, &d.name)?;
            Reply::PoolDelete(Empty)
        }
        Request::PoolRename(p) => {
            rename(s, &p.name, &p.to)?;
            Reply::PoolRename(Empty)
        }
        Request::UnitReplace(u) => {
            replace(s, u)?;
            Reply::UnitReplace(Empty)
        }
        Request::SpareAdd(a) => {
            spare_add(s, &a.pool, &a.disk, a.confirm)?;
            Reply::SpareAdd(Empty)
        }
        Request::SpareRemove(a) => {
            spare_remove(s, &a.pool, &a.disk)?;
            Reply::SpareRemove(Empty)
        }
        Request::Designate(d) => {
            designate(s, &d.pool)?;
            Reply::Designate(Empty)
        }
        Request::PoolExtend(e) => {
            // A pool's set has a fixed size: one that is there cannot grow.
            if s.vres.pools.iter().any(|p| p.name == e.pool) {
                return Err(no(
                    "QVS3019",
                    format!(
                        "Storage pool {} cannot be extended: a pool's size is fixed when it is made.",
                        e.pool
                    ),
                ));
            }
            return Err(no_pool(&e.pool));
        }
        _ => return Err(no_such("operation")),
    })
}

/// A disk for a pool: QS/VS's message when it cannot be one.
fn candidate(
    s: &System,
    name: &str,
    confirmed: bool,
    least: u64,
) -> Result<PoolDisk, (String, String)> {
    let Some(d) = s.vres.disks.iter().find(|d| d.name == name) else {
        return Err(("QVS3005".into(), format!("There is no disk {name}.")));
    };
    if !d.pool.is_empty() || (!d.free && !d.foreign) {
        return Err((
            "QVS3006".into(),
            format!("Disk {name} cannot be used: {}.", d.what.to_lowercase()),
        ));
    }
    if d.foreign && !confirmed {
        return Err((
            "QVS3007".into(),
            format!(
                "Disk {name} holds what is not this host's: {}.",
                d.what.to_lowercase()
            ),
        ));
    }
    if d.bytes < least.max(264 * MIB) {
        return Err((
            "QVS3008".into(),
            format!("Disk {name} is too small for a storage pool."),
        ));
    }
    Ok(d.clone())
}

/// Give disk `name` to `pool` (or back to none), in the pool service's
/// list and the hardware's.
fn own(s: &mut System, name: &str, pool: &str, what: &str) {
    if let Some(d) = s.vres.disks.iter_mut().find(|d| d.name == name) {
        d.pool = pool.into();
        d.what = what.into();
        d.free = pool.is_empty();
        d.foreign = false;
    }
    let mut changed = None;
    if let Some(h) = &mut s.hardware
        && let Some(i) = h
            .items
            .iter_mut()
            .find(|i| i.kind == ItemKind::Disk && i.name == name)
    {
        i.owner = if pool.is_empty() {
            Owner::None
        } else {
            Owner::Pool
        };
        i.owner_name = pool.into();
        i.what = what.into();
        changed = Some(i.clone());
    }
    if let Some(item) = changed {
        s.push(&AnyEvent::Host(host::Event::Item(host::ItemChanged {
            location: item.location.clone(),
            item: Some(item),
            time: now_s(),
        })));
    }
}

fn unit(number: u32, d: &PoolDisk, state: &str) -> PoolUnit {
    PoolUnit {
        number,
        disk: d.name.clone(),
        serial: d.serial.clone(),
        location: d.location.clone(),
        bytes: d.bytes,
        state: state.into(),
    }
}

#[allow(
    clippy::too_many_lines,
    reason = "every check of a create, then the pool"
)]
fn create(s: &mut System, c: &vres::PoolCreate) -> Result<StoragePool, Refusal> {
    let (word, lo, hi) = match c.protection {
        Protection::None => ("none", 1, 1),
        Protection::Mirrored => ("mirrored", 2, 2),
        Protection::Raid5 => ("raid5", 3, 18),
        Protection::Raid6 => ("raid6", 4, 18),
        Protection::Other(_) => return Err(no_such("protection")),
    };
    // Every problem at once, as QS/VS checks a create.
    let mut v: Refusal = Vec::new();
    if !name_ok(&c.name) {
        v.push((
            "QVS3001".into(),
            format!("Storage pool name {} is not valid.", c.name),
        ));
    } else if s.vres.pools.iter().any(|p| p.name == c.name) {
        v.push((
            "QVS3002".into(),
            format!("A storage pool named {} exists.", c.name),
        ));
    }
    if !(lo..=hi).contains(&c.disks.len()) {
        v.push((
            "QVS3004".into(),
            format!(
                "Protection {word} takes {lo} to {hi} disks; {} were chosen.",
                c.disks.len()
            ),
        ));
    }
    let mut members = Vec::new();
    for n in &c.disks {
        match candidate(s, n, c.confirm.contains(n), 0) {
            Ok(d) => members.push(d),
            Err(m) => v.push(m),
        }
    }
    let least = members.iter().map(|d| d.bytes).min().unwrap_or(0);
    let mut spares = Vec::new();
    if word == "none" && !c.spares.is_empty() {
        v.push((
            "QVS3017".into(),
            format!(
                "Hot spare {}: a pool with no protection has none.",
                c.spares[0]
            ),
        ));
    } else if c.spares.len() > MAX_SPARES {
        v.push((
            "QVS3017".into(),
            format!(
                "Hot spare {}: a pool has {MAX_SPARES} at most.",
                c.spares[MAX_SPARES]
            ),
        ));
    } else {
        for n in &c.spares {
            match candidate(s, n, c.confirm.contains(n), 0) {
                Ok(d) if d.bytes < least => v.push((
                    "QVS3010".into(),
                    format!(
                        "Disk {n} is smaller than the pool's units and cannot be its hot spare."
                    ),
                )),
                Ok(d) => spares.push(d),
                Err(m) => v.push(m),
            }
        }
    }
    let mut sizes: Vec<u32> = members
        .iter()
        .chain(&spares)
        .map(|d| d.block_size)
        .collect();
    sizes.dedup();
    if sizes.len() > 1 {
        let words: Vec<String> = sizes.iter().map(u32::to_string).collect();
        v.push((
            "QVS3009".into(),
            format!(
                "The disks chosen have different logical block sizes: {}.",
                words.join(", ")
            ),
        ));
    }
    let n = members.len() as u64;
    let bytes = match word {
        "raid5" => least * n.saturating_sub(1),
        "raid6" => least * n.saturating_sub(2),
        _ => least,
    };
    if bytes > MAX_POOL {
        v.push((
            "QVS3011".into(),
            format!("The pool would be {bytes} bytes; a storage pool is at most {MAX_POOL} bytes."),
        ));
    }
    if !v.is_empty() {
        return Err(v);
    }
    let mut units: Vec<PoolUnit> = Vec::new();
    for (i, d) in members.iter().enumerate() {
        let number = u32::try_from(i + 1).unwrap_or(0);
        units.push(unit(number, d, "Active"));
    }
    for d in &spares {
        let number = u32::try_from(units.len() + 1).unwrap_or(0);
        units.push(unit(number, d, "Hot spare"));
    }
    let pool = StoragePool {
        name: c.name.clone(),
        uuid: uuid(&format!("pool {} {}", c.name, now_s())),
        protection: word.into(),
        state: "Active".into(),
        work: String::new(),
        work_unit: 0,
        percent: 0,
        bytes,
        allocated: 0,
        provisioned: 0,
        repository: false,
        units: units.clone(),
        failure: String::new(),
    };
    s.vres.pools.push(pool.clone());
    for u in &units {
        let what = if u.state == "Hot spare" {
            format!("{} hot spare", c.name)
        } else {
            format!("{} unit {}, Active", c.name, u.number)
        };
        own(s, &u.disk, &c.name, &what);
    }
    pool_changed(s, &c.name);
    Ok(pool)
}

/// A pool by name, or QS/VS's message.
fn find<'a>(s: &'a System, name: &str) -> Result<&'a StoragePool, Refusal> {
    s.vres
        .pools
        .iter()
        .find(|p| p.name == name)
        .ok_or_else(|| no_pool(name))
}

/// The partition holding something in pool `name`, if one does.
fn holder_in(s: &System, name: &str) -> Option<u32> {
    let p = s.vres.pools.iter().find(|p| p.name == name)?;
    s.vres
        .vdisks
        .iter()
        .filter(|v| v.pool == name)
        .map(|v| v.holder.partition)
        .chain(if p.repository {
            s.vres
                .images
                .iter()
                .map(|i| i.holder.partition)
                .chain(s.vres.volumes.iter().map(|v| v.holder.partition))
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        })
        .find(|h| *h != 0)
}

fn delete(s: &mut System, name: &str) -> Result<(), Refusal> {
    let p = find(s, name)?.clone();
    let vdisks = s.vres.vdisks.iter().filter(|v| v.pool == name).count();
    let media = if p.repository {
        s.vres.images.len() + s.vres.volumes.len()
    } else {
        0
    };
    if vdisks + media > 0 {
        return Err(no(
            "QVS3012",
            format!(
                "Storage pool {name} holds {vdisks} virtual disks and {media} media, and cannot be deleted."
            ),
        ));
    }
    s.vres.pools.retain(|x| x.name != name);
    for u in &p.units {
        own(s, &u.disk, "", "Not configured");
    }
    pool_changed(s, name);
    Ok(())
}

fn rename(s: &mut System, name: &str, to: &str) -> Result<(), Refusal> {
    let p = find(s, name)?.clone();
    if !name_ok(to) {
        return Err(no(
            "QVS3001",
            format!("Storage pool name {to} is not valid."),
        ));
    }
    if s.vres.pools.iter().any(|x| x.name == to) {
        return Err(no("QVS3002", format!("A storage pool named {to} exists.")));
    }
    if let Some(h) = holder_in(s, name) {
        return Err(no(
            "QVS3013",
            format!(
                "Storage pool {name} is in use by partition {}.",
                s.part_name(h)
            ),
        ));
    }
    for x in &mut s.vres.pools {
        if x.name == name {
            x.name = to.into();
        }
    }
    for v in &mut s.vres.vdisks {
        if v.pool == name {
            v.pool = to.into();
        }
    }
    for u in &p.units {
        let what = if u.state == "Hot spare" {
            format!("{to} hot spare")
        } else {
            format!("{to} unit {}, {}", u.number, u.state)
        };
        own(s, &u.disk, to, &what);
    }
    pool_changed(s, name);
    pool_changed(s, to);
    Ok(())
}

fn replace(s: &mut System, u: &vres::UnitReplace) -> Result<(), Refusal> {
    let p = find(s, &u.pool)?.clone();
    let Some(old) = p.units.iter().find(|x| x.number == u.unit) else {
        return Err(no(
            "QVS3015",
            format!(
                "Unit {} of storage pool {} cannot be replaced: there is no such unit.",
                u.unit, u.pool
            ),
        ));
    };
    if !matches!(old.state.as_str(), "Failed" | "Missing") {
        return Err(no(
            "QVS3015",
            format!(
                "Unit {} of storage pool {} cannot be replaced: it is {}, not failed or missing.",
                u.unit,
                u.pool,
                old.state.to_lowercase()
            ),
        ));
    }
    let d = candidate(s, &u.disk, u.confirm, old.bytes).map_err(|m| vec![m])?;
    let old_disk = old.disk.clone();
    if let Some(x) = s.vres.pools.iter_mut().find(|x| x.name == u.pool) {
        if let Some(slot) = x.units.iter_mut().find(|y| y.number == u.unit) {
            *slot = unit(u.unit, &d, "Rebuilding");
        }
        x.state = "Rebuilding".into();
        x.work = "rebuilding".into();
        x.work_unit = u.unit;
        x.percent = 0;
    }
    if !old_disk.is_empty() {
        own(s, &old_disk, "", "Not configured");
    }
    own(
        s,
        &d.name,
        &u.pool,
        &format!("{} unit {}, Rebuilding", u.pool, u.unit),
    );
    pool_changed(s, &u.pool);
    // The simulator's rebuild is done at once.
    if let Some(x) = s.vres.pools.iter_mut().find(|x| x.name == u.pool) {
        if let Some(slot) = x.units.iter_mut().find(|y| y.number == u.unit) {
            slot.state = "Active".into();
        }
        let degraded = x
            .units
            .iter()
            .any(|y| matches!(y.state.as_str(), "Failed" | "Missing"));
        x.state = if degraded { "Degraded" } else { "Active" }.into();
        x.work.clear();
        x.work_unit = 0;
        x.percent = 0;
    }
    own(
        s,
        &d.name,
        &u.pool,
        &format!("{} unit {}, Active", u.pool, u.unit),
    );
    pool_changed(s, &u.pool);
    Ok(())
}

fn spare_add(s: &mut System, pool: &str, disk: &str, confirm: bool) -> Result<(), Refusal> {
    let p = find(s, pool)?.clone();
    if p.protection == "none" {
        return Err(no(
            "QVS3017",
            format!("Hot spare {disk}: a pool with no protection has none."),
        ));
    }
    let spares = p.units.iter().filter(|u| u.state == "Hot spare").count();
    if spares >= MAX_SPARES {
        return Err(no(
            "QVS3017",
            format!("Hot spare {disk}: a pool has {MAX_SPARES} at most."),
        ));
    }
    let d = candidate(s, disk, confirm, 0).map_err(|m| vec![m])?;
    let least = p
        .units
        .iter()
        .filter(|u| u.state != "Hot spare")
        .map(|u| u.bytes)
        .min()
        .unwrap_or(0);
    if d.bytes < least {
        return Err(no(
            "QVS3010",
            format!("Disk {disk} is smaller than the pool's units and cannot be its hot spare."),
        ));
    }
    if let Some(x) = s.vres.pools.iter_mut().find(|x| x.name == pool) {
        let number = u32::try_from(x.units.len() + 1).unwrap_or(0);
        x.units.push(unit(number, &d, "Hot spare"));
    }
    own(s, disk, pool, &format!("{pool} hot spare"));
    pool_changed(s, pool);
    Ok(())
}

fn spare_remove(s: &mut System, pool: &str, disk: &str) -> Result<(), Refusal> {
    let p = find(s, pool)?;
    if !p
        .units
        .iter()
        .any(|u| u.disk == disk && u.state == "Hot spare")
    {
        return Err(no(
            "QVS3017",
            format!("Hot spare {disk}: it is not one of {pool}'s hot spares."),
        ));
    }
    if let Some(x) = s.vres.pools.iter_mut().find(|x| x.name == pool) {
        x.units.retain(|u| u.disk != disk);
        for (i, u) in x.units.iter_mut().enumerate() {
            u.number = u32::try_from(i + 1).unwrap_or(0);
        }
    }
    own(s, disk, "", "Not configured");
    pool_changed(s, pool);
    Ok(())
}

fn designate(s: &mut System, pool: &str) -> Result<(), Refusal> {
    if !pool.is_empty() {
        find(s, pool)?;
    }
    let used = s
        .vres
        .images
        .iter()
        .map(|i| (i.name.as_str(), i.holder.partition))
        .chain(
            s.vres
                .volumes
                .iter()
                .map(|v| (v.name.as_str(), v.holder.partition)),
        )
        .find(|(_, h)| *h != 0);
    if let Some((what, h)) = used
        && s.vres.repository().is_some_and(|r| r.name != pool)
    {
        return Err(no(
            "QVS3205",
            format!("{what} is in use by partition {}.", s.part_name(h)),
        ));
    }
    let was: Vec<String> = s
        .vres
        .pools
        .iter()
        .filter(|p| p.repository != (p.name == pool))
        .map(|p| p.name.clone())
        .collect();
    for p in &mut s.vres.pools {
        p.repository = p.name == pool;
    }
    for name in was {
        pool_changed(s, &name);
    }
    Ok(())
}

/// The virtual disks' requests.
pub fn vdisks(s: &mut System, r: &Request) -> Result<Reply, Refusal> {
    Ok(match r {
        Request::Vdisks(q) => {
            if !q.pool.is_empty() {
                find(s, &q.pool)?;
            }
            Reply::Vdisks(
                s.vres
                    .vdisks
                    .iter()
                    .filter(|v| q.pool.is_empty() || v.pool == q.pool)
                    .cloned()
                    .collect(),
            )
        }
        Request::VdiskCreate(c) => Reply::VdiskCreate(create_vdisk(s, c)?),
        Request::VdiskResize(q) => Reply::VdiskResize(resize(s, &q.pool, &q.name, q.mib)?),
        Request::VdiskDelete(d) => {
            delete_vdisk(s, &d.pool, &d.name, d.force)?;
            Reply::VdiskDelete(Empty)
        }
        Request::VdiskNamed(q) => {
            vdisk(s, &q.pool, &q.name)?;
            Reply::VdiskNamed(named(s, &q.pool, &q.name))
        }
        _ => return Err(no_such("operation")),
    })
}

fn vdisk<'a>(s: &'a System, pool: &str, name: &str) -> Result<&'a Vdisk, Refusal> {
    find(s, pool)?;
    s.vres
        .vdisks
        .iter()
        .find(|v| v.pool == pool && v.name == name)
        .ok_or_else(|| {
            no(
                "QVS3104",
                format!("There is no virtual disk {name} in storage pool {pool}."),
            )
        })
}

/// The profiles that name virtual disk `pool/name`.
pub fn named(s: &System, pool: &str, name: &str) -> Vec<Naming> {
    let backing = format!("{pool}/{name}");
    let mut v = Vec::new();
    for p in &s.parts {
        for k in &p.profiles {
            for a in &k.profile.scsi {
                for l in a.luns.iter().filter(|l| l.backing == backing) {
                    v.push(Naming {
                        partition: p.definition.id,
                        name: p.status.name.clone(),
                        profile: k.profile.name.clone(),
                        slot: a.slot,
                        lun: l.lun,
                    });
                }
            }
        }
    }
    v
}

fn size_ok(mib: u64) -> Result<(), Refusal> {
    if (1..=MAX_VDISK_MIB).contains(&mib) {
        return Ok(());
    }
    Err(no(
        "QVS3102",
        format!("A virtual disk's size is 1 to {MAX_VDISK_MIB} MiB, not {mib}."),
    ))
}

fn room(p: &StoragePool, mib: u64) -> Result<(), Refusal> {
    if p.bytes.saturating_sub(p.allocated) >= mib * MIB {
        return Ok(());
    }
    Err(no(
        "QVS3107",
        format!("Storage pool {} has no room for {mib} MiB.", p.name),
    ))
}

fn create_vdisk(s: &mut System, c: &vres::VdiskCreate) -> Result<Vdisk, Refusal> {
    let p = find(s, &c.pool)?.clone();
    if !usable(&p.state) {
        return Err(no(
            "QVS3014",
            format!("Storage pool {} is {}.", p.name, p.state.to_lowercase()),
        ));
    }
    if !disk_name_ok(&c.name) {
        return Err(no(
            "QVS3101",
            format!("Virtual disk name {} is not valid.", c.name),
        ));
    }
    size_ok(c.mib)?;
    if s.vres
        .vdisks
        .iter()
        .any(|v| v.pool == c.pool && v.name == c.name)
    {
        return Err(no(
            "QVS3103",
            format!(
                "Storage pool {} has a virtual disk named {}.",
                c.pool, c.name
            ),
        ));
    }
    if c.thick {
        room(&p, c.mib)?;
    }
    let bytes = c.mib * MIB;
    let v = Vdisk {
        pool: c.pool.clone(),
        name: c.name.clone(),
        bytes,
        allocated: if c.thick { bytes } else { 0 },
        thick: c.thick,
        holder: qsi::host::model::Holder::default(),
    };
    s.vres.vdisks.push(v.clone());
    if let Some(x) = s.vres.pools.iter_mut().find(|x| x.name == c.pool) {
        x.allocated += v.allocated;
        x.provisioned += bytes;
    }
    vdisk_changed(s, &c.pool, &c.name, Some(v.clone()));
    pool_changed(s, &c.pool);
    Ok(v)
}

fn resize(s: &mut System, pool: &str, name: &str, mib: u64) -> Result<Vdisk, Refusal> {
    let v = vdisk(s, pool, name)?.clone();
    if v.holder.partition != 0 {
        return Err(no(
            "QVS3105",
            format!(
                "Virtual disk {name} is held by partition {}.",
                s.part_name(v.holder.partition)
            ),
        ));
    }
    size_ok(mib)?;
    if mib * MIB < v.bytes {
        return Err(no(
            "QVS3106",
            format!("Virtual disk {name} cannot be made {mib} MiB: a virtual disk only grows."),
        ));
    }
    let more = mib * MIB - v.bytes;
    if v.thick {
        room(find(s, pool)?, more / MIB)?;
    }
    let mut now = v.clone();
    now.bytes = mib * MIB;
    if now.thick {
        now.allocated = now.bytes;
    }
    if let Some(x) = s
        .vres
        .vdisks
        .iter_mut()
        .find(|x| x.pool == pool && x.name == name)
    {
        *x = now.clone();
    }
    if let Some(x) = s.vres.pools.iter_mut().find(|x| x.name == pool) {
        x.provisioned += more;
        if now.thick {
            x.allocated += more;
        }
    }
    vdisk_changed(s, pool, name, Some(now.clone()));
    pool_changed(s, pool);
    Ok(now)
}

/// Delete virtual disk `pool/name`: not while held, nor while a profile
/// names it unless `force`.
pub fn delete_vdisk(s: &mut System, pool: &str, name: &str, force: bool) -> Result<(), Refusal> {
    let v = vdisk(s, pool, name)?.clone();
    if v.holder.partition != 0 {
        return Err(no(
            "QVS3105",
            format!(
                "Virtual disk {name} is held by partition {}.",
                s.part_name(v.holder.partition)
            ),
        ));
    }
    if !force && let Some(n) = named(s, pool, name).first() {
        return Err(no(
            "QVS3108",
            format!(
                "Virtual disk {name} is named by partition {}, profile {}, slot {}.",
                n.name, n.profile, n.slot
            ),
        ));
    }
    s.vres
        .vdisks
        .retain(|x| !(x.pool == pool && x.name == name));
    if let Some(x) = s.vres.pools.iter_mut().find(|x| x.name == pool) {
        x.allocated = x.allocated.saturating_sub(v.allocated);
        x.provisioned = x.provisioned.saturating_sub(v.bytes);
    }
    vdisk_changed(s, pool, name, None);
    pool_changed(s, pool);
    Ok(())
}

/// Say a virtual disk changed.
pub fn vdisk_changed(s: &mut System, pool: &str, name: &str, vdisk: Option<Vdisk>) {
    s.push(&AnyEvent::Vres(vres::Event::Vdisk(vres::VdiskChanged {
        pool: pool.into(),
        name: name.into(),
        vdisk,
        time: now_s(),
    })));
}
