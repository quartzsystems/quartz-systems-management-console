// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! QS/MC as the base installer sees it: the machine's disks and links
//! from `base/inventory`, each with its location code, and the steps
//! that write a load source with `base/loadsource`. Every step starts
//! from what the one before left, and the first writes each disk's
//! table afresh, so an installation cut off anywhere is started over
//! from the beginning.

use std::fs::{self, File, OpenOptions};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use abrec::Slot;
use install::{Answers, DiskChoice, Step, Target};
use loadsource::boot::{self, EFIVARS};
use loadsource::disks::{self, LoadSource};
use loadsource::layout::SLOT_A_PATH;
use loadsource::record::{self, HostRecord};
use loadsource::set::{BLOCK, Member, MemberState, SetLabel};
use loadsource::{prepare, sys};
use panels::forms::LinkChoice;
use refcode::RefCode;
use release::LOAD_SOURCE;
use release::names::SLOT_B_PATH;
use rustix::mount::{MountFlags, UnmountFlags, mount, unmount};
use state::{Host, Store};
use uuid::Uuid;

use crate::media::Payload;

/// Where the new data file system is mounted while state is written.
const TARGET: &str = "/run/target";
/// The installer's log, copied to the new data file system.
pub const LOG: &str = "/run/install.log";

const fn step(name: &'static str, code: &RefCode) -> Step {
    Step {
        name,
        code: code.code,
    }
}

/// The steps, each with the code it is done under.
pub const STEPS: &[Step] = &[
    step("Partitioning the disks", &refcode::INSTALL_PARTITIONED),
    step("Setting up protection", &refcode::INSTALL_PROTECTED),
    step("Making the data file system", &refcode::INSTALL_FORMATTED),
    step("Copying slot A", &refcode::INSTALL_SLOT_A),
    step("Copying slot B", &refcode::INSTALL_SLOT_B),
    step("Writing the A/B record", &refcode::INSTALL_RECORD),
    step("Writing boot entries", &refcode::INSTALL_BOOT),
    step("Writing state", &refcode::INSTALL_STATE),
];

/// What the welcome says the installation does.
const WHAT: &[&str] = &[
    "This installs the management console on this machine. It erases the",
    "disks you choose and makes them its load source, then sets its host",
    "name, its open network (the one operators reach it on), the time and",
    "the first user of this local console. QS/VS hosts and their",
    "partitions are then managed from the console's web interface.",
];

fn text(e: impl std::fmt::Display) -> String {
    e.to_string()
}

/// Sixteen random bytes from the kernel.
fn random() -> Result<[u8; 16], String> {
    let mut bytes = [0u8; 16];
    File::open("/dev/urandom")
        .and_then(|mut f| f.read_exact(&mut bytes))
        .map_err(|e| format!("/dev/urandom: {e}"))?;
    Ok(bytes)
}

/// Whether a disk of the inventory can be offered: not the media, not
/// the seed volume, not an optical drive or one of the kernel's own.
fn offered(name: &str, skip: &[String]) -> bool {
    !skip.iter().any(|s| s == name)
        && !["sr", "loop", "ram", "zram", "ublk"]
            .iter()
            .any(|p| name.starts_with(p))
}

/// QS/MC, being installed.
pub struct Qsmc {
    payload: Payload,
    /// The disks that hold the media and the answers.
    skip: Vec<String>,
    /// The chosen disks as load sources, once partitioned.
    sources: Vec<LoadSource>,
    log: Box<dyn FnMut(&str)>,
}

impl Qsmc {
    /// The installer over checked media; `skip` names the disks that
    /// hold the media and the answers.
    pub fn new(payload: Payload, skip: Vec<String>, log: Box<dyn FnMut(&str)>) -> Self {
        Self {
            payload,
            skip,
            sources: Vec::new(),
            log,
        }
    }

    /// Bring every Ethernet link up, so its carrier can be shown.
    pub fn links_up() {
        let Ok(mut rtnl) = netlink::route::Rtnl::open() else {
            return;
        };
        for link in appliance::links::ethernet() {
            let _ = rtnl.set_up(link.index, true);
        }
    }

    fn say(&mut self, code: &RefCode, detail: &str) {
        (self.log)(&format!("{} {}", code.code, code.text_with(detail)));
    }

