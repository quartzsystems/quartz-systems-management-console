// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! `build fuzz`: `tools/fuzz`, a workspace of its own, built with
//! `SanitizerCoverage` counters in a target directory of its own, as QS/400's `build fuzz` builds its
//! fuzzer, then every target (or one) run for the seconds given.

use std::env;

use anyhow::Result;
use buildkit::Ctx;
use buildkit::host::cmd::Cmd;

/// The flags that give every edge an 8-bit counter (QS/400's).
const SANCOV: &str = "-C passes=sancov-module -C llvm-args=-sanitizer-coverage-level=3 \
                      -C llvm-args=-sanitizer-coverage-inline-8bit-counters";

/// The host triple: naming it keeps `RUSTFLAGS` off build scripts and
/// proc macros, which must not be instrumented.
const HOST: &str = "x86_64-unknown-linux-gnu";

/// Run `target` (every one when `None`) for `secs` seconds each.
pub fn fuzz(ctx: &Ctx, target: Option<&str>, secs: u64) -> Result<()> {
    anyhow::ensure!(cfg!(target_os = "linux"), "build fuzz runs on Linux (WSL)");
    let cargo = env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let mut c = Cmd::new(&cargo, "qsmcfuzz");
    c.cwd(&ctx.paths.repo)
        .env("RUSTFLAGS", SANCOV)
        .env("CARGO_TARGET_DIR", ctx.paths.target.join("fuzz"))
        .args([
            "run",
            "--locked",
            "--manifest-path",
            "tools/fuzz/Cargo.toml",
            "--profile",
            "fuzz",
            "--target",
            HOST,
            "--",
        ]);
    match target {
        Some(t) => c.args(["run", t]),
        None => c.arg("all"),
    };
    c.arg(secs.to_string());
    tracing::info!(
        "fuzz: {} for {secs} s each",
        target.unwrap_or("every target")
    );
    c.run(ctx)
}
