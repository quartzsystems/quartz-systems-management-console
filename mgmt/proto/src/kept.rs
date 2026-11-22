// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! What `mgmtd` keeps about a managed system: how it was paired, its
//! connection, and what it last said of itself, as QSI's own types. A
//! list page reads only this; it never waits on a managed system.

use ipc::{wire_enum, wire_struct};
use qsi::PeerKind;
use qsi::base::{Identity, Level, PowerState, Refcode, Space, State, SystemDump};
use qsi::host::dumps::{DumpInfo, PartitionSpace};
use qsi::host::host::{Device, Hardware, Host, PowerOn};
use qsi::host::model::Profile;
use qsi::host::panel::FunctionInfo;
use qsi::host::partitions::{Activation, Status};
use qsi::host::profiles::{Entry, ProfileUse};
use qsi::host::vres::{Drive, Image, Port, StoragePool, Switch, TapeDrive, Vdisk, Volume};
use qsi::update::Status as UpdateStatus;

/// The connection to a managed system.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Link {
    /// Not tried yet.
    #[default]
    Never,
    /// Being set up, or waiting to try again.
    Connecting,
    /// A session is open.
    Connected,
    /// It went, and is being tried again.
    Lost,
    /// It answers with a certificate other than the one pinned: not
    /// trusted until it is paired again.
    Changed,
}
wire_enum!(Link {
    0 => Never,
    1 => Connecting,
    2 => Connected,
    3 => Lost,
    4 => Changed,
});

/// A message a managed system issued, or the console's own.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Note {
    /// Its identifier: `QSI0202`, `QVS2307`, `QMC1101`.
    pub msgid: String,
    /// Its first-level text.
    pub text: String,
    /// Its second-level text, when it has one.
    pub help: String,
}
wire_struct!(Note { msgid, text, help });

/// A request refused: the first message, then the others that apply,
/// as the managed system gave them.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Refusal {
    /// The first message.
    pub main: Note,
    /// The others, in order.
    pub also: Vec<Note>,
}
wire_struct!(Refusal { main, also });

impl Refusal {
    /// A refusal of one message.
    pub fn new(msgid: &str, text: &str) -> Self {
        Self {
            main: Note {
                msgid: msgid.into(),
                text: text.into(),
                help: String::new(),
            },
            also: Vec::new(),
        }
    }

    /// Every message, the first first.
    pub fn notes(&self) -> Vec<&Note> {
        std::iter::once(&self.main).chain(&self.also).collect()
    }
}

/// A partition's last activated profile.
#[derive(Clone, Debug, PartialEq)]
pub struct PartProfile {
    /// The partition.
    pub id: u32,
    /// The profile.
    pub profile: Profile,
}
wire_struct!(PartProfile { id, profile });

/// A partition's profiles by name, as its host lists them.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PartProfiles {
    /// The partition.
    pub id: u32,
    /// Its profiles: name, revision, whether the default.
    pub entries: Vec<Entry>,
}
wire_struct!(PartProfiles { id, entries });

/// A profile of a partition, whole, at its revision.
#[derive(Clone, Debug, PartialEq)]
pub struct PartProfileBody {
    /// The partition.
    pub id: u32,
    /// The profile's revision.
    pub revision: u64,
    /// The profile.
    pub profile: Profile,
}
wire_struct!(PartProfileBody {
    id,
    revision,
    profile
});

/// When each profile of a partition was last activated.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PartUses {
    /// The partition.
    pub id: u32,
    /// Its profiles, by name.
    pub uses: Vec<ProfileUse>,
}
wire_struct!(PartUses { id, uses });

/// A running partition's optical and tape devices, and what is loaded.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PartDrives {
    /// The partition.
    pub id: u32,
    /// Its devices.
    pub drives: Vec<Drive>,
}
wire_struct!(PartDrives { id, drives });

/// The control panel functions a system carries out for a partition
/// (ID 0: the system itself), as it lists them.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PartFunctions {
    /// The partition; 0 for the system.
    pub id: u32,
    /// Its functions by number.
    pub functions: Vec<FunctionInfo>,
}
wire_struct!(PartFunctions { id, functions });

