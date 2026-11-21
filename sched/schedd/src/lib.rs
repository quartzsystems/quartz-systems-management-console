// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! Scheduled operations (`docs/SCHED.md`): a service of its own,
//! `lic/qsmcschedd`, supervised and with no capability, that runs each
//! operation at its time in its owner's name by asking `mgmtd`, as a
//! page does, its owner's role checked again when it runs. A run missed
//! while the console was down is recorded missed and not run late.
//!
//! - [`model`]: what is kept, `schedules.toml`.
//! - [`when`]: the next time an operation is due.
//! - [`service`]: the runs.
//! - [`calls`]: the web server's requests on its socket.

#![forbid(unsafe_code)]

pub mod calls;
pub mod model;
pub mod service;
pub mod when;

pub use calls::{Calls, Request, Sched};
pub use model::{Op, Outcome, Repeat, Run, Schedule, Schedules};
pub use service::{Config, Service};
