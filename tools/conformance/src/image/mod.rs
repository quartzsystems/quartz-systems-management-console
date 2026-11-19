// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! The images the suites boot and look into: a load source written
//! with `base/loadsource`'s writer ([`disks`]); a seed volume and a
//! copy of an image that keeps its holes are `base/qemu`'s.

pub mod disks;

pub use driver::disk::{seed_volume, sparse_copy};
