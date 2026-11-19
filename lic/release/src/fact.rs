// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! The keys of the facts PID 1 keeps, as the control socket's status
//! names them.

/// The product's name.
pub const PRODUCT: &str = "product";
/// The release, `V1R0M0`.
pub const RELEASE: &str = "release";
/// The build ID.
pub const BUILD: &str = "build";
/// `release` or `test`: whether the test hooks are in the image.
pub const IMAGE: &str = "image";
/// The slot this IPL came from: `A`, `B`, `media` or `unknown`.
pub const SLOT: &str = "slot";
/// The A/B record, or why there is none.
pub const RECORD: &str = "record";
/// The host name.
pub const HOST: &str = "host";
/// The management address, or the reason there is none.
pub const MANAGEMENT: &str = appliance::NOTE_MANAGEMENT;
/// Seconds since the epoch.
pub const TIME: &str = "time";
/// The date and time as displayed.
pub const DATE: &str = "date";
/// The load source: its protection and each unit's state.
pub const LOAD_SOURCE: &str = "load_source";
/// The current reference code and its text.
pub const REFERENCE: &str = "reference";
/// `/data`: the device mounted there, or `memory`.
pub const DATA: &str = "data";
/// `yes` once every service has been ready.
pub const READY: &str = "ready";
/// What the processor has of what the cryptography needs.
pub const PROCESSOR: &str = "processor";
