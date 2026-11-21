// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! The prototype's fixture data (`design/qsmc-prototype/README.md`):
//! the QS/VS host `QSHQVS1` with `QSDEV01`, `QSTEST01` and `QSPROD01`,
//! the standalone `QSHQ400`, and the 25 branch hosts of the paged
//! lists, `QSBR01VS1` to `QSBR25VS1`.

pub mod screen;

use std::sync::{Arc, Mutex};

use qsi::PeerKind;
use qsi::base::{Keylock, PowerState};
use qsi::host::host::{Amount, Hardware, Host, Item, ItemKind, Owner, ProcPool};
use qsi::host::model::{
    AutoStart, Boot, Channel, ConsoleMode, Current, Definition, Environment, IplSource, IplType,
    Memory, PartitionState, ProcessorMode, Processors, Profile, Sharing,
};
use qsi::host::partitions::Status;
use qsi::host::refcodes::Source;
use qsitrust::Identity;
use sha2::{Digest as _, Sha256};

use crate::system::{Part, Shared, System, codes};

/// An identity for `name` that is the same each run, unless `turn`
/// differs: a system whose certificate changed is the same name with
/// another turn.
pub fn identity(name: &str, serial: &str, turn: u32) -> Identity {
    let seed: [u8; 32] = Sha256::digest(format!("qsmcsim {name} {turn}").as_bytes()).into();
    let mut number = [0u8; 16];
    number.copy_from_slice(&seed[..16]);
    Identity::from_seed(&seed, name, serial, 1_790_000_000, number)
}

fn shared(s: System) -> Shared {
    Arc::new(Mutex::new(s))
}

fn status(
    id: u32,
    name: &str,
    state: PartitionState,
    code: &str,
    current: Option<Current>,
) -> Status {
    Status {
        id,
        name: name.into(),
        state,
        code: code.into(),
        text: String::new(),
        current,
        profile: "NORMAL".into(),
        last_end: String::new(),
        message: None,
        ethernet: Vec::new(),
        channel: Channel::default(),
    }
}

fn shared_current(units: u32, virtuals: u32, capped: bool, mib: u64) -> Current {
    Current {
        mode: ProcessorMode::Shared,
        cores: Vec::new(),
        pool: "DefaultPool".into(),
        units,
        virtuals,
        capped,
        weight: if capped { 0 } else { 128 },
        memory_mib: mib,
    }
}

fn profile(mode: ProcessorMode, units: u32, capped: bool, mem: (u64, u64, u64)) -> Profile {
    let shared = mode == ProcessorMode::Shared;
    Profile {
        name: "NORMAL".into(),
        processors: Processors {
            mode,
            minimum: 1,
            desired: 2,
            maximum: 4,
            minimum_units: shared.then_some(50),
            desired_units: shared.then_some(units),
            maximum_units: shared.then_some(400),
            sharing: shared.then_some(if capped {
                Sharing::Capped
            } else {
                Sharing::Uncapped
            }),
            weight: shared.then_some(128),
            pool: shared.then(|| "DefaultPool".into()),
        },
        memory: Memory {
            minimum_mib: mem.0,
            desired_mib: mem.1,
            maximum_mib: mem.2,
        },
        boot: Boot {
            source: IplSource::A,
            ipl_type: IplType::Unattended,
            console: ConsoleMode::Partition,
        },
        scsi: Vec::new(),
        ethernet: Vec::new(),
        optical: Vec::new(),
        tape: Vec::new(),
        physical_io: Vec::new(),
    }
}

fn part(id: u32, name: &str, serial: &str, status: Status, profile: Profile) -> Part {
    Part {
        definition: Definition {
            id,
            name: name.into(),
            environment: Environment::Standalone,
            serial: serial.into(),
            keylock: Keylock::Normal,
            default_profile: "NORMAL".into(),
            auto_start: AutoStart::default(),
            created: 1_788_000_000,
        },
        status,
        history: Vec::new(),
        profile: Some(profile),
        screen: None,
        keys: Vec::new(),
        viewer: None,
        taken_by: None,
    }
}

