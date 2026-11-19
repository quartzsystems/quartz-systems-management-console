// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! The QEMU driver: q35, OVMF by pflash, the serial port captured from
//! standard output, KVM with the host's processor when the build
//! machine has it. A boot waits for lines in order and stops QEMU.

pub mod channel;
pub mod firmware;
pub mod session;
pub mod vars;

use std::env;
use std::ffi::OsString;
use std::fs::OpenOptions;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::thread;
use std::time::{Duration, Instant};

use crate::error::HarnessError;

/// The QEMU binary, when not `qemu-system-x86_64` on `PATH`.
pub const QEMU_ENV: &str = "QSMC_QEMU";
/// Set to run QEMU without KVM.
pub const NO_KVM_ENV: &str = "QSMC_NO_KVM";
/// Seconds a boot may take to show what it must (60).
pub const TIMEOUT_ENV: &str = "QSMC_BOOT_TIMEOUT";

/// The default of [`TIMEOUT_ENV`].
const TIMEOUT: Duration = Duration::from_secs(60);

/// Whether `/dev/kvm` opens read-write.
pub fn kvm_usable() -> bool {
    cfg!(target_arch = "x86_64")
        && OpenOptions::new()
            .read(true)
            .write(true)
            .open("/dev/kvm")
            .is_ok()
}

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
        let firmware = firmware::locate()?;
        let vars = firmware::locate_vars(&firmware)?;
        let timeout = env::var(TIMEOUT_ENV)
            .ok()
            .and_then(|s| s.parse().ok())
            .map_or(TIMEOUT, Duration::from_secs);
        Ok(Self {
            qemu: env::var_os(QEMU_ENV).map_or_else(|| "qemu-system-x86_64".into(), PathBuf::from),
            firmware,
            vars,
            kvm: env::var_os(NO_KVM_ENV).is_none() && kvm_usable(),
            timeout,
        })
    }

    /// The processor model when none is asked for: the host's under
    /// KVM, else the emulator's fullest.
    pub fn default_cpu(&self) -> &'static str {
        if self.kvm { "host" } else { "max" }
    }
}

/// What a boot starts from.
#[derive(Clone, Debug)]
pub enum Drive {
    /// A disk image, on virtio, as a snapshot: the image is left as built.
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
    /// The processor model; the launch's default when `None`.
    pub cpu: Option<&'static str>,
    /// What the serial port must show, in this order.
    pub expect: Vec<String>,
}

fn join(head: &str, path: &Path) -> OsString {
    let mut s = OsString::from(head);
    s.push(path.as_os_str());
    s
}

/// QEMU's arguments for `boot`.
pub fn args(l: &Launch, boot: &Boot) -> Vec<OsString> {
    let mut a: Vec<OsString> = Vec::new();
    let mut push = |words: &[&str]| a.extend(words.iter().map(OsString::from));
    push(&["-machine", "q35", "-m", "1024", "-smp", "2"]);
    if l.kvm {
        push(&["-enable-kvm"]);
    }
    push(&["-cpu", boot.cpu.unwrap_or_else(|| l.default_cpu())]);
    push(&["-nodefaults", "-no-user-config", "-nographic", "-no-reboot"]);
    push(&["-rtc", "base=utc", "-vga", "std", "-serial", "stdio"]);
    push(&["-monitor", "none", "-nic", "none"]);
    a.push("-drive".into());
    a.push(join("if=pflash,format=raw,readonly=on,file=", &l.firmware));
    a.push("-drive".into());
    a.push(join("if=pflash,format=raw,file=", &boot.vars));
    match &boot.drive {
        Drive::Disk(path) => {
            a.push("-drive".into());
            a.push(join("if=virtio,format=raw,snapshot=on,file=", path));
        }
        Drive::Cdrom(path) => {
            a.push("-drive".into());
            a.push(join(
                "if=none,id=cd,media=cdrom,format=raw,readonly=on,file=",
                path,
            ));
            // On virtio-scsi: OVMF on q35's IDE CD-ROM hangs in the
            // firmware on about one boot in three, QS/VS's ISO too
            // (docs/OPEN_QUESTIONS.md item 10).
            a.extend(["-device", "virtio-scsi-pci,id=scsi", "-device"].map(OsString::from));
            a.push("scsi-cd,drive=cd,bus=scsi.0".into());
        }
    }
    a
}

