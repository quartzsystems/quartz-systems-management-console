// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! An IPL from installation media: the installer runs in place of the
//! service table and has the display and the serial port to itself.
//! When it ends with status 0 the machine restarts; an installer that
//! ends any other way is started again.

use std::fs::OpenOptions;
use std::process::{Command, Stdio};
use std::time::Duration;

use rustix::fs::sync;
use rustix::system::{RebootCommand, reboot};

use crate::host::Host;

/// Where the installer's lines besides its screens go: its log, which
/// its last step copies to the new data file system.
const LOG: &str = "/run/install.log";

fn log() -> Stdio {
    OpenOptions::new()
        .create(true)
        .append(true)
        .open(LOG)
        .map_or_else(|_| Stdio::null(), Stdio::from)
}

/// Run the installer until it says the machine is to restart.
pub fn run(mut host: Host) -> ! {
    loop {
        host.line("The installer has the display and the serial port from here");
        let mut command = Command::new(release::INSTALLER);
        command
            .env_clear()
            .stdin(Stdio::null())
            .stdout(log())
            .stderr(log());
        // PID 1 reads its signals from a descriptor and blocks them.
        let ended = supervise::signal::unblocked(&mut command)
            .spawn()
            .and_then(|mut child| child.wait());
        let how = match ended {
            Ok(status) if status.success() => restart(&mut host),
            Ok(status) => status.to_string(),
            Err(e) => e.to_string(),
        };
        host.line(&format!(
            "The installer ended unexpectedly ({how}); it starts again"
        ));
        std::thread::sleep(Duration::from_secs(3));
    }
}

fn restart(host: &mut Host) -> ! {
    host.line("The installer has ended: restarting");
    sync();
    // Let the serial port drain.
    std::thread::sleep(Duration::from_millis(200));
    let failed = reboot(RebootCommand::Restart);
    host.line(&format!("the console did not restart: {failed:?}"));
    loop {
        std::thread::sleep(Duration::from_secs(3600));
    }
}
