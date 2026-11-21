// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! A managed system as the simulator plays it: what it says of itself,
//! its codes and partitions with their profiles, its virtual resources,
//! its event log, the consoles it pinned, its pairing password and the
//! faults it has been told to show. One [`Shared`] a system, behind a
//! lock that every connection takes.

pub mod activate;
pub mod answer;
pub mod codes;
pub mod pairing;
pub mod partition;
pub mod profiles;
pub mod refuse;
#[cfg(test)]
mod tests;
pub mod vres;

use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use qsi::any::AnyEvent;
use qsi::base::{self, Keylock, PowerState, Refcode};
use qsi::host::host::{Device, Hardware, Host, PowerOff, PowerOn, Settings as HostSettings};
use qsi::host::model::{Current, Definition, PartitionState, Profile};
use qsi::host::partition::{Settings, WithHost};
use qsi::host::partitions::{self, Status};
use qsi::host::refcodes::Posted;
use qsi::host::vres::Drive;
use qsi::{Hello, PeerKind};
use qsipart::console::{Frame, Key};
use qsisess::{EventLog, Limits};
use qsitrust::{Fingerprint, Identity};

use crate::fixture::screen::Recorded;
pub use pairing::Password;
pub use vres::Resources;

/// A system's lock, shared by its connections and its controller.
pub type Shared = Arc<Mutex<System>>;

/// Take a system's lock, whatever a panicking holder left.
pub fn lock(s: &Shared) -> MutexGuard<'_, System> {
    s.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Now, in Unix seconds.
pub fn now_s() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// Now, in Unix milliseconds.
pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
}

/// `bytes` in lower-case hexadecimal.
pub fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    bytes.iter().fold(String::new(), |mut s, b| {
        let _ = write!(s, "{b:02x}");
        s
    })
}

/// A family's requests refused with one message (`refuse-family`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FamilyRefusal {
    /// The family's number.
    pub family: u16,
    /// One operation of it; every one when none.
    pub op: Option<u16>,
    /// The message identifier.
    pub id: String,
    /// Its text.
    pub text: String,
}

/// What the system has been told to do wrong.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
#[allow(clippy::struct_excessive_bools, reason = "one switch a fault")]
pub struct Faults {
    /// Close every connection at once, before TLS.
    pub refuse_connections: bool,
    /// Read and send nothing, keeping the connection: a system hung.
    pub stall: bool,
    /// Refuse every power request with this message.
    pub refuse_power: Option<(String, String)>,
    /// Refuse every panel function with this message.
    pub refuse_panel: Option<(String, String)>,
    /// Refuse a family's requests.
    pub refuse_family: Vec<FamilyRefusal>,
    /// What an activation finds missing: a virtual disk `POOL/NAME`, a
    /// switch, a device's location code.
    pub missing: Vec<String>,
    /// Send bytes that are no frame after the hello.
    pub nonsense: bool,
    /// Send frames longer than any message may be.
    pub oversize: bool,
    /// Push events as fast as the log takes them.
    pub flood: bool,
    /// Grant no credit on a console's stream.
    pub no_credit: bool,
}

/// A profile as the host keeps it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Kept {
    /// The profile.
    pub profile: Profile,
    /// Its revision.
    pub revision: u64,
    /// When it was last activated, Unix seconds.
    pub last: Option<u64>,
}

/// A partition of a host.
#[derive(Clone, Debug)]
pub struct Part {
    /// Its definition.
    pub definition: Definition,
    /// Its settings beside the definition (partition version 3).
    pub settings: Settings,
    /// Its status, as the partitions family lists it.
    pub status: Status,
    /// Its codes, oldest first.
    pub history: Vec<Posted>,
    /// Its profiles, by name.
    pub profiles: Vec<Kept>,
    /// What it holds of the host: given at activation, back at its end.
    pub holds: Option<Current>,
    /// Its optical and tape devices while it holds its resources.
    pub drives: Vec<Drive>,
    /// When its current activation began, Unix seconds.
    pub since: Option<u64>,
    /// This activation's keylock, in place of the definition's.
    pub keylock_now: Option<Keylock>,
    /// The screen its console shows, when it has one.
    pub screen: Option<Recorded>,
    /// The keys its console has received, in order.
    pub keys: Vec<Key>,
    /// The connection viewing its console.
    pub viewer: Option<u64>,
    /// The connection that took the console over from the viewer.
    pub taken_by: Option<u64>,
}

impl Part {
    /// A partition of `definition` and `status`, with nothing else yet.
    pub fn new(definition: Definition, status: Status) -> Self {
        Self {
            definition,
            settings: Settings {
                description: String::new(),
                panel: true,
                with_host: WithHost::Delayed,
                delay: 300,
            },
            status,
            history: Vec::new(),
            profiles: Vec::new(),
            holds: None,
            drives: Vec::new(),
            since: None,
            keylock_now: None,
            screen: None,
            keys: Vec::new(),
            viewer: None,
            taken_by: None,
        }
    }

