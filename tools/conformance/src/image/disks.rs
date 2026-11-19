// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! A load source written onto image files with `base/loadsource`'s
//! writer (`loadsource::prepare`), as an installer writes one: one disk,
//! or two mirrored. Each disk gets the table, the ESP with both slots'
//! UKIs and the A/B record; the data file system is made with 4096-byte
//! sectors, so it mounts from the partition and from the mirrored set's
//! device alike. A mirrored pair also gets the set's label on each disk
//! and the data partition's bytes on both, so both units are in step
//! and the label says so.
//!
//! The installer does this for an operator (`lic/qsmcinstall`); this is
//! the harness's, and `build loadsource` runs it.

use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use diskimg::Window;
use loadsource::multi::{At, Copies};
use loadsource::prepare;
use loadsource::record::{self, HostRecord};
use loadsource::set::{self, BLOCK, Member, MemberState, SetLabel};
use release::LOAD_SOURCE;
use uuid::Uuid;

/// The serial number of the harness's `n`th load-source disk.
pub fn serial(n: usize) -> String {
    format!("QSMCHD{n}")
}

/// One disk written: where its partitions are.
#[derive(Clone, Debug)]
pub struct Written {
    /// The image file.
    pub path: PathBuf,
    /// The record partition: its offset and length in bytes.
    pub record: (u64, u64),
    /// The data partition: its offset and length in bytes.
    pub data: (u64, u64),
    /// The data partition's GUID, its identity as a set member.
    pub data_guid: Uuid,
}

fn text(e: impl std::fmt::Display) -> String {
    e.to_string()
}

/// A disk's GUIDs, fixed and different for each `disk`: the disk's
/// and its three partitions'.
fn guids(disk: usize) -> diskimg::LayoutGuids {
    let one = |part: u8| {
        let mut bytes = [0u8; 16];
        bytes[..4].copy_from_slice(&LOAD_SOURCE.guid_seed);
        bytes[4] = u8::try_from(disk).unwrap_or(u8::MAX);
        bytes[5] = part;
        bytes[6..].copy_from_slice(b"harness\0\0\0");
        bytes
    };
    prepare::guids_from(&[one(0), one(1), one(2), one(3)])
}

/// Write one disk of `bytes` at `path`: the table, the ESP with the two
/// UKIs, the record partition's head and the data partition's head
/// cleared.
fn disk(path: &Path, bytes: u64, n: usize, esp: &[u8]) -> Result<Written, String> {
    let mut file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(true)
        .open(path)
        .map_err(|e| format!("{}: {e}", path.display()))?;
    file.set_len(bytes).map_err(text)?;
    let ids = guids(n);
    let data_guid = ids.parts[2];
    let layout = prepare::write_table(&LOAD_SOURCE, &mut file, bytes, esp, &ids, &mut |_, _| {})?;
    let at = |i: usize| -> Result<(u64, u64), String> {
        let p = &layout
            .parts
            .get(i)
            .ok_or("the layout has fewer than three partitions")?
            .part;
        Ok((p.offset(), p.bytes()))
    };
    file.sync_all().map_err(text)?;
    Ok(Written {
        path: path.to_path_buf(),
        record: at(1)?,
        data: at(2)?,
        data_guid,
    })
}

fn open(path: &Path) -> Result<File, String> {
    OpenOptions::new()
        .read(true)
        .write(true)
        .open(path)
        .map_err(|e| format!("{}: {e}", path.display()))
}

/// The record partitions of `disks`, each from `base` on.
fn copies(disks: &[Written], spec: abrec::Spec, base: u64) -> Result<Copies<At<File>>, String> {
    let devices = disks
        .iter()
        .map(|d| open(&d.path).map(|f| At::new(f, d.record.0 + base)))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Copies::new(spec, devices))
}

