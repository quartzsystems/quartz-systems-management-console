// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! The plan document: one TOML text through `base/state`, its
//! `schema_version` first, every table and key known ([`Plan`]). A
//! newer version is refused, an unknown key or value is refused, and
//! every name and number is held to its bounds ([`check`]).
//!
//! - [`check`]: the bounds.
//! - [`text`]: the reader and writer, with the size limit.
//! - [`words`]: the open values QSI carries as numbers, spelled.

pub mod check;
pub mod text;
pub mod words;

use serde::{Deserialize, Serialize};

pub use words::{
    BondMode, Console, IplKind, IplSource, Keylock, Mode, Protection, Sharing, WithHost,
};

/// The schema version this release writes.
pub const VERSION: u32 = 1;

/// A plan's file under `/data/plans` and as exported: its name and
/// this extension.
pub const EXTENSION: &str = "qmcplan";

/// A system plan.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Plan {
    /// Its name: 1 to 32 of `a-z`, `0-9`, `-`, `_` and `.`, a letter
    /// or digit first.
    pub name: String,
    /// What it is, in an operator's words.
    #[serde(default)]
    pub description: String,
    /// When it was made, Unix seconds.
    pub created: u64,
    /// The user who made it.
    pub by: String,
    /// The managed system it was captured from; empty for a template.
    #[serde(default)]
    pub system: String,
    /// Saved by the console before a code update, not by an operator.
    #[serde(default)]
    pub automatic: bool,
    /// The host's identity and size.
    pub host: HostPlan,
    /// Its storage pools.
    #[serde(default, rename = "pool")]
    pub pools: Vec<PoolPlan>,
    /// Its virtual switches.
    #[serde(default, rename = "switch")]
    pub switches: Vec<SwitchPlan>,
    /// Its shared processor pools but the default, which every host has.
    #[serde(default, rename = "procpool")]
    pub proc_pools: Vec<ProcPoolPlan>,
    /// The virtual disks the profiles name.
    #[serde(default, rename = "vdisk")]
    pub vdisks: Vec<VdiskPlan>,
    /// The tape volumes the profiles name.
    #[serde(default, rename = "volume")]
    pub volumes: Vec<VolumePlan>,
    /// The optical images the profiles name: by name, never their bytes.
    #[serde(default, rename = "image")]
    pub images: Vec<ImagePlan>,
    /// Every partition, its definition and every profile.
    #[serde(default, rename = "partition")]
    pub partitions: Vec<PartitionPlan>,
}

/// The host's identity and size.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HostPlan {
    /// Its name; empty for a template.
    #[serde(default)]
    pub name: String,
    /// Its type and model.
    #[serde(default)]
    pub model: String,
    /// Its serial number; empty for a template.
    #[serde(default)]
    pub serial: String,
    /// Its processor cores.
    pub cores: u32,
    /// Its memory, MiB.
    pub memory_mib: u64,
}

/// A storage pool.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PoolPlan {
    /// Its name.
    pub name: String,
    /// Its protection.
    pub protection: Protection,
    /// Its size, bytes.
    pub bytes: u64,
    /// It holds the media library.
    #[serde(default)]
    pub repository: bool,
    /// Its units, by location code.
    #[serde(default, rename = "unit")]
    pub units: Vec<UnitPlan>,
}

/// A unit of a storage pool.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UnitPlan {
    /// Where the disk is.
    pub location: String,
    /// The disk's name on its host (`sdb`).
    #[serde(default)]
    pub disk: String,
    /// Its size, bytes.
    pub bytes: u64,
    /// Its serial number.
    #[serde(default)]
    pub serial: String,
}

/// A virtual switch.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SwitchPlan {
    /// Its name.
    pub name: String,
    /// The VLANs it carries.
    pub vlans: Vec<u16>,
    /// Its MTU.
    pub mtu: u32,
    /// Its uplink's interfaces, by location code; none for an internal
    /// switch.
    #[serde(default)]
    pub uplink: Vec<String>,
    /// How two or more uplink interfaces are bonded.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bond: Option<BondMode>,
    /// The VLAN untagged frames on the uplink are in.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub native_vlan: Option<u16>,
}

/// A shared processor pool.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProcPoolPlan {
    /// Its name.
    pub name: String,
    /// Its cores.
    pub cores: Vec<u32>,
    /// The most its partitions use together, hundredths of a processor.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub maximum_units: Option<u32>,
}

