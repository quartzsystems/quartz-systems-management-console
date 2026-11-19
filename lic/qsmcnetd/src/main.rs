// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! QS/MC's network service: `base/netd` run as QS/MC. One management
//! interface, by DHCP or a static address, perhaps on a VLAN; carrier
//! loss and return; a change on trial; the clock kept with the
//! configured time servers and written back to the RTC. It listens on
//! nothing, and a console keeps no interface for other use.

#[cfg(target_os = "linux")]
fn main() {
    use refcode::post::remote;

    static APPLIANCE: release::Appliance = release::appliance(remote::post, remote::note);
    // It returns only when it could not start.
    let Err(e) = netd::serve::run(&APPLIANCE, Vec::new);
    eprintln!("the network service did not start: {e}");
    std::process::exit(1);
}

#[cfg(not(target_os = "linux"))]
fn main() {
    eprintln!("qsmcnetd is a service of a QS/MC console and runs on Linux only");
}
