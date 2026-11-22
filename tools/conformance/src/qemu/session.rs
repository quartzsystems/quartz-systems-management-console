// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! A running guest for the suites: the machine a [`Machine`] describes,
//! launched by the base driver, with QS/MC's cursor over its serial log
//! ([`Session::expect`] looks after what it found last), the test hooks'
//! channel on the second serial port, and the frames of the NIC captured
//! when asked.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use super::Launch;
use super::channel::Channel;
use super::term::Grid;

/// A disk of the machine.
#[derive(Clone, Debug)]
pub struct Disk {
    /// The raw image.
    pub path: PathBuf,
    /// Its serial number.
    pub serial: String,
    /// Whether the guest's writes are thrown away. The first disk's
    /// drive is `hd`, disk `n`'s `hd<n>`: `drive_del hd<n>` at the
    /// monitor fails every transfer to it from then on, as a disk that
    /// fails while the guest runs.
    pub snapshot: bool,
}

/// The machine a suite boots.
#[derive(Clone, Debug, Default)]
pub struct Machine {
    /// The disks, in order.
    pub disks: Vec<Disk>,
    /// This boot's variable store.
    pub vars: PathBuf,
    /// The processor model; the host's when `None`.
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
    /// The installation ISO, as a CD-ROM started before the disks.
    pub cdrom: Option<PathBuf>,
    /// The installation ISO as a USB stick, started before the disks.
    pub usb: Option<PathBuf>,
    /// The harness types at the serial port.
    pub keys: bool,
    /// The MAC of a second NIC, on a user network of its own: the
    /// `private` interface's.
    pub second_nic: Option<String>,
    /// Host ports forwarded to the guest's ports on the first NIC
    /// (`host`, `guest`): a browser's way in.
    pub forwards: Vec<(u16, u16)>,
    /// Memory and processors other than the launch's.
    pub size: Option<Size>,
}

/// A machine's memory and processors: a QS/VS host that runs a
/// partition.
#[derive(Clone, Copy, Debug)]
pub struct Size {
    /// Memory in MiB.
    pub memory_mb: u32,
    /// Processors.
    pub cpus: u32,
    /// The memory allocated before it runs.
    pub prealloc: bool,
}

/// The base driver's launch of `m`. The driver makes either every disk
/// a snapshot or none: when they differ, a disk that is to be one is
/// copied to `scratch` first and the copy written to.
pub fn launch_of(l: &Launch, m: &Machine, scratch: &Path) -> Result<driver::Launch, String> {
    let all = !m.disks.is_empty() && m.disks.iter().all(|d| d.snapshot);
    let mut disks = Vec::new();
    for (i, d) in m.disks.iter().enumerate() {
        if d.snapshot && !all {
            let copy = scratch.join(format!("snapshot-{i}.img"));
            driver::disk::sparse_copy(&d.path, &copy)?;
            disks.push((copy, d.serial.clone()));
        } else {
            disks.push((d.path.clone(), d.serial.clone()));
        }
    }
    let mut disks = disks.into_iter();
    let first = disks.next();
    // The monitor is there for the frames' capture when nothing else
    // asked for it.
    let base = l.base();
    let monitor = m
        .monitor
        .clone()
        .or_else(|| m.dump.as_ref().map(|d| d.with_extension("monitor.sock")));
    Ok(driver::Launch {
        vars: Some(m.vars.clone()),
        cpu: m.cpu.clone(),
        disk_serial: first.as_ref().map(|(_, s)| s.clone()).unwrap_or_default(),
        disk: first.map(|(p, _)| p),
        extra: disks
            .map(|(path, serial)| driver::Drive {
                path,
                serial,
                place: None,
            })
            .collect(),
        snapshot: all,
        nic: m.nic.clone(),
        hook: m.hook.clone(),
        monitor,
        oem: m.oem.clone(),
        cdrom: m.cdrom.clone(),
        usb: m.usb.clone(),
        media_first: m.cdrom.is_some() || m.usb.is_some(),
        keys: m.keys,
        forwards: m.forwards.clone(),
        uplinks: m
            .second_nic
            .iter()
            .map(|mac| driver::UplinkNic {
                mac: mac.clone(),
                far: driver::Far::User(None),
                mtu: None,
            })
            .collect(),
        memory_mb: m.size.map_or(base.memory_mb, |s| s.memory_mb),
        cpus: m.size.map_or(base.cpus, |s| s.cpus),
        prealloc: m.size.map_or(base.prealloc, |s| s.prealloc),
        ..base
    })
}

