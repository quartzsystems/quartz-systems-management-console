// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! `build userland`: every binary of the image, built for the musl
//! target and proved static and stripped, as `buildkit`'s step does,
//! with one thing more, as QS/VS's has it. The test hooks are compiled
//! in by an environment variable; this step sets it itself, on for a
//! test image and off for a release image whatever the caller's
//! environment says, and then looks in every binary for the hooks.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Result, ensure};
use buildkit::Ctx;
use buildkit::host::cmd::Cmd;
use buildkit::host::hash::sha256_file;
use buildkit::userland::{RUST_TARGET, elf};
use tracing::info;

use crate::product::{HOOK, HOOKS_ENV, HOOKS_MARKER, INIT, Image};

/// ELF `e_machine` for `x86_64`.
const ELF_MACHINE: u16 = 62;

/// Static, stripped, `x86_64`.
fn check(bin: &Path) -> Result<()> {
    let name = bin
        .file_name()
        .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
    let info = elf::inspect(bin)?;
    ensure!(
        info.machine == ELF_MACHINE,
        "{name}: e_machine {} is not x86_64",
        info.machine
    );
    ensure!(
        !info.has_interp,
        "{name}: has PT_INTERP, it is dynamically linked"
    );
    ensure!(
        info.needed.is_empty(),
        "{name}: has DT_NEEDED {:?}",
        info.needed
    );
    ensure!(!info.has_symtab, "{name}: not stripped (.symtab present)");
    info!(
        "userland: {name} static, stripped, {} bytes, sha256 {}",
        fs::metadata(bin)?.len(),
        sha256_file(bin)?
    );
    Ok(())
}

/// Whether `bytes` holds `marker`.
pub fn holds(bytes: &[u8], marker: &str) -> bool {
    bytes.windows(marker.len()).any(|w| w == marker.as_bytes())
}

/// The hooks are in a test image and in no release image: neither the
/// marker in PID 1, nor the hooks' package in the list, nor the hooks'
/// binary named by any binary (a test image's PID 1 starts it).
pub fn check_hooks(ctx: &Ctx, image: Image) -> Result<()> {
    let init = fs::read(ctx.paths.bin(INIT))?;
    let marked = holds(&init, HOOKS_MARKER);
    let packaged = ctx.product.packages.iter().any(|p| p.name == HOOK);
    match image {
        Image::Release => {
            ensure!(!marked, "the release image's {INIT} has the test hooks");
            ensure!(!packaged, "the release image lists {HOOK}");
            for package in ctx.product.packages {
                let bytes = fs::read(ctx.paths.bin(package.name))?;
                ensure!(
                    !holds(&bytes, HOOK) && !holds(&bytes, HOOKS_MARKER),
                    "the release image's {} names the test hooks",
                    package.name
                );
            }
            info!("userland: release image: no test hooks in any binary, no {HOOK}");
        }
        Image::Test => {
            ensure!(marked, "the test image's {INIT} lacks the test hooks");
            ensure!(packaged, "the test image does not list {HOOK}");
            info!("userland: TEST image: test hooks in {INIT}, {HOOK} listed");
        }
    }
    Ok(())
}

/// The build ID: `QSMC_BUILD_ID` when set, else the commit (`2f84225`,
/// `-dirty` after it for a changed tree). Not `vrm`'s `git describe`,
/// which counts from the last annotated tag, `v1.0.0`, and so named a
/// later release's build `v1.0.0-19-g2f84225`.
fn build_id(repo: &Path) -> String {
    if let Ok(id) = env::var("QSMC_BUILD_ID")
        && !id.is_empty()
    {
        return id;
    }
    std::process::Command::new("git")
        .arg("-C")
        .arg(repo)
        .args([
            "describe",
            "--always",
            "--dirty",
            "--abbrev=7",
            "--exclude=*",
        ])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "unknown".into())
}

/// `build userland`: the binaries, in the product's order.
pub fn userland(ctx: &Ctx, image: Image) -> Result<Vec<PathBuf>> {
    let cargo = env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let names: Vec<&str> = ctx.product.packages.iter().map(|p| p.name).collect();
    let hooks = match image {
        Image::Release => "0",
        Image::Test => "1",
    };
    info!(
        "userland: {HOOKS_ENV}={hooks} cargo build --release -p {} --target {RUST_TARGET}",
        names.join(" -p ")
    );
    // Which kind of key signs this image: the binaries say so.
    let key_kind = if env::var_os(ctx.product.env_name("SIGNING_KEY")).is_some() {
        "release"
    } else {
        "development"
    };
    info!("userland: QSMC_KEY_KIND={key_kind}");
    let build_id = build_id(&ctx.paths.repo);
    info!("userland: QSMC_BUILD_ID={build_id}");
    let mut cmd = Cmd::new(cargo, "userland");
    cmd.cwd(&ctx.paths.repo)
        .env(HOOKS_ENV, hooks)
        .env("QSMC_KEY_KIND", key_kind)
        .env("QSMC_BUILD_ID", &build_id)
        .args(["build", "--release", "--locked"]);
    for name in &names {
        cmd.args(["-p", name]);
    }
    cmd.args(["--target", RUST_TARGET])
        .arg("--target-dir")
        .arg(&ctx.paths.target)
        .run(ctx)?;
    let bins: Vec<PathBuf> = names.iter().map(|n| ctx.paths.bin(n)).collect();
    for bin in &bins {
        check(bin)?;
    }
    check_hooks(ctx, image)?;
    Ok(bins)
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_marker_is_found_anywhere_in_a_binary() {
        let marker = "TEST IMAGE:";
        assert!(super::holds(b"\x7fELF..TEST IMAGE: the hooks", marker));
        assert!(!super::holds(b"\x7fELF..TEST IMAGE", marker));
        assert!(!super::holds(b"", marker));
    }
}
