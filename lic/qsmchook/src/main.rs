// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! The test hooks' binary: `qsmchook channel` and `qsmchook flaky`, the
//! services a test image's PID 1 has in its table.

#[cfg(target_os = "linux")]
fn main() {
    match std::env::args().nth(1).as_deref() {
        Some("channel") => {
            let Err(e) = qsmchook::serve::channel();
            eprintln!("the channel did not start: {e}");
        }
        Some("flaky") => qsmchook::serve::flaky(),
        // The web server of the `webd-fails` hook: it fails at once.
        Some("fail") => {
            eprintln!("failing, as the {} hook asks", qsmchook::WEBD_FAILS);
            std::process::exit(3);
        }
        other => eprintln!("qsmchook channel | flaky | fail, not {other:?}"),
    }
    std::process::exit(1);
}

#[cfg(not(target_os = "linux"))]
fn main() {
    eprintln!("qsmchook runs in a QS/MC test image only");
}
