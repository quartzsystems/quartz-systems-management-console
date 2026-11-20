// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! A QS/MC backup (`docs/BACKUP.md`).
//!
//! - [`payload`]: what a backup holds, a manifest and the files of
//!   `/data/state` and `/data/web`, as plain bytes. PID 1 reads it when
//!   it applies a restore.
//! - [`seal`] (feature `seal`): the file, the payload encrypted under a
//!   passphrase: Argon2id for the key, AES-256-GCM over the payload.

#![forbid(unsafe_code)]

pub mod payload;
#[cfg(feature = "seal")]
pub mod seal;

pub use payload::{Backup, Entry, Manifest};

/// Why a backup was not read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    /// The bytes are not a backup file, or not one this release reads.
    NotBackup(String),
    /// The passphrase does not open it, or it was changed.
    Passphrase,
    /// It opened, but what it holds is not well formed.
    Damaged(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::NotBackup(why) => write!(f, "not a QS/MC backup: {why}"),
            Error::Passphrase => f.write_str("the passphrase does not open the backup"),
            Error::Damaged(why) => write!(f, "the backup is damaged: {why}"),
        }
    }
}

impl std::error::Error for Error {}
