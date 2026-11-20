// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! The running console as the pages see it: PID 1's facts and power,
//! the network service's interfaces, trial and clock, the kernel's
//! uptime and clock.

use std::time::{SystemTime, UNIX_EPOCH};

use netd::control::{Calls as _, Client as NetClient, NetStatus};
use supervise::control::{Calls, Client};

fn pid1() -> Client {
    Client::new(release::CONTROL_SOCKET)
}

fn net() -> NetClient {
    NetClient::new(release::appliance(|_| {}, |_, _| {}).net_socket())
}

/// The console the web server runs on.
pub struct Live;

impl super::Console for Live {
    fn name(&self) -> String {
        Some(self.fact(release::fact::HOST))
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
        pid1()
            .status()
            .ok()
            .and_then(|s| s.fact(key).map(str::to_string))
            .unwrap_or_default()
    }

    fn interfaces(&self) -> Vec<NetStatus> {
        net().interfaces().unwrap_or_default()
    }

    fn apply_network(&self, network: &str) -> Result<(), String> {
        net()
            .apply(network.to_string())
            .map_err(|e| e.to_string())?
    }

    fn apply_network_confirmed(&self, network: &str, seconds: u32) -> Result<(), String> {
        net()
            .apply_confirmed(network.to_string(), seconds)
            .map_err(|e| e.to_string())?
    }

    fn confirm_network(&self) -> Result<(), String> {
        net().confirm().map_err(|e| e.to_string())?
    }

    fn set_host(&self, name: &str, zone: &str, ntp: &[String]) -> Result<(), String> {
        pid1()
            .set_host(name.into(), zone.into(), ntp.to_vec())
            .map_err(|e| e.to_string())??;
        // The network service reads the name and the time servers again.
        net().reload().map_err(|e| e.to_string())?
    }

    fn set_clock(&self, secs: i64) -> Result<(), String> {
        net().set_clock(secs).map_err(|e| e.to_string())?
    }

    fn power_down(&self, restart: bool, by: &str) -> Result<(), String> {
        pid1()
            .power_down(restart, false, 0, by.into())
            .map_err(|e| e.to_string())?
    }

    fn kernel(&self) -> String {
        std::fs::read_to_string("/proc/sys/kernel/osrelease")
            .map(|k| k.trim().to_string())
            .unwrap_or_default()
    }

    fn data_space(&self) -> Option<(u64, u64)> {
        let s = rustix::fs::statvfs(release::DATA_DIR).ok()?;
        let all = s.f_blocks * s.f_frsize;
        Some((all, all.saturating_sub(s.f_bfree * s.f_frsize)))
    }
}
