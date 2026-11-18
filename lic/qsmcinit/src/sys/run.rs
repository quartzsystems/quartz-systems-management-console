// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! The running console's loop, as Phase 0 has it: `SIGCHLD` taken from
//! a descriptor (`supervise::signal`), every child that ended reaped,
//! and the heartbeat on the serial port every five seconds.

use std::time::{Duration, Instant};

use rustix::event::{PollFd, PollFlags, poll};
use rustix::time::Timespec;
use supervise::signal::{self, Signals};

use crate::console::Console;

/// Seconds between heartbeats.
const HEARTBEAT: Duration = Duration::from_secs(5);

/// The wait until `deadline`, as `poll` takes it.
fn timeout(deadline: Instant) -> Timespec {
    let left = deadline.saturating_duration_since(Instant::now());
    Timespec {
        tv_sec: i64::try_from(left.as_secs()).unwrap_or(i64::MAX),
        tv_nsec: i64::from(left.subsec_nanos()),
    }
}

/// Wait for a child or the next heartbeat, for ever.
pub fn forever(console: &mut Console) -> ! {
    let mut signals = match Signals::take(&[signal::CHLD]) {
        Ok(s) => Some(s),
        Err(e) => {
            console.line(&format!(
                "SIGCHLD not taken from a descriptor ({e}): reaping on the heartbeat"
            ));
            None
        }
    };
    let mut next = Instant::now() + HEARTBEAT;
    loop {
        if let Some(s) = signals.as_mut() {
            let ready = {
                let fd = s.fd();
                let mut fds = [PollFd::new(&fd, PollFlags::IN)];
                poll(&mut fds, Some(&timeout(next))).is_ok_and(|n| n > 0)
            };
            if ready {
                s.read();
            }
        } else {
            std::thread::sleep(next.saturating_duration_since(Instant::now()));
        }
        super::reap::ended();
        if Instant::now() >= next {
            console.serial(&refcode::post::line(&refcode::HEARTBEAT, ""));
            next += HEARTBEAT;
        }
    }
}
