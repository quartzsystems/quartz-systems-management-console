// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! The steps: what each boots, on which processor, and what it must
//! show.

use std::fs::{self, File};
use std::path::{Path, PathBuf};

use diskimg::efi::{self, EspLocation};
use diskimg::layout::ESP_TYPE_GUID;
use release::{KEY_PREFIX, NAME, RELEASE};

use super::{Report, Suite};
use crate::error::HarnessError;
use crate::qemu::{self, Boot, Drive, vars};

/// The processor model without what the cryptography needs: QEMU's
/// plain 64-bit one has no AVX2, ADX, AES or PCLMULQDQ.
pub const LACKING_CPU: &str = "qemu64";

/// Where a step boots from.
#[derive(Clone, Copy)]
enum From {
    SlotA,
    SlotB,
    Media,
}

/// One step.
pub struct Step {
    name: &'static str,
    from: From,
    cpu: Option<&'static str>,
    heartbeats: usize,
}

/// The disk image's steps.
pub const DISK: [Step; 3] = [
    Step {
        name: "ipl-slot-a",
        from: From::SlotA,
        cpu: None,
        heartbeats: 2,
    },
    Step {
        name: "ipl-slot-b",
        from: From::SlotB,
        cpu: None,
        heartbeats: 1,
    },
    Step {
        name: "processor-lacking",
        from: From::SlotA,
        cpu: Some(LACKING_CPU),
        heartbeats: 1,
    },
];

/// The ISO's step.
pub const ISO: [Step; 1] = [Step {
    name: "ipl-media",
    from: From::Media,
    cpu: None,
    heartbeats: 2,
}];

/// The processor's line on a processor with what is needed, and on one
/// without it.
const HAS: &str = "Processor has what the cryptography needs";
const LACKS: &str = "Processor lacks ";
/// What the media UKI says after its slot line.
const MEDIA: &str = "The installer arrives in Phase 2";

fn setup(step: &'static str, reason: &dyn std::fmt::Display) -> HarnessError {
    HarnessError::Setup {
        step,
        reason: reason.to_string(),
    }
}

/// Where the ESP is on the image, by its table.
fn esp(step: &'static str, image: &Path) -> Result<EspLocation, HarnessError> {
    let mut file = File::open(image)?;
    let entry = loadsource::gpt::entries(&mut file)?
        .into_iter()
        .find(|e| e.type_guid == ESP_TYPE_GUID)
        .ok_or_else(|| setup(step, &"the image has no ESP"))?;
    Ok(EspLocation {
        index: entry.index,
        first_lba: entry.first_lba,
        sectors: entry.last_lba + 1 - entry.first_lba,
        guid: entry.guid,
    })
}

impl Step {
    /// The lines the serial port must show, in order.
    fn expect(&self) -> Vec<String> {
        let mut e = vec![refcode::IPL_STARTED.tag(), format!("{NAME} {RELEASE} ")];
        match self.from {
            From::SlotA => e.push(format!("IPL slot A ({KEY_PREFIX}slot=A)")),
            From::SlotB => e.push(format!("IPL slot B ({KEY_PREFIX}slot=B)")),
            From::Media => {
                e.push(format!(
                    "IPL from installation media ({KEY_PREFIX}install=1)"
                ));
                e.push(MEDIA.to_string());
            }
        }
        e.push(refcode::MOUNTED.tag());
        if self.cpu == Some(LACKING_CPU) {
            e.push(LACKS.to_string());
            e.push(refcode::PROCESSOR_LACKS.tag());
        } else {
            e.push(HAS.to_string());
        }
        e.extend((0..self.heartbeats).map(|_| refcode::HEARTBEAT.tag()));
        e
    }

    /// This step's copy of the variable store: as OVMF ships it, or for
    /// slot B with a boot entry for `EFI/QSMC/B.EFI` first.
    fn vars(&self, suite: &Suite, image: &Path) -> Result<PathBuf, HarnessError> {
        let template = fs::read(&suite.launch.vars)?;
        let store = match self.from {
            From::SlotB => {
                let option = efi::load_option(
                    &format!("{NAME} slot B"),
                    &esp(self.name, image)?,
                    release::names::SLOT_B_PATH,
                );
                vars::with_boot_entry(&template, &option).map_err(|e| setup(self.name, &e))?
            }
            From::SlotA | From::Media => template,
        };
        let path = suite.work.join(format!("{}-vars.fd", self.name));
        fs::write(&path, store)?;
        Ok(path)
    }

    /// Boot and check.
    pub fn run(&self, suite: &Suite) -> Result<Report, HarnessError> {
        let (image, drive) = match self.from {
            From::SlotA | From::SlotB => {
                let disk = suite.images.disk.clone().ok_or(HarnessError::NotBuilt {
                    what: "the disk image",
                })?;
                (disk.clone(), Drive::Disk(disk))
            }
            From::Media => {
                let iso = suite
                    .images
                    .iso
                    .clone()
                    .ok_or(HarnessError::NotBuilt { what: "the ISO" })?;
                (iso.clone(), Drive::Cdrom(iso))
            }
        };
        let boot = Boot {
            vars: self.vars(suite, &image)?,
            drive,
            cpu: self.cpu,
            expect: self.expect(),
        };
        let (elapsed, log) = qemu::boot(&suite.launch, self.name, &boot)?;
        fs::write(suite.work.join(format!("{}.log", self.name)), &log)?;
        Ok(Report {
            name: self.name,
            elapsed,
            detail: shown(&log, &boot.expect),
        })
    }
}

/// The lines of `log` that hold what was expected, joined.
fn shown(log: &str, expect: &[String]) -> String {
    let mut lines: Vec<&str> = Vec::new();
    for line in log.lines().map(str::trim) {
        let wanted = expect.iter().any(|e| line.contains(e.as_str()));
        if wanted && !lines.contains(&line) && line != refcode::HEARTBEAT.line() {
            lines.push(line);
        }
    }
    let beats = log.matches(&refcode::HEARTBEAT.tag()).count();
    format!("{}; {beats} heartbeat(s)", lines.join(" | "))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_step_expects_the_banner_the_slot_and_the_processor() {
        let a = DISK[0].expect();
        assert_eq!(a[0], "REF C3001000");
        assert!(a[1].starts_with("QS/MC V"));
        assert_eq!(a[2], "IPL slot A (qsmc.slot=A)");
        assert_eq!(a.iter().filter(|l| *l == "REF C3001005").count(), 2);
        assert!(
            DISK[1]
                .expect()
                .contains(&"IPL slot B (qsmc.slot=B)".to_string())
        );
        let lacking = DISK[2].expect();
        assert!(lacking.contains(&"REF A3001015".to_string()));
        assert!(!lacking.contains(&HAS.to_string()));
        assert!(
            ISO[0]
                .expect()
                .contains(&"IPL from installation media (qsmc.install=1)".to_string())
        );
    }
}
