// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! `QSHQVS1`'s hardware and virtual resources as the virtual resources
//! pages show them: storage pools `POOL1` and `POOL2` and their disks,
//! virtual disks `VD01` to `VD06`, the media library, the physical tape
//! drive behind `C3`, switches `ETHSW0` and `ETHSW1`, and the devices
//! physical I/O sees. Holders are what the partitions hold.

use qsi::host::host::{Blocked, Blocker, Device, DeviceHolder, Hardware, Item, ItemKind, Owner};
use qsi::host::model::{Holder, Note};
use qsi::host::vres::{
    Image, Limits, Member, PoolDisk, PoolUnit, StoragePool, Switch, TapeDrive, Uplink, Vdisk,
    Volume,
};

use crate::system::Resources;
use crate::system::vres::storage::uuid;

/// The installation image `QSTEST01`'s IPL from D has loaded.
pub const INSTALL_ISO: &str = "QS400_V1R0M0_INSTALL.iso";
/// The image loaded in `QSDEV01`'s optical device.
pub const SAVF_ISO: &str = "HLINV_SAVF_20261028.iso";
/// The physical tape drive behind `C3`, `QSPROD01`'s.
pub const TAPE_DRIVE: &str = "U9Q10.V1.9F11A2-P1-C3-T1-L0";

const GIB: u64 = 1 << 30;
const MIB: u64 = 1 << 20;
/// A 1.92 TB SSD of the RAID controller.
const SSD: u64 = 1_920_383_410_176;
/// The 8 TB archive disk.
const HDD: u64 = 8_001_563_222_016;

/// The host's location code of `rest`.
pub fn loc(rest: &str) -> String {
    format!("U9Q10.V1.9F11A2-P1-{rest}")
}

fn item(kind: ItemKind, at: &str, name: &str, description: &str) -> Item {
    Item {
        location: loc(at),
        kind,
        name: name.into(),
        description: description.into(),
        serial: String::new(),
        bytes: 0,
        mac: String::new(),
        speed_mbps: 0,
        carrier: false,
        owner: Owner::None,
        owner_name: String::new(),
        partition: 0,
        what: String::new(),
    }
}

fn owned(mut i: Item, owner: Owner, owner_name: &str, what: &str) -> Item {
    i.owner = owner;
    i.owner_name = owner_name.into();
    i.what = what.into();
    if owner == Owner::Partition {
        i.partition = 3;
    }
    i
}

fn interface(at: &str, name: &str, mac: &str, speed: u32) -> Item {
    let mut i = item(ItemKind::Interface, at, name, "Ethernet port");
    i.mac = mac.into();
    i.speed_mbps = speed;
    i.carrier = speed > 0;
    i
}

/// A disk of the host.
struct Disk {
    name: String,
    serial: String,
    location: String,
    bytes: u64,
    what: String,
    pool: &'static str,
    owner: Owner,
}

fn disk(name: &str, serial: &str, at: &str, bytes: u64, what: &str) -> Disk {
    Disk {
        name: name.into(),
        serial: serial.into(),
        location: loc(at),
        bytes,
        what: what.into(),
        pool: "",
        owner: Owner::None,
    }
}

/// The disks: the load source, the archive disk of `POOL2`, the six
/// SSDs of `POOL1`, one more not configured and one of another host's.
fn disk_list() -> Vec<Disk> {
    let mut v = vec![
        Disk {
            owner: Owner::Host,
            ..disk(
                "sda",
                "S4EVNX0R100231",
                "T2-L0",
                480_103_981_056,
                "Load source",
            )
        },
        Disk {
            pool: "POOL2",
            owner: Owner::Pool,
            ..disk("sdb", "ZL2C4K7Q", "T2-L1", HDD, "POOL2 unit 1, Active")
        },
    ];
    for (i, n) in ["sdc", "sdd", "sde", "sdf", "sdg", "sdh"]
        .iter()
        .enumerate()
    {
        v.push(Disk {
            pool: "POOL1",
            owner: Owner::Pool,
            ..disk(
                n,
                &format!("S6Q1NA0T40{i:04}"),
                &format!("C2-T1-L{i}"),
                SSD,
                &format!("POOL1 unit {}, Active", i + 1),
            )
        });
    }
    v.push(disk(
        "sdi",
        "S6Q1NA0T400006",
        "C2-T1-L6",
        SSD,
        "Not configured",
    ));
    v.push(Disk {
        owner: Owner::Foreign,
        ..disk(
            "sdj",
            "S6Q1NA0T400007",
            "C2-T1-L7",
            SSD,
            "Another host's storage pool",
        )
    });
    v
}

