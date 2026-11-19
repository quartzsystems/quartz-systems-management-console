// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! Backup and restore (`console-backup.html`, `docs/BACKUP.md`): the
//! history of backups made, kept in `backups.toml`.

use appliance::time::Zone;
use serde::{Deserialize, Serialize};

use crate::app::App;

/// One backup made.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Made {
    /// When, Unix seconds.
    pub at: u64,
    /// Where it went: `Download`.
    pub destination: String,
    /// Its size in bytes.
    pub size: u64,
    /// `Completed` or why not.
    pub status: String,
    /// Who made it.
    pub user: String,
}

/// `backups.toml`: the backups made, the newest last.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct History {
    /// The backups.
    #[serde(default, rename = "backup")]
    pub made: Vec<Made>,
}

/// How many backups the history keeps.
pub const KEPT: usize = 20;

impl state::Document for History {
    const FILE: &'static str = "backups.toml";
    const VERSION: u32 = 1;
    const MIGRATIONS: &'static [state::Migration] = &[];

    fn validate(&self) -> Result<(), String> {
        if self.made.len() > KEPT {
            return Err(format!("more than {KEPT} backups in the history"));
        }
        Ok(())
    }
}

/// The history.
pub fn history(app: &App) -> History {
    app.store()
        .load::<History>()
        .ok()
        .flatten()
        .unwrap_or_default()
}

/// The landing page's line: the last backup, or none.
pub fn last(app: &App, zone: Zone, date_format: &str) -> String {
    history(app).made.last().map_or_else(
        || "No backup made yet".to_string(),
        |m| {
            let at = i64::try_from(m.at).unwrap_or(0);
            format!(
                "Last backup {} {} to {}",
                crate::fmt::date(at, zone, date_format),
                crate::fmt::time(at, zone, "24-hour", false),
                m.destination.to_lowercase()
            )
        },
    )
}
