// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! A deployment as the scheduler keeps it, one file a host under
//! `/data/plans/deploy/`: its latest job, every step with its state and
//! times, and what deployments have made there. The scheduler writes it
//! before and after each step, so the progress page reads the same
//! after a restart of the web server, and a restart of the console
//! finds the step that was running and marks it interrupted.

use serde::{Deserialize, Serialize};

use crate::deploy::Action;

/// How a job stands.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum JobState {
    /// Its steps are being run.
    #[default]
    Running,
    /// Every step ran.
    Done,
    /// A step failed: the job stopped before the next.
    Failed,
    /// Cancelled: it stopped before its next step.
    Cancelled,
    /// The console restarted while it ran: it stopped.
    Stopped,
}

impl JobState {
    /// Whether it ended.
    pub fn ended(self) -> bool {
        self != Self::Running
    }
}

/// How a step stands.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StepState {
    /// Not begun.
    #[default]
    Pending,
    /// Under way.
    Going,
    /// Done.
    Complete,
    /// Not done: its item is in Error.
    Skipped,
    /// The host refused it.
    Failed,
    /// The console restarted while it ran.
    Interrupted,
}

/// One step.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Step {
    /// What it is.
    pub what: String,
    /// What it asks; none when skipped.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub action: Option<Action>,
    /// How it stands.
    #[serde(default)]
    pub state: StepState,
    /// When it began, Unix seconds; 0 not yet.
    #[serde(default)]
    pub started: u64,
    /// When it ended.
    #[serde(default)]
    pub ended: u64,
    /// What it said: the host's message, why it was skipped, how far.
    #[serde(default)]
    pub message: String,
}

/// A deployment of a plan to a host.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Job {
    /// The plan.
    pub plan: String,
    /// The host.
    pub target: String,
    /// The user it runs for.
    pub user: String,
    /// When it began.
    pub began: u64,
    /// When it ended; 0 while it runs.
    #[serde(default)]
    pub ended: u64,
    /// How it stands.
    #[serde(default)]
    pub state: JobState,
    /// Cancel was asked: it stops before its next step.
    #[serde(default)]
    pub cancel: bool,
    /// The partitions chosen.
    #[serde(default)]
    pub partitions: Vec<u32>,
    /// They are activated after.
    #[serde(default)]
    pub activate: bool,
    /// Its steps.
    #[serde(default, rename = "step")]
    pub steps: Vec<Step>,
}

/// A host's deployments: the latest job and what they made.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Record {
    /// Partitions deployments made here, `ID NAME`: one a later
    /// deployment may complete.
    #[serde(default)]
    pub made: Vec<String>,
    /// The latest job.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub job: Option<Job>,
}

impl state::Document for Record {
    const FILE: &'static str = "deploy.toml";
    const VERSION: u32 = 1;
    const MIGRATIONS: &'static [state::Migration] = &[];

    fn validate(&self) -> Result<(), String> {
        Ok(())
    }
}

impl Job {
    /// What it made: the steps that completed.
    pub fn made(&self) -> Vec<&str> {
        self.steps
            .iter()
            .filter(|s| s.state == StepState::Complete)
            .map(|s| s.what.as_str())
            .collect()
    }

    /// The step under way.
    pub fn going(&self) -> Option<&Step> {
        self.steps.iter().find(|s| s.state == StepState::Going)
    }
}

/// A job's text, as the scheduler's socket carries it.
pub fn write(j: &Job) -> String {
    let r = Record {
        made: Vec::new(),
        job: Some(j.clone()),
    };
    state::store::render(&r).unwrap_or_default()
}

/// A job from its text.
pub fn read(text: &str) -> Option<Job> {
    state::store::parse::<Record>(text).ok().and_then(|r| r.job)
}
