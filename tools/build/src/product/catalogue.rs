// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! The catalogue of the QS/VS release this console is built with: its
//! reference codes and messages explained (`catalogue.json`, QS/VS's
//! `docs/catalogue.schema.json`), written by QS/VS's own build at the
//! pinned commit (`build catalogue`, from cargo's checkout) and put in a
//! slot's initramfs at `release::CATALOGUE`. Nothing of it is restated
//! here: QS/MC's pages read it as data.

use std::fs;
use std::process::Command;

use anyhow::{Context, Result, bail};
use buildkit::Ctx;

use crate::host::pins::{QSVS_GIT, pin, qsvs_checkout};

/// The catalogue's bytes, made once a pin in the target directory.
pub fn catalogue(ctx: &Ctx) -> Result<Vec<u8>> {
    let manifest = fs::read_to_string(ctx.paths.repo.join("Cargo.toml"))?;
    let rev = pin(&manifest, QSVS_GIT)?;
    let short = rev.get(..7).unwrap_or(&rev).to_string();
    let dir = ctx.paths.target.join("qsvs-catalogue");
    let out = dir.join(format!("catalogue-{short}.json"));
    if let Ok(bytes) = fs::read(&out) {
        return Ok(bytes);
    }
    fs::create_dir_all(&dir)?;
    let checkout = qsvs_checkout(&ctx.paths.repo)?;
    let status = Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".into()))
        .current_dir(&checkout)
        .args(["run", "-q", "--locked", "-p", "build", "--", "catalogue"])
        .arg(&out)
        .env("CARGO_TARGET_DIR", dir.join("cargo"))
        .env("QSVS_TARGET_DIR", dir.join("target"))
        .env("QSVS_BUILD_ID", &short)
        .status()
        .with_context(|| format!("QS/VS's build in {}", checkout.display()))?;
    if !status.success() {
        bail!("QS/VS's `build catalogue` at {short} failed ({status})");
    }
    let bytes = fs::read(&out).with_context(|| out.display().to_string())?;
    let text = std::str::from_utf8(&bytes).context("the catalogue is not text")?;
    if !text.contains("\"schema\": \"quartz.catalogue/1\"") {
        bail!("the catalogue at {short} is not of schema quartz.catalogue/1");
    }
    Ok(bytes)
}
