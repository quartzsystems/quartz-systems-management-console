// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! QS/MC PID 1, as Phase 0 has it: the kernel's file systems mounted,
//! the banner, the slot and the processor's line on the display and the
//! serial port, a heartbeat, children reaped, and a panic that posts
//! its code and parks.

// The parts that need no kernel are built and tested on any host.
#![cfg_attr(not(target_os = "linux"), allow(dead_code))]

mod console;
mod cpu;
mod ipl;
mod product;
#[cfg(target_os = "linux")]
mod sys;

#[cfg(target_os = "linux")]
fn main() {
    ipl::run();
}

#[cfg(not(target_os = "linux"))]
fn main() {
    eprintln!("qsmcinit is PID 1 of a QS/MC console and runs on Linux only");
}