    /// Its profile `name`.
    pub fn kept(&self, name: &str) -> Option<&Kept> {
        self.profiles.iter().find(|k| k.profile.name == name)
    }

    /// The profile it runs with, else its default: what its ports and
    /// adapters are.
    pub fn in_force(&self) -> Option<&Profile> {
        let name = if self.holds.is_some() {
            &self.status.profile
        } else {
            &self.definition.default_profile
        };
        self.kept(name).map(|k| &k.profile)
    }

    /// The keylock in force.
    pub fn keylock(&self) -> Keylock {
        self.keylock_now.unwrap_or(self.definition.keylock)
    }

    /// Whether it is activated: its VMM runs.
    pub fn active(&self) -> bool {
        self.status.state != PartitionState::NotActivated
    }
}

/// What the upload sink has seen (`upload-rate`, `uploaded`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct UploadStats {
    /// Bytes a second it takes; 0 for as fast as they come.
    pub rate: u64,
    /// It takes nothing at all.
    pub stalled: bool,
    /// The image of the last upload begun.
    pub name: String,
    /// Bytes of it that arrived.
    pub arrived: u64,
    /// Their SHA-256, once it ended.
    pub sha256: String,
    /// How it ended: empty while it runs, else `ok` or the message.
    pub end: String,
    /// The images being uploaded now.
    pub busy: Vec<String>,
}

/// What a simulated host says it answers: every host family and each
/// later bit this phase's pages use.
pub const HOST_CAPS: u64 = qsi::caps::HOST_V1
    | qsi::caps::PHYSICAL_IO
    | qsi::caps::PARTITION_V2
    | qsi::caps::SWITCHES
    | qsi::caps::POOLS
    | qsi::caps::PARTITION_V3
    | qsi::caps::ACTIVATIONS
    | qsi::caps::POWER_ON
    | qsi::caps::PANEL_FUNCTIONS;

/// The bits a host of QS/VS before handoff 24 lacks: what `withhold
/// QSHQVS1 new` takes away.
pub const NEW_BITS: u64 = qsi::caps::SWITCHES
    | qsi::caps::POOLS
    | qsi::caps::PARTITION_V3
    | qsi::caps::ACTIVATIONS
    | qsi::caps::POWER_ON
    | qsi::caps::PANEL_FUNCTIONS;

/// A managed system.
pub struct System {
    /// What its hello says.
    pub hello: Hello,
    /// Its type and model.
    pub model: String,
    /// Its build.
    pub build: String,
    /// Its power state.
    pub power: PowerState,
    /// Its keylock, when it has one.
    pub keylock: Option<Keylock>,
    /// The slot it runs from.
    pub slot: String,
    /// Its own codes, oldest first.
    pub codes: Vec<Refcode>,
    /// A host's resources.
    pub host: Option<Host>,
    /// A host's settings for its partitions.
    pub settings: HostSettings,
    /// A host's hardware.
    pub hardware: Option<Hardware>,
    /// A host's devices for physical I/O.
    pub devices: Vec<Device>,
    /// A host's partitions.
    pub parts: Vec<Part>,
    /// A host's virtual resources.
    pub vres: Resources,
    /// The revision the next profile change takes.
    pub next_revision: u64,
    /// Its events.
    pub log: EventLog,
    /// Its identity.
    pub identity: Identity,
    /// The consoles it pinned, with their names.
    pub pins: Vec<(Fingerprint, String)>,
    /// The most consoles it pins.
    pub max_pins: usize,
    /// Its pairing password, while one is out.
    pub password: Option<Password>,
    /// What it has been told to do wrong.
    pub faults: Faults,
    /// The upload sink's pace and what it saw.
    pub upload: UploadStats,
    /// Connections of an older generation are closed.
    pub generation: u64,
    /// Whether it answers connections at all (a machine powered off
    /// does not).
    pub up: bool,
    /// What it logged, a line each: requests with their users,
    /// pairings, sessions.
    pub lines: Vec<String>,
    /// When it was made.
    pub since: Instant,
    /// The number the next connection takes.
    pub next_conn: u64,
    /// A host's power-on settings.
    pub power_on: PowerOn,
}

