// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! QS/MC's storage service: `base/stgd` run as QS/MC, told each disk's
//! name, serial number and location code by `base/inventory`. PID 1
//! starts it before `/data` is mounted and stops it after `/data` is
//! unmounted. With a mirrored load source it serves the set through
//! ublk, and a restarted service takes the device over.

#[cfg(target_os = "linux")]
fn main() {
    use refcode::post::remote;
    use stgd::{DiskIdentity, Host};

    fn inventory() -> Vec<DiskIdentity> {
        inventory::disks(&inventory::Roots::default())
            .into_iter()
            .map(|d| DiskIdentity {
                name: d.name,
                serial: d.serial,
                location: d.location,
            })
            .collect()
    }

    static APPLIANCE: release::Appliance = release::appliance(remote::post, remote::note);
    // Confined before it starts a thread (docs/SECURITY.md).
    sandbox::confine_or_end(sandbox::Service::Stgd);
    // It returns only when it could not start.
    let e = stgd::serve::run(&APPLIANCE, Host { inventory });
    eprintln!("the storage service did not start: {e}");
    std::process::exit(1);
}

#[cfg(not(target_os = "linux"))]
fn main() {
    eprintln!("qsmcstgd is a service of a QS/MC console and runs on Linux only");
}