/// A running guest.
pub struct Session {
    inner: driver::Session,
    /// Where [`Session::expect`] looks from.
    at: usize,
    exited: bool,
    channel: Option<Channel>,
}

impl Session {
    /// Start QEMU for `m`, its scratch files under `scratch`.
    pub fn spawn(l: &Launch, m: &Machine, scratch: &Path) -> Result<Self, String> {
        let launch = launch_of(l, m, scratch)?;
        let inner = driver::Session::spawn(&launch).map_err(|e| e.to_string())?;
        let mut session = Self {
            inner,
            at: 0,
            exited: false,
            channel: None,
        };
        if let Some(dump) = &m.dump {
            // Added before the firmware has left the reset vector: the
            // console's first frame is seconds away.
            session.capture(dump)?;
        }
        Ok(session)
    }

    /// Write every frame of the NIC to `dump` from now on: QEMU's
    /// filter, added at the monitor. The monitor does not answer, and
    /// QEMU 8.2 can drop a command sent as it starts, so it is sent
    /// again until the filter has made its file.
    fn capture(&mut self, dump: &Path) -> Result<(), String> {
        let command = format!(
            "object_add filter-dump,id=dump0,netdev=net0,file={}",
            dump.display()
        );
        let give_up = Instant::now() + Duration::from_secs(10);
        loop {
            match self.inner.monitor(&command) {
                Ok(()) if dump.exists() => return Ok(()),
                Ok(()) if Instant::now() >= give_up => {
                    return Err(format!("QEMU made no {}", dump.display()));
                }
                Ok(()) => {}
                // QEMU makes the socket as it starts.
                Err(_) if Instant::now() < give_up => {
                    std::thread::sleep(Duration::from_millis(20));
                }
                Err(e) => return Err(e),
            }
        }
    }

    /// What the serial port has shown, and QEMU's standard error.
    pub fn log(&self) -> &str {
        &self.inner.log
    }

    /// Wait until `done` says so of the whole log, by `deadline`.
    pub fn wait_for(
        &mut self,
        what: &str,
        deadline: Instant,
        done: impl Fn(&str) -> bool,
    ) -> Result<(), String> {
        if self.exited && !done(&self.inner.log) {
            return Err(format!("{what}: QEMU ended first"));
        }
        let seen = self.inner.wait_for(what, deadline, done);
        if let Err(e) = &seen
            && e.contains("QEMU ended")
        {
            self.exited = true;
        }
        seen
    }

