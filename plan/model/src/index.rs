// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! The plans' index, `/data/plans/plans.toml`: a row a plan, who made
//! it, when, from which managed system, its description, its size; and
//! how many automatic plans are kept of each host.

use ipc::wire_struct;
use serde::{Deserialize, Serialize};

use crate::doc::Plan;

/// How many automatic plans are kept of a host when nothing says.
pub const AUTOMATIC_KEPT: u32 = 10;

/// A plan's row.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Entry {
    /// Its name.
    pub name: String,
    /// Its description.
    #[serde(default)]
    pub description: String,
    /// When it was made, Unix seconds.
    pub created: u64,
    /// Who made it.
    pub by: String,
    /// Its managed system; empty for a template.
    #[serde(default)]
    pub system: String,
    /// Saved before a code update.
    #[serde(default)]
    pub automatic: bool,
    /// Its partitions.
    pub partitions: u32,
    /// Its file's size.
    pub bytes: u64,
}
wire_struct!(Entry {
    name,
    description,
    created,
    by,
    system,
    automatic,
    partitions,
    bytes
});

impl Entry {
    /// The row of `plan`, whose text is `bytes` long.
    pub fn of(plan: &Plan, bytes: usize) -> Self {
        Self {
            name: plan.name.clone(),
            description: plan.description.clone(),
            created: plan.created,
            by: plan.by.clone(),
            system: plan.system.clone(),
            automatic: plan.automatic,
            partitions: u32::try_from(plan.partitions.len()).unwrap_or(u32::MAX),
            bytes: bytes as u64,
        }
    }
}

/// `plans.toml`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Index {
    /// How many automatic plans are kept of each host, the oldest
    /// removed first; an operator's are never removed for them.
    #[serde(default = "kept")]
    pub automatic_kept: u32,
    /// Every plan, by name.
    #[serde(default, rename = "plan")]
    pub plans: Vec<Entry>,
}

fn kept() -> u32 {
    AUTOMATIC_KEPT
}

impl Default for Index {
    fn default() -> Self {
        Self {
            automatic_kept: AUTOMATIC_KEPT,
            plans: Vec::new(),
        }
    }
}

impl state::Document for Index {
    const FILE: &'static str = "plans.toml";
    const VERSION: u32 = 1;
    const MIGRATIONS: &'static [state::Migration] = &[];

    fn validate(&self) -> Result<(), String> {
        if !(1..=1000).contains(&self.automatic_kept) {
            return Err("automatic_kept is 1 to 1000".into());
        }
        for e in &self.plans {
            if !crate::doc::check::plan_name_ok(&e.name) {
                return Err(format!("{}: not a plan's name", e.name));
            }
        }
        Ok(())
    }
}