    fn partition(&mut self, answers: &Answers) -> Result<(), String> {
        self.sources.clear();
        let names: Vec<&str> = answers.disks.iter().map(|d| d.location.as_str()).collect();
        self.say(&refcode::INSTALL_STARTED, &names.join(", "));
        // The ESP is made empty: the slots are copied by their own steps.
        let esp = prepare::esp_image(&LOAD_SOURCE, &[], &[])?;
        for disk in &answers.disks {
            let node = Path::new("/dev").join(&disk.name);
            let mut file = OpenOptions::new()
                .read(true)
                .write(true)
                .open(&node)
                .map_err(|e| format!("{}: {e}", node.display()))?;
            let guids = prepare::guids_from(&[random()?, random()?, random()?, random()?]);
            prepare::write_table(
                &LOAD_SOURCE,
                &mut file,
                disks::disk_bytes(&disk.name),
                &esp,
                &guids,
                &mut |_, _| {},
            )
            .map_err(|e| format!("disk at {}: {e}", disk.location))?;
            file.sync_all().map_err(text)?;
            sys::reread_partitions(&file)
                .map_err(|e| format!("disk at {}: its table was not read: {e}", disk.location))?;
        }
        let give_up = Instant::now() + Duration::from_secs(15);
        for disk in &answers.disks {
            loop {
                if let Some(source) = disks::load_source(&LOAD_SOURCE, &disk.name) {
                    self.sources.push(source);
                    break;
                }
                if Instant::now() > give_up {
                    return Err(format!(
                        "the partitions of the disk at {} did not appear",
                        disk.location
                    ));
                }
                std::thread::sleep(Duration::from_millis(50));
            }
        }
        Ok(())
    }

    /// The data file system's size: the data partition's, or the
    /// smaller of the two, in whole blocks.
    fn data_bytes(&self) -> u64 {
        let least = self.sources.iter().map(|s| s.data_bytes).min().unwrap_or(0);
        least / BLOCK * BLOCK
    }

    fn protect(&mut self, answers: &Answers) -> Result<(), String> {
        if !answers.mirrored() {
            return Ok(());
        }
        let member = |i: usize, state| Member {
            partition: self.sources[i].data_guid,
            state,
            serial: answers.disks[i].serial.clone(),
            location: answers.disks[i].location.clone(),
        };
        // The first IPL writes the second unit from the first: nothing
        // says the two disks hold the same bytes now.
        let label = SetLabel {
            id: Uuid::from_bytes(random()?),
            blocks: self.data_bytes() / BLOCK,
            clean: true,
            written: u64::try_from(appliance::time::now_secs()).unwrap_or(0),
            members: vec![
                member(0, MemberState::Active),
                member(1, MemberState::Resuming),
            ],
        };
        disks::labels(&LOAD_SOURCE, &self.sources)
            .map_err(text)?
            .write(&label.encode())
            .map(|_| ())
            .map_err(text)
    }

    fn format(&mut self) -> Result<(), String> {
        let first = self.sources.first().ok_or("no disk is partitioned")?;
        let mut data = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&first.data)
            .map_err(|e| format!("{}: {e}", first.data.display()))?;
        prepare::format_data(
            &LOAD_SOURCE,
            &mut data,
            self.data_bytes(),
            Uuid::from_bytes(random()?),
            appliance::time::now_secs(),
        )?;
        data.sync_all().map_err(text)
    }

    /// Write a slot's UKI to every disk's ESP and read it back.
    fn copy(&self, path: &str, uki: &[u8]) -> Result<(), String> {
        for source in &self.sources {
            let open = || {
                OpenOptions::new()
                    .read(true)
                    .write(true)
                    .open(&source.esp)
                    .map_err(|e| format!("{}: {e}", source.esp.display()))
            };
            let esp = open()?;
            diskimg::fat::write_file(&esp, path, uki)
                .map_err(|e| format!("{}: {e}", source.esp.display()))?;
            esp.sync_all().map_err(text)?;
            let back = diskimg::fat::read_file_at(open()?, path)
                .map_err(|e| format!("{}: {e}", source.esp.display()))?;
            if back != uki {
                return Err(format!(
                    "{path} on {} does not read back as written",
                    source.esp.display()
                ));
            }
        }
        Ok(())
    }

    fn record(&mut self) -> Result<(), String> {
        let mut records = disks::records(&LOAD_SOURCE, &self.sources).map_err(text)?;
        let (_, written) = record::write_all(&mut records, HostRecord::INITIAL).map_err(text)?;
        if written == self.sources.len() {
            Ok(())
        } else {
            Err(format!(
                "the record was written to {written} of {} disks",
                self.sources.len()
            ))
        }
    }

    /// A firmware that keeps no entries still starts a disk by its
    /// default path, so a refusal does not fail the step.
    fn boot_entries(&mut self) {
        let esps: Vec<_> = self.sources.iter().map(|s| s.esp_at).collect();
        let made = boot::make_default(
            Path::new(EFIVARS),
            &LOAD_SOURCE,
            release::NAME,
            &esps,
            Slot::A,
        );
        if let Err(e) = made {
            self.say(&refcode::INSTALL_BOOT_NOT_WRITTEN, &e.clone());
        }
    }

    fn state(&mut self, answers: &Answers) -> Result<(), String> {
        if let Some(clock) = answers.time.clock {
            set_clock(clock)?;
        }
        let first = self.sources.first().ok_or("no disk is partitioned")?;
        fs::create_dir_all(TARGET).map_err(|e| format!("{TARGET}: {e}"))?;
        let flags = MountFlags::NOSUID | MountFlags::NODEV | MountFlags::NOEXEC;
        mount(&first.data, TARGET, "xfs", flags, None)
            .map_err(|e| format!("{}: {e}", first.data.display()))?;
        let written = (|| {
            let dir = PathBuf::from(TARGET).join("state");
            fs::create_dir_all(&dir).map_err(text)?;
            let store = Store::at(&dir);
            store.save(&answers.network).map_err(text)?;
            let now = u64::try_from(appliance::time::now_secs()).unwrap_or(0);
            let users = users::first(&answers.user_id, &answers.password_hash, now)?;
            state::atomic_write(&dir.join(users::FILE), &users).map_err(text)?;
            // The installer's log, for the console's own; it holds no
            // password and no hash.
            let log = PathBuf::from(TARGET).join("log");
            fs::create_dir_all(&log).map_err(text)?;
            if let Ok(said) = fs::read(LOG) {
                fs::write(log.join("install.log"), said).map_err(text)?;
            }
            // `host.toml` last: it says the console is configured.
            store
                .save(&Host {
                    name: Some(answers.host.clone()),
                    time_zone: answers.time.zone.clone(),
                    ntp_servers: answers.time.servers.clone(),
                })
                .map_err(text)
        })();
        rustix::fs::sync();
        let unmounted =
            unmount(TARGET, UnmountFlags::empty()).map_err(|e| format!("{TARGET}: {e}"));
        written.and(unmounted)
    }
}

