// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! What the storage service is told of the machine's disks: each one's
//! name and the serial number its device reports. QS/MC has no hardware
//! inventory (QS/VS's is `lic/hostinv`, outside `base/`;
//! `docs/OPEN_QUESTIONS.md`), so there are no location codes: a unit is
//! named by its number and serial number.

#![cfg_attr(not(target_os = "linux"), allow(dead_code))]

use std::fs;
use std::path::Path;

use stgd::DiskIdentity;

/// Where the serial number is: a virtio disk has it beside its queue,
/// an `NVMe`, SCSI or SATA disk on its device.
const SERIAL: [&str; 2] = ["serial", "device/serial"];

/// The serial number of the disk at `dir`, a `/sys/block` entry.
fn serial(dir: &Path) -> String {
    SERIAL
        .iter()
        .find_map(|f| fs::read_to_string(dir.join(f)).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_default()
}

/// The disks under `block`, in name order.
pub fn identities_in(block: &Path) -> Vec<DiskIdentity> {
    let mut out: Vec<DiskIdentity> = fs::read_dir(block)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            DiskIdentity {
                serial: serial(&e.path()),
                name,
                location: String::new(),
            }
        })
        .collect();
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

/// The machine's disks, read now.
pub fn identities() -> Vec<DiskIdentity> {
    identities_in(Path::new("/sys/block"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_disk_s_serial_is_read_where_its_kind_keeps_it() {
        let dir = std::env::temp_dir().join(format!("qsmcstgd-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("vda")).unwrap();
        fs::write(dir.join("vda/serial"), "QSMCHD0\n").unwrap();
        fs::create_dir_all(dir.join("nvme0n1/device")).unwrap();
        fs::write(dir.join("nvme0n1/device/serial"), "  S1 \n").unwrap();
        fs::create_dir_all(dir.join("sda")).unwrap();
        let disks = identities_in(&dir);
        let pairs: Vec<(&str, &str)> = disks
            .iter()
            .map(|d| (d.name.as_str(), d.serial.as_str()))
            .collect();
        assert_eq!(pairs, [("nvme0n1", "S1"), ("sda", ""), ("vda", "QSMCHD0")]);
        assert!(disks.iter().all(|d| d.location.is_empty()));
        fs::remove_dir_all(&dir).unwrap();
    }
}