/// A host's virtual resources, as its events keep them.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Resources {
    /// Storage pools.
    pub pools: Vec<StoragePool>,
    /// Virtual disks, of every pool.
    pub vdisks: Vec<Vdisk>,
    /// The media library's images.
    pub images: Vec<Image>,
    /// Tape volumes.
    pub volumes: Vec<Volume>,
    /// Virtual switches.
    pub switches: Vec<Switch>,
    /// Partitions' ports on them.
    pub ports: Vec<Port>,
    /// The host's own tape drives (physical I/O).
    pub tape_drives: Vec<TapeDrive>,
    /// The host's PCI devices for partitions (physical I/O).
    pub devices: Vec<Device>,
    /// Running partitions' drives.
    pub drives: Vec<PartDrives>,
}
wire_struct!(Resources {
    pools,
    vdisks,
    images,
    volumes,
    switches,
    ports,
    tape_drives,
    devices,
    drives
});

/// What `mgmtd` keeps about one managed system.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Kept {
    /// Its name, as it said it when paired.
    pub name: String,
    /// The address it is reached at.
    pub address: String,
    /// Its QSI port.
    pub port: u16,
    /// What it is: a host or a standalone system.
    pub kind: Kind,
    /// The SHA-256 of its pinned certificate, in hexadecimal.
    pub fingerprint: String,
    /// When it was paired, Unix seconds.
    pub paired: u64,
    /// Its serial number when paired.
    pub serial: String,
    /// The connection.
    pub link: Link,
    /// Why the connection last failed, in words an operator can act on;
    /// empty while connected.
    pub reason: String,
    /// When it last answered, Unix seconds; 0 never.
    pub last_contact: u64,
    /// The console's address its session leaves from; the pages name
    /// its interface (`private` when configured and the route takes it,
    /// else `open`). Empty while not connected.
    pub via: String,
    /// What it says of itself.
    pub identity: Option<Identity>,
    /// Its state and current code.
    pub state: Option<State>,
    /// A host's resources.
    pub host: Option<Host>,
    /// A host's hardware.
    pub hardware: Option<Hardware>,
    /// A host's partitions.
    pub partitions: Vec<Status>,
    /// Their last activated profiles.
    pub profiles: Vec<PartProfile>,
    /// The capabilities the two agreed on.
    pub caps: u64,
    /// A serial number its hello said that is not the one paired;
    /// empty when the same.
    pub said_serial: String,
    /// The last event seen.
    pub last_seq: u64,
    /// The console's own note about it.
    pub description: String,
    /// A host's virtual resources.
    pub resources: Resources,
    /// Each partition's profiles by name.
    pub profile_lists: Vec<PartProfiles>,
    /// When each partition's activation began (activation times).
    pub activations: Vec<Activation>,
    /// A host's power-on settings.
    pub power_on: Option<PowerOn>,
    /// The control panel functions, by partition (0: the system).
    pub functions: Vec<PartFunctions>,
    /// Every profile of every partition, whole.
    pub profile_bodies: Vec<PartProfileBody>,
    /// When each partition's profiles were last activated.
    pub profile_uses: Vec<PartUses>,
    /// The system's own dumps (base's bit 4), oldest first.
    pub dumps: Vec<SystemDump>,
    /// The space they take (bit 26).
    pub dump_space: Option<Space>,
    /// Each partition's dumps (the dumps family).
    pub part_dumps: Vec<PartDumps>,
    /// The space each partition's dumps take (bit 26).
    pub part_space: Vec<PartitionSpace>,
    /// A host's code slots and what is staged (the update family).
    pub update: Option<UpdateStatus>,
    /// What its update did, as its events said, oldest first.
    pub update_log: Vec<UpdateLine>,
    /// How its last activation ended, after the restart.
    pub update_end: Option<UpdateEnd>,
    /// An image being brought to it by the console.
    pub staging: Option<Staging>,
    /// Its cumulative fix level (bit 27).
    pub fix_level: Option<Level>,
}
wire_struct!(Kept {
    name,
    address,
    port,
    kind,
    fingerprint,
    paired,
    serial,
    link,
    reason,
    last_contact,
    via,
    identity,
    state,
    host,
    hardware,
    partitions,
    profiles,
    caps,
    said_serial,
    last_seq,
    description,
    resources,
    profile_lists,
    activations,
    power_on,
    functions,
    profile_bodies,
    profile_uses,
    dumps,
    dump_space,
    part_dumps,
    part_space,
    update,
    update_log,
    update_end,
    staging,
    fix_level
});

