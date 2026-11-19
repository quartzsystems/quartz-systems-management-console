// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! The initramfs of every UKI: `/init` (qsmcinit), the mount points and
//! a console node. A slot's has the services under `/qsmc/bin/`, and
//! `qsmchook` there in a test image; the media's has PID 1 alone, which
//! starts no services there until the installer comes in Phase 2.

use anyhow::Result;
use buildkit::image::cpio::Entry;
use buildkit::image::initramfs::binary;
use buildkit::{Ctx, UkiKind};

/// The entries of a UKI of `kind`. The kernel does not mount devtmpfs
/// for an initramfs, so a static console node lets it open the first
/// console. `/data` is where `QSMCDATA` is mounted, and holds the state
/// in memory when it is not; `/etc` is for the resolver's file.
fn all(ctx: &Ctx, hooks: bool, kind: UkiKind) -> Result<Vec<Entry>> {
    let mut entries = vec![
        Entry::dir("data"),
        Entry::dir("dev"),
        Entry::chardev("dev/console", 5, 1),
        Entry::dir("etc"),
        Entry::file("init", binary(ctx, super::INIT)?),
        Entry::dir("proc"),
        Entry::dir("qsmc"),
        Entry::dir("qsmc/bin"),
        Entry::dir("run"),
        Entry::dir("sys"),
    ];
    if kind != UkiKind::Media {
        let hook = hooks.then_some(super::HOOK);
        for name in super::SERVICES.into_iter().chain(hook) {
            entries.push(Entry::file(&format!("qsmc/bin/{name}"), binary(ctx, name)?));
        }
    }
    Ok(entries)
}

/// The release image's entries.
pub fn entries(ctx: &Ctx, kind: UkiKind) -> Result<Vec<Entry>> {
    all(ctx, false, kind)
}

/// The test image's entries: the release image's and the test hooks.
pub fn test_entries(ctx: &Ctx, kind: UkiKind) -> Result<Vec<Entry>> {
    all(ctx, true, kind)
}
