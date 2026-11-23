// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! `build release` and `build stream`: QS/MC's steps and files around
//! `base/buildaid`'s release, as QS/VS has them.
//!
//! A release is signed with release keys and never with a development
//! key: the release signing key (`QSMC_SIGNING_KEY`, ed25519) signs the
//! manifest, the installation payload, the update image, `SHA256SUMS`
//! and the stream's index; the Secure Boot db key (`QSMC_SB_KEY` and
//! `QSMC_SB_CERT`, RSA-2048) signs the three UKIs before the disk and
//! the ISO are made of them. The image trusts the three streams'
//! release keys (`QSMC_STREAM_KEYS`, `product::keys`).
//!
//! `target/release/<VRM>/` holds the ISO (buildkit's name,
//! `QSMC-<release>-<hash>.iso`), the update image, the manifest and its
//! signature, the notices, the release notes, the db certificate, the
//! catalogue and its form, `SHA256SUMS` and its signature, and
//! `stream/` with the index of this release. With compare (the
//! default) the whole release is built a second time from clean at the
//! same target path, and every file must be the same.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use buildkit::Ctx;
use ed25519_dalek::SigningKey;
use tracing::{info, warn};

use crate::product::Image;
use crate::product::keys::NAMES;

/// The catalogue's form, kept in the tree.
const SCHEMA_FILE: &str = "docs/catalogue.schema.json";

/// The release notes of `vrm`.
fn notes(ctx: &Ctx, vrm: &str) -> PathBuf {
    ctx.paths
        .repo
        .join("docs")
        .join(format!("RELEASE_NOTES_{vrm}.md"))
}

/// What a release adds at the media's root: the certificate an operator
/// enrols, the notices and the release notes (`images/media-root`, which
/// `product::media` takes).
fn media_root(ctx: &Ctx, cert: &[u8], vrm: &str) -> Result<()> {
    let dir = ctx.paths.images().join(crate::product::MEDIA_ROOT);
    if dir.exists() {
        fs::remove_dir_all(&dir)?;
    }
    fs::create_dir_all(&dir)?;
    fs::write(dir.join(NAMES.cert()), cert)?;
    fs::copy(
        ctx.paths.repo.join(crate::product::NOTICES_FILE),
        dir.join("NOTICES.txt"),
    )?;
    fs::copy(notes(ctx, vrm), dir.join("RELEASE-NOTES.md"))?;
    Ok(())
}

fn copy(from: &Path, to_dir: &Path) -> Result<String> {
    let name = from
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    fs::copy(from, to_dir.join(&name))
        .with_context(|| format!("cannot copy {}", from.display()))?;
    Ok(name)
}

/// Every step of a release, once, into `target/release/<VRM>/`.
fn build_once(ctx: &Ctx, vrm: &str, key: &SigningKey, db: &pesign::DbKey) -> Result<PathBuf> {
    buildkit::notices::notices(ctx, true)?;
    buildkit::kernel::fetch::fetch(ctx)?;
    buildkit::kernel::kernel(ctx)?;
    super::userland::userland(ctx, Image::Release)?;
    buildkit::image::initramfs::initramfs(ctx)?;
    buildkit::kernel::uki(ctx, false)?;
    buildaid::release::sign_ukis(ctx, db)?;
    super::disk::disk(ctx, None)?;
    media_root(ctx, &db.cert, vrm)?;
    let iso = buildkit::image::iso::iso(ctx);
    let _ = fs::remove_dir_all(ctx.paths.images().join(crate::product::MEDIA_ROOT));
    let iso = iso?;
    let upd = super::update::update(ctx)?;
    let manifest = super::manifest::manifest(ctx, Image::Release)?;

    let out = ctx.paths.target.join("release").join(vrm);
    if out.exists() {
        fs::remove_dir_all(&out)?;
    }
    fs::create_dir_all(&out)?;
    let mut files = Vec::new();
    for from in [&iso, &upd] {
        files.push(copy(from, &out)?);
    }
    fs::copy(&manifest, out.join("MANIFEST.json"))?;
    fs::copy(
        manifest.with_file_name("manifest.sig"),
        out.join("MANIFEST.sig"),
    )?;
    fs::copy(
        ctx.paths.repo.join(crate::product::NOTICES_FILE),
        out.join("NOTICES.txt"),
    )?;
    fs::copy(notes(ctx, vrm), out.join(format!("RELEASE_NOTES_{vrm}.md")))?;
    fs::write(out.join(NAMES.cert()), &db.cert)?;
    fs::write(
        out.join("catalogue.json"),
        crate::host::catalogue::json(&ctx.product.release, &ctx.product.build_id),
    )?;
    fs::copy(
        ctx.paths.repo.join(SCHEMA_FILE),
        out.join("catalogue.schema.json"),
    )?;
    for f in [
        "MANIFEST.json".to_string(),
        "MANIFEST.sig".into(),
        "NOTICES.txt".into(),
        format!("RELEASE_NOTES_{vrm}.md"),
        NAMES.cert(),
        "catalogue.json".into(),
        "catalogue.schema.json".into(),
    ] {
        files.push(f);
    }
    // The stream's index of this release alone; `build stream add`
    // joins it to the published one. Then the sums of every file.
    let (iso_name, upd_name) = (files[0].clone(), files[1].clone());
    buildaid::release::index_and_sums(
        &out,
        &NAMES,
        vrm,
        &ctx.product.build_id,
        super::update::named_fixes(ctx)?,
        &upd_name,
        &iso_name,
        &mut files,
        key,
    )?;
    Ok(out)
}

