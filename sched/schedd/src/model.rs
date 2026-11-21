// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! What is kept: each scheduled operation with its owner, what it does
//! to which system or partition, when and how often; the runs, newest
//! last, bounded. One file, `schedules.toml`, through `base/state`.

use ipc::{wire_enum, wire_struct};
use serde::{Deserialize, Serialize};

/// What an operation does.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Op {
    /// Activate a partition with a profile.
    #[default]
    Activate,
    /// Shut a partition down, delayed.
    ShutDownDelayed,
    /// Shut a partition down at once.
    ShutDownImmediate,
    /// Restart a partition.
    Restart,
    /// Power a managed system off.
    PowerOff,
    /// Power a managed system on: not done by this console.
    PowerOn,
    /// Back up the console's data: done from a browser only.
    Backup,
    /// Install a code update: a later phase's.
    Update,
}
wire_enum!(Op {
    0 => Activate,
    1 => ShutDownDelayed,
    2 => ShutDownImmediate,
    3 => Restart,
    4 => PowerOff,
    5 => PowerOn,
    6 => Backup,
    7 => Update,
});

impl Op {
    /// Every operation, in the form's order.
    pub const ALL: [Op; 8] = [
        Op::Activate,
        Op::ShutDownDelayed,
        Op::ShutDownImmediate,
        Op::Restart,
        Op::PowerOn,
        Op::PowerOff,
        Op::Backup,
        Op::Update,
    ];

    /// As the form names it.
    pub fn words(self) -> &'static str {
        match self {
            Op::Activate => "Activate partition",
            Op::ShutDownDelayed => "Shut down partition (delayed)",
            Op::ShutDownImmediate => "Shut down partition (immediate)",
            Op::Restart => "Restart partition",
            Op::PowerOn => "Power on managed system",
            Op::PowerOff => "Power off managed system",
            Op::Backup => "Backup console data",
            Op::Update => "Install code update",
        }
    }

    /// The operation the form named `words`.
    pub fn of(words: &str) -> Option<Op> {
        Op::ALL.into_iter().find(|o| o.words() == words)
    }

    /// Whether it acts on a partition, not a whole system.
    pub fn on_partition(self) -> bool {
        matches!(
            self,
            Op::Activate | Op::ShutDownDelayed | Op::ShutDownImmediate | Op::Restart
        )
    }
}

/// How often.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Repeat {
    /// `once`, `daily`, `weekly`, `monthly`.
    pub every: String,
    /// The weekdays of a weekly one, 0 for Sunday.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub days: Vec<u8>,
    /// The day of the month of a monthly one.
    #[serde(default)]
    pub day: u8,
}
wire_struct!(Repeat { every, days, day });

/// A scheduled operation.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Schedule {
    /// Its number.
    pub id: u32,
    /// The user it runs as: their role is checked when it runs.
    pub owner: String,
    /// What it does.
    pub op: Op,
    /// The managed system.
    pub system: String,
    /// The partition; 0 for the system itself.
    #[serde(default)]
    pub partition: u32,
    /// The partition's name when it was scheduled.
    #[serde(default)]
    pub partition_name: String,
    /// An activation's profile; empty for the default.
    #[serde(default)]
    pub profile: String,
    /// A delayed shut down's or power off's delay, seconds.
    #[serde(default)]
    pub delay: u32,
    /// The first time it is due, Unix seconds.
    pub first: i64,
    /// How often.
    pub repeat: Repeat,
    /// After this it is not due again, Unix seconds; 0 for no end.
    #[serde(default)]
    pub until: i64,
    /// The next time it is due; 0 for never again.
    #[serde(default)]
    pub next: i64,
}
wire_struct!(Schedule {
    id,
    owner,
    op,
    system,
    partition,
    partition_name,
    profile,
    delay,
    first,
    repeat,
    until,
    next
});

/// How a run ended.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Outcome {
    /// The managed system took it.
    #[default]
    Completed,
    /// The managed system, or the console, refused it.
    Failed,
    /// Its time passed while the console was down: not run late.
    Missed,
    /// Its owner may not do it now.
    Refused,
}
wire_enum!(Outcome {
    0 => Completed,
    1 => Failed,
    2 => Missed,
    3 => Refused,
});

/// One run.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Run {
    /// The schedule.
    pub id: u32,
    /// The time it was due, Unix seconds.
    pub due: i64,
    /// When it was run or found missed.
    pub at: i64,
    /// How it ended.
    pub outcome: Outcome,
    /// The message that says why, when it failed: its identifier.
    #[serde(default)]
    pub msgid: String,
    /// Its text.
    #[serde(default)]
    pub text: String,
    /// The user it ran as.
    pub owner: String,
    /// It was asked to run now, from the list.
    #[serde(default)]
    pub now: bool,
}
wire_struct!(Run {
    id,
    due,
    at,
    outcome,
    msgid,
    text,
    owner,
    now
});

/// The most runs kept.
pub const RUNS: usize = 200;

/// `schedules.toml`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Schedules {
    /// The number the next one takes.
    #[serde(default)]
    pub next_id: u32,
    /// The operations.
    #[serde(default, rename = "schedule")]
    pub schedules: Vec<Schedule>,
    /// The runs, oldest first.
    #[serde(default, rename = "run")]
    pub runs: Vec<Run>,
}
wire_struct!(Schedules {
    next_id,
    schedules,
    runs
});

impl state::Document for Schedules {
    const FILE: &'static str = "schedules.toml";
    const VERSION: u32 = 1;
    const MIGRATIONS: &'static [state::Migration] = &[];

    fn validate(&self) -> Result<(), String> {
        if self.runs.len() > RUNS {
            return Err("more runs are kept than the most".into());
        }
        for s in &self.schedules {
            if !matches!(
                s.repeat.every.as_str(),
                "once" | "daily" | "weekly" | "monthly"
            ) {
                return Err(format!("schedule {}: repeat {}", s.id, s.repeat.every));
            }
        }
        Ok(())
    }
}

impl Schedules {
    /// Keep `run`, the oldest let go past the most.
    pub fn record(&mut self, run: Run) {
        self.runs.push(run);
        let over = self.runs.len().saturating_sub(RUNS);
        self.runs.drain(..over);
    }
}
