// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! The media library (optical images and tape volumes, in the pool
//! that is the media repository), an upload's checks and its end, and
//! the optical and tape devices of a running partition with what is
//! loaded in them.

use qsi::Empty;
use qsi::any::AnyEvent;
use qsi::host::model::{Holder, Note};
use qsi::host::vres::{
    self, Drive, DriveKind, Image, ImageUpload, Reply, Request, Unloaded, Volume,
};

use super::pool_changed;
use super::storage::MIB;
use crate::system::refuse::{Refusal, media_name_ok, no, no_partition, no_such, not_activated};
use crate::system::{System, now_s};

/// The largest image, bytes: 64 GiB.
pub const MAX_IMAGE: u64 = 64 << 30;
/// A tape volume's least and most capacity, MiB: 16 MiB to 16 TiB.
pub const VOLUME_MIB: std::ops::RangeInclusive<u64> = 16..=(16 << 20);

fn no_repository() -> Refusal {
    no("QVS3201", "No storage pool is the media repository.")
}

fn no_media(what: &str, name: &str) -> Refusal {
    no(
        "QVS3204",
        format!("There is no {what} {name} in the media library."),
    )
}

fn in_use(s: &System, what: &str, partition: u32) -> Refusal {
    no(
        "QVS3205",
        format!("{what} is in use by partition {}.", s.part_name(partition)),
    )
}

/// What the media library's requests come to.
pub fn answer(s: &mut System, r: &Request) -> Result<Reply, Refusal> {
    Ok(match r {
        Request::Images(_) => Reply::Images(s.vres.images.clone()),
        Request::ImageRename(q) => {
            image_rename(s, &q.name, &q.to)?;
            Reply::ImageRename(Empty)
        }
        Request::ImageDelete(q) => {
            image_delete(s, &q.name)?;
            Reply::ImageDelete(Empty)
        }
        Request::Drives(q) => {
            let p = s.part_ref(q.id).ok_or_else(|| no_partition(q.id))?;
            if !p.active() {
                return Err(not_activated(&p.status.name));
            }
            Reply::Drives(p.drives.clone())
        }
        Request::Load(l) => {
            load(s, l.id, l.slot, &l.name)?;
            Reply::Load(Empty)
        }
        Request::Unload(u) => Reply::Unload(unload(s, u.id, u.slot, u.forced)?),
        Request::Volumes(_) => Reply::Volumes(s.vres.volumes.clone()),
        Request::VolumeCreate(c) => Reply::VolumeCreate(volume_create(s, c)?),
        Request::VolumeDelete(d) => {
            volume_delete(s, &d.name)?;
            Reply::VolumeDelete(Empty)
        }
        Request::VolumeProtect(p) => {
            let v = volume(s, &p.name)?.clone();
            if v.holder.partition != 0 {
                return Err(in_use(
                    s,
                    &format!("Tape volume {}", p.name),
                    v.holder.partition,
                ));
            }
            if let Some(x) = s.vres.volumes.iter_mut().find(|x| x.name == p.name) {
                x.protected = p.on;
            }
            volume_changed(s, &p.name);
            Reply::VolumeProtect(Empty)
        }
        Request::TapeDrives(_) => Reply::TapeDrives(s.vres.tape_drives.clone()),
        _ => return Err(no_such("operation")),
    })
}

fn image<'a>(s: &'a System, name: &str) -> Result<&'a Image, Refusal> {
    s.vres
        .images
        .iter()
        .find(|i| i.name == name)
        .ok_or_else(|| no_media("image", name))
}

fn volume<'a>(s: &'a System, name: &str) -> Result<&'a Volume, Refusal> {
    s.vres
        .volumes
        .iter()
        .find(|v| v.name == name)
        .ok_or_else(|| no_media("tape volume", name))
}

/// A medium's new name: of the rule, and not the library's already.
fn new_name(s: &System, what: &str, name: &str) -> Result<(), Refusal> {
    if !media_name_ok(name) {
        return Err(no("QVS3202", format!("Media name {name} is not valid.")));
    }
    let taken = s.vres.images.iter().any(|i| i.name == name)
        || s.vres.volumes.iter().any(|v| v.name == name);
    if taken {
        return Err(no(
            "QVS3203",
            format!("The media library has a {what} named {name}."),
        ));
    }
    Ok(())
}

