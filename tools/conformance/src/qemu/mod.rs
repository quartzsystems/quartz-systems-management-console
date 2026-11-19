// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! `base/qemu` (`driver`) as QS/MC's harness uses it: QS/MC's variables,
//! its machine's size, and the boots of Phase 0's steps. The session's
//! cursor, the machine a suite describes and the test hooks' channel are
//! the harness's own (`session`, `channel`).

pub mod channel;
pub mod session;

use std::env;
use std::path::PathBuf;
use std::time::{Duration, Instant};

pub use driver::{term, vars};

use crate::error::HarnessError;

/// The QEMU binary, when not `qemu-system-x86_64` on `PATH`.
pub const QEMU_ENV: &str = "QSMC_QEMU";
/// Set to run QEMU without KVM.
pub const NO_KVM_ENV: &str = "QSMC_NO_KVM";
/// Seconds a boot may take to show what it must (60).
pub const TIMEOUT_ENV: &str = "QSMC_BOOT_TIMEOUT";
/// The variable naming OVMF's code image.
pub const CODE_ENV: &str = "QSMC_OVMF_CODE";
/// The variable naming OVMF's variable store template.
pub const VARS_ENV: &str = "QSMC_OVMF_VARS";

/// The default of [`TIMEOUT_ENV`].
const TIMEOUT: Duration = Duration::from_secs(60);
/// A console's memory, MiB.
pub const MEMORY_MB: u32 = 1024;
/// A console's processors.
pub const CPUS: u32 = 2;

/// What every boot shares: the binary, the firmware, KVM or not.
#[derive(Clone, Debug)]
pub struct Launch {
    /// `qemu-system-x86_64`.
    pub qemu: PathBuf,
    /// OVMF's code image.
    pub firmware: PathBuf,
    /// OVMF's variable store template.
    pub vars: PathBuf,
    /// Whether KVM is used.
    pub kvm: bool,
    /// How long a boot may take.
    pub timeout: Duration,
}

impl Launch {
    /// From the host and the environment.
    pub fn detect() -> Result<Self, HarnessError> {
        let firmware = driver::firmware::locate(CODE_ENV)?;
        let vars = driver::firmware::locate_vars(&firmware, VARS_ENV)?;
        let timeout = env::var(TIMEOUT_ENV)
            .ok()
            .and_then(|s| s.parse().ok())
            .map_or(TIMEOUT, Duration::from_secs);
        Ok(Self {
            qemu: driver::launch::locate(QEMU_ENV)?,
            firmware,
            vars,
            kvm: driver::launch::kvm_wanted(NO_KVM_ENV),
            timeout,
        })
    }

    /// The base driver's launch of a console: its firmware, size and
    /// KVM, and nothing attached yet.
    pub fn base(&self) -> driver::Launch {
        driver::Launch {
            qemu: self.qemu.clone(),
            firmware: self.firmware.clone(),
            kvm: self.kvm,
            memory_mb: MEMORY_MB,
            cpus: CPUS,
            ..driver::Launch::default()
        }
    }
}

/// What a boot starts from.
#[derive(Clone, Debug)]
pub enum Drive {
    /// A disk image, as a snapshot: the image is left as built.
    Disk(PathBuf),
    /// An ISO, as a CD-ROM.
    Cdrom(PathBuf),
}

/// One boot.
#[derive(Clone, Debug)]
pub struct Boot {
    /// What it starts from.
    pub drive: Drive,
    /// This boot's own copy of the variable store.
    pub vars: PathBuf,
    /// The processor model; the host's when `None`.
    pub cpu: Option<&'static str>,
    /// What the serial port must show, in this order.
    pub expect: Vec<String>,
}

/// The base driver's launch of `boot`.
pub fn launch_of(l: &Launch, boot: &Boot) -> driver::Launch {
    let mut out = driver::Launch {
        vars: Some(boot.vars.clone()),
        cpu: boot.cpu.map(str::to_string),
        snapshot: true,
        disk_serial: crate::image::disks::serial(0),
        ..l.base()
    };
    match &boot.drive {
        Drive::Disk(path) => out.disk = Some(path.clone()),
        Drive::Cdrom(path) => out.cdrom = Some(path.clone()),
    }
    out
}

/// How many of `expect` `log` shows, each after the one before.
pub fn matched(log: &str, expect: &[String]) -> usize {
    let mut at = 0;
    let mut found = 0;
    for want in expect {
        match log[at..].find(want.as_str()) {
            Some(i) => {
                at += i + want.len();
                found += 1;
            }
            None => break,
        }
    }
    found
}

/// Boot, and wait until the serial port has shown everything expected,
/// in order. Returns the time it took and the log; QEMU is stopped
/// either way.
pub fn boot(l: &Launch, step: &'static str, b: &Boot) -> Result<(Duration, String), HarnessError> {
    let launch = launch_of(l, b);
    let mut session = driver::Session::spawn(&launch)?;
    let start = Instant::now();
    let deadline = start + l.timeout;
    let seen = session.wait_for("what the step expects", deadline, |log| {
        matched(log, &b.expect) == b.expect.len()
    });
    session.kill();
    let log = std::mem::take(&mut session.log);
    match seen {
        Ok(()) => Ok((start.elapsed(), log)),
        Err(e) => Err(HarnessError::Timeout {
            step,
            missing: b
                .expect
                .get(matched(&log, &b.expect))
                .cloned()
                .unwrap_or_default(),
            after: start.elapsed(),
            exited: e.contains("QEMU ended"),
            log,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn launch() -> Launch {
        Launch {
            qemu: "qemu".into(),
            firmware: "/f/CODE.fd".into(),
            vars: "/f/VARS.fd".into(),
            kvm: true,
            timeout: TIMEOUT,
        }
    }

    fn words(l: &driver::Launch) -> Vec<String> {
        driver::args(l)
            .into_iter()
            .map(|w| w.to_string_lossy().into_owned())
            .collect()
    }

    fn has(w: &[String], run: &[&str]) -> bool {
        w.windows(run.len()).any(|x| x == run)
    }

    #[test]
    fn a_step_boots_q35_with_ovmf_and_its_drive() {
        let b = Boot {
            drive: Drive::Disk("/t/d.img".into()),
            vars: "/t/v.fd".into(),
            cpu: None,
            expect: Vec::new(),
        };
        let w = words(&launch_of(&launch(), &b));
        assert!(has(&w, &["-machine", "q35"]));
        assert!(has(&w, &["-enable-kvm", "-cpu", "host"]));
        assert!(has(&w, &["-m", "1024"]));
        assert!(has(&w, &["-serial", "stdio"]));
        assert!(w.contains(&"if=pflash,format=raw,file=/t/v.fd".into()));
        assert!(w.iter().any(|a| a.ends_with("file=/t/d.img")));
        assert!(w.contains(&"-snapshot".into()));
        let iso = Boot {
            drive: Drive::Cdrom("/t/m.iso".into()),
            cpu: Some("qemu64"),
            ..b
        };
        let w = words(&launch_of(&launch(), &iso));
        assert!(has(&w, &["-cpu", "qemu64"]));
        assert!(w.contains(&"scsi-cd,drive=cd,bus=cdscsi.0".into()));
    }

    #[test]
    fn lines_are_matched_in_order() {
        let want: Vec<String> = ["a", "b", "b"].map(String::from).to_vec();
        assert_eq!(matched("a b b", &want), 3);
        assert_eq!(matched("b a b", &want), 2);
        assert_eq!(matched("", &want), 0);
    }
}