/// A partition's dumps.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PartDumps {
    /// The partition.
    pub id: u32,
    /// Its dumps, oldest first.
    pub dumps: Vec<DumpInfo>,
}
wire_struct!(PartDumps { id, dumps });

/// A line of what an update did: the console's words of an event.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct UpdateLine {
    /// When, Unix seconds.
    pub time: u64,
    /// The step it is of: `stage`, `verify`, `install`, `activate`,
    /// `restart`, `confirm`.
    pub step: String,
    /// What happened.
    pub text: String,
    /// The step ended badly.
    pub failed: bool,
}
wire_struct!(UpdateLine {
    time,
    step,
    text,
    failed
});

/// How a host's last activation ended, after its restart.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct UpdateEnd {
    /// The slot it was of.
    pub slot: String,
    /// It confirmed; it rolled back when not.
    pub confirmed: bool,
    /// The release that runs after it.
    pub release: String,
    /// Its build.
    pub build: String,
    /// Why it rolled back, the host's words.
    pub reason: String,
    /// The reference code that says why, where there is one.
    pub code: String,
    /// When, Unix seconds.
    pub time: u64,
}
wire_struct!(UpdateEnd {
    slot,
    confirmed,
    release,
    build,
    reason,
    code,
    time
});

/// An image the console brings to a host.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Staging {
    /// Its name.
    pub name: String,
    /// Its size.
    pub bytes: u64,
    /// What has gone to the host.
    pub sent: u64,
    /// Who asked.
    pub user: String,
    /// When it began, Unix seconds.
    pub began: u64,
    /// All of it went.
    pub done: bool,
    /// Why it stopped, when it did.
    pub error: Option<Note>,
}
wire_struct!(Staging {
    name,
    bytes,
    sent,
    user,
    began,
    done,
    error
});

/// What a managed system is.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Kind {
    /// A QS/VS host.
    #[default]
    Host,
    /// A standalone QS/400 system.
    Standalone,
}
wire_enum!(Kind {
    0 => Host,
    1 => Standalone,
});

impl Kind {
    /// What a hello's kind is.
    pub fn of(k: PeerKind) -> Self {
        if k == PeerKind::Host {
            Self::Host
        } else {
            Self::Standalone
        }
    }

    /// As the pages name it.
    pub fn words(self) -> &'static str {
        match self {
            Self::Host => "QS/VS host",
            Self::Standalone => "Standalone QS/400",
        }
    }
}

/// The code a system shows when it has posted none.
pub const NO_CODE: &str = "00000000";

