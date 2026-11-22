// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! `QSHQVS2`, the second host of the prototype's system plan deployment
//! (`plan-deploy-1.html` to `-progress.html`), as its step 2 finds it:
//! the same type, model and size as `QSHQVS1`, `POOL1` on the same six
//! SSDs at `C2` and holding its media library with the installation
//! image, no disk where `POOL2`'s was (`T2-L1`), slot `C3` empty, the
//! `C1` adapter's ports free, no switch, no named processor pool and no
//! partitions.

use qsi::PeerKind;
use qsi::host::host::{
    Amount, Blocked, Blocker, Device, Hardware, Host, Item, ItemKind, Owner, ProcPool,
};
use qsi::host::model::{Holder, Note};
use qsi::host::vres::{Image, Limits, PoolDisk, PoolUnit, StoragePool};

use super::{identity, shared};
use crate::system::vres::storage::uuid;
use crate::system::{Resources, Shared, System};

/// Its name.
pub const NAME: &str = "QSHQVS2";
/// Its serial number.
pub const SERIAL: &str = "10-7C22B4";

const GIB: u64 = 1 << 30;
const SSD: u64 = 1_920_383_410_176;

/// Its location code of `rest`.
pub fn loc(rest: &str) -> String {
    format!("U9Q10.V1.7C22B4-P1-{rest}")
}

fn item(kind: ItemKind, at: &str, name: &str, description: &str, owner: Owner) -> Item {
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
        owner,
        owner_name: String::new(),
        partition: 0,
        what: String::new(),
    }
}

fn interface(at: &str, name: &str, mac: &str, speed: u32, owner: Owner) -> Item {
    let mut i = item(ItemKind::Interface, at, name, "Ethernet port", owner);
    i.mac = mac.into();
    i.speed_mbps = speed;
    i.carrier = speed > 0;
    if owner == Owner::Host {
        i.what = "Management".into();
    }
    i
}

/// Its disks: the load source, `POOL1`'s six, one free, one another
/// host's.
fn disks() -> Vec<PoolDisk> {
    let disk = |name: &str, serial: &str, at: &str, bytes: u64, what: &str, pool: &str| PoolDisk {
        name: name.into(),
        serial: serial.into(),
        location: loc(at),
        bytes,
        block_size: 512,
        what: what.into(),
        pool: pool.into(),
        free: false,
        foreign: false,
    };
    let mut v = vec![disk(
        "sda",
        "S4EVNX0R200117",
        "T2-L0",
        480_103_981_056,
        "Load source",
        "",
    )];
    for (i, n) in ["sdc", "sdd", "sde", "sdf", "sdg", "sdh"]
        .iter()
        .enumerate()
    {
        v.push(disk(
            n,
            &format!("S6Q1NA0T50{i:04}"),
            &format!("C2-T1-L{i}"),
            SSD,
            &format!("POOL1 unit {}, Active", i + 1),
            "POOL1",
        ));
    }
    let mut free = disk(
        "sdi",
        "S6Q1NA0T500006",
        "C2-T1-L6",
        SSD,
        "Not configured",
        "",
    );
    free.free = true;
    let mut foreign = disk(
        "sdj",
        "S6Q1NA0T500007",
        "C2-T1-L7",
        SSD,
        "Another host's storage pool",
        "",
    );
    foreign.foreign = true;
    v.push(free);
    v.push(foreign);
    v
}

fn hardware(disks: &[PoolDisk]) -> Hardware {
    let pci = |slot: &str, d: &str, owner| item(ItemKind::Pci, slot, slot, d, owner);
    let mut items = vec![
        pci("C1", "10 GbE SFP+ 2-port adapter", Owner::None),
        pci("C2", "SAS RAID controller (6 x 1.9 TB SSD)", Owner::Pool),
        pci("C3", "Empty", Owner::None),
        pci("C4", "1 GbE 4-port adapter", Owner::Host),
        pci("T1", "USB 3.0 controller", Owner::Host),
        interface("C1-T1", "eth2", "00:1b:21:cc:00:01", 10_000, Owner::None),
        interface("C1-T2", "eth3", "00:1b:21:cc:00:02", 0, Owner::None),
        interface("C4-T1", "eth0", "00:1b:21:dd:00:01", 1000, Owner::Host),
        interface("C4-T2", "eth1", "00:1b:21:dd:00:02", 1000, Owner::None),
    ];
    for d in disks {
        let owner = if d.foreign {
            Owner::Foreign
        } else if !d.pool.is_empty() {
            Owner::Pool
        } else if d.free {
            Owner::None
        } else {
            Owner::Host
        };
        let mut i = Item {
            location: d.location.clone(),
            serial: d.serial.clone(),
            bytes: d.bytes,
            ..item(ItemKind::Disk, "", &d.name, "1.9 TB SSD", owner)
        };
        i.owner_name.clone_from(&d.pool);
        i.what.clone_from(&d.what);
        items.push(i);
    }
    Hardware {
        manufacturer: "Quartz Systems".into(),
        product: "9Q10-V1".into(),
        processor: "8 cores".into(),
        virtualization: "KVM".into(),
        items,
    }
}

