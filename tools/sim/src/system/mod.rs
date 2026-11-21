// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! A managed system as the simulator plays it: what it says of itself,
//! its codes and partitions, its event log, the consoles it pinned, its
//! pairing password and the faults it has been told to show. One
//! [`Shared`] a system, behind a lock that every connection takes.

pub mod answer;
pub mod codes;
pub mod pairing;

use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use qsi::any::AnyEvent;
use qsi::base::{self, Keylock, PowerState, Refcode};
use qsi::host::host::{Hardware, Host, PowerOff, PowerOn};
use qsi::host::model::{Definition, PartitionState, Profile};
use qsi::host::partitions::{self, Status};
use qsi::host::refcodes::Posted;
use qsi::{Hello, PeerKind};
use qsipart::console::{Frame, Key};
use qsisess::{EventLog, Limits};
use qsitrust::{Fingerprint, Identity};

use crate::fixture::screen::Recorded;
pub use pairing::Password;

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
    /// Send bytes that are no frame after the hello.
    pub nonsense: bool,
    /// Send frames longer than any message may be.
    pub oversize: bool,
    /// Push events as fast as the log takes them.
    pub flood: bool,
    /// Grant no credit on a console's stream.
    pub no_credit: bool,
}

/// A partition of a host.
#[derive(Clone, Debug)]
pub struct Part {
    /// Its definition.
    pub definition: Definition,
    /// Its status, as the partitions family lists it.
    pub status: Status,
    /// Its codes, oldest first.
    pub history: Vec<Posted>,
    /// Its last activated profile.
    pub profile: Option<Profile>,
    /// The screen its console shows, when it has one.
    pub screen: Option<Recorded>,
    /// The keys its console has received, in order.
    pub keys: Vec<Key>,
    /// The connection viewing its console.
    pub viewer: Option<u64>,
    /// The connection that took the console over from the viewer.
    pub taken_by: Option<u64>,
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
    /// A host's hardware.
    pub hardware: Option<Hardware>,
    /// A host's partitions.
    pub parts: Vec<Part>,
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
            hardware: None,
            parts: Vec::new(),
            log: EventLog::new(&Limits::default()),
            identity,
            pins: Vec::new(),
            max_pins: 2,
            password: None,
            faults: Faults::default(),
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
