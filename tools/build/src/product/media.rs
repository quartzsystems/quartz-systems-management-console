// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! The installation media: the boot image with the media UKI and the
//! same UKI loose in the tree. No payload yet: the installer and what it
//! writes come in Phase 2.

use anyhow::Result;
use buildkit::Ctx;
use buildkit::image::iso::{Item, Media};
use loadsource::payload;

/// What goes on the media around the media UKI.
#[allow(
    clippy::unnecessary_wraps,
    reason = "buildkit's MediaFn; Phase 2's payload can fail"
)]
pub fn media(ctx: &Ctx, _uki: &[u8]) -> Result<Media> {
    Ok(Media {
        volume_id: payload::volume_id(ctx.product.vendor, &ctx.product.release),
        tree: vec![Item::Boot],
        boot_files: Vec::new(),
        uki_copies: Vec::new(),
    })
}
