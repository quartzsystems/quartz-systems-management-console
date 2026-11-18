// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! Phase 0's suite. Within the boot timeout each boot must show, in the
//! order PID 1 prints them: `REF C3001000`, the banner with this
//! repository's release, where the IPL came from, the processor's line
//! and heartbeats.
//!
//! - `disk`: slot A, with two heartbeats; slot B by its firmware boot
//!   entry; slot A again on a processor without what the cryptography
//!   needs, which says so and runs on.
//! - `iso`: the installation media, with two heartbeats.

mod steps;

use std::fmt;
use std::fs;
use std::path::PathBuf;
use std::str::FromStr;
use std::time::Duration;

use crate::error::HarnessError;
use crate::qemu::Launch;

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
    /// The disk image.
    pub disk: Option<PathBuf>,
    /// The installation ISO.
    pub iso: Option<PathBuf>,
}

/// One step that passed.
#[derive(Clone, Debug)]
pub struct Report {
    /// The step.
    pub name: &'static str,
    /// From QEMU's start to the last line expected.
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

    /// Run the steps of `media`, calling `passed` after each.
    pub fn run(
        &self,
        media: Media,
        mut passed: impl FnMut(&Report),
    ) -> Result<Vec<Report>, HarnessError> {
        let steps: &[steps::Step] = match media {
            Media::Disk => &steps::DISK,
            Media::Iso => &steps::ISO,
        };
        let mut reports = Vec::new();
        for step in steps {
            let report = step.run(self)?;
            passed(&report);
            reports.push(report);
        }
        Ok(reports)
    }
}
