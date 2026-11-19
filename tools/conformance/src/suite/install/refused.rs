// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! The media half of suite 12: an ISO whose payload manifest was
//! changed after it was signed is refused before anything is written.
//! The change is one byte of the manifest's build line in a copy of the
//! test image's ISO, so the signature no longer holds.

use std::time::Instant;

use super::{
    DISK_BYTES, INSTALL_WITHIN, Media, TITLE, blank_disks, golden, installing, iso, titled_with,
};
use crate::image::disks;
use crate::suite::base::Run;

/// A copy of `iso` with one byte of its manifest changed: where.
fn tampered(iso: &[u8]) -> Result<(Vec<u8>, usize), String> {
    let head = loadsource::payload::head(&release::LOAD_SOURCE);
    let at = iso
        .windows(head.len())
        .position(|w| w == head.as_bytes())
        .ok_or("the ISO has no payload manifest")?;
    let build = iso[at..]
        .windows(6)
        .position(|w| w == b"build ")
        .map(|n| at + n + 6)
        .ok_or("the manifest has no build line")?;
    let mut copy = iso.to_vec();
    copy[build] = if copy[build] == b'0' { b'1' } else { b'0' };
    Ok((copy, build))
}

/// Suite 12, the media.
pub fn media(run: &Run<'_>) -> Result<String, String> {
    let bytes = std::fs::read(iso(run)?).map_err(|e| e.to_string())?;
    let (copy, at) = tampered(&bytes)?;
    let path = run.file("tampered.iso");
    std::fs::write(&path, copy).map_err(|e| e.to_string())?;
    let disks = blank_disks(run, "refused", 1, DISK_BYTES)?;
    let mut machine = installing(run, &disks, Media::Cdrom)?;
    machine.cdrom = Some(path);
    let mut guest = run.boot(&machine)?;
    let deadline = Instant::now() + INSTALL_WITHIN;
    let shown = titled_with(
        &mut guest,
        TITLE,
        "This installation media is refused.",
        deadline,
    )?;
    let flat = shown.split_whitespace().collect::<Vec<_>>().join(" ");
    if !flat.contains("not signed by a key this installer trusts") {
        return Err(format!("the screen does not say why:\n{shown}"));
    }
    golden("install-media-refused", &shown)?;
    guest.kill();
    if disks::has_table(&disks[0].path)? {
        return Err("the disk was written by an installer on refused media".into());
    }
    Ok(format!(
        "the ISO with byte {at} of its manifest changed is refused: `This installation media \
         is refused. Nothing was written.`, the manifest not signed by a trusted key; the disk \
         is untouched"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_byte_of_the_build_line_is_changed() {
        let iso = b"....QSMC-PAYLOAD 1\nproduct QS/MC\nrelease V1R0M0\nbuild 0abc\n....";
        let (copy, at) = tampered(iso).unwrap();
        assert_eq!(copy[at], b'1');
        assert_eq!(
            copy.iter().zip(iso.iter()).filter(|(a, b)| a != b).count(),
            1
        );
        assert!(tampered(b"nothing").is_err());
    }
}
