// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! `build test`: the conformance suite against the built images. The
//! serial log is printed on failure.

use anyhow::Result;
use buildkit::Ctx;
use buildkit::host::paths;
use conformance::suite::Images;
use tracing::{error, info};

/// Run the steps of `media`.
pub fn test(ctx: &Ctx, media: conformance::Media) -> Result<()> {
    let images = Images {
        disk: paths::read_pointer(&ctx.paths.disk_pointer()).ok(),
        iso: paths::read_pointer(&ctx.paths.iso_pointer()).ok(),
    };
    let suite = conformance::Suite::new(images, ctx.paths.target.join("conformance"))?;
    let l = suite.launch();
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
            "test: PASS {:<18} {:5.1}s  {}",
            r.name,
            r.elapsed.as_secs_f64(),
            r.detail
        );
    };
    match suite.run(media, passed) {
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