fn pci(slot: &str, description: &str, owner: Owner, owner_name: &str, what: &str) -> Item {
    Item {
        location: format!("U9Q10.V1.9F11A2-P1-{slot}"),
        kind: ItemKind::Pci,
        name: slot.into(),
        description: description.into(),
        serial: String::new(),
        bytes: 0,
        mac: String::new(),
        speed_mbps: 0,
        carrier: false,
        owner,
        owner_name: owner_name.into(),
        partition: 0,
        what: what.into(),
    }
}

/// `QSHQVS1`, the prototype's QS/VS host.
#[allow(clippy::too_many_lines, reason = "the fixture, top to bottom")]
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
        pools: vec![ProcPool {
            name: "DefaultPool".into(),
            cores: vec![2, 3, 4, 5, 6, 7],
            maximum: 600,
            entitled: 250,
            available: 350,
            partitions: 2,
        }],
        partitions: 3,
        max_partitions: 64,
    });
    s.hardware = Some(Hardware {
        manufacturer: "Quartz Systems".into(),
        product: "9Q10-V1".into(),
        processor: "8 cores".into(),
        virtualization: "KVM".into(),
        items: vec![
            pci(
                "C1",
                "10 GbE SFP+ 2-port adapter",
                Owner::Switch,
                "ETHSW0",
                "shared through virtual Ethernet",
            ),
            pci(
                "C2",
                "SAS RAID controller (6 x 1.9 TB SSD)",
                Owner::Pool,
                "POOL1",
                "storage pool POOL1",
            ),
            pci(
                "C3",
                "Fibre Channel 8 Gb 2-port (QLE2562)",
                Owner::Partition,
                "QSPROD01",
                "physical assignment",
            ),
            pci("C4", "1 GbE 4-port adapter", Owner::Host, "", "management"),
            pci("C5", "Empty", Owner::None, "", ""),
            pci("T1", "USB 3.0 controller", Owner::Host, "", ""),
        ],
    });
    let mut dev = part(
        1,
        "QSDEV01",
        "K2M4101",
        status(
            1,
            "QSDEV01",
            PartitionState::Running,
            "00000000",
            Some(shared_current(100, 2, false, 16_384)),
        ),
        profile(ProcessorMode::Shared, 100, false, (8192, 16_384, 32_768)),
    );
    if let Ok(r) = screen::read(screen::WRKACTJOB) {
        dev.status.channel.system.clone_from(&r.system);
        dev.screen = Some(r);
    }
    let test = part(
        2,
        "QSTEST01",
        "K2M4102",
        status(
            2,
            "QSTEST01",
            PartitionState::NotActivated,
            "A6005008",
            None,
        ),
        profile(ProcessorMode::Shared, 150, true, (4096, 8192, 16_384)),
    );
    let mut prod_current = shared_current(200, 2, false, 65_536);
    prod_current.mode = ProcessorMode::Dedicated;
    prod_current.cores = vec![0, 1];
    prod_current.pool = String::new();
    let prod = part(
        3,
        "QSPROD01",
        "K2M4103",
        status(
            3,
            "QSPROD01",
            PartitionState::Running,
            "00000000",
            Some(prod_current),
        ),
        profile(ProcessorMode::Dedicated, 0, false, (32_768, 65_536, 98_304)),
    );
    s.parts = vec![dev, test, prod];
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

/// `QSHQ400`, the prototype's standalone system.
pub fn qshq400() -> Shared {
    let mut s = System::new(
        PeerKind::Standalone,
        "QSHQ400",
        "10-7A2B9C",
        "9Q04-M2",
        identity("QSHQ400", "10-7A2B9C", 0),
    );
    s.hello.release = "V1R2M0".into();
    s.build = "26270".into();
    // Its own console and panel, at ID 0.
    let mut own = part(
        0,
        "QSHQ400",
        "10-7A2B9C",
        status(0, "QSHQ400", PartitionState::Running, "00000000", None),
        profile(ProcessorMode::Dedicated, 0, false, (0, 0, 0)),
    );
    own.profile = None;
    if let Ok(r) = screen::read(screen::WRKACTJOB) {
        own.screen = Some(r);
    }
    s.parts = vec![own];
    shared(s)
}