/// Forward chunks of a pipe into a channel; it closes at the pipe's end.
fn pump<R: Read + Send + 'static>(mut reader: R) -> Receiver<String> {
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let mut buf = [0u8; 4096];
        loop {
            match reader.read(&mut buf) {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    let text = String::from_utf8_lossy(&buf[..n]).replace('\r', "");
                    if tx.send(text).is_err() {
                        break;
                    }
                }
            }
        }
    });
    rx
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
    let mut child = Command::new(&l.qemu)
        .args(args(l, b))
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| HarnessError::Spawn {
            qemu: l.qemu.clone(),
            reason: e.to_string(),
        })?;
    let out = child.stdout.take().map(pump);
    let err = child.stderr.take().map(pump);
    let start = Instant::now();
    let mut log = String::new();
    let mut exited = false;
    let result = loop {
        if matched(&log, &b.expect) == b.expect.len() {
            break Some(start.elapsed());
        }
        let left = l.timeout.saturating_sub(start.elapsed());
        if left.is_zero() || exited {
            break None;
        }
        let wait = left.min(Duration::from_millis(200));
        match out.as_ref().map(|rx| rx.recv_timeout(wait)) {
            Some(Ok(text)) => log.push_str(&text),
            Some(Err(RecvTimeoutError::Timeout)) => {}
            Some(Err(RecvTimeoutError::Disconnected)) | None => exited = true,
        }
        if let Some(rx) = &err {
            while let Ok(text) = rx.try_recv() {
                log.push_str(&text);
            }
        }
        if !exited && matches!(child.try_wait(), Ok(Some(_))) {
            // Take what is left in the pipe before saying it ended.
            if let Some(rx) = &out {
                while let Ok(text) = rx.recv_timeout(Duration::from_millis(100)) {
                    log.push_str(&text);
                }
            }
            exited = true;
        }
    };
    let _ = child.kill();
    let _ = child.wait();
    if let Some(elapsed) = result {
        return Ok((elapsed, log));
    }
    let missing = b
        .expect
        .get(matched(&log, &b.expect))
        .cloned()
        .unwrap_or_default();
    Err(HarnessError::Timeout {
        step,
        missing,
        after: start.elapsed(),
        exited,
        log,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn launch(kvm: bool) -> Launch {
        Launch {
            qemu: "qemu".into(),
            firmware: "/f/CODE.fd".into(),
            vars: "/f/VARS.fd".into(),
            kvm,
            timeout: TIMEOUT,
        }
    }

    fn words(l: &Launch, b: &Boot) -> Vec<String> {
        args(l, b)
            .into_iter()
            .map(|w| w.to_string_lossy().into_owned())
            .collect()
    }

    fn has(w: &[String], run: &[&str]) -> bool {
        w.windows(run.len()).any(|x| x == run)
    }

    #[test]
    fn the_machine_is_q35_with_ovmf_and_the_drive() {
        let b = Boot {
            drive: Drive::Disk("/t/d.img".into()),
            vars: "/t/v.fd".into(),
            cpu: None,
            expect: Vec::new(),
        };
        let w = words(&launch(true), &b);
        assert!(has(&w, &["-machine", "q35"]));
        assert!(has(&w, &["-enable-kvm", "-cpu", "host"]));
        assert!(has(&w, &["-serial", "stdio"]));
        assert!(w.contains(&"if=pflash,format=raw,readonly=on,file=/f/CODE.fd".into()));
        assert!(w.contains(&"if=pflash,format=raw,file=/t/v.fd".into()));
        assert!(w.contains(&"if=virtio,format=raw,snapshot=on,file=/t/d.img".into()));
        let tcg = words(
            &launch(false),
            &Boot {
                cpu: Some("qemu64"),
                ..b
            },
        );
        assert!(!tcg.contains(&"-enable-kvm".into()));
        assert!(has(&tcg, &["-cpu", "qemu64"]));
    }

    #[test]
    fn lines_are_matched_in_order() {
        let want: Vec<String> = ["a", "b", "b"].map(String::from).to_vec();
        assert_eq!(matched("a b b", &want), 3);
        assert_eq!(matched("b a b", &want), 2);
        assert_eq!(matched("", &want), 0);
    }
}
