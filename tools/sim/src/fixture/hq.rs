// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! `QSHQVS1`, the prototype's QS/VS host: its processors and memory,
//! its processor pools, and its partitions with their profiles as the
//! partition, profile and activation pages show them.

use qsi::PeerKind;
use qsi::host::host::{Amount, Host, ProcPool};
use qsi::host::model::{
    AutoStart, Boot, ConsoleMode, Current, EthernetAdapter, IplSource, IplType, Lun, MediaDevice,
    Memory, PartitionState, PhysicalIo, ProcessorMode, Processors, Profile, ScsiAdapter, Sharing,
};
use qsi::host::partition::{Settings, WithHost};
use qsi::host::refcodes::Source;
use qsi::host::vres::{Drive, DriveKind};

use super::vres::{self, TAPE_DRIVE};
use super::{identity, kept, part, screen, shared, shared_current, status};
use crate::system::activate::links;
use crate::system::{Shared, System, codes};

/// When `QSDEV01`'s activation began: 10/30/26 06:02:14.
pub const DEV_SINCE: u64 = 1_793_340_134;
/// When `QSPROD01`'s began: 10/12/26 05:30.
pub const PROD_SINCE: u64 = 1_791_783_000;

fn shared_proc(vp: (u32, u32, u32), units: (u32, u32, u32), capped: bool) -> Processors {
    Processors {
        mode: ProcessorMode::Shared,
        minimum: vp.0,
        desired: vp.1,
        maximum: vp.2,
        minimum_units: Some(units.0),
        desired_units: Some(units.1),
        maximum_units: Some(units.2),
        sharing: Some(if capped {
            Sharing::Capped
        } else {
            Sharing::Uncapped
        }),
        weight: (!capped).then_some(128),
        pool: Some("DefaultPool".into()),
    }
}

fn eth(slot: u32, switch: &str, vlan: u32, mac: &str, required: bool) -> EthernetAdapter {
    EthernetAdapter {
        slot,
        switch: Some(switch.into()),
        vlan,
        tagged: Vec::new(),
        mac: Some(mac.into()),
        required,
    }
}

fn scsi(slot: u32, backings: &[&str]) -> ScsiAdapter {
    ScsiAdapter {
        slot,
        luns: backings
            .iter()
            .zip(0..)
            .map(|(b, lun)| Lun {
                lun,
                backing: (*b).into(),
                serial: None,
            })
            .collect(),
    }
}

fn media(slot: u32, name: Option<&str>) -> MediaDevice {
    MediaDevice {
        slot,
        media: name.map(Into::into),
    }
}

/// A profile of the host's partitions: its console the service
/// channel's.
#[allow(clippy::too_many_arguments, reason = "a profile's parts, in its order")]
fn profile(
    name: &str,
    processors: Processors,
    memory: (u64, u64, u64),
    boot: (IplSource, IplType),
    scsi: Vec<ScsiAdapter>,
    ethernet: Vec<EthernetAdapter>,
    optical: Vec<MediaDevice>,
    tape: Vec<MediaDevice>,
) -> Profile {
    Profile {
        name: name.into(),
        processors,
        memory: Memory {
            minimum_mib: memory.0,
            desired_mib: memory.1,
            maximum_mib: memory.2,
        },
        boot: Boot {
            source: boot.0,
            ipl_type: boot.1,
            console: ConsoleMode::Partition,
        },
        scsi,
        ethernet,
        optical,
        tape,
        physical_io: Vec::new(),
    }
}

fn drive(slot: u32, kind: DriveKind, name: &str, prevented: bool) -> Drive {
    Drive {
        slot,
        kind,
        name: name.into(),
        prevented,
    }
}

fn settings(description: &str) -> Settings {
    Settings {
        description: description.into(),
        panel: true,
        with_host: WithHost::Delayed,
        delay: 300,
    }
}

const NORMAL: (IplSource, IplType) = (IplSource::A, IplType::Unattended);
const INSTALL: (IplSource, IplType) = (IplSource::D, IplType::Attended);

