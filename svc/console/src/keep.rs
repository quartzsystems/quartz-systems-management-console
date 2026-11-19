// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! The console as PID 1 runs it when the units of the mirrored load
//! source ran apart: `base/panels`' one panel, on the display and the
//! serial port at once, asking which unit to keep. Nobody signs on: the
//! service tools users are on `/data`, which is not mounted. Times are
//! UTC, the display zone being there too. It asks the storage service
//! and ends when that has kept a unit.

use appliance::time::Zone;
use panels::Named;
use panels::keep::{ApartUnit, Units};
use stgd::{Calls as _, Client};

use crate::env::sentence;

/// What the panel asks, answered by the storage service.
pub struct Keep {
    storage: Client,
}

impl Keep {
    /// The storage service of this console.
    pub fn new() -> Self {
        Self {
            storage: Client::of(&release::appliance(|_| {}, |_, _| {})),
        }
    }
}

/// When a disk was last written, as displayed.
pub fn written(secs: u64) -> String {
    match i64::try_from(secs) {
        Ok(secs) if secs > 0 => Zone::UTC.display(secs),
        _ => "not known".to_string(),
    }
}

impl Named for Keep {
    fn system(&self) -> String {
        release::NAME.into()
    }
}

impl Units for Keep {
    fn apart(&mut self) -> Option<Vec<ApartUnit>> {
        let status = self.storage.status().ok()?;
        // Before it has looked at the disks it has nothing to say.
        if status.protection.is_empty() {
            return None;
        }
        if !status.choose {
            return Some(Vec::new());
        }
        Some(
            status
                .units
                .into_iter()
                .map(|u| ApartUnit {
                    number: u.number,
                    disk: u.disk,
                    location: u.location,
                    serial: u.serial,
                    written: written(u.written),
                })
                .collect(),
        )
    }

    fn keep(&mut self, disk: &str) -> Result<(), String> {
        self.storage
            .keep(disk.to_string())
            .map_err(|e| format!("The storage service does not answer: {e}."))?
            .map_err(|e| sentence(&e))
    }
}

/// Ask until a unit is kept, then end.
#[cfg(target_os = "linux")]
pub fn main() -> ! {
    use std::io::Write as _;
    use std::path::Path;
    use std::time::Duration;

    use panels::keep::{KeepUnit, TITLE};
    use panels::screens::{Ended, run};
    use panels::{Runner, kit};

    let (screens, serial) = crate::on_console::both(Path::new(crate::on_console::SERIAL));
    println!("the unit to keep is asked for on {}", screens.describe());
    let mut env = Keep::new();
    let kept = loop {
        let mut runner: Runner<Keep, u32> = Runner::new(Box::new(KeepUnit));
        if let Ended::Outcome(kept) = run(&screens, &mut runner, &mut env, None) {
            break kept;
        }
    };
    let said = if kept == 0 {
        "The storage service asks no more.".to_string()
    } else {
        println!("unit {kept} is kept: chosen at the console");
        format!("Unit {kept} is kept.")
    };
    screens.show(kit::page(
        TITLE,
        release::NAME,
        &kit::lines(&[&said, "", "The IPL goes on."]),
        "",
    ));
    std::thread::sleep(Duration::from_millis(300));
    drop(screens);
    // The terminal as PID 1's lines want it.
    if let Some(mut out) = serial.and_then(|s| s.writer().ok()) {
        let _ = out.write_all(b"\x1b[0m\x1b[2J\x1b[H");
    }
    std::process::exit(0);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_time_is_utc_and_an_unknown_one_is_said_so() {
        assert_eq!(written(0), "not known");
        assert!(written(1_794_478_923).ends_with("UTC"));
    }
}