/// Copy the data partition of `from` onto `to`'s, leaving holes where
/// it is zero: the two units then hold the same bytes.
fn copy_data(from: &Written, to: &Written, bytes: u64) -> Result<(), String> {
    let mut src = open(&from.path)?;
    let mut dst = open(&to.path)?;
    let mut piece = vec![0u8; 1 << 20];
    let mut done = 0u64;
    while done < bytes {
        let n = usize::try_from((bytes - done).min(piece.len() as u64)).map_err(text)?;
        src.seek(SeekFrom::Start(from.data.0 + done))
            .map_err(text)?;
        src.read_exact(&mut piece[..n]).map_err(text)?;
        if piece[..n].iter().any(|b| *b != 0) {
            dst.seek(SeekFrom::Start(to.data.0 + done)).map_err(text)?;
            dst.write_all(&piece[..n]).map_err(text)?;
        }
        done += n as u64;
    }
    dst.sync_all().map_err(text)
}

/// Write a load source onto `paths`: one disk, or two mirrored, each of
/// `bytes`, with `slot_a` and `slot_b` in the ESP. `now` is the time
/// the data file system and the label are made at.
pub fn write(
    paths: &[PathBuf],
    bytes: u64,
    slot_a: &[u8],
    slot_b: &[u8],
    now: i64,
) -> Result<Vec<Written>, String> {
    if !(1..=2).contains(&paths.len()) {
        return Err(format!(
            "a load source is one disk or two, not {}",
            paths.len()
        ));
    }
    let esp = prepare::esp_image(&LOAD_SOURCE, slot_a, slot_b)?;
    let disks = paths
        .iter()
        .enumerate()
        .map(|(n, p)| disk(p, bytes, n, &esp))
        .collect::<Result<Vec<_>, _>>()?;
    let data_bytes = disks.iter().map(|d| d.data.1).min().unwrap_or(0) / BLOCK * BLOCK;

    let mut first = open(&disks[0].path)?;
    let mut window = Window::new(&mut first, disks[0].data.0, disks[0].data.1);
    let uuid = Uuid::from_u128(LOAD_SOURCE.data_fs_uuid);
    prepare::format_data(&LOAD_SOURCE, &mut window, data_bytes, uuid, now)?;
    first.sync_all().map_err(text)?;

    let mut records = copies(&disks, record::spec(&LOAD_SOURCE), 0)?;
    let (_, written) = record::write_all(&mut records, HostRecord::INITIAL).map_err(text)?;
    if written != disks.len() {
        return Err(format!(
            "the record was written to {written} of {} disks",
            disks.len()
        ));
    }

    if let [one, two] = disks.as_slice() {
        copy_data(one, two, data_bytes)?;
        let member = |n: usize, d: &Written| Member {
            partition: d.data_guid,
            state: MemberState::Active,
            serial: serial(n),
            location: String::new(),
        };
        let label = SetLabel {
            id: Uuid::from_u128(LOAD_SOURCE.data_fs_uuid ^ 1),
            blocks: data_bytes / BLOCK,
            clean: true,
            written: u64::try_from(now).unwrap_or(0),
            members: vec![member(0, one), member(1, two)],
        };
        copies(&disks, set::label_spec(&LOAD_SOURCE), set::LABEL_OFFSET)?
            .write(&label.encode())
            .map_err(text)?;
    }
    Ok(disks)
}

/// The A/B record on the image at `path` and its sequence.
pub fn record_of(path: &Path) -> Result<(HostRecord, u64), String> {
    let disk = find(path)?;
    let mut records = copies(&[disk], record::spec(&LOAD_SOURCE), 0)?;
    record::read_all(&mut records).map_err(text)
}

/// Write `next` as the A/B record of the image at `path`.
pub fn set_record(path: &Path, next: HostRecord) -> Result<(), String> {
    let disk = find(path)?;
    let mut records = copies(&[disk], record::spec(&LOAD_SOURCE), 0)?;
    record::write_all(&mut records, next)
        .map(|_| ())
        .map_err(text)
}

/// The set's label on the image at `path`, when it has one.
pub fn label_of(path: &Path) -> Result<Option<SetLabel>, String> {
    let disk = find(path)?;
    let mut labels = copies(&[disk], set::label_spec(&LOAD_SOURCE), set::LABEL_OFFSET)?;
    Ok(labels
        .read()
        .ok()
        .and_then(|stored| SetLabel::decode(&stored.payload)))
}