/// `QSDEV01`: running `default`, with `install` and `big-test`.
fn dev() -> crate::system::Part {
    let adapters = || {
        (
            vec![scsi(3, &["POOL1/VD03"])],
            vec![
                eth(2, "ETHSW0", 20, "02:51:53:00:01:02", true),
                eth(4, "ETHSW1", 99, "02:51:53:00:01:03", false),
            ],
            vec![media(5, None)],
            vec![media(6, Some("VTAP01"))],
        )
    };
    let (sc, et, op, ta) = adapters();
    let default = profile(
        "default",
        shared_proc((1, 2, 4), (25, 100, 200), false),
        (8192, 16_384, 32_768),
        NORMAL,
        sc,
        et,
        op,
        ta,
    );
    let install = profile(
        "install",
        shared_proc((1, 1, 2), (25, 50, 100), true),
        (4096, 8192, 16_384),
        INSTALL,
        vec![scsi(3, &["POOL1/VD03"])],
        Vec::new(),
        vec![media(5, Some(vres::INSTALL_ISO))],
        Vec::new(),
    );
    let (sc, et, op, ta) = adapters();
    let big = profile(
        "big-test",
        shared_proc((2, 3, 4), (100, 200, 300), true),
        (16_384, 32_768, 65_536),
        NORMAL,
        sc,
        et,
        op,
        ta,
    );
    let mut st = status(
        1,
        "QSDEV01",
        PartitionState::Running,
        "00000000",
        Some(shared_current(100, 2, false, 16_384)),
    );
    st.profile = "default".into();
    let mut p = part(
        1,
        "K2M4101",
        st,
        vec![
            kept(default, 7, Some(1_793_340_000)),
            kept(install, 3, Some(1_790_932_500)),
            kept(big, 5, Some(1_792_003_200)),
        ],
    );
    p.settings = settings("Development partition");
    p.definition.auto_start = AutoStart {
        enabled: true,
        order: 1,
        delay_secs: 0,
        wait: false,
        wait_secs: 0,
    };
    p.since = Some(DEV_SINCE);
    p.drives = vec![
        drive(5, DriveKind::Optical, vres::SAVF_ISO, false),
        drive(6, DriveKind::Tape, "VTAP01", true),
    ];
    if let Ok(r) = screen::read(screen::WRKACTJOB) {
        p.status.channel.system.clone_from(&r.system);
        p.screen = Some(r);
    }
    p
}

/// `QSTEST01`: not activated, its IPL from D stopped at `A6005008`
/// still holding what it was given (the processors page counts it).
fn test() -> crate::system::Part {
    let et = || {
        vec![
            eth(2, "ETHSW0", 20, "02:51:53:00:02:02", true),
            eth(4, "ETHSW1", 99, "02:51:53:00:02:03", false),
        ]
    };
    let install = profile(
        "install",
        shared_proc((1, 2, 4), (50, 150, 400), true),
        (4096, 8192, 16_384),
        INSTALL,
        vec![scsi(3, &["POOL1/VD04"])],
        et(),
        vec![media(5, Some(vres::INSTALL_ISO))],
        vec![media(6, Some("VTAP02"))],
    );
    let default = profile(
        "default",
        shared_proc((1, 2, 4), (50, 150, 400), true),
        (4096, 8192, 16_384),
        NORMAL,
        vec![scsi(3, &["POOL1/VD04"])],
        et(),
        vec![media(5, None)],
        vec![media(6, Some("VTAP02"))],
    );
    let small = profile(
        "test-small",
        shared_proc((1, 1, 2), (25, 50, 100), true),
        (2048, 4096, 8192),
        NORMAL,
        vec![scsi(3, &["POOL1/VD04"])],
        vec![eth(2, "ETHSW0", 20, "02:51:53:00:02:02", true)],
        vec![media(5, None)],
        Vec::new(),
    );
    let mut st = status(
        2,
        "QSTEST01",
        PartitionState::NotActivated,
        "A6005008",
        None,
    );
    st.profile = "install".into();
    let mut p = part(
        2,
        "K2M4102",
        st,
        vec![
            kept(install, 4, Some(1_790_840_700)),
            kept(default, 2, Some(1_789_891_200)),
            kept(small, 1, None),
        ],
    );
    p.settings = settings("Test partition");
    p.holds = Some(shared_current(150, 2, true, 8192));
    p.drives = vec![
        drive(5, DriveKind::Optical, vres::INSTALL_ISO, false),
        drive(6, DriveKind::Tape, "VTAP02", false),
    ];
    p
}

