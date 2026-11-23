// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! QS/MC's release (this repository's `VERSION`, or `QSMC_RELEASE`) and
//! build ID (`git describe`, or `QSMC_BUILD_ID`), under `QSMC_`; and
//! which kind of key signs the image (`QSMC_KEY_KIND`, which the build's
//! userland step sets: `release`, or `development` when no signing key
//! was given).

use std::path::Path;

fn main() {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("the release crate is under lic/");
    vrm::build::capture(repo, "QSMC_");
    println!("cargo::rerun-if-env-changed=QSMC_KEY_KIND");
    // Built outside the pipeline (a unit test, a developer's cargo):
    // no release key signed it.
    let kind = match std::env::var("QSMC_KEY_KIND").as_deref() {
        Ok("release") => "release",
        _ => "development",
    };
    println!("cargo::rustc-env=QSMC_KEY_KIND={kind}");
}