/// Say an image changed.
fn image_changed(s: &mut System, name: &str) {
    let image = s.vres.images.iter().find(|i| i.name == name).cloned();
    s.push(&AnyEvent::Vres(vres::Event::Image(vres::ImageChanged {
        name: name.into(),
        image,
        time: now_s(),
    })));
}

/// Say a tape volume changed.
fn volume_changed(s: &mut System, name: &str) {
    let volume = s.vres.volumes.iter().find(|v| v.name == name).cloned();
    s.push(&AnyEvent::Vres(vres::Event::Volume(vres::VolumeChanged {
        name: name.into(),
        volume,
        time: now_s(),
    })));
}

/// Count `bytes` in (or out of) the media repository.
fn count(s: &mut System, bytes: u64, add: bool) {
    let Some(name) = s.vres.repository().map(|p| p.name.clone()) else {
        return;
    };
    if let Some(p) = s.vres.pools.iter_mut().find(|p| p.name == name) {
        if add {
            p.allocated += bytes;
            p.provisioned += bytes;
        } else {
            p.allocated = p.allocated.saturating_sub(bytes);
            p.provisioned = p.provisioned.saturating_sub(bytes);
        }
    }
    pool_changed(s, &name);
}

fn image_rename(s: &mut System, name: &str, to: &str) -> Result<(), Refusal> {
    let i = image(s, name)?;
    if i.holder.partition != 0 {
        return Err(in_use(s, &format!("Image {name}"), i.holder.partition));
    }
    new_name(s, "image", to)?;
    if let Some(x) = s.vres.images.iter_mut().find(|x| x.name == name) {
        x.name = to.into();
    }
    image_changed(s, name);
    image_changed(s, to);
    Ok(())
}

fn image_delete(s: &mut System, name: &str) -> Result<(), Refusal> {
    let i = image(s, name)?.clone();
    if i.holder.partition != 0 {
        return Err(in_use(s, &format!("Image {name}"), i.holder.partition));
    }
    s.vres.images.retain(|x| x.name != name);
    image_changed(s, name);
    count(s, i.bytes, false);
    Ok(())
}

/// Whether an upload of `u` may begin: QS/VS's message when not.
/// `busy` names the images being uploaded now.
pub fn upload_check(s: &System, u: &ImageUpload, busy: &[String]) -> Result<(), Refusal> {
    let Some(repo) = s.vres.repository() else {
        return Err(no_repository());
    };
    if busy.contains(&u.name) {
        return Err(no(
            "QVS3209",
            "An import into the media library is under way.",
        ));
    }
    new_name(s, "image", &u.name)?;
    if u.bytes == 0 || !u.bytes.is_multiple_of(2048) || u.bytes > MAX_IMAGE {
        return Err(no(
            "QVS3207",
            format!(
                "The image {} is not the size it was said to be, or is not whole 2048-byte blocks.",
                u.name
            ),
        ));
    }
    if repo.bytes.saturating_sub(repo.allocated) < u.bytes {
        return Err(no(
            "QVS3107",
            format!(
                "Storage pool {} has no room for {} MiB.",
                repo.name,
                u.bytes.div_ceil(MIB)
            ),
        ));
    }
    Ok(())
}

/// An upload of image `name` ended: in the library with its `label`, or
/// not, for `error`. Says so.
pub fn upload_ended(s: &mut System, name: &str, bytes: u64, label: &str, error: Option<Note>) {
    let image = if error.is_none() {
        let i = Image {
            name: name.into(),
            bytes,
            label: label.into(),
            holder: Holder::default(),
        };
        s.vres.images.push(i.clone());
        image_changed(s, name);
        count(s, bytes, true);
        Some(i)
    } else {
        None
    };
    s.push(&AnyEvent::Vres(vres::Event::Upload(vres::UploadEnded {
        name: name.into(),
        image,
        error,
        time: now_s(),
    })));
}

