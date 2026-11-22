// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! `fetchd`, QS/MC's download service (`docs/UPDATES.md`): the only
//! process of the console that connects to the update server. It runs as
//! a user of its own and holds no key of the console's. It checks the
//! three product streams on request (or by its settings, off until an
//! operator turns them on), keeps what verifies against the keys built
//! into the image, and downloads what they name within a limit, checking
//! each file's size and SHA-256 against the index before anything else
//! may read it.
//!
//! - [`settings`]: `updates.toml`, its settings and the sequences seen.
//! - [`streams`]: the three streams, their paths and keys.
//! - [`verify`]: an index, a catalog and a file, read as hostile.
//! - [`service`]: checks, downloads, the automatic check.
//! - [`net`]: the update server over `httpc`.
//! - [`proto`]: its socket.

#![forbid(unsafe_code)]

pub mod net;
pub mod proto;
pub mod service;
pub mod settings;
pub mod streams;
pub mod verify;

pub use service::{Config, Get, Service};
pub use streams::Stream;

/// The folder under `/data` that is the service's own: `updates.toml`.
pub const BACKUP_DIR: &str = "fetch";
