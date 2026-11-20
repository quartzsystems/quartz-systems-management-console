// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! The managed-systems service's sockets (`docs/MGMT.md`): what the web
//! server asks of `mgmtd` and what it is answered. The web server holds
//! no QSI key and no session; everything of a managed system it shows
//! or does comes through here.
//!
//! - [`kept`]: what `mgmtd` keeps about each system.
//! - [`calls`]: the requests, one a connection, on
//!   `release::MGMTD_SOCKET`.
//! - [`console`]: a terminal window's connection, on
//!   `release::MGMTD_CONSOLES`.

#![forbid(unsafe_code)]

pub mod calls;
pub mod console;
pub mod kept;

pub use calls::{Calls, Mgmt, Request, dispatch};
pub use kept::{Kept, Link, Note, Refusal};
