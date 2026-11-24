// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! A managed system's hardware as its I/O and Network interfaces tabs
//! show it: each PCI function with what it is, who uses it and whether
//! a partition can be given it; each network interface with its link
//! and its use. PCI's class codes and the common vendors are named
//! here; a device's own name is the host's words.

use std::fmt::Write as _;

use qsweb::partials::{Cell, Col, List, list_table};

use mgmtproto::Kept;
use qsi::host::host::{Blocker, Device, Item, ItemKind, Owner};

/// One row of the I/O tab.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct IoRow {
    /// Its location code.
    pub location: String,
    /// Its PCI address.
    pub slot: String,
    /// What kind of device it is (`Ethernet controller`).
    pub kind: String,
    /// The vendor, the host's words for it and its IDs.
    pub device: String,
    /// Who uses it now.
    pub used: String,
    /// Whether a partition can be given it, and why not.
    pub partitions: String,
}

/// One row of the Network interfaces tab.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct NicRow {
    /// Its location code.
    pub location: String,
    /// The host's name for it (`eth0`, `enp3s0f0`).
    pub name: String,
    /// Its MAC address.
    pub mac: String,
    /// `10 Gb/s`, or `No link`.
    pub link: String,
    /// Its driver or model, in the host's words.
    pub driver: String,
    /// What it does: management, a switch's uplink, a partition's, free.
    pub used: String,
}

/// PCI's name for a class code's base class and subclass.
pub fn class_name(class: u32) -> &'static str {
    let base = (class >> 16) & 0xff;
    let sub = (class >> 8) & 0xff;
    match (base, sub) {
        (0x00, _) => "Unclassified device",
        (0x01, 0x00) => "SCSI controller",
        (0x01, 0x01) => "IDE controller",
        (0x01, 0x04) => "RAID controller",
        (0x01, 0x06) => "SATA controller",
        (0x01, 0x07) => "SAS controller",
        (0x01, 0x08) => "NVMe controller",
        (0x01, _) => "Storage controller",
        (0x02, 0x00) => "Ethernet controller",
        (0x02, 0x07) => "InfiniBand controller",
        (0x02, _) => "Network controller",
        (0x03, 0x00) => "VGA display controller",
        (0x03, _) => "Display controller",
        (0x04, _) => "Multimedia controller",
        (0x05, 0x00) => "RAM controller",
        (0x05, _) => "Memory controller",
        (0x06, 0x00) => "Host bridge",
        (0x06, 0x01) => "ISA bridge",
        (0x06, 0x04) => "PCI bridge",
        (0x06, _) => "Bridge",
        (0x07, 0x00) => "Serial controller",
        (0x07, _) => "Communication controller",
        (0x08, 0x05) => "SD host controller",
        (0x08, 0x06) => "IOMMU",
        (0x08, _) => "System peripheral",
        (0x09, _) => "Input device controller",
        (0x0b, _) => "Processor",
        (0x0c, 0x03) => "USB controller",
        (0x0c, 0x04) => "Fibre Channel controller",
        (0x0c, 0x05) => "SMBus controller",
        (0x0c, 0x07) => "IPMI interface",
        (0x0c, _) => "Serial bus controller",
        (0x0d, _) => "Wireless controller",
        (0x10, _) => "Encryption controller",
        (0x11, _) => "Signal processing controller",
        (0x12, _) => "Processing accelerator",
        (0x13, _) => "Non-essential instrumentation",
        _ => "Device",
    }
}

/// A vendor's name by its PCI identifier, for the common ones.
pub fn vendor_name(id: u16) -> Option<&'static str> {
    Some(match id {
        0x8086 | 0x8087 => "Intel",
        0x1022 => "AMD",
        0x1002 => "AMD (ATI)",
        0x14e4 => "Broadcom",
        0x1000 => "Broadcom / LSI",
        0x15b3 => "NVIDIA (Mellanox)",
        0x10de => "NVIDIA",
        0x1077 => "QLogic",
        0x10df | 0x19a2 => "Emulex",
        0x1924 => "Solarflare",
        0x1d6a => "Aquantia",
        0x1b4b => "Marvell",
        0x10ec => "Realtek",
        0x144d => "Samsung",
        0x1344 => "Micron",
        0x1c5c => "SK hynix",
        0x1e0f => "KIOXIA",
        0x1179 => "Toshiba",
        0x15b7 | 0x1c58 => "Western Digital",
        0x1987 => "Phison",
        0x9005 => "Adaptec",
        0x11f8 => "Microchip",
        0x117c => "ATTO",
        0x102b => "Matrox",
        0x1a03 => "ASPEED",
        0x1556 => "PLDA",
        0x1b21 => "ASMedia",
        0x1912 => "Renesas",
        0x104c => "Texas Instruments",
        0x1166 => "Broadcom (ServerWorks)",
        0x1028 => "Dell",
        0x103c | 0x1590 => "HPE",
        0x17aa => "Lenovo",
        0x177d => "Cavium",
        0x19e5 => "Huawei",
        0x1af4 | 0x1b36 => "Red Hat (virtual)",
        0x1234 => "QEMU (virtual)",
        0x15ad => "VMware (virtual)",
        _ => return None,
    })
}