/// A virtual disk.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VdiskPlan {
    /// Its pool.
    pub pool: String,
    /// Its name in the pool.
    pub name: String,
    /// Its size, MiB.
    pub mib: u64,
    /// Written whole when made, not as it is used.
    #[serde(default)]
    pub thick: bool,
}

/// A tape volume.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VolumePlan {
    /// Its name.
    pub name: String,
    /// The volume identifier a partition reads.
    #[serde(default)]
    pub identifier: String,
    /// Its capacity, MiB.
    pub mib: u64,
}

/// An optical image a profile names.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImagePlan {
    /// Its name in the media library.
    pub name: String,
    /// Its size, bytes, as its host had it.
    #[serde(default)]
    pub bytes: u64,
}

/// A partition: its definition and every profile.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PartitionPlan {
    /// Its ID.
    pub id: u32,
    /// Its name.
    pub name: String,
    /// Its serial number on the host it came from; empty in a template.
    #[serde(default)]
    pub serial: String,
    /// Its keylock.
    pub keylock: Keylock,
    /// Its default profile.
    pub default_profile: String,
    /// Whether and how it starts with its host.
    #[serde(default)]
    pub auto_start: AutoStartPlan,
    /// Its settings, when its host keeps them (QSI's partition family,
    /// version 3).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub settings: Option<SettingsPlan>,
    /// Every profile, by name.
    #[serde(default, rename = "profile")]
    pub profiles: Vec<ProfilePlan>,
}

/// A partition's start with its host.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AutoStartPlan {
    /// It starts.
    #[serde(default)]
    pub enabled: bool,
    /// Its place in the order.
    #[serde(default)]
    pub order: u32,
    /// Seconds after the one before.
    #[serde(default)]
    pub delay_secs: u32,
    /// Wait for it before the next.
    #[serde(default)]
    pub wait: bool,
    /// The longest that wait, seconds.
    #[serde(default)]
    pub wait_secs: u32,
}

/// A partition's settings.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SettingsPlan {
    /// What it is for.
    #[serde(default)]
    pub description: String,
    /// Control panel functions are allowed from a console.
    #[serde(default)]
    pub panel: bool,
    /// How it is shut down with its host.
    pub with_host: WithHost,
    /// A delayed shut down's delay, seconds.
    #[serde(default)]
    pub delay: u32,
}

/// A profile.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProfilePlan {
    /// Its name.
    pub name: String,
    /// Its processors.
    pub processors: ProcessorsPlan,
    /// Its memory.
    pub memory: MemoryPlan,
    /// How it IPLs.
    pub boot: BootPlan,
    /// Its virtual SCSI adapters.
    #[serde(default)]
    pub scsi: Vec<ScsiPlan>,
    /// Its virtual Ethernet adapters.
    #[serde(default)]
    pub ethernet: Vec<EthernetPlan>,
    /// Its virtual optical devices.
    #[serde(default)]
    pub optical: Vec<MediaPlan>,
    /// Its virtual tape devices.
    #[serde(default)]
    pub tape: Vec<MediaPlan>,
    /// Its physical I/O, by location code.
    #[serde(default)]
    pub physical_io: Vec<PhysicalIoPlan>,
}

/// A profile's processors.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProcessorsPlan {
    /// Dedicated or shared.
    pub mode: Mode,
    /// Processors: dedicated cores, or shared virtual processors.
    pub minimum: u32,
    /// See [`ProcessorsPlan::minimum`].
    pub desired: u32,
    /// See [`ProcessorsPlan::minimum`].
    pub maximum: u32,
    /// Shared: processing units, hundredths of a processor.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub minimum_units: Option<u32>,
    /// See [`ProcessorsPlan::minimum_units`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub desired_units: Option<u32>,
    /// See [`ProcessorsPlan::minimum_units`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub maximum_units: Option<u32>,
    /// Shared: capped or uncapped.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sharing: Option<Sharing>,
    /// Shared, uncapped: its weight.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub weight: Option<u32>,
    /// Shared: its processor pool.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pool: Option<String>,
}

/// A profile's memory, MiB.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MemoryPlan {
    /// The least it activates with.
    pub minimum_mib: u64,
    /// What it asks for.
    pub desired_mib: u64,
    /// The most it may have.
    pub maximum_mib: u64,
}