/// The control panel functions the fixture's host lists: the design's.
pub const FUNCTIONS: [(u8, &str); 30] = [
    (1, "Display selected IPL type, side and keylock"),
    (2, "Select IPL type, side and keylock"),
    (3, "Start IPL (restart partition)"),
    (4, "Lamp test"),
    (5, "Display reference code (SRC) words 1-9"),
    (7, "Display SPCN status"),
    (8, "Fast power off"),
    (11, "Display SRC word 1"),
    (12, "Display SRC words 2-5"),
    (13, "Display SRC words 6-9"),
    (20, "Display system type, model, feature and serial"),
    (21, "Force dedicated service tools (DST) on the console"),
    (22, "Force main storage dump"),
    (25, "Service switch 1 (enable 50-70)"),
    (26, "Service switch 2"),
    (33, "Reorder SPCN addressing"),
    (34, "Retry main storage dump IPL"),
    (50, "Service processor stop"),
    (51, "Service processor status"),
    (52, "Service processor start"),
    (53, "Path switch"),
    (55, "Display service processor log"),
    (63, "Display system status SRC trace"),
    (64, "Display service processor diagnostic SRC trace"),
    (65, "Deactivate remote service"),
    (66, "Activate remote service"),
    (67, "Disk unit IOP reset"),
    (68, "Concurrent maintenance power domain power off"),
    (69, "Concurrent maintenance power domain power on"),
    (70, "Dump service processor control storage"),
];

/// The functions a standalone QS/400 carries out at ID 0, as its
/// listener lists them (QS/400's `docs/QSI.md`, section 11).
pub const STANDALONE_FUNCTIONS: [(u8, &str); 15] = [
    (1, "Display selected IPL type, side and keylock"),
    (3, "Start IPL"),
    (8, "Fast power off"),
    (11, "Display SRC word 1"),
    (12, "Display SRC word 2"),
    (13, "Display SRC word 3"),
    (14, "Display SRC word 4"),
    (15, "Display SRC word 5"),
    (16, "Display SRC word 6"),
    (17, "Display SRC word 7"),
    (18, "Display SRC word 8"),
    (19, "Display SRC word 9"),
    (20, "Display system type, model and serial"),
    (21, "Force dedicated service tools (DST) on the console"),
    (22, "Force main storage dump"),
];

/// Branch host `n` (1 to 25) of the paged lists.
pub fn branch(n: u32) -> Shared {
    let i = n.saturating_sub(1);
    let name = format!("QSBR{n:02}VS1");
    let serial = format!("10-9F{i:02X}{i:02X}");
    let mut s = System::new(
        PeerKind::Host,
        &name,
        &serial,
        "9Q10-V1",
        identity(&name, &serial, 0),
    );
    s.build = "26280".into();
    let units = 100 + 50 * u64::from(i % 4);
    let mib = 16_384 + 8192 * u64::from(i % 3);
    let count = i % 4 + 1;
    s.host = Some(Host {
        sockets: 1,
        cores: 4,
        threads_per_core: 1,
        processors: Amount {
            installed: 400,
            configurable: 400,
            available: units,
        },
        memory: Amount {
            installed: 65_536,
            configurable: 65_536,
            available: mib,
        },
        memory_block_mib: 256,
        reserved_cores: Vec::new(),
        reserved_memory_mib: 2048,
        pools: Vec::new(),
        partitions: count,
        max_partitions: 64,
    });
    s.parts = (1..=count)
        .map(|id| {
            let pname = format!("QSBR{n:02}P{id}");
            part(
                id,
                &pname,
                &format!("K2B{n:02}{id:02}"),
                status(
                    id,
                    &pname,
                    PartitionState::Running,
                    "00000000",
                    Some(shared_current(50, 1, true, 4096)),
                ),
                profile(ProcessorMode::Shared, 50, true, (2048, 4096, 8192)),
            )
        })
        .collect();
    if n % 7 == 4 {
        s.power = PowerState::Starting;
    }
    if n % 7 == 6 {
        codes::post(&mut s, "B6000100", "System partition ended abnormally");
    }
    shared(s)
}

/// Whether branch host `n` is one the paged list shows not connected.
pub fn branch_away(n: u32) -> bool {
    n % 7 == 6
}

/// Every system of the prototype: the two at headquarters, then the
/// branch hosts.
pub fn prototype() -> Vec<Shared> {
    let mut v = vec![qshq400(), qshqvs1()];
    v.extend((1..=25).map(branch));
    v
}