/// `QSPROD01`: running `prod` on dedicated cores 0 and 1, with the
/// Fibre Channel adapter and its tape drive.
fn prod() -> crate::system::Part {
    let mut prod = profile(
        "prod",
        Processors {
            mode: ProcessorMode::Dedicated,
            minimum: 1,
            desired: 2,
            maximum: 4,
            minimum_units: None,
            desired_units: None,
            maximum_units: None,
            sharing: None,
            weight: None,
            pool: None,
        },
        (32_768, 65_536, 98_304),
        NORMAL,
        vec![
            scsi(3, &["POOL1/VD01", "POOL1/VD02"]),
            scsi(4, &["POOL2/VD05"]),
        ],
        vec![eth(2, "ETHSW0", 10, "02:51:53:00:03:02", true)],
        vec![media(5, None)],
        vec![MediaDevice::drive(6, TAPE_DRIVE)],
    );
    prod.physical_io = vec![PhysicalIo {
        location: vres::loc("C3"),
        required: true,
    }];
    let current = Current {
        mode: ProcessorMode::Dedicated,
        cores: vec![0, 1],
        pool: String::new(),
        units: 200,
        virtuals: 2,
        capped: false,
        weight: 0,
        memory_mib: 65_536,
    };
    let mut st = status(
        3,
        "QSPROD01",
        PartitionState::Running,
        "00000000",
        Some(current),
    );
    st.profile = "prod".into();
    let mut p = part(3, "K2M4103", st, vec![kept(prod, 9, Some(PROD_SINCE))]);
    p.settings = settings("Production partition");
    p.since = Some(PROD_SINCE);
    p.drives = vec![
        drive(5, DriveKind::Optical, "", false),
        drive(6, DriveKind::PhysicalTape, TAPE_DRIVE, false),
    ];
    p
}

/// `QSHQVS1`, the prototype's QS/VS host.
pub fn qshqvs1() -> Shared {
    let mut s = System::new(
        PeerKind::Host,
        "QSHQVS1",
        "10-9F11A2",
        "9Q10-V1",
        identity("QSHQVS1", "10-9F11A2", 0),
    );
    s.build = "26280".into();
    s.host = Some(Host {
        sockets: 1,
        cores: 8,
        threads_per_core: 1,
        processors: Amount {
            installed: 800,
            configurable: 800,
            available: 350,
        },
        memory: Amount {
            installed: 131_072,
            configurable: 131_072,
            available: 40_960,
        },
        memory_block_mib: 256,
        reserved_cores: Vec::new(),
        reserved_memory_mib: 2048,
        // TESTPOOL's cores are the default pool's two highest as well:
        // the processors page shows 6.00 in the default pool and the
        // pools page 2.00 in TESTPOOL, of eight cores, two dedicated.
        pools: vec![
            ProcPool {
                name: "DefaultPool".into(),
                cores: vec![2, 3, 4, 5, 6, 7],
                maximum: 600,
                entitled: 250,
                available: 350,
                partitions: 2,
            },
            ProcPool {
                name: "TESTPOOL".into(),
                cores: vec![6, 7],
                maximum: 200,
                entitled: 0,
                available: 200,
                partitions: 0,
            },
        ],
        partitions: 3,
        max_partitions: 64,
    });
    s.hardware = Some(vres::hardware());
    s.devices = vres::devices();
    s.vres = vres::resources();
    s.parts = vec![dev(), test(), prod()];
    for p in &mut s.parts {
        p.status.ethernet = links(p);
    }
    s.next_revision = 20;
    codes::post_part(
        &mut s,
        2,
        "A6005008",
        "Partition IPL stopped: a resource is missing",
        Source::System,
        false,
    );
    shared(s)
}
