// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! What the managed-systems service runs as its own user with: its
//! folder (`release::MGMT_DIR`, `systems.toml`, `mgmt.toml`, the
//! reference code history) and the console's QSI identity. Made that
//! user's at each IPL, so what a restore wrote as root is its too. The
//! key is that user's alone: the web server, root without capabilities,
//! cannot read it.

use std::fs;
use std::io;
use std::os::unix::fs::{PermissionsExt as _, chown};
use std::path::Path;

fn own(path: &Path, mode: u32) -> io::Result<()> {
    chown(path, Some(release::MGMT_ID), Some(release::MGMT_ID))?;
    fs::set_permissions(path, fs::Permissions::from_mode(mode))
}

fn own_tree(dir: &Path) -> io::Result<()> {
    own(dir, 0o700)?;
    for e in fs::read_dir(dir)? {
        let p = e?.path();
        if p.is_dir() {
            own_tree(&p)?;
        } else {
            own(&p, 0o600)?;
        }
    }
    Ok(())
}

/// Make the service's folder and the identity in `state` its own.
///
/// # Errors
/// What could not be made or given; the service then cannot start.
pub fn share(state: &Path) -> io::Result<()> {
    let dir = Path::new(release::MGMT_DIR);
    fs::create_dir_all(dir)?;
    own_tree(dir)?;
    let key = state.join(identity::KEY_FILE);
    if key.exists() {
        own(&key, 0o600)?;
    }
    let cert = state.join(identity::CERT_FILE);
    if cert.exists() {
        own(&cert, 0o644)?;
    }
    Ok(())
}
