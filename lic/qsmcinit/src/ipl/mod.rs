// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! The console's IPL, in order. Each step posts its reference code:
//! the kernel's file systems and cgroup2, the load-source disks and the
//! A/B record, the storage service and `QSMCDATA` at `/data`, a restore
//! waiting for this IPL, the clock from the RTC, the seed, the state, the identity, the services. "Base
//! ready" is posted by the loop when every service has said so, and the
//! slot confirmed by the rule (`crate::slot::rule`). On installation
//! media the installer runs after the clock, in place of the rest.

#[cfg(target_os = "linux")]
mod choose;
#[cfg(target_os = "linux")]
mod clock;
pub mod cmdline;
#[cfg(target_os = "linux")]
mod install;
#[cfg(target_os = "linux")]
pub mod mounts;
pub mod restore;
pub mod state;

#[cfg(target_os = "linux")]
pub use on_console::run;

#[cfg(target_os = "linux")]
mod on_console {
    use std::os::unix::net::UnixListener;
    use std::path::{Path, PathBuf};
    use std::time::{Duration, Instant};

    use abrec::Slot;
    use appliance::time::Zone;
    use loadsource::disks::{self, LoadSource};
    use rustix::system::{RebootCommand, reboot};
    use state::Store;
    use supervise::run::Supervisor;
    use supervise::signal::{CHLD, INT, Signals, TERM};

    use super::choose::Chooser;
    use super::cmdline::{self, Source};
    use super::{clock, install, mounts, restore, state as state_step};
    use crate::console::Console;
    use crate::host::{Host, table};
    use crate::product::QSMC;

    /// Where the seed volume is mounted while it is read.
    const SEED_MOUNT: &str = "/run/seed";
    /// How long the load-source disks may take to appear: some
    /// controllers are probed after PID 1 has started.
    const DISK_WAIT: Duration = Duration::from_secs(10);
    /// How long the storage service may take to settle.
    const STORAGE_WAIT: Duration = Duration::from_secs(60);

    /// The record step. On installation media there is no slot and no
    /// record.
    fn record(host: &mut Host, source: Source, sources: &[LoadSource]) {
        let booted = match source {
            Source::SlotA => Slot::A,
            Source::SlotB => Slot::B,
            Source::Install | Source::Unknown => {
                host.line("A/B record not read: this IPL is from no slot");
                return;
            }
        };
        let mut posted = Vec::new();
        let slots = crate::slot::at_ipl(booted, sources, |code, detail| {
            posted.push((code, detail));
        });
        for (code, detail) in posted {
            if code.code == refcode::SLOT_FELL_BACK.code {
                let slot = slots
                    .state()
                    .fell_back
                    .chars()
                    .next()
                    .map(String::from)
                    .unwrap_or_default();
                host.post_about(code, &detail, &[refcode::event::slot(&slot)]);
            } else {
                host.post(code, &detail);
            }
        }
        host.slots = Some(slots);
    }

    /// The storage step: the control socket, then the service table
    /// with everything after the storage service held back, and the
    /// loop turned until that service is ready or will not be. A
    /// mirrored pair whose units each ran without the other is not
    /// assembled until the unit to keep is chosen at the console's one
    /// panel: nothing else starts meanwhile, for as long as it takes.
    fn storage(host: &mut Host) {
        let control = QSMC.control_socket();
        let _ = std::fs::remove_file(&control);
        let listener = UnixListener::bind(&control)
            .and_then(|l| l.set_nonblocking(true).map(|()| l))
            .map_err(|e| host.line(&format!("{}: {e}", control.display())))
            .ok();
        // A table that is not one is a failure of PID 1 itself.
        let mut sup = Supervisor::new(table::chosen(), Instant::now())
            .unwrap_or_else(|e| panic!("the service table: {e}"));
        sup.hold_after(table::STGD.order);
        host.quiet = true;
        host.start(sup, listener);
        let marker = stgd::choose_marker(&QSMC);
        let mut chooser = Chooser::new(marker.clone());
        let mut give_up = Instant::now() + STORAGE_WAIT;
        let mut waiting = false;
        while !host.storage_settled() {
            // Said while the serial port is still PID 1's.
            if !waiting && marker.exists() {
                waiting = true;
                host.line(
                    "The units of the load source ran apart: the IPL waits until the unit to keep is chosen",
                );
            }
            if chooser.wanted(host) {
                host.storage_waits();
                give_up = Instant::now() + STORAGE_WAIT;
            } else if Instant::now() >= give_up {
                break;
            }
            host.pump();
        }
        chooser.end(host);
        if waiting {
            host.line("The unit to keep was chosen: the IPL goes on");
        }
    }

