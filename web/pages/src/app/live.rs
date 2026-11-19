// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! The running console as the pages see it: its name from PID 1, its
//! uptime and the clock from the kernel.

use std::time::{SystemTime, UNIX_EPOCH};

use supervise::control::{Calls, Client};

/// The console the web server runs on.
pub struct Live;

impl super::Console for Live {
    fn name(&self) -> String {
        Client::new(release::CONTROL_SOCKET)
            .status()
            .ok()
            .and_then(|s| s.fact(release::fact::HOST).map(str::to_string))
            .filter(|h| !h.is_empty())
            .unwrap_or_else(|| release::NAME.to_string())
    }

    fn uptime(&self) -> u64 {
        std::fs::read_to_string("/proc/uptime")
            .ok()
            .and_then(|t| t.split('.').next().and_then(|s| s.parse().ok()))
            .unwrap_or(0)
    }

    fn now(&self) -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_secs())
    }

    fn fact(&self, key: &str) -> String {
        Client::new(release::CONTROL_SOCKET)
            .status()
            .ok()
            .and_then(|s| s.fact(key).map(str::to_string))
            .unwrap_or_default()
    }
}
