// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! `QSMC_TEST_HOOKS=1` builds a test image's confinement: the learning
//! mode and the forbidden call are compiled in under `cfg(test_hooks)`
//! and in no other build. With `QSMC_CONFINE_LEARN=1` besides, a test
//! image learns at every IPL (`cfg(confine_learn)`): the run that
//! derives the policies.

fn main() {
    println!("cargo::rustc-check-cfg=cfg(test_hooks)");
    println!("cargo::rustc-check-cfg=cfg(confine_learn)");
    println!("cargo::rerun-if-env-changed=QSMC_TEST_HOOKS");
    println!("cargo::rerun-if-env-changed=QSMC_CONFINE_LEARN");
    let on = |name: &str| std::env::var(name).is_ok_and(|v| v == "1");
    if on("QSMC_TEST_HOOKS") {
        println!("cargo::rustc-cfg=test_hooks");
        if on("QSMC_CONFINE_LEARN") {
            println!("cargo::rustc-cfg=confine_learn");
        }
    }
}
