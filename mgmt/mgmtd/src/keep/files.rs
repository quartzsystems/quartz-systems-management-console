// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! The two state files `mgmtd` writes, through `base/state`:
//! `systems.toml`, the managed systems as paired, and `mgmt.toml`, the
//! service's limits. Only `mgmtd` writes them; both are in a backup.

use serde::{Deserialize, Serialize};

/// One managed system as paired.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Paired {
    /// Its name, as it said it when paired.
    pub name: String,
    /// The address it is reached at.
    pub address: String,
    /// Its QSI port.
    pub port: u16,
    /// `host` or `standalone`.
    pub kind: String,
    /// The SHA-256 of its certificate, hexadecimal.
    pub fingerprint: String,
    /// When it was paired, Unix seconds.
    pub paired: u64,
    /// Its serial number when paired.
    pub serial: String,
    /// The last event seen, so a session resumes from it.
    #[serde(default)]
    pub last_seq: u64,
    /// The console's own note about it, from its General page.
    #[serde(default)]
    pub description: String,
}

/// `systems.toml`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Systems {
    /// Every managed system, in the order added.
    #[serde(default)]
    pub systems: Vec<Paired>,
}

impl state::Document for Systems {
    const FILE: &'static str = "systems.toml";
    const VERSION: u32 = 1;
    const MIGRATIONS: &'static [state::Migration] = &[];

    fn validate(&self) -> Result<(), String> {
        for (i, s) in self.systems.iter().enumerate() {
            if s.name.is_empty() || s.address.is_empty() || s.port == 0 {
                return Err(format!("system {} lacks a name, address or port", i + 1));
            }
            if qsitrust::Fingerprint::from_hex(&s.fingerprint).is_none() {
                return Err(format!("{}: the fingerprint is not one", s.name));
            }
            if self.systems[..i].iter().any(|o| o.name == s.name) {
                return Err(format!("{} twice", s.name));
            }
        }
        Ok(())
    }
}

fn systems() -> u32 {
    256
}
fn setups() -> u32 {
    8
}
fn consoles() -> u32 {
    32
}
fn history() -> u32 {
    4096
}
fn unreached() -> u32 {
    30
}

/// `mgmt.toml`: the service's limits.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Limits {
    /// The most managed systems.
    #[serde(default = "systems")]
    pub max_systems: u32,
    /// The most connections being set up at once.
    #[serde(default = "setups")]
    pub max_setups: u32,
    /// The most terminal windows open at once, across every system.
    #[serde(default = "consoles")]
    pub max_consoles: u32,
    /// The reference codes kept for each system.
    #[serde(default = "history")]
    pub history: u32,
    /// Minutes a system may go unreached before the console opens a
    /// problem of its own for it (`A3005030`).
    #[serde(default = "unreached")]
    pub unreached_minutes: u32,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_systems: systems(),
            max_setups: setups(),
            max_consoles: consoles(),
            history: history(),
            unreached_minutes: unreached(),
        }
    }
}

impl state::Document for Limits {
    const FILE: &'static str = "mgmt.toml";
    const VERSION: u32 = 1;
    const MIGRATIONS: &'static [state::Migration] = &[];

    fn validate(&self) -> Result<(), String> {
        if !(1..=4096).contains(&self.max_systems) {
            return Err("max_systems is 1 to 4096".into());
        }
        if !(1..=64).contains(&self.max_setups) {
            return Err("max_setups is 1 to 64".into());
        }
        if !(1..=256).contains(&self.max_consoles) {
            return Err("max_consoles is 1 to 256".into());
        }
        if !(64..=65_536).contains(&self.history) {
            return Err("history is 64 to 65536".into());
        }
        if !(1..=10_080).contains(&self.unreached_minutes) {
            return Err("unreached_minutes is 1 to 10080".into());
        }
        Ok(())
    }
}
