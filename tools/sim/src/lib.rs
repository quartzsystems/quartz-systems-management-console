// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! A managed system for tests (`docs/SIM.md`): a QS/VS host or a
//! standalone system on QS/400's `qsisess` (its managed side) and
//! `qsitrust`, speaking real QSI on a real socket. It holds the
//! prototype's fixture data, runs partitions through reference code
//! sequences, serves a console from a recorded screen, and can be told
//! to drop, stall, refuse and send nonsense. A host tool, in no image.
//!
//! - [`fixture`]: the prototype's systems and the recorded screen.
//! - [`system`]: a managed system's state and what it answers.
//! - [`net`]: the listener and each connection's session.
//! - [`control`]: what a harness or a person tells it.

#![forbid(unsafe_code)]

pub mod control;
pub mod fixture;
pub mod net;
pub mod system;

pub use system::{Shared, System, lock};
