// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! The initramfs of every UKI: `/init` (qsmcinit), the mount points and
//! a console node. A slot's has the services and the local console under
//! `/qsmc/bin/`, and `qsmchook` there in a test image; the media's has
//! the installer there and the trusted signer's public key under
//! `/qsmc/keys/`, which the installer checks the payload against.

use anyhow::Result;
use buildkit::image::cpio::Entry;
use buildkit::image::initramfs::binary;
use buildkit::{Ctx, UkiKind};
use ed25519_dalek::SigningKey;

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
    if kind == UkiKind::Media {
        entries.push(Entry::file(
            &format!("qsmc/bin/{}", super::INSTALLER),
            binary(ctx, super::INSTALLER)?,
        ));
        entries.push(Entry::dir("qsmc/keys"));
        let key = buildkit::image::manifest::signing_key(ctx)?;
        entries.push(Entry::file(
            release::TRUSTED_KEYS.trim_start_matches('/'),
            trusted_keys(&key),
        ));
    } else {
        let hook = hooks.then_some(super::HOOK);
        let names = super::SERVICES.into_iter().chain([super::CONSOLE]);
        for name in names.chain(hook) {
            entries.push(Entry::file(&format!("qsmc/bin/{name}"), binary(ctx, name)?));
        }
    }
    Ok(entries)
}

/// The trusted signers' public keys, 32 bytes each: the build's own.
/// A release's further keys are Phase 8's.
fn trusted_keys(key: &SigningKey) -> Vec<u8> {
    key.verifying_key().to_bytes().to_vec()
}

/// The release image's entries.
pub fn entries(ctx: &Ctx, kind: UkiKind) -> Result<Vec<Entry>> {
    all(ctx, false, kind)
}

/// The test image's entries: the release image's and the test hooks.
pub fn test_entries(ctx: &Ctx, kind: UkiKind) -> Result<Vec<Entry>> {
    all(ctx, true, kind)
}