impl Kept {
    /// The state as the pages name it: `Operating`, `Power off`,
    /// `Standby` (starting or ending), `Error` (halted), or `No
    /// connection`.
    pub fn status(&self) -> &'static str {
        if self.link != Link::Connected {
            return "No connection";
        }
        match self.state.as_ref().map(|s| s.power) {
            Some(PowerState::Running) => "Operating",
            Some(PowerState::Off) => "Power off",
            Some(PowerState::Halted) => "Error",
            Some(PowerState::Starting | PowerState::Ending) => "Standby",
            _ => "No connection",
        }
    }

    /// Its current code, never empty.
    pub fn code(&self) -> String {
        self.state
            .as_ref()
            .and_then(|s| s.refcode.as_ref())
            .map_or_else(|| NO_CODE.into(), |r| r.code.clone())
    }

    /// Its current code whole, when it posted one.
    pub fn refcode(&self) -> Option<&Refcode> {
        self.state.as_ref().and_then(|s| s.refcode.as_ref())
    }

    /// Its type and model.
    pub fn model(&self) -> &str {
        self.identity.as_ref().map_or("", |i| i.model.as_str())
    }

    /// Its release.
    pub fn release(&self) -> &str {
        self.identity.as_ref().map_or("", |i| i.release.as_str())
    }

    /// The partitions it shows: a host's defined, one for a standalone
    /// system.
    pub fn partition_count(&self) -> usize {
        match self.kind {
            Kind::Host => self.partitions.len(),
            Kind::Standalone => 1,
        }
    }

    /// Its partitions running.
    pub fn running(&self) -> usize {
        self.partitions
            .iter()
            .filter(|p| p.state == qsi::host::model::PartitionState::Running)
            .count()
    }

    /// Partition `id` as the pages show it: a host's, or a standalone
    /// system itself at ID 0, whose panel and console are there.
    pub fn part(&self, id: u32) -> Option<Status> {
        if self.kind == Kind::Standalone {
            return (id == qsi::host::model::SYSTEM_ID).then(|| self.itself());
        }
        self.partitions.iter().find(|p| p.id == id).cloned()
    }

    /// A standalone system as one partition's row: its name, its state
    /// as a partition's, its current code and its release.
    pub fn itself(&self) -> Status {
        use qsi::host::model::PartitionState as S;
        let state = match self.state.as_ref().map(|s| s.power) {
            Some(PowerState::Running) => S::Running,
            Some(PowerState::Starting) => S::Starting,
            Some(PowerState::Ending) => S::ShuttingDown,
            Some(PowerState::Halted) => S::Error,
            _ => S::NotActivated,
        };
        Status {
            id: qsi::host::model::SYSTEM_ID,
            name: self.name.clone(),
            state,
            code: self.code(),
            text: self.refcode().map(|r| r.text.clone()).unwrap_or_default(),
            current: None,
            profile: String::new(),
            last_end: String::new(),
            message: None,
            ethernet: Vec::new(),
            channel: qsi::host::model::Channel {
                up: self.link == Link::Connected,
                system: self.name.clone(),
                release: self.release().into(),
                ..Default::default()
            },
        }
    }

    /// The control panel functions it lists for partition `id` (0 for
    /// a standalone system); empty when its hello does not offer them.
    pub fn functions(&self, id: u32) -> &[FunctionInfo] {
        self.functions
            .iter()
            .find(|f| f.id == id)
            .map_or(&[], |f| f.functions.as_slice())
    }

    /// A partition's profiles by name, as kept.
    pub fn profile_list(&self, id: u32) -> &[Entry] {
        self.profile_lists
            .iter()
            .find(|p| p.id == id)
            .map_or(&[], |p| p.entries.as_slice())
    }

    /// A partition's profile `name`, whole, as kept.
    pub fn profile_body(&self, id: u32, name: &str) -> Option<&PartProfileBody> {
        self.profile_bodies
            .iter()
            .find(|b| b.id == id && b.profile.name == name)
    }

    /// When a partition's profile `name` was last activated; `None`
    /// for never or not said.
    pub fn last_used(&self, id: u32, name: &str) -> Option<u64> {
        self.profile_uses
            .iter()
            .find(|u| u.id == id)
            .and_then(|u| u.uses.iter().find(|x| x.name == name))
            .and_then(|x| x.last)
    }

    /// Whether its hello agreed every bit of `bits`.
    pub fn offers(&self, bits: u64) -> bool {
        self.caps & bits == bits
    }

    /// A partition's last activated profile.
    pub fn profile(&self, id: u32) -> Option<&Profile> {
        self.profiles
            .iter()
            .find(|p| p.id == id)
            .map(|p| &p.profile)
    }

    /// The profile whose devices a partition has: its last activated,
    /// or its default as kept when it was never activated.
    pub fn devices(&self, id: u32) -> Option<&Profile> {
        self.profile(id).or_else(|| {
            let default = self.profile_list(id).iter().find(|e| e.default)?;
            self.profile_body(id, &default.name).map(|b| &b.profile)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_kept_system_goes_and_comes_back_whole() {
        let k = Kept {
            name: "QSHQVS1".into(),
            port: 2300,
            link: Link::Changed,
            reason: "its certificate changed".into(),
            ..Kept::default()
        };
        let back: Kept = ipc::decode(&ipc::encode(&k)).unwrap();
        assert_eq!(back, k);
        assert_eq!(k.status(), "No connection");
        assert_eq!(k.code(), NO_CODE);
    }
}
