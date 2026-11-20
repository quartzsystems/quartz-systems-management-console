// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! `QSMC_TEST_HOOKS=1` builds the pages of a test image: the pattern
//! reference under `/dev/` is served under `cfg(test_hooks)` and in no
//! other build.

fn main() {
    println!("cargo::rustc-check-cfg=cfg(test_hooks)");
    println!("cargo::rerun-if-env-changed=QSMC_TEST_HOOKS");
    if std::env::var("QSMC_TEST_HOOKS").is_ok_and(|v| v == "1") {
        println!("cargo::rustc-cfg=test_hooks");
    }
}