    /// Wait for `text` after what was expected before, by `deadline`.
    pub fn expect(&mut self, text: &str, deadline: Instant) -> Result<(), String> {
        let from = self.at;
        self.wait_for(&format!("`{text}`"), deadline, |log| {
            log[from..].contains(text)
        })?;
        if let Some(i) = self.inner.log[from..].find(text) {
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
        self.inner.said(text)
    }

    /// The rest of the line after what was expected last.
    pub fn rest_of_line(&mut self, deadline: Instant) -> Result<String, String> {
        let from = self.at;
        self.wait_for("the end of the line", deadline, |log| {
            log[from..].contains('\n')
        })?;
        let rest = &self.inner.log[from..];
        Ok(rest[..rest.find('\n').unwrap_or(rest.len())].to_string())
    }

    /// Mark everything said so far as passed over by [`Session::expect`].
    pub fn skip(&mut self) {
        self.inner.drain();
        self.at = self.inner.log.len();
    }

    /// A command to QEMU's monitor.
    pub fn monitor(&mut self, command: &str) -> Result<(), String> {
        self.inner.monitor(command)
    }

    /// Type `keys` at the guest's keyboard through the monitor: each a
    /// `sendkey` name.
    pub fn send_keys(&mut self, keys: &[String]) -> Result<(), String> {
        self.inner.send_keys(keys)
    }

    /// Type `text` at the guest's serial port.
    pub fn type_keys(&mut self, text: &str) -> Result<(), String> {
        self.inner.type_keys(text)
    }

    /// The serial port as a 24x80 terminal shows it now.
    pub fn grid(&mut self) -> &Grid {
        self.inner.grid()
    }

    /// Wait until the terminal's grid satisfies `found` and has stopped
    /// changing, by `deadline`: its text then.
    pub fn screen(
        &mut self,
        what: &str,
        deadline: Instant,
        found: impl Fn(&Grid) -> bool,
    ) -> Result<String, String> {
        self.inner.screen(what, deadline, found)
    }

    /// The test hooks' channel, connected and answering, by `deadline`.
    pub fn channel(&mut self, deadline: Instant) -> Result<&mut Channel, String> {
        if self.channel.is_none() {
            let path = self
                .inner
                .hook()
                .map(Path::to_path_buf)
                .ok_or("this guest has no channel")?;
            let mut channel = Channel::connect(&path, deadline)?;
            channel.wait_ready(deadline)?;
            self.channel = Some(channel);
        }
        self.channel.as_mut().ok_or_else(|| "no channel".into())
    }

    /// Wait for QEMU to end by itself, by `deadline`: how long it took.
    pub fn wait_exit(&mut self, deadline: Instant) -> Result<Duration, String> {
        let took = self.inner.wait_exit(deadline)?;
        self.exited = true;
        Ok(took)
    }

    /// Pull the power: QEMU is killed.
    pub fn kill(&mut self) {
        self.inner.kill();
        self.exited = true;
        self.channel = None;
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
            timeout: Duration::from_secs(60),
        }
    }

    fn words(l: &driver::Launch) -> Vec<String> {
        driver::args(l)
            .into_iter()
            .map(|w| w.to_string_lossy().into_owned())
            .collect()
    }

    #[test]
    fn the_machine_s_switches_are_on_the_command_line() {
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
        let w = words(&launch_of(&launch(), &m, Path::new("/t")).unwrap());
        let has = |x: &str| w.iter().any(|a| a == x);
        assert!(has("virtio-blk-pci,drive=hd,serial=QSMCHD0"));
        assert!(has("-snapshot"));
        assert!(has("socket,id=hook,path=/t/hook.sock,server=on,wait=off"));
        assert!(has("chardev:hook"));
        assert!(has("type=11,value=qsmc.hook=churn"));
        assert!(has("virtio-net-pci,netdev=net0,mac=52:54:00:12:34:56"));
        // The capture's monitor.
        assert!(has("unix:/t/net.monitor.sock,server=on,wait=off"));
    }

    #[test]
    fn a_disk_that_is_kept_beside_one_that_is_not_is_copied() {
        let dir = std::env::temp_dir().join(format!("qsmc-session-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("a.img"), b"image").unwrap();
        let m = Machine {
            disks: vec![
                Disk {
                    path: dir.join("a.img"),
                    serial: "QSMCHD0".into(),
                    snapshot: true,
                },
                Disk {
                    path: dir.join("b.img"),
                    serial: "QSMCHD1".into(),
                    snapshot: false,
                },
            ],
            ..Machine::default()
        };
        let l = launch_of(&launch(), &m, &dir).unwrap();
        assert!(!l.snapshot);
        assert_eq!(l.disk, Some(dir.join("snapshot-0.img")));
        assert_eq!(std::fs::read(dir.join("snapshot-0.img")).unwrap(), b"image");
        assert_eq!(l.extra[0].path, dir.join("b.img"));
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
