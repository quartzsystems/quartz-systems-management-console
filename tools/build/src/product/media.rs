// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! The installation media: the boot image with the media UKI, and the
//! payload the installer writes a load source from, as
//! `base/loadsource::payload` describes it: both slots' UKIs and a
//! manifest of them, signed with the build's key. The media UKI's
//! initramfs holds that key's public half (`initramfs.rs`), so the
//! media and the installer of one build trust each other.

use std::fs;

use anyhow::{Context, Result};
use buildkit::host::hash::sha256_bytes;
use buildkit::host::paths::read_pointer;
use buildkit::image::iso::{Item, Media};
use buildkit::{Ctx, UkiKind};
use ed25519_dalek::Signer;
use loadsource::payload::{self, File, Manifest};
use release::LOAD_SOURCE;
use tracing::info;

/// A slot's UKI and its line of the manifest.
fn slot(ctx: &Ctx, kind: UkiKind, name: &str) -> Result<(Vec<u8>, File)> {
    let path = read_pointer(&ctx.paths.uki_pointer(kind))
        .with_context(|| format!("no UKI for {name}; run `uki` first"))?;
    let bytes = fs::read(&path).with_context(|| format!("cannot read {}", path.display()))?;
    let file = File {
        name: name.to_string(),
        bytes: bytes.len() as u64,
        sha256: sha256_bytes(&bytes),
    };
    Ok((bytes, file))
}

/// What goes on the media around the media UKI.
pub fn media(ctx: &Ctx, _uki: &[u8]) -> Result<Media> {
    let (slot_a, file_a) = slot(ctx, UkiKind::SlotA, payload::SLOT_A)?;
    let (slot_b, file_b) = slot(ctx, UkiKind::SlotB, payload::SLOT_B)?;
    let manifest = Manifest {
        product: ctx.product.name.to_string(),
        release: ctx.product.release.clone(),
        build: ctx.product.build_id.clone(),
        files: vec![file_a, file_b],
    };
    let text = manifest.render(&LOAD_SOURCE);
    let key = buildkit::image::manifest::signing_key(ctx)?;
    let signature = hex::encode(key.sign(text.as_bytes()).to_bytes());
    info!(
        "iso: payload of {} {} build {}: both slots, signed by {}",
        manifest.product,
        manifest.release,
        manifest.build,
        hex::encode(key.verifying_key().to_bytes())
    );
    let item = |name: &str, bytes: Vec<u8>| {
        Item::File(format!("{}/{name}", payload::dir(&LOAD_SOURCE)), bytes)
    };
    Ok(Media {
        volume_id: payload::volume_id(ctx.product.vendor, &ctx.product.release),
        tree: vec![
            Item::Boot,
            item(payload::MANIFEST, text.into_bytes()),
            item(payload::SIGNATURE, format!("{signature}\n").into_bytes()),
            item(payload::SLOT_A, slot_a),
            item(payload::SLOT_B, slot_b),
        ],
        boot_files: Vec::new(),
        uki_copies: Vec::new(),
    })
}
