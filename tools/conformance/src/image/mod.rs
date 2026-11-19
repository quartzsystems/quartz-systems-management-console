// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! The images the suites boot and look into: a load source written
//! with `base/loadsource`'s writer ([`disks`]), a seed volume, and
//! a copy of an image that keeps its holes.

pub mod disks;

use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::path::Path;

use diskimg::fat;

/// A seed volume at `path`: a FAT volume labelled `label` holding
/// `seed.toml` with `text`.
pub fn seed_volume(path: &Path, label: &str, text: &str) -> Result<(), String> {
    let spec = fat::Spec {
        label: fat::label(label).map_err(|e| e.to_string())?,
        volume_id: 0x5345_4544,
        ..fat::MEDIA_ESP
    };
    let image =
        fat::build(&spec, &[(seed::format::FILE, text.as_bytes())]).map_err(|e| e.to_string())?;
    std::fs::write(path, image).map_err(|e| format!("{}: {e}", path.display()))
}

/// Copy the image `from` to `to`, leaving a hole wherever a MiB of it is
/// zero, so a copy of a 1 GiB image costs what is in it.
pub fn sparse_copy(from: &Path, to: &Path) -> Result<(), String> {
    let mut src = File::open(from).map_err(|e| format!("{}: {e}", from.display()))?;
    let len = src.metadata().map_err(|e| e.to_string())?.len();
    let mut dst = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(to)
        .map_err(|e| format!("{}: {e}", to.display()))?;
    dst.set_len(len).map_err(|e| e.to_string())?;
    let mut piece = vec![0u8; 1 << 20];
    let mut at = 0u64;
    loop {
        let n = src.read(&mut piece).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        if piece[..n].iter().any(|b| *b != 0) {
            use std::io::Seek;
            dst.seek(std::io::SeekFrom::Start(at))
                .map_err(|e| e.to_string())?;
            dst.write_all(&piece[..n]).map_err(|e| e.to_string())?;
        }
        at += n as u64;
    }
    dst.sync_all().map_err(|e| e.to_string())
}