    /// What `/data` is mounted from: what the storage service named,
    /// or, when it named nothing and the load source is not mirrored,
    /// the data partition itself.
    fn data_device(sources: &[LoadSource]) -> Result<PathBuf, String> {
        let named = std::fs::read_to_string(stgd::data_device(&QSMC))
            .ok()
            .map(|d| d.trim().to_string())
            .filter(|d| !d.is_empty());
        if let Some(device) = named {
            return Ok(PathBuf::from(device));
        }
        if disks::label(QSMC.load_source, sources).is_some() {
            return Err("the storage service does not serve the mirrored load source".into());
        }
        sources
            .first()
            .map(|s| s.data.clone())
            .ok_or_else(|| "no load-source disk was found".into())
    }

    /// The data step: `QSMCDATA` at `/data`, or state in memory. It is
    /// never reformatted.
    fn data(host: &mut Host, sources: &[LoadSource]) {
        let mounted = data_device(sources).and_then(|device| {
            mounts::data(&device, release::DATA_DIR)
                .map(|()| device.display().to_string())
                .map_err(|e| format!("{}: {e}", device.display()))
        });
        match mounted {
            Ok(device) => {
                host.post(&refcode::DATA_MOUNTED, &device);
                host.served = device.starts_with("/dev/ublkb");
                host.facts.data = device;
                host.data_mounted = true;
            }
            Err(e) => host.post(
                &refcode::DATA_NOT_MOUNTED,
                &format!("{e}; state is in memory only"),
            ),
        }
    }

    /// The restore step: a restore the web server left is put in place
    /// before anything reads state (`docs/BACKUP.md`).
    fn restore(host: &mut Host) {
        if !host.data_mounted {
            return;
        }
        match restore::at_ipl(Path::new(release::DATA_DIR), release::RELEASE) {
            restore::Outcome::Nothing => {}
            restore::Outcome::Applied(m) => host.post(
                &refcode::RESTORE_APPLIED,
                &format!(
                    "the backup of {} made {}",
                    m.console,
                    Zone::UTC.display(i64::try_from(m.made).unwrap_or(0))
                ),
            ),
            restore::Outcome::Refused(why) => host.post(&refcode::RESTORE_REFUSED, &why),
            restore::Outcome::TakenBack => host.post(&refcode::RESTORE_TAKEN_BACK, ""),
        }
    }

    fn clock(host: &mut Host) {
        match clock::set_from_rtc() {
            Ok(secs) => host.post(&refcode::CLOCK_SET, &Zone::UTC.display(secs)),
            Err(e) => host.post(&refcode::CLOCK_NOT_SET, &e),
        }
    }

    fn seed(host: &mut Host, store: &Store) {
        use seed::Outcome;
        match seed::run(QSMC.seed_label, Path::new(SEED_MOUNT), store) {
            Outcome::NotNeeded => host.post(&refcode::SEED_CHECKED, "the console is configured"),
            Outcome::Absent => host.post(&refcode::SEED_CHECKED, "no seed volume"),
            Outcome::Applied => host.post(&refcode::SEED_APPLIED, ""),
            Outcome::Invalid(reason) => host.post(&refcode::SEED_INVALID, &reason),
        }
    }

