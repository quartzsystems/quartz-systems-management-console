// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! What can go wrong in a run, with the serial log where there is one.

use std::time::Duration;

/// A failed run.
#[derive(Debug, thiserror::Error)]
pub enum HarnessError {
    /// QEMU or OVMF could not be found, or QEMU did not start.
    #[error(transparent)]
    Driver(#[from] driver::Error),
    /// An image the step needs is not built.
    #[error("{what} is not built; run `cargo run -p build -- all` first")]
    NotBuilt {
        /// Which image.
        what: &'static str,
    },
    /// A step could not be set up.
    #[error("{step}: {reason}")]
    Setup {
        /// The step.
        step: &'static str,
        /// Why.
        reason: String,
    },
    /// What was expected was not seen in time.
    #[error("{step}: `{missing}` not seen within {}s{}", .after.as_secs(), if *.exited { " (QEMU exited)" } else { "" })]
    Timeout {
        /// The step.
        step: &'static str,
        /// The first line not seen.
        missing: String,
        /// How long it waited.
        after: Duration,
        /// Whether QEMU ended first.
        exited: bool,
        /// The serial log.
        log: String,
    },
    /// A suite of the base system did not pass.
    #[error("{suite}: {reason}")]
    Suite {
        /// The suite.
        suite: &'static str,
        /// Why.
        reason: String,
        /// The serial logs of its guests.
        log: String,
    },
    /// No step or suite has the name asked for.
    #[error("no step or suite is called `{0}`; `--suite list` lists them")]
    NoSuchSuite(String),
    /// A file could not be read or written.
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

impl HarnessError {
    /// The serial log, when the failure has one.
    pub fn log(&self) -> Option<&str> {
        match self {
            HarnessError::Timeout { log, .. } | HarnessError::Suite { log, .. } => Some(log),
            _ => None,
        }
    }
}
