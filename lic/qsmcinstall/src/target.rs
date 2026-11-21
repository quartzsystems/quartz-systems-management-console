// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! QS/MC as the base installer sees it: the machine's disks and links
//! from `base/inventory`, each with its location code, and the eight
//! steps of `base/install::write` for QS/MC's load source. The state
//! step is followed by the installer's log, copied for the console's own.

use std::fs;
use std::path::{Path, PathBuf};

use install::write::{self, STEP_NAMES, Writer};
use install::{Answers, DiskChoice, Step, Target};
use loadsource::boot::{self, EFIVARS};
use loadsource::payload::Payload;
use panels::forms::LinkChoice;
use refcode::RefCode;
use release::LOAD_SOURCE;
use rustix::mount::{MountFlags, UnmountFlags, mount, unmount};
use state::Host;

/// Where the new data file system is mounted while the log is copied.
const TARGET: &str = "/run/target";
/// The installer's log, copied to the new data file system.
pub const LOG: &str = "/run/install.log";

const fn step(index: usize, code: &RefCode) -> Step {
    Step {
        name: STEP_NAMES[index],
        code: code.code,
    }
}

/// The steps, each with the code it is done under.
pub const STEPS: &[Step] = &[
    step(0, &refcode::INSTALL_PARTITIONED),
    step(1, &refcode::INSTALL_PROTECTED),
    step(2, &refcode::INSTALL_FORMATTED),
    step(3, &refcode::INSTALL_SLOT_A),
    step(4, &refcode::INSTALL_SLOT_B),
    step(5, &refcode::INSTALL_RECORD),
    step(6, &refcode::INSTALL_BOOT),
    step(7, &refcode::INSTALL_STATE),
];

/// What is QS/MC's in writing its load source.
static PRODUCT: write::Product = write::Product {
    names: &LOAD_SOURCE,
    name: release::NAME,
    started: (refcode::INSTALL_STARTED.code, refcode::INSTALL_STARTED.text),
    boot_not_written: (
        refcode::INSTALL_BOOT_NOT_WRITTEN.code,
        refcode::INSTALL_BOOT_NOT_WRITTEN.text,
    ),
};

/// What the welcome says the installation does.
const WHAT: &[&str] = &[
    "This installs the management console on this machine. It erases the",
    "disks you choose and makes them its load source, then sets its host",
    "name, its open network (the one operators reach it on), the time and",
    "the first user of this local console. QS/VS hosts and their",
    "partitions are then managed from the console's web interface.",
];

/// Whether a disk of the inventory can be offered: not the media, not
/// the seed volume, not an optical drive or one of the kernel's own.
fn offered(name: &str, skip: &[String]) -> bool {
    !skip.iter().any(|s| s == name)
        && !["sr", "loop", "ram", "zram", "ublk"]
            .iter()
            .any(|p| name.starts_with(p))
}

/// The installer's log onto the first disk's data file system, which
/// the state step has just written (a second unit is written from the
/// first at the first IPL). It holds no password and no hash.
fn copy_log(answers: &Answers) -> Result<(), String> {
    let first = answers.disks.first().ok_or("no disk is chosen")?;
    let source = loadsource::disks::load_source(&LOAD_SOURCE, &first.name)
        .ok_or_else(|| format!("the disk at {} has no data partition", first.location))?;
    fs::create_dir_all(TARGET).map_err(|e| format!("{TARGET}: {e}"))?;
    let flags = MountFlags::NOSUID | MountFlags::NODEV | MountFlags::NOEXEC;
    mount(&source.data, TARGET, "xfs", flags, None)
        .map_err(|e| format!("{}: {e}", source.data.display()))?;
    let copied = (|| {
        let log = PathBuf::from(TARGET).join("log");
        fs::create_dir_all(&log).map_err(|e| e.to_string())?;
        match fs::read(LOG) {
            Ok(said) => fs::write(log.join("install.log"), said).map_err(|e| e.to_string()),
            Err(_) => Ok(()),
        }
    })();
    rustix::fs::sync();
    let unmounted = unmount(TARGET, UnmountFlags::empty()).map_err(|e| format!("{TARGET}: {e}"));
    copied.and(unmounted)
}

/// QS/MC, being installed.
pub struct Qsmc {
    /// The disks that hold the media and the answers.
    skip: Vec<String>,
    writer: Writer,
}

impl Qsmc {
    /// The installer over verified media; `skip` names the disks that
    /// hold the media and the answers.
    pub fn new(payload: Payload, skip: Vec<String>, log: Box<dyn FnMut(&str)>) -> Self {
        Self {
            skip,
            writer: Writer::new(&PRODUCT, payload, log),
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
        self.writer.step(index, answers)?;
        if index == STEPS.len() - 1 {
            copy_log(answers)?;
        }
        Ok(())
    }

    fn failed(&self) -> &'static str {
        refcode::INSTALL_FAILED.code
    }

    fn log(&mut self, line: &str) {
        self.writer.log(line);
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
