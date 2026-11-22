// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! The base system's suites, on the test image. Each boots the image
//! under QEMU and asserts on the serial console and, through the test
//! hooks' channel, on the control socket's status.

mod identity;
mod mirror;
pub mod network;
mod power;
pub(crate) mod slot;
mod status;
mod supervision;
pub(crate) mod time;

use std::cell::RefCell;
use std::ops::{Deref, DerefMut};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use refcode::RefCode;
use release::{fact, screen};

use crate::image::disks;
use crate::qemu::Launch;
use crate::qemu::session::{Disk, Machine, Session};

/// A suite: what it saw, or why it did not pass.
pub type SuiteFn = fn(&Run<'_>) -> Result<String, String>;

/// The suites, in the order of the handoff (Phase 0's are run before
/// them, on the release image).
pub const SUITES: &[(&str, SuiteFn)] = &[
    ("status", status::status),
    ("dhcp", network::dhcp),
    ("static-vlan", network::static_vlan),
    ("time", time::time),
    ("supervision", supervision::supervision),
    ("power-pull", power::power_pull),
    ("mirror", mirror::mirror),
    ("apart", mirror::apart),
    ("slot-confirm", slot::slot_confirm),
    ("shutdown", power::shutdown),
    ("identity", identity::identity),
    ("sockets", network::sockets),
    ("processor", status::processor),
];

/// The MAC of the guest's NIC, when a suite gives it one.
pub const MAC: &str = "52:54:00:12:34:56";
/// What QEMU's user network gives a guest by DHCP.
pub const DHCP_ADDRESS: &str = "10.0.2.15/24";
/// Its gateway.
pub const DHCP_GATEWAY: &str = "10.0.2.2";
/// The state directory on the guest.
pub const STATE_DIR: &str = "/data/state";

/// The test image a suite runs on.
#[derive(Clone, Debug)]
pub struct TestImage {
    /// The disk image (`build disk` of the test image).
    pub disk: PathBuf,
    /// Its slot A UKI.
    pub slot_a: PathBuf,
    /// Its slot B UKI.
    pub slot_b: PathBuf,
    /// Its installation ISO, when it is built: the installer's suites
    /// install the test image from it.
    pub iso: Option<PathBuf>,
    /// Its own update image, when it is built.
    pub update: Option<PathBuf>,
    /// The update images of its variants, when they are built
    /// (`--variant next`, `--variant broken`): what Phase 6's suites
    /// install.
    pub next: Option<PathBuf>,
    /// The variant whose web server cannot start.
    pub broken: Option<PathBuf>,
    /// The development key it is signed with, which it trusts: the
    /// refusals' images are made with it.
    pub key: Option<PathBuf>,
}

/// What a suite is run with.
pub struct Run<'a> {
    /// The suite's name.
    pub name: &'static str,
    /// The test image.
    pub image: &'a TestImage,
    /// Where scratch files go.
    pub work: &'a Path,
    /// How guests are launched.
    pub launch: &'a Launch,
    /// The serial logs of the suite's guests, for a failure.
    pub log: &'a RefCell<String>,
}

/// A running guest. When it goes, its serial log joins the suite's.
pub struct Guest<'a> {
    session: Session,
    log: &'a RefCell<String>,
}

impl Deref for Guest<'_> {
    type Target = Session;

    fn deref(&self) -> &Session {
        &self.session
    }
}

impl DerefMut for Guest<'_> {
    fn deref_mut(&mut self) -> &mut Session {
        &mut self.session
    }
}

impl Drop for Guest<'_> {
    fn drop(&mut self) {
        self.session.kill();
        let mut log = self.log.borrow_mut();
        log.push_str(self.session.log());
        log.push_str("\n----- the guest ended -----\n");
    }
}

impl<'a> Run<'a> {
    /// A scratch file of this suite.
    pub fn file(&self, name: &str) -> PathBuf {
        self.work.join(format!("{}-{name}", self.name))
    }

    /// A Unix socket of this suite: in the temporary directory, since a
    /// socket's path has room for 107 bytes.
    pub fn socket(&self, name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("qsmc-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        dir.join(format!("{}-{name}", self.name))
    }

    /// A fresh copy of the firmware's variable store, `name`.
    pub fn vars(&self, name: &str) -> Result<PathBuf, String> {
        let path = self.file(&format!("{name}-vars.fd"));
        std::fs::copy(&self.launch.vars, &path).map_err(|e| e.to_string())?;
        Ok(path)
    }

    /// The machine every suite starts from: the test image's disk as a
    /// snapshot, a variable store of its own, no NIC, and the channel.
    pub fn machine(&self) -> Result<Machine, String> {
        Ok(Machine {
            disks: vec![Disk {
                path: self.image.disk.clone(),
                serial: disks::serial(0),
                snapshot: true,
            }],
            vars: self.vars("boot")?,
            hook: Some(self.socket("hook.sock")),
            ..Machine::default()
        })
    }

    /// A copy of the test image's disk the guest writes to: `name`.
    pub fn disk_copy(&self, name: &str) -> Result<Disk, String> {
        let path = self.file(&format!("{name}.img"));
        crate::image::sparse_copy(&self.image.disk, &path)?;
        Ok(Disk {
            path,
            serial: disks::serial(0),
            snapshot: false,
        })
    }

    /// A mirrored load source of the test image's UKIs, written with
    /// the base crate's writer: its two disks, `name`.
    pub fn mirrored(&self, name: &str) -> Result<[Disk; 2], String> {
        let paths = [
            self.file(&format!("{name}-0.img")),
            self.file(&format!("{name}-1.img")),
        ];
        let read = |p: &Path| std::fs::read(p).map_err(|e| format!("{}: {e}", p.display()));
        disks::write(
            &paths,
            loadsource::layout::DEFAULT_DISK_BYTES,
            &read(&self.image.slot_a)?,
            &read(&self.image.slot_b)?,
            appliance::time::now_secs(),
        )?;
        let [a, b] = paths;
        Ok([
            Disk {
                path: a,
                serial: disks::serial(0),
                snapshot: false,
            },
            Disk {
                path: b,
                serial: disks::serial(1),
                snapshot: false,
            },
        ])
    }

    /// Start a guest.
    pub fn boot(&self, machine: &Machine) -> Result<Guest<'a>, String> {
        let session = Session::spawn(self.launch, machine, self.work)?;
        Ok(Guest {
            session,
            log: self.log,
        })
    }

