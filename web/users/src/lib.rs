// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! QS/MC's web users (`docs/WEBUSERS.md`): their list and the role
//! matrix, kept in `webusers.toml` through `base/state`, and the
//! sign-on rules. Passwords are hashed and checked with QS/400's
//! `svcusers` and `base/users`'s rules; nothing of either is here.
//!
//! - [`role`]: roles, tasks and the matrix.
//! - [`list`]: the users and what may be done to them.
//! - [`sign`]: sign-on: the lock-out and the limit per address.

#![forbid(unsafe_code)]

pub mod list;
pub mod role;
pub mod sign;

pub use list::{User, WebUsers};
pub use role::{Matrix, ROLES, Role, TASKS, Task};
pub use sign::{Refused, Rules};
