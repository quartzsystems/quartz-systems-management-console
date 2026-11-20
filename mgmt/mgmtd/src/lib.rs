// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! QS/MC's managed-systems service (`docs/MGMT.md`). It holds the
//! console's QSI identity and a session with every managed system; the
//! web server holds neither and asks it on its local sockets
//! (`mgmtproto`). QSI's messages, its session and its trust rules are
//! QS/400's `qsi`, `qsisess` and `qsitrust`: nothing of them is here.
//!
//! - [`keep`]: `systems.toml`, `mgmt.toml` and the reference code
//!   history.
//! - [`peer`]: a connection to a managed system, and pairing.
//! - [`system`]: one thread a system: its session, what is kept, its
//!   consoles shared among windows.
//! - [`service`]: the requests the web server makes, and the console
//!   socket.

#![forbid(unsafe_code)]

pub mod keep;
pub mod peer;
pub mod service;
pub mod system;
pub mod words;

pub use service::{Config, Service};
