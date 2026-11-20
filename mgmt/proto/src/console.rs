// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! A terminal window's connection on `release::MGMTD_CONSOLES`. The web
//! server sends one [`Watch`] as an `ipc` frame and is answered
//! `Result<Watching, Refusal>` the same way; after that the connection
//! carries QSI's own console pieces (`qsi::host::console`): `Out` from
//! `mgmtd` (the latest whole screen first, then its updates, and
//! `Ended` with why), `In` from the web server (keys, a refresh). The
//! host has one viewer of a partition's console, `mgmtd`, which shares
//! it among every window that has it open.

use ipc::wire_struct;

/// What a window asks to watch.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Watch {
    /// The managed system.
    pub system: String,
    /// The partition.
    pub id: u32,
    /// The signed-on user, for the managed system's log.
    pub user: String,
    /// Take the console from another console that views it.
    pub take_over: bool,
    /// Whether the user may type; a key from a window that may not is
    /// dropped.
    pub typing: bool,
}
wire_struct!(Watch {
    system,
    id,
    user,
    take_over,
    typing
});

/// What a window watches.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Watching {
    /// The name the screen's status row shows: the partition's system,
    /// as its channel says it, or the partition's own name.
    pub system: String,
    /// Whether the console is a serial port, with no frames.
    pub serial: bool,
}
wire_struct!(Watching { system, serial });
