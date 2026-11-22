// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! A host as it is read now: what a capture takes and a validation
//! checks against. Read through `mgmtd` with requests it already
//! carries ([`read`]): nothing of `mgmtd` was added for plans.

pub mod read;

use qsi::host::host::{Device, Hardware, Host, ItemKind, Owner};
use qsi::host::model::{Definition, Profile};
use qsi::host::partition::Settings;
use qsi::host::vres::{Image, PoolDisk, StoragePool, Switch, TapeDrive, Vdisk, Volume};

/// One partition of a host.
#[derive(Clone, Debug, PartialEq)]
pub struct PartView {
    /// Its definition.
    pub definition: Definition,
    /// Its settings, when the host keeps them.
    pub settings: Option<Settings>,
    /// Every profile, whole.
    pub profiles: Vec<Profile>,
}

/// A host.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct HostView {
    /// Its name.
    pub name: String,
    /// A QS/VS host, not a standalone system.
    pub is_host: bool,
    /// Its session is open.
    pub connected: bool,
    /// Why not, an operator's words.
    pub reason: String,
    /// The capabilities its hello agreed.
    pub caps: u64,
    /// Its type and model.
    pub model: String,
    /// Its serial number.
    pub serial: String,
    /// Its processors, memory and processor pools.
    pub host: Option<Host>,
    /// Its hardware: interfaces and disks with their owners.
    pub hardware: Option<Hardware>,
    /// Its PCI devices for partitions.
    pub devices: Vec<Device>,
    /// Its whole disks, as a pool sees them.
    pub disks: Vec<PoolDisk>,
    /// Its storage pools.
    pub pools: Vec<StoragePool>,
    /// Its virtual disks.
    pub vdisks: Vec<Vdisk>,
    /// Its media library.
    pub images: Vec<Image>,
    /// Its tape volumes.
    pub volumes: Vec<Volume>,
    /// Its tape drives for partitions.
    pub tape_drives: Vec<TapeDrive>,
    /// Its virtual switches.
    pub switches: Vec<Switch>,
    /// Its partitions.
    pub partitions: Vec<PartView>,
}

/// Where a location code is within its machine: the code without the
/// machine's own prefix (`U9Q10.V1.9F11A2-P1-C3` is `C3`; a QS/VS code
/// such as `SPCIE-SLOT-3` is itself). Two hosts' devices are the same
/// place when their places are equal.
pub fn place(location: &str) -> &str {
    let Some((first, rest)) = location.split_once('-') else {
        return location;
    };
    if first.starts_with('U') && first.contains('.') {
        // The planar (`P1-`) is the machine's too.
        return match rest.split_once('-') {
            Some((planar, r))
                if planar.starts_with('P') && planar[1..].bytes().all(|b| b.is_ascii_digit()) =>
            {
                r
            }
            _ => rest,
        };
    }
    location
}

/// A place as a message says it: `C1-T1` is `C1 port 1`, `C3` is
/// `C3`.
pub fn shown(location: &str) -> String {
    let p = place(location);
    if let Some((slot, port)) = p.rsplit_once('-')
        && let Some(n) = port.strip_prefix('T').or_else(|| port.strip_prefix('E'))
        && !n.is_empty()
        && n.bytes().all(|b| b.is_ascii_digit())
    {
        return format!("{slot} port {n}");
    }
    p.to_string()
}

impl HostView {
    /// Its device at the place of `location`.
    pub fn device_at(&self, location: &str) -> Option<&Device> {
        let want = place(location);
        self.devices.iter().find(|d| place(&d.location) == want)
    }

    /// Its tape drive at the place of `location`.
    pub fn drive_at(&self, location: &str) -> Option<&TapeDrive> {
        let want = place(location);
        self.tape_drives.iter().find(|d| place(&d.location) == want)
    }

    /// Its disk at the place of `location`.
    pub fn disk_at(&self, location: &str) -> Option<&PoolDisk> {
        let want = place(location);
        self.disks.iter().find(|d| place(&d.location) == want)
    }

    /// Its network interface at the place of `location`: its owner.
    pub fn interface_at(&self, location: &str) -> Option<(Owner, &str)> {
        let want = place(location);
        self.hardware.as_ref()?.items.iter().find_map(|i| {
            (i.kind == ItemKind::Interface && place(&i.location) == want)
                .then_some((i.owner, i.location.as_str()))
        })
    }

    /// Its partition by ID.
    pub fn partition(&self, id: u32) -> Option<&PartView> {
        self.partitions.iter().find(|p| p.definition.id == id)
    }

    /// Its partition by name.
    pub fn partition_named(&self, name: &str) -> Option<&PartView> {
        self.partitions.iter().find(|p| p.definition.name == name)
    }

    /// The pool that holds its media library.
    pub fn repository(&self) -> Option<&StoragePool> {
        self.pools.iter().find(|p| p.repository)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_place_is_the_code_within_its_machine() {
        assert_eq!(place("U9Q10.V1.9F11A2-P1-C3"), "C3");
        assert_eq!(place("U9Q10.V1.7C22B4-P1-C1-T1"), "C1-T1");
        assert_eq!(place("SPCIE-SLOT-3-E0"), "SPCIE-SLOT-3-E0");
        assert_eq!(place("NOPCI-MMCBLK0"), "NOPCI-MMCBLK0");
        assert_eq!(shown("U9Q10.V1.9F11A2-P1-C1-T1"), "C1 port 1");
        assert_eq!(shown("U9Q10.V1.9F11A2-P1-C3"), "C3");
        assert_eq!(shown("SPCIE-SLOT-3-E0"), "SPCIE-SLOT-3 port 0");
    }
}
