// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! The suites. Phase 0's steps ([`steps`]) boot the release image: within
//! the boot timeout each boot must show, in the order PID 1 prints them,
//! `REF C3001000`, the banner, where the IPL came from, the processor's
//! line and heartbeats. Phase 1's suites ([`base`]) boot the test image
//! and reach its control socket through the test hooks' channel.
//!
//! - `disk`: Phase 0's three steps (slot A, slot B by its firmware boot
//!   entry, a processor without what the cryptography needs), then the
//!   base system's suites, then Phase 2's ([`install`]): the installer
//!   from the test image's ISO and the local console of what it
//!   installed.
//! - `iso`: the installation media, to the installer's first screen.

pub mod base;
pub mod install;
pub mod mgmt;
pub(crate) mod steps;
pub mod web;

use std::cell::RefCell;
use std::fmt;
use std::fs;
use std::path::PathBuf;
use std::str::FromStr;
use std::time::{Duration, Instant};

use crate::error::HarnessError;
use crate::qemu::Launch;
pub use base::TestImage;

/// What `build test` boots.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Media {
    /// The disk image.
    Disk,
    /// The installation ISO.
    Iso,
}

impl FromStr for Media {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, String> {
        match s {
            "disk" => Ok(Media::Disk),
            "iso" => Ok(Media::Iso),
            other => Err(format!("`{other}` is not a medium: disk or iso")),
        }
    }
}

impl fmt::Display for Media {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Media::Disk => "disk",
            Media::Iso => "iso",
        })
    }
}

/// The images built, when they are.
#[derive(Clone, Debug, Default)]
pub struct Images {
    /// The release image's disk.
    pub disk: Option<PathBuf>,
    /// The release image's installation ISO.
    pub iso: Option<PathBuf>,
    /// The test image.
    pub test: Option<TestImage>,
}

/// One step that passed.
#[derive(Clone, Debug)]
pub struct Report {
    /// The step.
    pub name: &'static str,
    /// How long it took.
    pub elapsed: Duration,
    /// What it showed.
    pub detail: String,
}

/// The suite over the images.
pub struct Suite {
    images: Images,
    work: PathBuf,
    launch: Launch,
}

/// Every step and suite `media` runs, by name, in order.
pub fn names(media: Media) -> Vec<&'static str> {
    match media {
        Media::Disk => steps::DISK
            .iter()
            .map(steps::Step::name)
            .chain(base::SUITES.iter().map(|(name, _)| *name))
            .chain(install::SUITES.iter().map(|(name, _)| *name))
            .chain(web::SUITES.iter().map(|(name, _)| *name))
            .chain(mgmt::SUITES.iter().map(|(name, _)| *name))
            .collect(),
        Media::Iso => steps::ISO.iter().map(steps::Step::name).collect(),
    }
}

impl Suite {
    /// The suite, with its working files under `work`.
    pub fn new(images: Images, work: PathBuf) -> Result<Self, HarnessError> {
        fs::create_dir_all(&work)?;
        Ok(Self {
            images,
            work,
            launch: Launch::detect()?,
        })
    }

    /// How QEMU is run.
    pub fn launch(&self) -> &Launch {
        &self.launch
    }

    /// One suite of the base system.
    fn base(&self, name: &'static str, suite: base::SuiteFn) -> Result<Report, HarnessError> {
        let image = self.images.test.as_ref().ok_or(HarnessError::NotBuilt {
            what: "the test image (`all --test-image`)",
        })?;
        let log = RefCell::new(String::new());
        let run = base::Run {
            name,
            image,
            work: &self.work,
            launch: &self.launch,
            log: &log,
        };
        let started = Instant::now();
        let outcome = suite(&run);
        let log = log.into_inner();
        fs::write(self.work.join(format!("{name}.log")), &log)?;
        match outcome {
            Ok(detail) => Ok(Report {
                name,
                elapsed: started.elapsed(),
                detail,
            }),
            Err(reason) => Err(HarnessError::Suite {
                suite: name,
                reason,
                log,
            }),
        }
    }

    /// Run the steps and suites of `media`, or only the one named
    /// `only`, calling `passed` after each.
    pub fn run(
        &self,
        media: Media,
        only: Option<&str>,
        mut passed: impl FnMut(&Report),
    ) -> Result<Vec<Report>, HarnessError> {
        if let Some(name) = only
            && !names(media).contains(&name)
        {
            return Err(HarnessError::NoSuchSuite(name.to_string()));
        }
        let wanted = |name: &str| only.is_none_or(|o| o == name);
        let phase0: &[steps::Step] = match media {
            Media::Disk => &steps::DISK,
            Media::Iso => &steps::ISO,
        };
        let mut reports = Vec::new();
        for step in phase0.iter().filter(|s| wanted(s.name())) {
            let report = step.run(self)?;
            passed(&report);
            reports.push(report);
        }
        if media == Media::Disk {
            let all = base::SUITES
                .iter()
                .chain(install::SUITES)
                .chain(web::SUITES)
                .chain(mgmt::SUITES);
            // A suite run by name only, when it is named.
            let named = mgmt::NAMED.iter().filter(|(n, _)| only == Some(*n));
            for (name, suite) in all.filter(|(n, _)| wanted(n)).chain(named) {
                let report = self.base(name, *suite)?;
                passed(&report);
                reports.push(report);
            }
        }
        Ok(reports)
    }
}
