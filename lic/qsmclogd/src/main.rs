// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! QS/MC's log service: `base/logd` run as QS/MC. The system log, the
//! reference code history and each boot's kernel log under
//! `/data/log`; what PID 1 queued before `/data` was mounted arrives
//! first.

#[cfg(target_os = "linux")]
fn main() {
    use refcode::post::remote;

    static APPLIANCE: release::Appliance = release::appliance(remote::post, remote::note);
    // It returns only when it could not start.
    let Err(e) = logd::serve::run(&APPLIANCE, logd::files::Limits::default());
    eprintln!("the log service did not start: {e}");
    std::process::exit(1);
}

#[cfg(not(target_os = "linux"))]
fn main() {
    eprintln!("qsmclogd is a service of a QS/MC console and runs on Linux only");
}
