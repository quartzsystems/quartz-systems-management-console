// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! QS/MC's storage service: `base/stgd` run as QS/MC. PID 1 starts it
//! before `/data` is mounted and stops it after `/data` is unmounted.
//! With a mirrored load source it serves the set through ublk, and a
//! restarted service takes the device over.

mod disks;

#[cfg(target_os = "linux")]
fn main() {
    use refcode::post::remote;
    use stgd::Host;

    static APPLIANCE: release::Appliance = release::appliance(remote::post, remote::note);
    // It returns only when it could not start.
    let e = stgd::serve::run(
        &APPLIANCE,
        Host {
            inventory: disks::identities,
        },
    );
    eprintln!("the storage service did not start: {e}");
    std::process::exit(1);
}

#[cfg(not(target_os = "linux"))]
fn main() {
    eprintln!("qsmcstgd is a service of a QS/MC console and runs on Linux only");
}