/// The bytes of the ESP of the disk at `path`, by its table.
pub fn esp_of(path: &Path) -> Result<Vec<u8>, String> {
    let mut file = open(path)?;
    let entries = loadsource::gpt::entries(&mut file).map_err(text)?;
    let esp = entries
        .iter()
        .find(|e| e.type_guid == diskimg::layout::ESP_TYPE_GUID)
        .ok_or_else(|| format!("{} has no ESP", path.display()))?;
    let at = esp.first_lba * diskimg::SECTOR;
    let len = (esp.last_lba + 1 - esp.first_lba) * diskimg::SECTOR;
    let mut bytes = vec![0u8; usize::try_from(len).map_err(text)?];
    file.seek(SeekFrom::Start(at)).map_err(text)?;
    file.read_exact(&mut bytes).map_err(text)?;
    Ok(bytes)
}

/// Whether the disk at `path` has a GPT: whether anything wrote one.
pub fn has_table(path: &Path) -> Result<bool, String> {
    let mut file = open(path)?;
    Ok(loadsource::gpt::entries(&mut file).is_ok_and(|e| !e.is_empty()))
}

/// Where the partitions of the load-source disk at `path` are, by its
/// table.
pub fn find(path: &Path) -> Result<Written, String> {
    let mut file = open(path)?;
    let entries = loadsource::gpt::entries(&mut file).map_err(text)?;
    let of = |ty: Uuid| {
        entries
            .iter()
            .find(|e| e.type_guid == ty)
            .ok_or_else(|| format!("{} has no partition of type {ty}", path.display()))
    };
    let span = |e: &loadsource::gpt::Entry| {
        (
            e.first_lba * diskimg::SECTOR,
            (e.last_lba + 1 - e.first_lba) * diskimg::SECTOR,
        )
    };
    let record = of(loadsource::layout::record_type(&LOAD_SOURCE))?;
    let data = of(loadsource::layout::data_type(&LOAD_SOURCE))?;
    Ok(Written {
        path: path.to_path_buf(),
        record: span(record),
        data: span(data),
        data_guid: data.guid,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_mirrored_pair_has_the_record_and_the_label_on_both_disks() {
        let dir = std::env::temp_dir().join(format!("qsmc-ls-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let paths = [dir.join("a.img"), dir.join("b.img")];
        let bytes = loadsource::layout::min_disk_bytes(&LOAD_SOURCE) + (64 << 20);
        let disks = write(&paths, bytes, b"slot a", b"slot b", 1_794_998_523).unwrap();
        assert_ne!(disks[0].data_guid, disks[1].data_guid);
        for path in &paths {
            let (found, sequence) = record_of(path).unwrap();
            assert_eq!((found, sequence), (HostRecord::INITIAL, 1));
            let label = label_of(path).unwrap().unwrap();
            assert!(label.clean);
            assert_eq!(label.members.len(), 2);
            assert!(label.members.iter().all(|m| m.state == MemberState::Active));
            assert_eq!(label.members[1].serial, "QSMCHD1");
        }
        // The same data file system on both units.
        let head = |d: &Written| {
            let mut f = File::open(&d.path).unwrap();
            let mut sb = vec![0u8; 4096];
            f.seek(SeekFrom::Start(d.data.0)).unwrap();
            f.read_exact(&mut sb).unwrap();
            sb
        };
        assert_eq!(&head(&disks[0])[..4], b"XFSB");
        assert_eq!(head(&disks[0]), head(&disks[1]));
        set_record(&paths[0], HostRecord::new(abrec::Slot::B)).unwrap();
        assert_eq!(record_of(&paths[0]).unwrap().0.default_slot, abrec::Slot::B);

        let one = [dir.join("one.img")];
        write(&one, bytes, b"a", b"b", 0).unwrap();
        assert_eq!(label_of(&one[0]).unwrap(), None);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
