// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! The harness's own product streams (`docs/UPDATES.md`): the test keys
//! a test image trusts for each stream, and a stream server the console
//! is pointed at, which can be made slow, wrong or absent.
//!
//! - [`keys`]: the three streams' test keys, the same for every build.
//! - [`server`]: signed indexes and a catalog under them, update images,
//!   notes, served over plain HTTP.

pub mod keys;
pub mod server;