    /// When a boot started now must be done.
    pub fn deadline(&self) -> Instant {
        Instant::now() + self.launch.timeout
    }

    /// When a boot of a mirrored load source with a unit missing must be
    /// done: the storage service waits for the missing disk, ten seconds
    /// each time it looks (`base/stgd`), so it is given twice as long.
    pub fn deadline_degraded(&self) -> Instant {
        Instant::now() + self.launch.timeout * 2
    }
}

/// The line a code is posted as, with what the posting adds.
pub fn posted(code: &RefCode, detail: &str) -> String {
    format!("{} {}", code.tag(), code.text_with(detail))
}

/// Wait for "Base ready": the seconds since the kernel started that
/// PID 1 posted with it.
pub fn base_ready(guest: &mut Session, deadline: Instant) -> Result<f64, String> {
    guest.expect(&format!("{}: ", posted(&refcode::BASE_READY, "")), deadline)?;
    let rest = guest.rest_of_line(deadline)?;
    rest.split_ascii_whitespace()
        .next()
        .and_then(|secs| secs.parse().ok())
        .ok_or_else(|| format!("`{rest}` does not start with the seconds since the kernel started"))
}

/// Boot to "Base ready" and connect the channel.
pub fn booted<'a>(run: &Run<'a>, machine: &Machine) -> Result<Guest<'a>, String> {
    let mut guest = run.boot(machine)?;
    let deadline = run.deadline();
    base_ready(&mut guest, deadline)?;
    guest.channel(deadline)?;
    Ok(guest)
}

/// [`booted`] for a mirrored load source with a unit missing.
pub fn booted_degraded<'a>(run: &Run<'a>, machine: &Machine) -> Result<Guest<'a>, String> {
    let mut guest = run.boot(machine)?;
    let deadline = run.deadline_degraded();
    base_ready(&mut guest, deadline)?;
    guest.channel(deadline)?;
    Ok(guest)
}

/// The guest's channel, already connected.
pub fn channel(guest: &mut Session) -> Result<&mut crate::qemu::channel::Channel, String> {
    guest.channel(Instant::now() + Duration::from_secs(10))
}

/// Whether `log` has a whole status screen with every one of `rows`.
pub fn has_screen(log: &str, rows: &[String]) -> bool {
    log.split(screen::BEGIN).skip(1).any(|after| {
        after
            .split_once(screen::END)
            .is_some_and(|(block, _)| rows.iter().all(|row| block.contains(row.as_str())))
    })
}

/// Wait for a status screen on the serial port that has `rows`.
pub fn screen_with(guest: &mut Session, rows: &[String], deadline: Instant) -> Result<(), String> {
    let what = format!("a status screen with {rows:?}");
    guest.wait_for(&what, deadline, |log| has_screen(log, rows))
}

/// The value of a fact, or that the status lacks it.
pub fn fact_of<'s>(
    status: &'s supervise::control::HostStatus,
    key: &str,
) -> Result<&'s str, String> {
    status
        .fact(key)
        .ok_or_else(|| format!("the status has no `{key}`"))
}

/// The console is up and whole: ready, and every service ready.
pub fn all_ready(status: &supervise::control::HostStatus) -> Result<(), String> {
    if fact_of(status, fact::READY)? != "yes" {
        return Err("the status says the base is not ready".into());
    }
    match status.services.iter().find(|s| s.state != "ready") {
        Some(s) => Err(format!("service {} is {}", s.name, s.state)),
        None => Ok(()),
    }
}

/// `seed.toml` with the tables given.
pub fn seed_toml(host: &str, network: &str) -> String {
    format!("schema_version = 1\n\n[host]\n{host}\n[network]\n{network}")
}

/// A seed volume of `text` as a further disk.
pub fn seed_disk(run: &Run<'_>, text: &str) -> Result<Disk, String> {
    let path = run.file("seed.img");
    crate::image::seed_volume(&path, release::SEED_LABEL, text)?;
    Ok(Disk {
        path,
        serial: "QSMCSEED0".into(),
        snapshot: true,
    })
}

/// Power the guest off through the control socket and wait for QEMU.
pub fn power_off(guest: &mut Session) -> Result<(), String> {
    channel(guest)?.tell(qsmchook::proto::command::POWER_OFF)?;
    guest
        .wait_exit(Instant::now() + Duration::from_secs(30))
        .map(|_| ())
}
