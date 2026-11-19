// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! A mirrored load source whose units each ran without the other. The
//! storage service assembles nothing and says so with a marker file;
//! PID 1 then runs the local console in its one-panel form, which asks
//! the operator which unit to keep and tells the storage service. It
//! has the display and the serial port meanwhile. Nobody signs on: the
//! service tools users are on the data file system this is about. In a
//! test image the harness may answer through its channel instead.

use std::fs::OpenOptions;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use crate::host::Host;

/// Where the panel's lines besides its screens go.
const LOG: &str = "/run/keep-unit.log";
/// How soon a panel that ended without a choice is started again.
const AGAIN: Duration = Duration::from_secs(3);

fn log() -> Stdio {
    OpenOptions::new()
        .create(true)
        .append(true)
        .open(LOG)
        .map_or_else(|_| Stdio::null(), Stdio::from)
}

/// The panel that asks, while it is wanted.
pub struct Chooser {
    marker: PathBuf,
    child: Option<Child>,
    /// The serial port is the panel's.
    lent: bool,
    /// When a panel may next be started.
    next: Instant,
}

impl Chooser {
    /// A chooser that looks for the storage service's `marker`.
    pub fn new(marker: PathBuf) -> Self {
        Self {
            marker,
            child: None,
            lent: false,
            next: Instant::now(),
        }
    }

    /// Whether the storage service waits for the operator. While it
    /// does, the panel runs; when it waits no more, the serial port is
    /// PID 1's again.
    pub fn wanted(&mut self, host: &mut Host) -> bool {
        if !self.marker.exists() {
            self.end(host);
            return false;
        }
        let running = self
            .child
            .as_mut()
            .is_some_and(|child| matches!(child.try_wait(), Ok(None)));
        if !running && Instant::now() >= self.next {
            if self.child.take().is_some() {
                host.lend_serial(false);
                host.line("The panel ended before a unit was chosen; it starts again");
            } else {
                host.line("The units of the load source ran apart: the console asks which to keep");
            }
            self.next = Instant::now() + AGAIN;
            let mut command = Command::new(release::CONSOLE);
            command
                .arg(release::KEEP_UNIT)
                .env_clear()
                .stdin(Stdio::null())
                .stdout(log())
                .stderr(log());
            // PID 1 reads its signals from a descriptor and blocks them.
            match supervise::signal::unblocked(&mut command).spawn() {
                Ok(child) => {
                    self.child = Some(child);
                    self.lent = true;
                    host.lend_serial(true);
                }
                Err(e) => host.line(&format!("{}: {e}", release::CONSOLE)),
            }
        }
        true
    }

    /// The choice is made, or the wait is over: the panel ends and the
    /// serial port is PID 1's again.
    pub fn end(&mut self, host: &mut Host) {
        if let Some(mut child) = self.child.take() {
            // It ends by itself once answered; one that has not is ended.
            let give_up = Instant::now() + Duration::from_secs(2);
            while matches!(child.try_wait(), Ok(None)) && Instant::now() < give_up {
                host.pump();
            }
            if matches!(child.try_wait(), Ok(None)) {
                let _ = child.kill();
            }
        }
        if self.lent {
            self.lent = false;
            host.lend_serial(false);
        }
    }
}
