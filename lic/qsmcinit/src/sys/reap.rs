// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! Reaping. PID 1 is the parent of every orphan.

use rustix::process::{WaitOptions, wait};

/// Reap every child that has ended; how many there were.
pub fn ended() -> usize {
    let mut count = 0;
    while let Ok(Some(_)) = wait(WaitOptions::NOHANG) {
        count += 1;
    }
    count
}