/// The drive in `slot` of running partition `id`.
fn drive(s: &System, id: u32, slot: u32) -> Result<Drive, Refusal> {
    let p = s.part_ref(id).ok_or_else(|| no_partition(id))?;
    let name = &p.status.name;
    if !p.active() {
        return Err(not_activated(name));
    }
    let Some(d) = p.drives.iter().find(|d| d.slot == slot) else {
        return Err(no(
            "QVS2113",
            format!("Partition {name} has no optical or tape device in slot {slot}."),
        ));
    };
    if d.kind == DriveKind::PhysicalTape {
        return Err(no(
            "QVS2421",
            format!(
                "The tape device in slot {slot} of partition {name} is the physical drive {}.",
                d.name
            ),
        ));
    }
    Ok(d.clone())
}

/// Put `drive` in partition `id`'s place for it; say so.
fn set_drive(s: &mut System, id: u32, drive: Drive) {
    if let Some(p) = s.part(id)
        && let Some(d) = p.drives.iter_mut().find(|d| d.slot == drive.slot)
    {
        *d = drive.clone();
    }
    s.rehold();
    s.push(&AnyEvent::Vres(vres::Event::Drive(vres::DriveChanged {
        id,
        drive,
        time: now_s(),
    })));
}

fn load(s: &mut System, id: u32, slot: u32, name: &str) -> Result<(), Refusal> {
    let mut d = drive(s, id, slot)?;
    let pname = s.part_name(id);
    if !d.name.is_empty() {
        return Err(no(
            "QVS2115",
            format!(
                "The medium in slot {slot} of partition {pname} was not changed: {} is loaded.",
                d.name
            ),
        ));
    }
    if d.kind == DriveKind::Optical {
        image(s, name)?;
    } else {
        let v = volume(s, name)?;
        if v.holder.partition != 0 && v.holder.partition != id {
            return Err(in_use(
                s,
                &format!("Tape volume {name}"),
                v.holder.partition,
            ));
        }
    }
    d.name = name.into();
    d.prevented = false;
    set_drive(s, id, d);
    Ok(())
}

fn unload(s: &mut System, id: u32, slot: u32, forced: bool) -> Result<Unloaded, Refusal> {
    let mut d = drive(s, id, slot)?;
    let pname = s.part_name(id);
    if d.name.is_empty() {
        return Err(no(
            "QVS2115",
            format!(
                "The medium in slot {slot} of partition {pname} was not changed: the device is empty."
            ),
        ));
    }
    if d.prevented && !forced {
        return Err(no(
            "QVS2114",
            format!("Partition {pname} has prevented the removal of the medium in slot {slot}."),
        ));
    }
    let name = std::mem::take(&mut d.name);
    d.prevented = false;
    set_drive(s, id, d);
    Ok(Unloaded { name })
}

fn volume_create(s: &mut System, c: &vres::VolumeCreate) -> Result<Volume, Refusal> {
    if s.vres.repository().is_none() {
        return Err(no_repository());
    }
    new_name(s, "tape volume", &c.name)?;
    let id_ok = (1..=6).contains(&c.identifier.len())
        && c.identifier
            .chars()
            .all(|ch| ch.is_ascii_uppercase() || ch.is_ascii_digit());
    if !id_ok {
        return Err(no(
            "QVS3208",
            format!(
                "Tape volume {} cannot be made: its identifier {} is not 1 to 6 of A to Z and 0 to 9.",
                c.name, c.identifier
            ),
        ));
    }
    if !VOLUME_MIB.contains(&c.mib) {
        return Err(no(
            "QVS3208",
            format!(
                "Tape volume {} cannot be made: its capacity is 16 MiB to 16 TiB, not {} MiB.",
                c.name, c.mib
            ),
        ));
    }
    let v = Volume {
        name: c.name.clone(),
        identifier: c.identifier.clone(),
        capacity_mib: c.mib,
        used: 0,
        protected: false,
        holder: Holder::default(),
    };
    s.vres.volumes.push(v.clone());
    volume_changed(s, &c.name);
    count(s, 0, true);
    Ok(v)
}

fn volume_delete(s: &mut System, name: &str) -> Result<(), Refusal> {
    let v = volume(s, name)?.clone();
    if v.holder.partition != 0 {
        return Err(in_use(
            s,
            &format!("Tape volume {name}"),
            v.holder.partition,
        ));
    }
    s.vres.volumes.retain(|x| x.name != name);
    volume_changed(s, name);
    count(s, v.used, false);
    Ok(())
}
