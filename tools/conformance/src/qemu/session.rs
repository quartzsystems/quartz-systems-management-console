// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! A running guest for the base system's suites: the machine as a
//! [`Machine`] describes it, its serial port read as it comes, the test
//! hooks' channel on its second serial port and QEMU's monitor on a
//! Unix socket.

use std::ffi::OsString;
use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::time::{Duration, Instant};

use super::Launch;
use super::channel::Channel;

/// A disk of the machine.
#[derive(Clone, Debug)]
pub struct Disk {
    /// The raw image.
    pub path: PathBuf,
    /// Its serial number.
    pub serial: String,
    /// Whether the guest's writes are thrown away. Disk `n`'s drive is
    /// `hd<n>`: `drive_del hd<n>` at the monitor fails every transfer to
    /// it from then on, as a disk that fails while the guest runs.
    pub snapshot: bool,
}

/// The machine a suite boots.
#[derive(Clone, Debug, Default)]
pub struct Machine {
    /// The disks, in order.
    pub disks: Vec<Disk>,
    /// This boot's variable store.
    pub vars: PathBuf,
    /// The processor model; the launch's default when `None`.
    pub cpu: Option<String>,
    /// The MAC of a virtio NIC on QEMU's user network; none when `None`.
    pub nic: Option<String>,
    /// A file every frame of the NIC is written to, as pcap.
    pub dump: Option<PathBuf>,
    /// The Unix socket of the test hooks' channel.
    pub hook: Option<PathBuf>,
    /// The Unix socket of QEMU's monitor.
    pub monitor: Option<PathBuf>,
    /// SMBIOS OEM strings: the test hooks' switches.
    pub oem: Vec<String>,
}

/// QEMU's arguments for `m`.
pub fn args(l: &Launch, m: &Machine) -> Vec<OsString> {
    let mut a: Vec<OsString> = Vec::new();
    let mut push = |words: &[&str]| a.extend(words.iter().map(OsString::from));
    push(&["-machine", "q35", "-m", "1024", "-smp", "2"]);
    if l.kvm {
        push(&["-enable-kvm"]);
    }
    let cpu = m.cpu.clone().unwrap_or_else(|| l.default_cpu().to_string());
    a.extend(["-cpu".into(), cpu.into()]);
    a.extend(
        [
            "-nodefaults",
            "-no-user-config",
            "-nographic",
            "-no-reboot",
            "-rtc",
            "base=utc",
            "-vga",
            "std",
            "-serial",
            "stdio",
        ]
        .map(OsString::from),
    );
    let mut pflash = OsString::from("if=pflash,format=raw,readonly=on,file=");
    pflash.push(&l.firmware);
    a.extend(["-drive".into(), pflash]);
    let mut vars = OsString::from("if=pflash,format=raw,file=");
    vars.push(&m.vars);
    a.extend(["-drive".into(), vars]);
    for (i, d) in m.disks.iter().enumerate() {
        let mut drive = OsString::from(format!(
            "if=none,id=hd{i},format=raw,discard=unmap,detect-zeroes=unmap,snapshot={},file=",
            if d.snapshot { "on" } else { "off" }
        ));
        drive.push(&d.path);
        a.extend(["-drive".into(), drive, "-device".into()]);
        a.push(format!("virtio-blk-pci,drive=hd{i},serial={}", d.serial).into());
    }
    if let Some(hook) = &m.hook {
        let mut chardev = OsString::from("socket,id=hook,path=");
        chardev.push(hook);
        chardev.push(",server=on,wait=off");
        a.extend([
            "-chardev".into(),
            chardev,
            "-serial".into(),
            "chardev:hook".into(),
        ]);
    }
    a.push("-monitor".into());
    match &m.monitor {
        Some(path) => {
            let mut unix = OsString::from("unix:");
            unix.push(path);
            unix.push(",server=on,wait=off");
            a.push(unix);
        }
        None => a.push("none".into()),
    }
    match &m.nic {
        Some(mac) => {
            a.extend(["-netdev".into(), "user,id=net0".into()]);
            if let Some(dump) = &m.dump {
                let mut filter = OsString::from("filter-dump,id=dump0,netdev=net0,file=");
                filter.push(dump);
                a.extend(["-object".into(), filter]);
            }
            a.push("-device".into());
            a.push(format!("virtio-net-pci,netdev=net0,mac={mac}").into());
        }
        None => a.extend(["-nic", "none"].map(OsString::from)),
    }
    for oem in &m.oem {
        a.extend(["-smbios".into(), format!("type=11,value={oem}").into()]);
    }
    a
}

