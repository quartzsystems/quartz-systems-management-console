// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! QS/MC conformance harness. Boots the disk image or the installation
//! ISO under `qemu-system-x86_64` with OVMF, captures the serial port
//! and checks what PID 1 says there. `build test` drives it.
//!
//! QS/VS's harness (`tools/conformance` there) cannot be taken from
//! outside: it depends on most of QS/VS's own crates and reads `QSVS_`
//! variables. This is the small one Phase 0 needs; docs/PHASE0.md says
//! what QS/VS would have to expose for QS/MC to pin its driver instead.

#![forbid(unsafe_code)]

pub mod error;
pub mod qemu;
pub mod suite;

pub use error::HarnessError;
pub use qemu::firmware;
pub use suite::{Images, Media, Report, Suite};
