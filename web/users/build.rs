// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! `QSMC_NEXT_SCHEMA=1` builds the test release whose `web.toml` is of a
//! newer schema (`docs/UPDATES.md`, suite 8), under
//! `cfg(qsmc_next_schema)`, and no other build.

fn main() {
    println!("cargo::rustc-check-cfg=cfg(qsmc_next_schema)");
    println!("cargo::rerun-if-env-changed=QSMC_NEXT_SCHEMA");
    if std::env::var("QSMC_NEXT_SCHEMA").is_ok_and(|v| v == "1") {
        println!("cargo::rustc-cfg=qsmc_next_schema");
    }
}
