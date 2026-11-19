// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! QS/MC conformance harness. Boots the disk image or the installation
//! ISO under `qemu-system-x86_64` with OVMF, captures the serial port
//! and checks what PID 1 says there; on a test image it reaches the
//! control socket through the test hooks' channel on the second serial
//! port. `build test` drives it.
//!
//! - [`qemu`]: the QEMU driver, the session of a running guest and the
//!   channel's end.
//! - [`image`]: a load source written onto image files with
//!   `base/loadsource`'s writer, a seed volume, a sparse copy.
//! - [`suite`]: Phase 0's steps and the base system's suites.
//!
//! QS/VS's harness (`tools/conformance` there) cannot be taken from
//! outside: it depends on most of QS/VS's own crates and reads `QSVS_`
//! variables. This one is QS/MC's own; `docs/OPEN_QUESTIONS.md` says what
//! QS/VS would have to expose for QS/MC to pin its driver instead.

#![forbid(unsafe_code)]

pub mod error;
pub mod image;
pub mod qemu;
pub mod suite;

pub use error::HarnessError;
pub use suite::{Images, Media, Report, Suite, TestImage};
