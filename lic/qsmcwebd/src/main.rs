// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! QS/MC's web server: `web/webd` run as a service. It listens on
//! nothing of its own: PID 1 hands it 443 and 80 on the `open` address.

#[cfg(target_os = "linux")]
fn main() {
    use std::sync::Arc;

    // It returns only when it cannot go on; supervision starts it again.
    if let Err(e) = webd::run::run(Arc::new(webd::serve::NoPages)) {
        eprintln!("the web server stopped: {e}");
    }
    std::process::exit(1);
}

#[cfg(not(target_os = "linux"))]
fn main() {
    eprintln!("qsmcwebd is a service of a QS/MC console and runs on Linux only");
}