/// `build release VERSION`: refused without release keys or with a
/// development key, without the streams' release keys, from a dirty
/// tree, or for a version not the tree's; then the release, and with
/// `compare` a second one from clean, file for file the same.
pub fn release(ctx: &Ctx, version: &str, compare: bool) -> Result<PathBuf> {
    let (key, db) = buildaid::release::release_keys(ctx)?;
    // The image trusts the streams' release keys, never development ones.
    crate::product::keys::files(ctx, Image::Release)?;
    let vrm = buildaid::release::check_tree(ctx, version, |named| notes(ctx, named))?;
    info!("release: {vrm} build {}", ctx.product.build_id);
    if !compare {
        let out = build_once(ctx, &vrm, &key, &db)?;
        warn!("release: not built a second time (--no-compare)");
        return Ok(out);
    }
    let again = buildaid::release::twice(ctx, || build_once(ctx, &vrm, &key, &db))?;
    for line in fs::read_to_string(again.join("SHA256SUMS"))?.lines() {
        info!("release: {line}");
    }
    Ok(again)
}

/// `build stream add RELEASE_DIR STREAM_DIR`: the release's files copied
/// into the published directory and its entry joined to the index there,
/// its sequence one further, signed again with the release signing key.
pub fn stream_add(ctx: &Ctx, release_dir: &Path, stream_dir: &Path) -> Result<()> {
    buildaid::release::stream_add(ctx, &NAMES, release_dir, stream_dir)
}

/// `build stream verify STREAM_DIR`: the index checked against the keys
/// `QSMC_TRUSTED_KEYS` names and the release key, and every file it
/// names against its size and SHA-256.
pub fn stream_verify(ctx: &Ctx, stream_dir: &Path) -> Result<()> {
    let index = buildaid::release::stream_verify(ctx, stream_dir)?;
    info!(
        "stream: {} verified, sequence {}, {} release(s)",
        index.stream,
        index.sequence,
        index.releases.len()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_release_names_are_qs_mc_s() {
        let ctx = Ctx::new(crate::product::qsmc(Image::Release).unwrap(), false).unwrap();
        assert_eq!(NAMES.cert(), "qsmc-db.cer");
        assert_eq!(NAMES.stream, "qsmc");
        assert!(
            notes(&ctx, "V1R0M0").ends_with("docs/RELEASE_NOTES_V1R0M0.md"),
            "{}",
            notes(&ctx, "V1R0M0").display()
        );
        assert_eq!(ctx.product.env_name("SIGNING_KEY"), "QSMC_SIGNING_KEY");
        assert_eq!(ctx.product.env_name("SB_KEY"), "QSMC_SB_KEY");
        assert!(ctx.paths.repo.join(SCHEMA_FILE).is_file());
    }
}