/// The hardware: the PCI slots as the I/O page shows them, then the
/// network interfaces and the disks.
pub fn hardware() -> Hardware {
    let pci = |slot: &str, d: &str| item(ItemKind::Pci, slot, slot, d);
    let mut items = vec![
        owned(
            pci("C1", "10 GbE SFP+ 2-port adapter"),
            Owner::Switch,
            "ETHSW0",
            "shared through virtual Ethernet",
        ),
        owned(
            pci("C2", "SAS RAID controller (6 x 1.9 TB SSD)"),
            Owner::Pool,
            "POOL1",
            "storage pool POOL1",
        ),
        owned(
            pci("C3", "Fibre Channel 8 Gb 2-port (QLE2562)"),
            Owner::Partition,
            "QSPROD01",
            "physical assignment",
        ),
        owned(
            pci("C4", "1 GbE 4-port adapter"),
            Owner::Host,
            "",
            "management",
        ),
        owned(pci("C5", "Empty"), Owner::None, "", ""),
        owned(pci("T1", "USB 3.0 controller"), Owner::Host, "", ""),
    ];
    items.extend([
        owned(
            interface("C1-T1", "eth2", "00:1b:21:aa:00:01", 10_000),
            Owner::Switch,
            "ETHSW0",
            "Uplink of ETHSW0",
        ),
        interface("C1-T2", "eth3", "00:1b:21:aa:00:02", 0),
        owned(
            interface("C4-T1", "eth0", "00:1b:21:bb:00:01", 1000),
            Owner::Host,
            "",
            "Management",
        ),
        interface("C4-T2", "eth1", "00:1b:21:bb:00:02", 1000),
        interface("C4-T3", "eth4", "00:1b:21:bb:00:03", 0),
        interface("C4-T4", "eth5", "00:1b:21:bb:00:04", 0),
    ]);
    for d in disk_list() {
        let mut i = item(ItemKind::Disk, "", &d.name, "1.9 TB SSD");
        i.location = d.location;
        i.serial = d.serial;
        i.bytes = d.bytes;
        if d.bytes == HDD {
            i.description = "8 TB HDD".into();
        }
        items.push(owned(i, d.owner, d.pool, &d.what));
    }
    Hardware {
        manufacturer: "Quartz Systems".into(),
        product: "9Q10-V1".into(),
        processor: "8 cores".into(),
        virtualization: "KVM".into(),
        items,
    }
}

fn host_uses(slot: &str, why: &str) -> Blocked {
    Blocked {
        reason: Blocker::HostUses,
        note: Note {
            msgid: "QVS2406".into(),
            text: format!("Device {} is the host's: {why}.", loc(slot)),
            help: String::new(),
        },
    }
}

/// The PCI devices as physical I/O sees them: only `C3` is released,
/// and `QSPROD01` holds it.
pub fn devices() -> Vec<Device> {
    let dev =
        |slot: &str, address: &str, class: u32, ids: (u16, u16), d: &str, group: u32| Device {
            location: loc(slot),
            address: address.into(),
            class,
            vendor: ids.0,
            device: ids.1,
            description: d.into(),
            released: false,
            assignable: false,
            blocked: None,
            group,
            with: Vec::new(),
            holder: None,
        };
    let mut c1 = dev(
        "C1",
        "0000:41:00.0",
        0x02_0000,
        (0x8086, 0x10fb),
        "10 GbE SFP+ 2-port adapter",
        21,
    );
    c1.blocked = Some(host_uses("C1", "the uplink of ETHSW0"));
    let mut c2 = dev(
        "C2",
        "0000:42:00.0",
        0x01_0400,
        (0x1000, 0x0097),
        "SAS RAID controller",
        22,
    );
    c2.blocked = Some(host_uses("C2", "the disks of storage pool POOL1"));
    let mut c3 = dev(
        "C3",
        "0000:43:00.0",
        0x0c_0400,
        (0x1077, 0x2532),
        "Fibre Channel 8 Gb 2-port (QLE2562)",
        23,
    );
    c3.released = true;
    c3.holder = Some(DeviceHolder {
        partition: 3,
        name: "QSPROD01".into(),
        profile: "prod".into(),
    });
    let mut c4 = dev(
        "C4",
        "0000:44:00.0",
        0x02_0000,
        (0x8086, 0x1521),
        "1 GbE 4-port adapter",
        24,
    );
    c4.blocked = Some(host_uses("C4", "its management interface"));
    let mut t1 = dev(
        "T1",
        "0000:00:14.0",
        0x0c_0330,
        (0x8086, 0xa36d),
        "USB 3.0 controller",
        7,
    );
    t1.blocked = Some(Blocked {
        reason: Blocker::NotReleased,
        note: Note {
            msgid: "QVS2402".into(),
            text: format!("Device {} is not released for partitions.", loc("T1")),
            help: String::new(),
        },
    });
    vec![c1, c2, c3, c4, t1]
}

fn held(partition: u32, slot: u32) -> Holder {
    Holder { partition, slot }
}

fn vdisk(name: &str, pool: &str, gib: u64, used_gib: u64, thick: bool, holder: Holder) -> Vdisk {
    Vdisk {
        pool: pool.into(),
        name: name.into(),
        bytes: gib * GIB,
        allocated: if thick { gib * GIB } else { used_gib * GIB },
        thick,
        holder,
    }
}

fn image(name: &str, mib: u64, label: &str, holder: Holder) -> Image {
    Image {
        name: name.into(),
        bytes: mib * MIB,
        label: label.into(),
        holder,
    }
}

