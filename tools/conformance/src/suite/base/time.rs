// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! Suite 4: time. An SNTP responder run by the harness, three days
//! ahead of the host's clock, moves the console's clock, and the
//! network service writes the RTC from it.

use std::net::UdpSocket;
use std::sync::{Arc, Mutex};

use appliance::time::now_secs;
use ntp::Clock;
use ntp::clock::system_now;
use release::fact;

use super::{DHCP_GATEWAY, MAC, Run, booted, channel, fact_of, posted, seed_disk, seed_toml};
use crate::qemu::session::Machine;

/// How far ahead the responder's clock is, in seconds.
const AHEAD: i64 = 3 * 86_400;
/// How near the console's clock and its RTC must come to it, in
/// seconds.
const WITHIN: i64 = 10;
/// Where the console's RTC is read.
const RTC: &str = "/sys/class/rtc/rtc0/since_epoch";

/// The host's clock, ahead; it cannot be adjusted.
struct Ahead;

impl Clock for Ahead {
    fn now(&self) -> i64 {
        system_now() + AHEAD * 1_000_000_000
    }

    fn slew(&self, _: i64) -> Result<(), String> {
        Err("the responder's clock is not adjusted".into())
    }

    fn step(&self, _: i64) -> Result<(), String> {
        Err("the responder's clock is not adjusted".into())
    }
}

/// An SNTP responder on the host's loopback, with `clock`: the server
/// as the console names it (QEMU's user network hands what the guest
/// sends to the gateway to the host's loopback) and the responder.
pub fn responder(clock: Arc<dyn Clock>) -> Result<(String, ntp::server::Running), String> {
    let socket = UdpSocket::bind("127.0.0.1:0").map_err(|e| e.to_string())?;
    let port = socket.local_addr().map_err(|e| e.to_string())?.port();
    let running = ntp::server::serve(socket, clock, Arc::new(Mutex::new(None)), false, None)
        .map_err(|e| format!("the SNTP responder: {e}"))?;
    Ok((format!("{DHCP_GATEWAY}:{port}"), running))
}

/// The suite.
pub fn time(run: &Run<'_>) -> Result<String, String> {
    let (server, responder) = responder(Arc::new(Ahead))?;
    let host = format!("ntp_servers = [\"{server}\"]\n");
    let mut machine = Machine {
        nic: Some(MAC.to_string()),
        ..run.machine()?
    };
    machine.disks.push(seed_disk(run, &seed_toml(&host, ""))?);
    let mut guest = booted(run, &machine)?;
    let deadline = run.deadline();
    let before: i64 = fact_of(&channel(&mut guest)?.status()?, fact::TIME)?
        .parse()
        .map_err(|_| "the status's time is not a number")?;
    guest.seen(&posted(&refcode::TIME_SYNCHRONIZED, &server), deadline)?;
    let status = channel(&mut guest)?.status()?;
    let after: i64 = fact_of(&status, fact::TIME)?
        .parse()
        .map_err(|_| "the status's time is not a number")?;
    let rtc: i64 = channel(&mut guest)?
        .text(RTC)?
        .trim()
        .parse()
        .map_err(|_| "the RTC's time is not a number")?;
    responder.stop();
    let wanted = now_secs() + AHEAD;
    let off = after - wanted;
    if off.abs() > WITHIN {
        return Err(format!(
            "the console's clock is {off} s from the responder's after synchronizing"
        ));
    }
    let rtc_off = rtc - wanted;
    if rtc_off.abs() > WITHIN {
        return Err(format!(
            "the RTC is {rtc_off} s from the responder's clock: it was not written back"
        ));
    }
    Ok(format!(
        "the responder at {server}, {AHEAD} s ahead, moved the console's clock {} s; it is now \
         {}; the RTC is {rtc_off} s from it",
        after - before,
        fact_of(&status, fact::DATE)?
    ))
}
