// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! The values QSI carries as open enumerations, spelled in the
//! document. A value QSI has and this release does not know is refused
//! when a plan is captured ([`Unknown`]), never written as a number.

use serde::{Deserialize, Serialize};

use qsi::base::Keylock as QKeylock;
use qsi::host::model::{
    ConsoleMode, IplSource as QSource, IplType, ProcessorMode, Sharing as QSharing,
};
use qsi::host::partition::WithHost as QWithHost;
use qsi::host::vres::{Bond, Protection as QProtection};

/// A value of `what` this release has no word for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Unknown(pub String);

impl std::fmt::Display for Unknown {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "the host said a {} this release does not know", self.0)
    }
}

macro_rules! words {
    ($(#[$m:meta])* $name:ident, $q:ty, $what:literal { $($v:ident = $word:literal => $qv:path),* $(,)? }) => {
        $(#[$m])*
        #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
        pub enum $name {
            $(
                #[doc = $word]
                #[serde(rename = $word)]
                $v,
            )*
            /// Not a value: the type's default while a document is made.
            #[default]
            #[serde(skip)]
            Unset,
        }

        impl $name {
            /// As the document spells it.
            pub fn word(self) -> &'static str {
                match self {
                    $(Self::$v => $word,)*
                    Self::Unset => "",
                }
            }

            /// QSI's value; `None` for [`Self::Unset`].
            pub fn qsi(self) -> Option<$q> {
                match self {
                    $(Self::$v => Some($qv),)*
                    Self::Unset => None,
                }
            }

            /// The document's word of QSI's value.
            pub fn of(q: $q) -> Result<Self, Unknown> {
                $(if q == $qv { return Ok(Self::$v); })*
                Err(Unknown($what.into()))
            }
        }
    };
}

words!(
    /// A profile's processors.
    Mode, ProcessorMode, "processor mode" {
        Dedicated = "dedicated" => ProcessorMode::Dedicated,
        Shared = "shared" => ProcessorMode::Shared,
    }
);

words!(
    /// Shared processors' sharing.
    Sharing, QSharing, "sharing mode" {
        Capped = "capped" => QSharing::Capped,
        Uncapped = "uncapped" => QSharing::Uncapped,
    }
);

words!(
    /// The IPL source.
    IplSource, QSource, "IPL source" {
        A = "A" => QSource::A,
        B = "B" => QSource::B,
        Recorded = "recorded" => QSource::Recorded,
        D = "D" => QSource::D,
    }
);

words!(
    /// The IPL type.
    IplKind, IplType, "IPL type" {
        Unattended = "unattended" => IplType::Unattended,
        Attended = "attended" => IplType::Attended,
    }
);

words!(
    /// A profile's console.
    Console, ConsoleMode, "console device" {
        Serial = "serial" => ConsoleMode::Serial,
        Local = "local" => ConsoleMode::Local,
        Partition = "partition" => ConsoleMode::Partition,
    }
);

words!(
    /// A partition's keylock.
    Keylock, QKeylock, "keylock position" {
        Normal = "normal" => QKeylock::Normal,
        Manual = "manual" => QKeylock::Manual,
        Auto = "auto" => QKeylock::Auto,
        Secure = "secure" => QKeylock::Secure,
    }
);

words!(
    /// How a partition is shut down with its host.
    WithHost, QWithHost, "shut down with the host" {
        Delayed = "delayed" => QWithHost::Delayed,
        Immediate = "immediate" => QWithHost::Immediate,
    }
);

words!(
    /// How a switch's uplink interfaces are bonded.
    BondMode, Bond, "bond mode" {
        ActiveBackup = "active-backup" => Bond::ActiveBackup,
        Lacp = "lacp" => Bond::Lacp,
    }
);

words!(
    /// A storage pool's protection.
    Protection, QProtection, "pool protection" {
        None = "none" => QProtection::None,
        Mirrored = "mirrored" => QProtection::Mirrored,
        Raid5 = "raid5" => QProtection::Raid5,
        Raid6 = "raid6" => QProtection::Raid6,
    }
);

impl Protection {
    /// A host's words for a pool's protection (`raid6`, `none`).
    pub fn of_words(w: &str) -> Result<Self, Unknown> {
        let w = w.to_ascii_lowercase();
        [Self::None, Self::Mirrored, Self::Raid5, Self::Raid6]
            .into_iter()
            .find(|p| p.word() == w)
            .ok_or_else(|| Unknown("pool protection".into()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_word_goes_to_qsi_and_back() {
        assert_eq!(Mode::of(ProcessorMode::Shared), Ok(Mode::Shared));
        assert_eq!(Mode::Shared.qsi(), Some(ProcessorMode::Shared));
        assert_eq!(Keylock::of(QKeylock::Secure).unwrap().word(), "secure");
        assert!(Keylock::of(QKeylock::Other(9)).is_err());
        assert_eq!(Protection::of_words("RAID6"), Ok(Protection::Raid6));
        assert!(Protection::of_words("raid10").is_err());
        assert_eq!(Mode::Unset.qsi(), None);
    }
}
