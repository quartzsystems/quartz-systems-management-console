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
pub mod help;
pub mod pages;
pub mod signon;
pub mod url;
pub mod welcome;

pub use app::App;
