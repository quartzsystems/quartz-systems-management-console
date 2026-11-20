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
/// The `open` interface's line: its address, or the reason there is
/// none. The base crates' management line.
pub const MANAGEMENT: &str = appliance::NOTE_MANAGEMENT;
/// The `private` interface's line, as the network service notes it.
pub const PRIVATE: &str = "private";
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
/// The machine: its serial number and what the inventory counts.
pub const MACHINE: &str = "machine";
/// The outcome of the last web interface request made to PID 1 while
/// the web server was down: `<key>: done` or `<key>: <why not>`.
pub const WEB_REQUEST: &str = "web.request";
/// The last synchronization with a time server: `<seconds since the
/// epoch> <server>`; empty until there is one.
pub const SYNCED: &str = "synced";
/// Whose the serial port is: [`SERIAL_SESSION`] while the local console
/// has a session there, [`SERIAL_IDLE`] while it is PID 1's.
pub const SERIAL_CONSOLE: &str = "console.serial";
/// [`SERIAL_CONSOLE`] while a session is on the serial port.
pub const SERIAL_SESSION: &str = "session";
/// [`SERIAL_CONSOLE`] when the port is PID 1's again.
pub const SERIAL_IDLE: &str = "idle";
