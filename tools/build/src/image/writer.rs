// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! `build loadsource`: a load source written onto image files with
//! `base/loadsource`'s writer, one disk or two mirrored, from the built
//! image's slot UKIs. The harness writes its own the same way; the
//! release `disk` image stays the single-disk one `buildkit` writes.

use std::path::PathBuf;

use anyhow::{Context, Result, anyhow, ensure};
use buildkit::host::paths;
use buildkit::{Ctx, UkiKind};
use conformance::image::disks;
use tracing::info;

/// Write a load source of `bytes` (1 GiB when `None`) onto `out`: one
/// path for one disk, two for a mirrored pair.
pub fn write(ctx: &Ctx, out: &[PathBuf], bytes: Option<u64>) -> Result<()> {
    ensure!(
        (1..=2).contains(&out.len()),
        "a load source is one disk or two"
    );
    let uki = |kind| -> Result<Vec<u8>> {
        let path = paths::read_pointer(&ctx.paths.uki_pointer(kind))
            .context("the UKIs are not built; run `uki` first")?;
        std::fs::read(&path).with_context(|| format!("cannot read {}", path.display()))
    };
    let bytes = bytes.unwrap_or(loadsource::layout::DEFAULT_DISK_BYTES);
    let now = appliance::time::now_secs();
    let written = disks::write(
        out,
        bytes,
        &uki(UkiKind::SlotA)?,
        &uki(UkiKind::SlotB)?,
        now,
    )
    .map_err(|e| anyhow!(e))?;
    let kind = if written.len() == 2 {
        "mirrored"
    } else {
        "one disk"
    };
    for (n, disk) in written.iter().enumerate() {
        info!(
            "loadsource: {} ({kind}), serial {} when the harness boots it",
            disk.path.display(),
            disks::serial(n)
        );
    }
    Ok(())
}
