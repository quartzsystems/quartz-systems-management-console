// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! The console's update image (`docs/UPDATES.md`), by `base/buildaid`:
//! both slots' UKIs and a manifest of the release, its fixes
//! (`QSMC_FIXES`) and the state files it reads (`qsmcupdd::schemas`),
//! signed with the build's key in QS/MC's format. The same inputs make
//! the same bytes; the artifact manifest lists it.

use std::path::PathBuf;

use anyhow::Result;
use buildkit::Ctx;

/// Write the update image of what was built: its path.
pub fn update(ctx: &Ctx) -> Result<PathBuf> {
    buildaid::updimg::update(ctx, &qsmcupdd::FORMAT, qsmcupdd::schemas::this_release())
}
