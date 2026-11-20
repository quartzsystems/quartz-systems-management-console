// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! QS/MC's pages, each the prototype's page (`design/qsmc-prototype/`)
//! rendered from a view: the console's own state when served, the
//! prototype's sample data in the golden tests (`tests/golden.rs`).
//! Built from `qsweb`'s frame and partials; nothing of them is here.
//!
//! - [`app`]: sign-on, sessions, the task of each route, audit.
//! - [`pages`]: the signed-on pages by route.
//! - one module a page: its view and its rendering.

#![forbid(unsafe_code)]

pub mod app;
pub mod backup;
pub mod backup_routes;
pub mod certs;
pub mod certs_routes;
pub mod console;
pub mod datetime;
pub mod datetime_routes;
pub mod fmt;
pub mod help;
pub mod locale;
pub mod locale_routes;
pub mod network;
pub mod network_routes;
pub mod pages;
pub mod shutdown;
pub mod shutdown_routes;
pub mod signon;
pub mod url;
pub mod user_routes;
pub mod users;
pub mod version;
pub mod version_routes;
pub mod welcome;

pub use app::App;
