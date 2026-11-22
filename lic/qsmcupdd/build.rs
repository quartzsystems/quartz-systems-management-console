// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! `QSMC_TEST_HOOKS=1` builds the update service of a test image: the
//! product an image must be for is the test image's, under
//! `cfg(test_hooks)`, and in no other build.

fn main() {
    println!("cargo::rustc-check-cfg=cfg(test_hooks)");
    println!("cargo::rustc-check-cfg=cfg(qsmc_next_schema)");
    println!("cargo::rerun-if-env-changed=QSMC_TEST_HOOKS");
    println!("cargo::rerun-if-env-changed=QSMC_NEXT_SCHEMA");
    if std::env::var("QSMC_NEXT_SCHEMA").is_ok_and(|v| v == "1") {
        println!("cargo::rustc-cfg=qsmc_next_schema");
    }
    if std::env::var("QSMC_TEST_HOOKS").is_ok_and(|v| v == "1") {
        println!("cargo::rustc-cfg=test_hooks");
    }
}