fn devices() -> Vec<Device> {
    let blocked = |slot: &str, why: &str| Blocked {
        reason: Blocker::HostUses,
        note: Note {
            msgid: "QVS2406".into(),
            text: format!("Device {} is the host's: {why}.", loc(slot)),
            help: String::new(),
        },
    };
    let dev = |slot: &str, address: &str, d: &str, group: u32| Device {
        location: loc(slot),
        address: address.into(),
        class: 0x02_0000,
        vendor: 0x8086,
        device: 0x10fb,
        description: d.into(),
        released: false,
        assignable: false,
        blocked: None,
        group,
        with: Vec::new(),
        holder: None,
    };
    let mut c1 = dev("C1", "0000:41:00.0", "10 GbE SFP+ 2-port adapter", 21);
    c1.blocked = Some(Blocked {
        reason: Blocker::NotReleased,
        note: Note {
            msgid: "QVS2402".into(),
            text: format!("Device {} is not released for partitions.", loc("C1")),
            help: String::new(),
        },
    });
    let mut c2 = dev("C2", "0000:42:00.0", "SAS RAID controller", 22);
    c2.blocked = Some(blocked("C2", "the disks of storage pool POOL1"));
    let mut c4 = dev("C4", "0000:44:00.0", "1 GbE 4-port adapter", 24);
    c4.blocked = Some(blocked("C4", "its management interface"));
    vec![c1, c2, c4]
}

fn resources(disks: Vec<PoolDisk>) -> Resources {
    let units: Vec<PoolUnit> = disks
        .iter()
        .filter(|d| d.pool == "POOL1")
        .zip(1..)
        .map(|(d, number)| PoolUnit {
            number,
            disk: d.name.clone(),
            serial: d.serial.clone(),
            location: d.location.clone(),
            bytes: d.bytes,
            state: "Active".into(),
        })
        .collect();
    let install = Image {
        name: super::vres::INSTALL_ISO.into(),
        bytes: 4120 << 20,
        label: "QS400_V1R0M0".into(),
        holder: Holder::default(),
    };
    let pools = vec![StoragePool {
        name: "POOL1".into(),
        uuid: uuid("QSHQVS2 POOL1"),
        protection: "raid6".into(),
        state: "Active".into(),
        work: String::new(),
        work_unit: 0,
        percent: 0,
        bytes: 5400 * GIB,
        allocated: 0,
        provisioned: install.bytes,
        repository: true,
        units,
        failure: String::new(),
    }];
    Resources {
        pools,
        disks,
        vdisks: Vec::new(),
        images: vec![install],
        volumes: Vec::new(),
        tape_drives: Vec::new(),
        switches: Vec::new(),
        limits: Limits {
            switches: 16,
            vlans_per_switch: 256,
            adapters_per_partition: 8,
            ports_per_switch: 256,
        },
    }
}

/// `QSHQVS2`.
pub fn qshqvs2() -> Shared {
    let mut s = System::new(
        PeerKind::Host,
        NAME,
        SERIAL,
        "9Q10-V1",
        identity(NAME, SERIAL, 0),
    );
    s.build = "26280".into();
    s.host = Some(Host {
        sockets: 1,
        cores: 8,
        threads_per_core: 1,
        processors: Amount {
            installed: 800,
            configurable: 800,
            available: 800,
        },
        memory: Amount {
            installed: 131_072,
            configurable: 131_072,
            available: 131_072,
        },
        memory_block_mib: 256,
        reserved_cores: Vec::new(),
        reserved_memory_mib: 2048,
        pools: vec![ProcPool {
            name: "DefaultPool".into(),
            cores: (0..8).collect(),
            maximum: 800,
            entitled: 0,
            available: 800,
            partitions: 0,
        }],
        partitions: 0,
        max_partitions: 64,
    });
    let disks = disks();
    s.hardware = Some(hardware(&disks));
    s.devices = devices();
    s.vres = resources(disks);
    s.parts = Vec::new();
    s.next_revision = 1;
    // Its own MAC prefix: what it makes is not QSHQVS1's.
    s.settings.mac_prefix = Some("02:51:54".into());
    shared(s)
}
