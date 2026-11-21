// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! `Work with disk units`: `base/panels::units` with the console's help
//! and nothing beside the load source's own options.

use panels::units::{DiskUnits, LoadSource, Product};

use crate::env::Env;
use crate::menu::Outcome;

/// The console's words on the load source's panel.
pub static PRODUCT: Product<Env, Outcome> = Product {
    options: &[],
    open: |_| None,
    help: &[
        "The load source is the disk the console starts from and keeps",
        "its own data on, or a mirrored pair of disks. Each unit is",
        "shown by its location code, serial number, size and state.",
        "",
        "1 adds a second disk to a load source of one: it becomes a",
        "mirrored pair at the next IPL, and the new disk is written",
        "from the first. 2 puts a new disk in the place of a unit",
        "that is missing or failed; it is rebuilt while the console",
        "runs. The disk chosen is erased. 5 shows the units again.",
        "",
        "While a unit is not in step the console runs, but a second",
        "failure would stop it.",
    ],
    others: "",
    home: || Box::new(crate::menu::MainMenu),
};

/// The panel option 2 opens.
pub fn panel() -> DiskUnits<Env, Outcome> {
    DiskUnits::new(&PRODUCT)
}

impl LoadSource for Env {
    fn storage(&mut self) -> Option<stgd::Status> {
        Env::storage(self)
    }

    fn machine_disks(&mut self) -> Vec<state::doc::inventory::Disk> {
        crate::env::inventory().disks
    }

    fn start_protection(&mut self, disk: &str) -> Result<(), String> {
        Env::start_protection(self, disk)
    }

    fn replace(&mut self, number: u32, disk: &str) -> Result<(), String> {
        Env::replace(self, number, disk)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_help_fits_its_window_and_names_the_console() {
        assert!(PRODUCT.help.iter().all(|l| l.len() <= 64));
        assert!(PRODUCT.help.iter().all(|l| !l.contains("host")));
    }
}
