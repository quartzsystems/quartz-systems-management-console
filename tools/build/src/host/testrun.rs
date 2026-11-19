// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! `build test`: Phase 0's steps on the release image, then the base
//! system's suites on the test image. The serial log is printed on
//! failure and kept under the target's `conformance/`.

use anyhow::Result;
use buildkit::host::paths;
use buildkit::{Ctx, UkiKind};
use conformance::{Images, TestImage};
use tracing::{error, info};

/// The test image, when it is built.
fn test_image(test: &Ctx) -> Option<TestImage> {
    let read = |p: std::path::PathBuf| paths::read_pointer(&p).ok();
    Some(TestImage {
        disk: read(test.paths.disk_pointer())?,
        slot_a: read(test.paths.uki_pointer(UkiKind::SlotA))?,
        slot_b: read(test.paths.uki_pointer(UkiKind::SlotB))?,
    })
}

/// Run the steps and suites of `media`, or only `suite`.
pub fn test(
    release: &Ctx,
    test: &Ctx,
    media: conformance::Media,
    suite: Option<&str>,
) -> Result<()> {
    if suite == Some("list") {
        for name in conformance::suite::names(media) {
            println!("{name}");
        }
        return Ok(());
    }
    let images = Images {
        disk: paths::read_pointer(&release.paths.disk_pointer()).ok(),
        iso: paths::read_pointer(&release.paths.iso_pointer()).ok(),
        test: test_image(test),
    };
    let runner = conformance::Suite::new(images, release.paths.target.join("conformance"))?;
    let l = runner.launch();
    info!(
        "test: {} with {} ({})",
        l.qemu.display(),
        l.firmware.display(),
        if l.kvm {
            "kvm, -cpu host"
        } else {
            "tcg, -cpu max"
        }
    );
    let passed = |r: &conformance::Report| {
        info!(
            "test: PASS {:<18} {:6.1}s  {}",
            r.name,
            r.elapsed.as_secs_f64(),
            r.detail
        );
    };
    match runner.run(media, suite, passed) {
        Ok(reports) => {
            info!("test: PASS, {} step(s), media {media}", reports.len());
            Ok(())
        }
        Err(e) => {
            if let Some(log) = e.log() {
                eprintln!("----- serial log -----\n{log}\n----- end serial log -----");
            }
            error!("test: FAIL: {e}");
            Err(e.into())
        }
    }
}