/// `8086 3e33 060000`, the host's words for a device it does not name:
/// vendor, device and class.
fn raw_ids(words: &str) -> Option<(u16, u16, u32)> {
    let mut w = words.split_whitespace();
    let (v, d, c) = (w.next()?, w.next()?, w.next()?);
    if w.next().is_some() || v.len() != 4 || d.len() != 4 || c.len() != 6 {
        return None;
    }
    Some((
        u16::from_str_radix(v, 16).ok()?,
        u16::from_str_radix(d, 16).ok()?,
        u32::from_str_radix(c, 16).ok()?,
    ))
}

/// The device's words: the vendor, the host's name for it unless that
/// is only its IDs, and the IDs.
fn device_words(vendor: u16, device: u16, host_words: &str) -> String {
    let who = vendor_name(vendor).map_or_else(|| format!("Vendor {vendor:04x}"), str::to_string);
    let ids = format!("{vendor:04x}:{device:04x}");
    let named = host_words.trim();
    if named.is_empty() || raw_ids(named).is_some() {
        format!("{who} ({ids})")
    } else {
        format!("{who} {named} ({ids})")
    }
}

/// Who uses an item, in words.
fn owner_words(i: &Item) -> String {
    let who = match i.owner {
        Owner::None => return "Free".into(),
        Owner::Host => "QS/VS host".to_string(),
        Owner::Pool => format!("Storage pool {}", i.owner_name),
        Owner::Switch => format!("Virtual switch {}", i.owner_name),
        Owner::Partition => format!("Partition {}", i.owner_name),
        Owner::Foreign => "Not this host's".to_string(),
        Owner::Other(_) => i.owner_name.clone(),
    };
    if i.what.is_empty() || i.what == who {
        who
    } else {
        format!("{who} ({})", i.what)
    }
}

/// Whether a partition can be given a device, and why not.
fn for_partitions(d: &Device) -> String {
    let mut s = if let Some(h) = &d.holder {
        format!("Given to {} (profile {})", h.name, h.profile)
    } else if d.assignable {
        "Released: can be given".into()
    } else {
        let why = d.blocked.as_ref().map(|b| match b.reason {
            Blocker::HostUses => "No: the host uses it",
            Blocker::NotReleased => "Not released (Service Tools option 10 at the host)",
            Blocker::GroupNotReleased => "Not released: its group's other devices are not",
            Blocker::NoReset => "No: it cannot be reset",
            Blocker::NoRemapping => "No: the host has no interrupt remapping",
            Blocker::Other(_) => "No",
        });
        match (d.released, why) {
            (_, Some(w)) => w.into(),
            (true, None) => "Released".into(),
            (false, None) => "Not released (Service Tools option 10 at the host)".into(),
        }
    };
    if !d.with.is_empty() {
        let _ = write!(s, "; group {} with {}", d.group, d.with.join(", "));
    }
    s
}

/// The I/O tab's rows: each PCI function of the hardware, with what
/// physical I/O says of it when the host says.
pub fn io_of(k: &Kept) -> Vec<IoRow> {
    let Some(h) = k.hardware.as_ref() else {
        return Vec::new();
    };
    h.items
        .iter()
        .filter(|i| i.kind == ItemKind::Pci)
        .map(|i| {
            let d = k
                .resources
                .devices
                .iter()
                .find(|d| d.location == i.location);
            let (vendor, device, class) = match d {
                Some(d) => (d.vendor, d.device, Some(d.class)),
                None => raw_ids(&i.description).map_or((0, 0, None), |(v, d, c)| (v, d, Some(c))),
            };
            let host_words = d.map_or(i.description.as_str(), |d| d.description.as_str());
            IoRow {
                location: i.location.clone(),
                slot: i.name.clone(),
                kind: class.map_or_else(String::new, |c| class_name(c).into()),
                device: if class.is_some() {
                    device_words(vendor, device, host_words)
                } else {
                    i.description.clone()
                },
                used: owner_words(i),
                partitions: d.map_or_else(String::new, for_partitions),
            }
        })
        .collect()
}

