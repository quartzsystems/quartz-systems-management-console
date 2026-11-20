// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! What `mgmtd` keeps about a managed system: how it was paired, its
//! connection, and what it last said of itself, as QSI's own types. A
//! list page reads only this; it never waits on a managed system.

use ipc::{wire_enum, wire_struct};
use qsi::PeerKind;
use qsi::base::{Identity, PowerState, Refcode, State};
use qsi::host::host::{Hardware, Host};
use qsi::host::model::Profile;
use qsi::host::partitions::Status;

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
    last_seq
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

    /// A partition's last activated profile.
    pub fn profile(&self, id: u32) -> Option<&Profile> {
        self.profiles
            .iter()
            .find(|p| p.id == id)
            .map(|p| &p.profile)
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
