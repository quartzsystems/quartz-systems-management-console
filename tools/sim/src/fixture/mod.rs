// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! The prototype's fixture data (`design/qsmc-prototype/README.md`):
//! the QS/VS host `QSHQVS1` with `QSDEV01`, `QSTEST01` and `QSPROD01`,
//! the standalone `QSHQ400`, and the 25 branch hosts of the paged
//! lists, `QSBR01VS1` to `QSBR25VS1`.

mod hq;
pub mod screen;
mod service;
mod vres;
mod vs2;

pub use hq::{DEV_SINCE, PROD_SINCE, qshqvs1};
pub use vres::loc;
pub use vs2::qshqvs2;

use std::sync::{Arc, Mutex};

use qsi::PeerKind;
use qsi::base::{Keylock, PowerState};
use qsi::host::host::{Amount, Host};
use qsi::host::model::{
    AutoStart, Boot, Channel, ConsoleMode, Current, Definition, Environment, IplSource, IplType,
    Memory, PartitionState, ProcessorMode, Processors, Profile, Sharing,
};
use qsi::host::partitions::Status;
use qsitrust::Identity;
use sha2::{Digest as _, Sha256};

use crate::system::{Kept, Part, Shared, System, codes};

/// An identity for `name` that is the same each run, unless `turn`
/// differs: a system whose certificate changed is the same name with
/// another turn.
pub fn identity(name: &str, serial: &str, turn: u32) -> Identity {
    let seed: [u8; 32] = Sha256::digest(format!("qsmcsim {name} {turn}").as_bytes()).into();
    let mut number = [0u8; 16];
    number.copy_from_slice(&seed[..16]);
    Identity::from_seed(&seed, name, serial, 1_790_000_000, number)
}

/// A system behind its lock.
pub fn shared(s: System) -> Shared {
    Arc::new(Mutex::new(s))
}

/// A partition's status, its profile `NORMAL`.
pub fn status(
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

/// A running shared partition's resources.
pub fn shared_current(units: u32, virtuals: u32, capped: bool, mib: u64) -> Current {
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

/// A branch host's profile `NORMAL`: shared, IPL from side A.
fn normal(units: u32, capped: bool, mem: (u64, u64, u64)) -> Profile {
    Profile {
        name: "NORMAL".into(),
        processors: Processors {
            mode: ProcessorMode::Shared,
            minimum: 1,
            desired: 2,
            maximum: 4,
            minimum_units: Some(50),
            desired_units: Some(units),
            maximum_units: Some(400),
            sharing: Some(if capped {
                Sharing::Capped
            } else {
                Sharing::Uncapped
            }),
            weight: Some(128),
            pool: Some("DefaultPool".into()),
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

/// A partition with its profiles, the first its default; a running
/// one holds what its status says.
pub fn part(id: u32, serial: &str, status: Status, profiles: Vec<Kept>) -> Part {
    let default = profiles
        .first()
        .map(|k| k.profile.name.clone())
        .unwrap_or_default();
    let holds = status.current.clone();
    let mut p = Part::new(
        Definition {
            id,
            name: status.name.clone(),
            environment: Environment::Standalone,
            serial: serial.into(),
            keylock: Keylock::Normal,
            default_profile: default,
            auto_start: AutoStart::default(),
            created: 1_788_000_000,
        },
        status,
    );
    p.profiles = profiles;
    p.holds = holds;
    p
}

/// A profile kept at `revision`, last activated at `last`.
pub fn kept(profile: Profile, revision: u64, last: Option<u64>) -> Kept {
    Kept {
        profile,
        revision,
        last,
    }
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
        "10-7A2B9C",
        status(0, "QSHQ400", PartitionState::Running, "00000000", None),
        Vec::new(),
    );
    if let Ok(r) = screen::read(screen::WRKACTJOB) {
        own.screen = Some(r);
    }
    s.parts = vec![own];
    service::standalone(&mut s);
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
    let s = like_branch(
        &format!("QSBR{n:02}VS1"),
        &format!("10-9F{i:02X}{i:02X}"),
        n,
        i % 4 + 1,
        &format!("QSBR{n:02}P"),
        &format!("K2B{n:02}"),
    );
    shared(s)
}

/// Host `n` (1 to 999) of the soak's fleet (Phase 8): a branch host
/// with `partitions` partitions (64 at most), named `QSSKnnnVS1`, and
/// QSHQVS1's hardware, devices and virtual resources, so a partition
/// made has `VD06` to load from and a plan can be captured.
pub fn fleet(n: u32, partitions: u32) -> Shared {
    let mut s = like_branch(
        &format!("QSSK{n:03}VS1"),
        &format!("10-8E{n:04X}"),
        n,
        partitions.min(64),
        &format!("QSSK{n:03}P"),
        &format!("K3{n:03}"),
    );
    s.hardware = Some(vres::hardware());
    s.devices = vres::devices();
    s.vres = vres::resources();
    shared(s)
}

/// A branch-like host: `count` partitions named `prefix` and its ID,
/// their serials `serials` and two digits.
fn like_branch(
    name: &str,
    serial: &str,
    n: u32,
    count: u32,
    prefix: &str,
    serials: &str,
) -> System {
    let i = n.saturating_sub(1);
    let mut s = System::new(
        PeerKind::Host,
        name,
        serial,
        "9Q10-V1",
        identity(name, serial, 0),
    );
    s.build = "26280".into();
    let units = 100 + 50 * u64::from(i % 4);
    let mib = 16_384 + 8192 * u64::from(i % 3);
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
            let pname = format!("{prefix}{id}");
            part(
                id,
                &format!("{serials}{id:02}"),
                status(
                    id,
                    &pname,
                    PartitionState::Running,
                    "00000000",
                    Some(shared_current(50, 1, true, 4096)),
                ),
                vec![kept(normal(50, true, (2048, 4096, 8192)), 1, None)],
            )
        })
        .collect();
    if n % 7 == 4 {
        s.power = PowerState::Starting;
    }
    if n % 7 == 6 {
        codes::post(&mut s, "B6000100", "System partition ended abnormally");
    }
    s.servicing.slots = vec![
        crate::system::servicing::slot("A", "V1R0M0", "26300", true),
        crate::system::servicing::slot("B", "", "", false),
    ];
    s
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