/// The pools, virtual disks, media, tape drive and switches.
#[allow(clippy::too_many_lines, reason = "the fixture, top to bottom")]
pub fn resources() -> Resources {
    let disks: Vec<PoolDisk> = disk_list()
        .into_iter()
        .map(|d| PoolDisk {
            name: d.name,
            serial: d.serial,
            location: d.location,
            bytes: d.bytes,
            block_size: 512,
            what: d.what,
            pool: d.pool.into(),
            free: d.owner == Owner::None,
            foreign: d.owner == Owner::Foreign,
        })
        .collect();
    let units = |pool: &str| -> Vec<PoolUnit> {
        disks
            .iter()
            .filter(|d| d.pool == pool)
            .zip(1..)
            .map(|(d, number)| PoolUnit {
                number,
                disk: d.name.clone(),
                serial: d.serial.clone(),
                location: d.location.clone(),
                bytes: d.bytes,
                state: "Active".into(),
            })
            .collect()
    };
    let vdisks = vec![
        vdisk("VD01", "POOL1", 400, 400, true, held(3, 3)),
        vdisk("VD02", "POOL1", 400, 400, true, held(3, 3)),
        vdisk("VD03", "POOL1", 120, 46, false, held(1, 3)),
        vdisk("VD04", "POOL1", 120, 38, false, held(2, 3)),
        vdisk("VD05", "POOL2", 2000, 2000, true, held(3, 4)),
        vdisk("VD06", "POOL1", 80, 0, false, Holder::default()),
    ];
    let images = vec![
        image(INSTALL_ISO, 4120, "QS400_V1R0M0", held(2, 5)),
        image(
            "QS400_CUM_26290.iso",
            412,
            "QS400_CUM_26290",
            Holder::default(),
        ),
        image(
            "QS400_LICPGM_V1R0M0.iso",
            2840,
            "QS400_LICPGM",
            Holder::default(),
        ),
        image(SAVF_ISO, 96, "HLINV_SAVF", held(1, 5)),
        image("blank_4700MB.iso", 0, "", Holder::default()),
    ];
    let volumes = vec![
        Volume {
            name: "VTAP01".into(),
            identifier: "VTAP01".into(),
            capacity_mib: 200 * 1024,
            used: 61 * GIB,
            protected: false,
            holder: held(1, 6),
        },
        Volume {
            name: "VTAP02".into(),
            identifier: "VTAP02".into(),
            capacity_mib: 200 * 1024,
            used: 12 * GIB,
            protected: false,
            holder: held(2, 6),
        },
    ];
    let provisioned = |pool: &str| -> u64 {
        vdisks
            .iter()
            .filter(|v| v.pool == pool)
            .map(|v| v.bytes)
            .sum()
    };
    let media: u64 = images.iter().map(|i| i.bytes).sum::<u64>()
        + volumes.iter().map(|v| v.capacity_mib * MIB).sum::<u64>();
    let pools = vec![
        StoragePool {
            name: "POOL1".into(),
            uuid: uuid("QSHQVS1 POOL1"),
            protection: "raid6".into(),
            state: "Active".into(),
            work: String::new(),
            work_unit: 0,
            percent: 0,
            bytes: 5400 * GIB,
            allocated: 2870 * GIB,
            provisioned: provisioned("POOL1"),
            repository: false,
            units: units("POOL1"),
            failure: String::new(),
        },
        StoragePool {
            name: "POOL2".into(),
            uuid: uuid("QSHQVS1 POOL2"),
            protection: "none".into(),
            state: "Active".into(),
            work: String::new(),
            work_unit: 0,
            percent: 0,
            bytes: 7450 * GIB,
            allocated: 1200 * GIB,
            provisioned: provisioned("POOL2") + media,
            repository: true,
            units: units("POOL2"),
            failure: String::new(),
        },
    ];
    let tape_drives = vec![TapeDrive {
        location: TAPE_DRIVE.into(),
        vendor: "QS-HQ-VTL".into(),
        product: "VTL drive 0".into(),
        serial: "21000024FF3A1B02".into(),
        released: true,
        loaded: true,
        holder: held(3, 6),
    }];
    let uplink_at = loc("C1-T1");
    let switches = vec![
        Switch {
            name: "ETHSW0".into(),
            id: 0,
            vlans: vec![10, 20, 30],
            mtu: 1500,
            uplink: Some(Uplink {
                interfaces: vec![uplink_at.clone()],
                bond: None,
                native_vlan: None,
            }),
            uplink_state: "Up".into(),
            members: vec![Member {
                location: uplink_at,
                name: "eth2".into(),
                carrier: true,
                speed_mbps: 10_000,
            }],
            ports: 0,
            failure: String::new(),
        },
        Switch {
            name: "ETHSW1".into(),
            id: 1,
            vlans: vec![99],
            mtu: 1500,
            uplink: None,
            uplink_state: "None".into(),
            members: Vec::new(),
            ports: 0,
            failure: String::new(),
        },
    ];
    Resources {
        pools,
        disks,
        vdisks,
        images,
        volumes,
        tape_drives,
        switches,
        limits: Limits {
            switches: 16,
            vlans_per_switch: 256,
            adapters_per_partition: 8,
            ports_per_switch: 256,
        },
    }
}
