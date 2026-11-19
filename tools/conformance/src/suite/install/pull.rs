// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! Suite 8: the power pulled during an unattended installation twenty
//! times, at moments drawn from a seed the report gives. Each time the
//! installer starts over from the beginning and gets to its progress
//! again; the installation after the last pull is let finish, and the
//! console it leaves IPLs and is the one asked for. The media is the ISO
//! as a USB stick, so that way of installing is exercised too.

use std::time::{Duration, Instant};

use super::{
    DISK_BYTES, HOST, INSTALL_WITHIN, INSTALLER_ENDED, Media, TITLE, answers, blank_disks,
    boot_installed, installed_machine, installing, is_installed, keep_vars, titled_with, with_seed,
};
use crate::suite::base::{Run, power_off};

/// How many pulls.
const PULLS: usize = 20;
/// The longest a pull waits after the progress display appears: past
/// the end of an installation of the suites' disks.
const SPREAD_MS: u64 = 8_000;

/// A small generator of the pull moments, from its seed.
struct Moments(u64);

impl Moments {
    fn next(&mut self) -> u64 {
        // xorshift64*
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D) % SPREAD_MS
    }
}

/// Suite 8.
pub fn power_pull(run: &Run<'_>) -> Result<String, String> {
    let disks = blank_disks(run, "pulled", 2, DISK_BYTES)?;
    let seed = answers(&["QSMCHD0", "QSMCHD1"], &format!("name = \"{HOST}\""), "")?;
    let mut machine = installing(run, &disks, Media::Usb)?;
    with_seed(run, &mut machine, &seed)?;
    let start = u64::from(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(1, |d| d.subsec_nanos()),
    ) | 1;
    let mut moments = Moments(start);
    let mut finished = 0;
    for _ in 0..PULLS {
        let mut guest = run.boot(&machine)?;
        let deadline = Instant::now() + INSTALL_WITHIN;
        titled_with(
            &mut guest,
            TITLE,
            "Installing. Do not power the machine off.",
            deadline,
        )?;
        std::thread::sleep(Duration::from_millis(moments.next()));
        if guest.said(INSTALLER_ENDED) {
            finished += 1;
        }
        guest.kill();
    }
    // The last one, let finish.
    let mut guest = run.boot(&machine)?;
    let deadline = Instant::now() + INSTALL_WITHIN;
    guest.seen(INSTALLER_ENDED, deadline)?;
    guest.wait_exit(deadline)?;
    keep_vars(run, &machine)?;
    drop(guest);
    let mut guest = boot_installed(run, &installed_machine(run, &disks), run.launch.timeout * 2)?;
    is_installed(&mut guest, HOST)?;
    power_off(&mut guest)?;
    Ok(format!(
        "{PULLS} power pulls (seed {start}) during installations from a USB stick: each \
         IPL started the installer over and reached its progress again ({finished} had \
         finished before the pull); the last installation finished and its console IPLs \
         as {HOST}"
    ))
}