impl System {
    /// A system of `kind` named `name`, with nothing on it yet.
    pub fn new(kind: PeerKind, name: &str, serial: &str, model: &str, identity: Identity) -> Self {
        let caps = match kind {
            PeerKind::Host => HOST_CAPS,
            _ => qsi::caps::STANDALONE,
        };
        Self {
            hello: Hello {
                version: qsi::VERSION,
                caps,
                kind,
                name: name.into(),
                release: "V1R0M0".into(),
                serial: serial.into(),
            },
            model: model.into(),
            build: String::new(),
            power: PowerState::Running,
            keylock: Some(Keylock::Normal),
            slot: "A".into(),
            codes: Vec::new(),
            host: None,
            settings: HostSettings {
                max_partitions: 64,
                memory_block_mib: 256,
                reserved_cores: 0,
                reserved_memory_mib: Some(2048),
                mac_prefix: Some("02:51:53".into()),
                delayed_grace_secs: 120,
                system_timeout_secs: 300,
                host_timeout_secs: 300,
                channel_wait_secs: 30,
                max_dumps: 3,
                max_dump_mib: 262_144,
            },
            hardware: None,
            devices: Vec::new(),
            parts: Vec::new(),
            vres: Resources::default(),
            next_revision: 1,
            log: EventLog::new(&Limits::default()),
            identity,
            pins: Vec::new(),
            max_pins: 2,
            password: None,
            faults: Faults::default(),
            upload: UploadStats::default(),
            generation: 0,
            up: true,
            lines: Vec::new(),
            since: Instant::now(),
            next_conn: 1,
            power_on: PowerOn {
                start_partitions: true,
                power_off: PowerOff::Delayed,
                delay: 300,
            },
        }
    }

    /// Its name.
    pub fn name(&self) -> &str {
        &self.hello.name
    }

    /// Its current code: the last it posted, or `00000000`.
    pub fn current(&self) -> Option<Refcode> {
        self.codes.last().cloned()
    }

    /// Its state, as base's `State` says it.
    pub fn state(&self) -> base::State {
        base::State {
            power: self.power,
            refcode: self.current(),
            release: self.hello.release.clone(),
            slot: self.slot.clone(),
            keylock: self.keylock,
        }
    }

    /// Its identity, as base's `Identify` says it.
    pub fn identify(&self) -> base::Identity {
        base::Identity {
            kind: self.hello.kind,
            name: self.hello.name.clone(),
            release: self.hello.release.clone(),
            serial: self.hello.serial.clone(),
            model: self.model.clone(),
            build: self.build.clone(),
        }
    }

    /// Whether it is a host.
    pub fn is_host(&self) -> bool {
        self.hello.kind == PeerKind::Host
    }

    /// A partition by ID; a standalone system's own console and panel
    /// are its part at ID 0.
    pub fn part(&mut self, id: u32) -> Option<&mut Part> {
        self.parts.iter_mut().find(|p| p.definition.id == id)
    }

    /// A partition by ID, to read.
    pub fn part_ref(&self, id: u32) -> Option<&Part> {
        self.parts.iter().find(|p| p.definition.id == id)
    }

    /// A partition's name by ID; the ID when there is none.
    pub fn part_name(&self, id: u32) -> String {
        self.part_ref(id)
            .map_or_else(|| id.to_string(), |p| p.status.name.clone())
    }

    /// The revision a profile change takes.
    pub fn revision(&mut self) -> u64 {
        let r = self.next_revision;
        self.next_revision += 1;
        r
    }

    /// Push an event to the log.
    pub fn push(&mut self, e: &AnyEvent) {
        let _ = self.log.push_any(e);
    }

    /// The partitions' list.
    pub fn statuses(&self) -> Vec<Status> {
        self.parts.iter().map(|p| p.status.clone()).collect()
    }

    /// Set a partition's state and say so.
    pub fn set_state(&mut self, id: u32, state: PartitionState) {
        let Some(p) = self.part(id) else {
            return;
        };
        p.status.state = state;
        let name = p.status.name.clone();
        self.push(&AnyEvent::Partitions(partitions::Event::State(
            partitions::StateChanged {
                id,
                name,
                state,
                time: now_s(),
            },
        )));
    }

    /// Say a partition was created, changed or deleted.
    pub fn defined(&mut self, id: u32, name: &str, exists: bool) {
        self.push(&AnyEvent::Partitions(partitions::Event::Defined(
            partitions::Defined {
                id,
                name: name.into(),
                exists,
                time: now_s(),
            },
        )));
    }

    /// Note a line of its log.
    pub fn note(&mut self, line: String) {
        if self.lines.len() > 10_000 {
            self.lines.drain(..1000);
        }
        self.lines.push(line);
    }

    /// Take a partition's console for connection `conn`: the screen to
    /// send first.
    pub fn screen_of(&self, id: u32) -> Option<Frame> {
        self.parts
            .iter()
            .find(|p| p.definition.id == id)
            .and_then(|p| p.screen.as_ref().map(|r| r.frame.clone()))
    }
}