/// Set the system clock, and the RTC from it.
fn set_clock(secs: i64) -> Result<(), String> {
    use rustix::time::{ClockId, Timespec, clock_settime};
    clock_settime(
        ClockId::Realtime,
        Timespec {
            tv_sec: secs,
            tv_nsec: 0,
        },
    )
    .map_err(|e| format!("the clock was not set: {e}"))?;
    // A machine without an RTC keeps the time until it powers off.
    let _ = netd::sys::clock::write_rtc();
    Ok(())
}

impl Target for Qsmc {
    fn product(&self) -> &'static str {
        release::NAME
    }

    fn banner(&self) -> String {
        format!("{} ({})", release::banner(), release::PRODUCT)
    }

    fn what(&self) -> &'static [&'static str] {
        WHAT
    }

    fn disks(&mut self) -> Vec<DiskChoice> {
        inventory::disks(&inventory::Roots::default())
            .into_iter()
            .filter(|d| offered(&d.name, &self.skip))
            .map(DiskChoice::from)
            .collect()
    }

    fn min_disk_bytes(&self) -> u64 {
        release::MIN_DISK_BYTES.max(loadsource::layout::min_disk_bytes(&LOAD_SOURCE))
    }

    fn links(&mut self) -> Vec<LinkChoice> {
        inventory::interfaces(&inventory::Roots::default())
            .into_iter()
            .map(LinkChoice::from)
            .collect()
    }

    fn default_host(&self) -> String {
        let first = appliance::links::ethernet().first().map(|l| l.mac);
        Host::default_name(release::HOST_PREFIX, first)
    }

    fn now(&self) -> i64 {
        appliance::time::now_secs()
    }

    fn steps(&self) -> &'static [Step] {
        STEPS
    }

    fn step(&mut self, index: usize, answers: &Answers) -> Result<(), String> {
        match index {
            0 => self.partition(answers),
            1 => self.protect(answers),
            2 => self.format(),
            3 => self.copy(SLOT_A_PATH, &self.payload.slot_a),
            4 => self.copy(SLOT_B_PATH, &self.payload.slot_b),
            5 => self.record(),
            6 => {
                self.boot_entries();
                Ok(())
            }
            7 => self.state(answers),
            _ => Err(format!("there is no step {index}")),
        }
    }

    fn failed(&self) -> &'static str {
        refcode::INSTALL_FAILED.code
    }

    fn log(&mut self, line: &str) {
        (self.log)(line);
    }

    fn secure_boot(&self) -> Option<bool> {
        boot::secure_boot(Path::new(EFIVARS))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_steps_are_eight_each_with_its_code_in_order() {
        assert_eq!(STEPS.len(), 8);
        for (i, s) in STEPS.iter().enumerate() {
            assert!(s.code.starts_with("C3004"), "{}", s.code);
            assert!(refcode::find(s.code).is_some(), "{}", s.code);
            assert!(STEPS[..i].iter().all(|o| o.code < s.code));
        }
        assert_eq!(STEPS[0].name, "Partitioning the disks");
        assert_eq!(STEPS[7].name, "Writing state");
    }

    #[test]
    fn the_media_the_seed_and_the_kernel_s_own_disks_are_not_offered() {
        let skip = vec!["sdb".to_string()];
        assert!(offered("vda", &skip));
        assert!(offered("nvme0n1", &skip));
        for name in ["sdb", "sr0", "loop0", "ublkb0", "zram0"] {
            assert!(!offered(name, &skip), "{name}");
        }
    }

    #[test]
    fn the_welcome_fits_the_screen_and_names_no_other_product_s_host() {
        assert!(WHAT.iter().all(|l| l.len() <= 74));
        assert!(WHAT.iter().all(|l| !l.contains("hypervisor")));
    }
}
