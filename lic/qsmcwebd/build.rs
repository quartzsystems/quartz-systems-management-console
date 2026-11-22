// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! `QSMC_TEST_HOOKS=1` with `QSMC_TEST_WEBD_BROKEN=1` builds a test
//! image whose web server cannot start (`cfg(webd_broken)`): the update
//! a console rolls back from (`docs/UPDATES.md`, suite 8). No release
//! image is ever built so.

fn main() {
    println!("cargo::rustc-check-cfg=cfg(webd_broken)");
    println!("cargo::rerun-if-env-changed=QSMC_TEST_HOOKS");
    println!("cargo::rerun-if-env-changed=QSMC_TEST_WEBD_BROKEN");
    let on = |k: &str| std::env::var(k).is_ok_and(|v| v == "1");
    if on("QSMC_TEST_HOOKS") && on("QSMC_TEST_WEBD_BROKEN") {
        println!("cargo::rustc-cfg=webd_broken");
    }
}