/// How a profile IPLs.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BootPlan {
    /// The IPL source.
    pub source: IplSource,
    /// Unattended or attended.
    pub ipl_type: IplKind,
    /// Its console.
    pub console: Console,
}

/// A virtual SCSI adapter.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScsiPlan {
    /// Its slot.
    pub slot: u32,
    /// Its units.
    #[serde(default)]
    pub luns: Vec<LunPlan>,
}

/// A unit of a virtual SCSI adapter.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LunPlan {
    /// Its number.
    pub lun: u32,
    /// The virtual disk: `POOL/NAME`.
    pub backing: String,
    /// The serial number the partition sees.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub serial: Option<String>,
}

/// A virtual Ethernet adapter.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EthernetPlan {
    /// Its slot.
    pub slot: u32,
    /// Its switch.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub switch: Option<String>,
    /// Its port's VLAN.
    pub vlan: u32,
    /// The VLANs it carries tagged.
    #[serde(default)]
    pub tagged: Vec<u32>,
    /// Its MAC address on the host it came from.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mac: Option<String>,
    /// An activation needs it.
    #[serde(default)]
    pub required: bool,
}

/// A virtual optical or tape device.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MediaPlan {
    /// Its slot.
    pub slot: u32,
    /// What it holds: an image, a tape volume, or `@` and a tape
    /// drive's location code; none when empty.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub media: Option<String>,
}

/// A physical device a profile assigns.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PhysicalIoPlan {
    /// Where it is.
    pub location: String,
    /// An activation needs it.
    #[serde(default)]
    pub required: bool,
}

impl Plan {
    /// A partition by ID.
    pub fn partition(&self, id: u32) -> Option<&PartitionPlan> {
        self.partitions.iter().find(|p| p.id == id)
    }

    /// Whether it names no managed system.
    pub fn template(&self) -> bool {
        self.system.is_empty()
    }

    /// The same plan as a template named `name`: its host's name and
    /// serial number, its partitions' serial numbers and every MAC
    /// address removed.
    #[must_use]
    pub fn as_template(&self, name: &str) -> Self {
        let mut t = self.clone();
        t.name = name.into();
        t.system.clear();
        t.automatic = false;
        t.host.name.clear();
        t.host.serial.clear();
        for p in &mut t.partitions {
            p.serial.clear();
            for pr in &mut p.profiles {
                for e in &mut pr.ethernet {
                    e.mac = None;
                }
            }
        }
        t
    }

    /// The pool and name of a backing, `POOL/NAME`.
    pub fn backing(b: &str) -> Option<(&str, &str)> {
        b.split_once('/')
    }
}

impl ProfilePlan {
    /// The virtual disks it names, as `(pool, name)`.
    pub fn disks(&self) -> impl Iterator<Item = (&str, &str)> {
        self.scsi
            .iter()
            .flat_map(|a| a.luns.iter())
            .filter_map(|l| Plan::backing(&l.backing))
    }

    /// The disks of its load source: its first SCSI adapter's.
    pub fn load_source(&self) -> Vec<(&str, &str)> {
        self.scsi
            .iter()
            .min_by_key(|a| a.slot)
            .map(|a| {
                a.luns
                    .iter()
                    .filter_map(|l| Plan::backing(&l.backing))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// The tape volumes its tape devices hold.
    pub fn volumes(&self) -> impl Iterator<Item = &str> {
        self.tape
            .iter()
            .filter_map(|m| m.media.as_deref())
            .filter(|m| !m.starts_with(qsi::host::model::DRIVE_MARK))
    }

    /// The tape drives it passes through, by location code.
    pub fn drives(&self) -> impl Iterator<Item = &str> {
        self.tape
            .iter()
            .filter_map(|m| m.media.as_deref())
            .filter_map(|m| m.strip_prefix(qsi::host::model::DRIVE_MARK))
    }

    /// The optical images it holds.
    pub fn images(&self) -> impl Iterator<Item = &str> {
        self.optical.iter().filter_map(|m| m.media.as_deref())
    }
}

impl state::Document for Plan {
    const FILE: &'static str = "plan.qmcplan";
    const VERSION: u32 = VERSION;
    const MIGRATIONS: &'static [state::Migration] = &[];

    fn validate(&self) -> Result<(), String> {
        check::plan(self)
    }
}