/// A running guest.
pub struct Session {
    child: Child,
    rx: Receiver<String>,
    /// What the serial port has shown, and QEMU's standard error.
    pub log: String,
    /// Where [`Session::expect`] looks from.
    at: usize,
    exited: bool,
    hook: Option<PathBuf>,
    monitor: Option<PathBuf>,
    channel: Option<Channel>,
}

fn pump<R: Read + Send + 'static>(mut reader: R, tx: mpsc::Sender<String>) {
    std::thread::spawn(move || {
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
}

impl Session {
    /// Start QEMU for `m`.
    pub fn spawn(l: &Launch, m: &Machine) -> Result<Self, String> {
        for socket in [&m.hook, &m.monitor].into_iter().flatten() {
            let _ = std::fs::remove_file(socket);
        }
        let mut child = Command::new(&l.qemu)
            .args(args(l, m))
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("cannot start {}: {e}", l.qemu.display()))?;
        let (tx, rx) = mpsc::channel();
        if let Some(out) = child.stdout.take() {
            pump(out, tx.clone());
        }
        if let Some(err) = child.stderr.take() {
            pump(err, tx);
        }
        Ok(Self {
            child,
            rx,
            log: String::new(),
            at: 0,
            exited: false,
            hook: m.hook.clone(),
            monitor: m.monitor.clone(),
            channel: None,
        })
    }

    /// Take what has come, waiting at most `wait`.
    fn read(&mut self, wait: Duration) {
        match self.rx.recv_timeout(wait) {
            Ok(text) => {
                self.log.push_str(&text);
                while let Ok(more) = self.rx.try_recv() {
                    self.log.push_str(&more);
                }
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => self.exited = true,
        }
    }

    /// Wait until `done` says so of the whole log, by `deadline`.
    pub fn wait_for(
        &mut self,
        what: &str,
        deadline: Instant,
        done: impl Fn(&str) -> bool,
    ) -> Result<(), String> {
        loop {
            if done(&self.log) {
                return Ok(());
            }
            if self.exited {
                return Err(format!("{what}: QEMU ended first"));
            }
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() {
                return Err(format!("{what}: not seen in time"));
            }
            self.read(left.min(Duration::from_millis(100)));
        }
    }

    /// Wait for `text` after what was expected before, by `deadline`.
    pub fn expect(&mut self, text: &str, deadline: Instant) -> Result<(), String> {
        let from = self.at;
        self.wait_for(&format!("`{text}`"), deadline, |log| {
            log[from..].contains(text)
        })?;
        if let Some(i) = self.log[from..].find(text) {
            self.at = from + i + text.len();
        }
        Ok(())
    }

    /// Wait for `text` anywhere in the log, by `deadline`.
    pub fn seen(&mut self, text: &str, deadline: Instant) -> Result<(), String> {
        self.wait_for(&format!("`{text}`"), deadline, |log| log.contains(text))
    }

    /// Whether the log has `text` so far.
    pub fn said(&mut self, text: &str) -> bool {
        self.read(Duration::ZERO);
        self.log.contains(text)
    }

    /// The rest of the line after what was expected last.
    pub fn rest_of_line(&mut self, deadline: Instant) -> Result<String, String> {
        let from = self.at;
        self.wait_for("the end of the line", deadline, |log| {
            log[from..].contains('\n')
        })?;
        let rest = &self.log[from..];
        Ok(rest[..rest.find('\n').unwrap_or(rest.len())].to_string())
    }

    /// Mark everything said so far as passed over by [`Session::expect`].
    pub fn skip(&mut self) {
        self.read(Duration::ZERO);
        self.at = self.log.len();
    }

    /// A command to QEMU's monitor.
    pub fn monitor(&mut self, command: &str) -> Result<(), String> {
        let path = self.monitor.clone().ok_or("this guest has no monitor")?;
        let mut stream = UnixStream::connect(&path).map_err(|e| format!("the monitor: {e}"))?;
        stream
            .write_all(format!("{command}\n").as_bytes())
            .map_err(|e| format!("the monitor: {e}"))?;
        // Its banner and the command's echo: the command was taken.
        std::thread::sleep(Duration::from_millis(300));
        Ok(())
    }

    /// The test hooks' channel, connected and answering, by `deadline`.
    pub fn channel(&mut self, deadline: Instant) -> Result<&mut Channel, String> {
        if self.channel.is_none() {
            let path = self.hook.clone().ok_or("this guest has no channel")?;
            let mut channel = Channel::connect(&path, deadline)?;
            channel.wait_ready(deadline)?;
            self.channel = Some(channel);
        }
        self.channel.as_mut().ok_or_else(|| "no channel".into())
    }

    /// Wait for QEMU to end by itself, by `deadline`: how long it took.
    pub fn wait_exit(&mut self, deadline: Instant) -> Result<Duration, String> {
        let start = Instant::now();
        loop {
            if matches!(self.child.try_wait(), Ok(Some(_))) {
                // What is left in the pipes.
                while !self.exited {
                    self.read(Duration::from_millis(100));
                    if start.elapsed() > Duration::from_secs(60) {
                        break;
                    }
                }
                return Ok(start.elapsed());
            }
            if Instant::now() >= deadline {
                return Err("QEMU did not end in time".into());
            }
            self.read(Duration::from_millis(100));
        }
    }

    /// Pull the power: QEMU is killed.
    pub fn kill(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        self.read(Duration::from_millis(50));
        self.channel = None;
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_machine_s_switches_are_on_the_command_line() {
        let l = Launch {
            qemu: "qemu".into(),
            firmware: "/f/CODE.fd".into(),
            vars: "/f/VARS.fd".into(),
            kvm: true,
            timeout: Duration::from_secs(60),
        };
        let m = Machine {
            disks: vec![Disk {
                path: "/t/a.img".into(),
                serial: "QSMCHD0".into(),
                snapshot: true,
            }],
            vars: "/t/v.fd".into(),
            nic: Some("52:54:00:12:34:56".into()),
            dump: Some("/t/net.pcap".into()),
            hook: Some("/t/hook.sock".into()),
            oem: vec!["qsmc.hook=churn".into()],
            ..Machine::default()
        };
        let w: Vec<String> = args(&l, &m)
            .into_iter()
            .map(|w| w.to_string_lossy().into_owned())
            .collect();
        let has = |x: &str| w.iter().any(|a| a == x);
        assert!(has("virtio-blk-pci,drive=hd0,serial=QSMCHD0"));
        assert!(
            w.iter()
                .any(|a| a.contains("snapshot=on") && a.ends_with("/t/a.img"))
        );
        assert!(has("socket,id=hook,path=/t/hook.sock,server=on,wait=off"));
        assert!(has("chardev:hook"));
        assert!(has("type=11,value=qsmc.hook=churn"));
        assert!(has("filter-dump,id=dump0,netdev=net0,file=/t/net.pcap"));
        assert!(has("virtio-net-pci,netdev=net0,mac=52:54:00:12:34:56"));
        assert!(has("none"));
    }
}