/// `10 Gb/s`, `100 Mb/s`, or `No link`.
fn link_words(carrier: bool, mbps: u32) -> String {
    match (carrier, mbps) {
        (false, _) => "No link".into(),
        (true, 0) => "Up".into(),
        (true, m) if m >= 1000 && m % 1000 == 0 => format!("{} Gb/s", m / 1000),
        (true, m) if m >= 1000 => format!("{:.1} Gb/s", f64::from(m) / 1000.0),
        (true, m) => format!("{m} Mb/s"),
    }
}

/// The Network interfaces tab's rows.
pub fn network_of(k: &Kept) -> Vec<NicRow> {
    k.hardware.as_ref().map_or_else(Vec::new, |h| {
        h.items
            .iter()
            .filter(|i| i.kind == ItemKind::Interface)
            .map(|i| NicRow {
                location: i.location.clone(),
                name: i.name.clone(),
                mac: i.mac.clone(),
                link: link_words(i.carrier, i.speed_mbps),
                driver: i.description.clone(),
                used: owner_words(i),
            })
            .collect()
    })
}

/// The I/O tab's body.
pub fn io(rows: &[IoRow]) -> String {
    let list = List {
        cols: [
            "Location code",
            "Slot",
            "Type",
            "Device",
            "Used by",
            "For partitions",
        ]
        .iter()
        .map(|c| Col::plain(c))
        .collect(),
        rows: rows
            .iter()
            .map(|r| {
                vec![
                    Cell::mono(&r.location),
                    Cell::mono(&r.slot),
                    Cell::text(&r.kind),
                    Cell::text(&r.device),
                    Cell::text(&r.used),
                    Cell::text(&r.partitions),
                ]
            })
            .collect(),
        empty: "The system reports no adapters.".into(),
        keep: Vec::new(),
    };
    format!(
        "{}<p class=\"small grey\">A partition is given a device, with the rest of its isolation group, from its profile's Hardware &gt; I/O page, once the host has released the device: Service Tools option 10 at the QS/VS host. The host never releases what it uses: its load source, its pools' disks, its management interface, its switches' uplinks, its display and its bridges.</p>",
        list_table(&list)
    )
}

/// The Network interfaces tab's body.
pub fn network(rows: &[NicRow]) -> String {
    let list = List {
        cols: [
            "Location code",
            "Interface",
            "MAC address",
            "Link",
            "Driver",
            "Used for",
        ]
        .iter()
        .map(|c| Col::plain(c))
        .collect(),
        rows: rows
            .iter()
            .map(|r| {
                vec![
                    Cell::mono(&r.location),
                    Cell::mono(&r.name),
                    Cell::mono(&r.mac),
                    Cell::text(&r.link),
                    Cell::text(&r.driver),
                    Cell::text(&r.used),
                ]
            })
            .collect(),
        empty: "The host reports no network interfaces.".into(),
        keep: Vec::new(),
    };
    format!(
        "{}<p class=\"small grey\">The host's Ethernet ports. A free port can be a virtual switch's uplink (Virtual resources &gt; Virtual Ethernet) or, with its adapter released, a partition's physical I/O. The management interface is how this console reaches the host.</p>",
        list_table(&list)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_device_the_host_does_not_name_is_named_by_its_ids() {
        assert_eq!(
            raw_ids("8086 3e33 060000"),
            Some((0x8086, 0x3e33, 0x06_0000))
        );
        assert_eq!(class_name(0x06_0000), "Host bridge");
        assert_eq!(class_name(0x02_0000), "Ethernet controller");
        assert_eq!(class_name(0x01_0700), "SAS controller");
        assert_eq!(
            device_words(0x14e4, 0x1657, "14e4 1657 020000"),
            "Broadcom (14e4:1657)"
        );
        assert_eq!(
            device_words(0x8086, 0x1521, "I350 Gigabit"),
            "Intel I350 Gigabit (8086:1521)"
        );
        assert_eq!(device_words(0xabcd, 1, ""), "Vendor abcd (abcd:0001)");
        assert_eq!(link_words(true, 10_000), "10 Gb/s");
        assert_eq!(link_words(true, 2500), "2.5 Gb/s");
        assert_eq!(link_words(false, 1000), "No link");
    }
}
