// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! The steps that are QS/MC's own: the userland with or without the
//! test hooks, the A/B record on the disk image and the image read
//! back, the manifest held to its kind of image, and a load source
//! written onto image files for the harness (`writer`), and the release
//! with its stream (`release`).

pub mod disk;
pub mod manifest;
pub mod release;
pub mod update;
pub mod userland;
pub mod writer;
