// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! The two services of a test image's table: the channel, the
//! harness's way to the control socket and the files of a running
//! console over the second serial port; and `flaky`, which fails on
//! demand. PID 1 supervises both like any service.
//!
//! The channel is first in the table, before the storage service, so it
//! answers while the IPL waits; the churn waits for `/data` itself.

mod commands;

use std::convert::Infallible;
use std::fs::{File, OpenOptions};
use std::io::{self, BufRead, BufReader, Write};
use std::time::Duration;

use rustix::termios::{OptionalActions, tcgetattr, tcsetattr};

use crate::proto::{END, PORT};

/// The serial port without the line discipline: what the harness sends
/// arrives as it was sent.
fn raw(port: &File) {
    if let Ok(mut settings) = tcgetattr(port) {
        settings.make_raw();
        let _ = tcsetattr(port, OptionalActions::Now, &settings);
    }
}

/// Whether `/data` is mounted.
fn data_mounted() -> bool {
    std::fs::read_to_string("/proc/mounts").is_ok_and(|m| {
        m.lines()
            .any(|l| l.split(' ').nth(1) == Some(release::DATA_DIR))
    })
}

/// The churn, once `/data` is mounted: the state files are on it.
fn churn() -> ! {
    while !data_mounted() {
        std::thread::sleep(Duration::from_millis(50));
    }
    let appliance = release::appliance(|_| {}, |_, _| {});
    crate::churn::run(&state::Store::at(appliance.state_dir()))
}

/// Run the channel. It returns only when the port cannot be used.
pub fn channel() -> io::Result<Infallible> {
    let port = OpenOptions::new().read(true).write(true).open(PORT)?;
    raw(&port);
    if crate::on(crate::CHURN) {
        std::thread::spawn(|| churn());
    }
    supervise::ready::signal()?;
    let mut out = port.try_clone()?;
    let mut lines = BufReader::new(port);
    let mut line = String::new();
    loop {
        line.clear();
        if lines.read_line(&mut line)? == 0 {
            // Nobody at the other end yet.
            std::thread::sleep(Duration::from_millis(50));
            continue;
        }
        let command = line.trim_matches(|c| c == '\r' || c == '\n' || c == '\0');
        if command.is_empty() {
            continue;
        }
        let mut answer = commands::answer(command).join("\n");
        if !answer.is_empty() {
            answer.push('\n');
        }
        answer.push_str(END);
        answer.push('\n');
        out.write_all(answer.as_bytes())?;
        out.flush()?;
    }
}

/// The service that fails on demand: with the `flaky-fails` hook on it
/// ends at once every time it is started; without, it says it is ready
/// and does nothing.
pub fn flaky() -> ! {
    if crate::on(crate::FLAKY_FAILS) {
        eprintln!("failing, as the {} hook asks", crate::FLAKY_FAILS);
        std::process::exit(3);
    }
    if let Err(e) = supervise::ready::signal() {
        eprintln!("not ready: {e}");
        std::process::exit(1);
    }
    loop {
        std::thread::sleep(Duration::from_secs(3600));
    }
}