    fn state(host: &mut Host, store: &Store) {
        let loaded = state_step::load(store, QSMC.host_prefix);
        for refused in &loaded.refused {
            host.post(&refcode::STATE_INVALID, refused);
        }
        host.post(
            &refcode::STATE_LOADED,
            &format!("host {}", loaded.host_name),
        );
        host.facts.host = loaded.host_name;
        host.facts.zone = loaded.zone;
        host.facts.serial = loaded.serial;
        host.facts.machine = loaded.machine;
    }

    /// The console's identity: made at the first IPL that has a data
    /// file system to keep it on, and never again. It is the
    /// certificate the console presents to the systems it manages.
    fn identity(host: &mut Host) {
        if !host.data_mounted {
            return;
        }
        let made = identity::ensure(
            &QSMC.state_dir(),
            &host.facts.host,
            &host.facts.serial,
            appliance::time::now_secs(),
        );
        match made {
            Ok(true) => host.post(&refcode::IDENTITY_GENERATED, identity::CERT_FILE),
            Ok(false) => {}
            Err(e) => host.post(&refcode::IDENTITY_NOT_GENERATED, &e.to_string()),
        }
    }

    /// The services step: what waited for `/data` is started.
    fn services(host: &mut Host) {
        host.release_services();
        host.quiet = false;
        let names: Vec<&str> = table::chosen().iter().map(|s| s.name).collect();
        host.post(&refcode::SERVICES_STARTED, &names.join(", "));
    }

    /// The processor's line, and its code when it lacks something.
    fn processor(host: &mut Host) {
        let lacking = processor::lacking();
        host.line(&release::processor::line(&lacking));
        host.facts.processor = release::processor::fact(&lacking);
        if !lacking.is_empty() {
            host.post(&refcode::PROCESSOR_LACKS, &lacking.join(", "));
        }
    }

    /// Run the IPL, then the console, and never return: PID 1 must not
    /// exit.
    pub fn run() -> ! {
        crate::sys::panic::install();
        // Before anything is started: children are reaped and a
        // shutdown is asked for through these.
        let signals = Signals::take(&[CHLD, TERM, INT])
            .unwrap_or_else(|e| panic!("the signal descriptor: {e}"));
        // Ctrl-Alt-Del comes to PID 1 instead of resetting the machine.
        let _ = reboot(RebootCommand::CadOff);
        let failed = mounts::kernel(QSMC.run_dir);
        let mut host = Host::new(Console::open(), signals, &QSMC.log_socket());

        host.post(&refcode::IPL_STARTED, "");
        host.line(&release::banner());
        #[cfg(test_hooks)]
        host.line(crate::hooks::MARKER);
        let line = std::fs::read_to_string("/proc/cmdline").unwrap_or_default();
        let source = cmdline::parse(&line);
        host.line(&source.line());
        if source == Source::Install {
            host.line(cmdline::MEDIA_LINE);
        }
        host.facts.slot = source.name().to_string();
        if failed.is_empty() {
            host.post(&refcode::MOUNTED, "");
        } else {
            let what: Vec<String> = failed.iter().map(|(at, e)| format!("{at} ({e})")).collect();
            host.post(&refcode::NOT_MOUNTED, &what.join(", "));
        }
        processor(&mut host);
        if source == Source::Install {
            clock(&mut host);
            install::run(host);
        }
        let sources = disks::load_sources_within(QSMC.load_source, DISK_WAIT);
        record(&mut host, source, &sources);
        storage(&mut host);
        data(&mut host, &sources);
        restore(&mut host);
        clock(&mut host);
        let store = Store::at(QSMC.state_dir());
        seed(&mut host, &store);
        state(&mut host, &store);
        identity(&mut host);
        services(&mut host);
        host.run()
    }
}
