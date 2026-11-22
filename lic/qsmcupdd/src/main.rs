// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! QS/MC's update service (`docs/UPDATES.md`): `base/update` run as
//! QS/MC, with the release that runs, the keys its image trusts, its
//! load-source names, and its directory on `/data`. Its socket is
//! root's: the web server and the local console, root without
//! capabilities, ask it.

/// Nobody else's: every client is root.
#[cfg(target_os = "linux")]
#[allow(clippy::unnecessary_wraps, reason = "the base crate's share hook")]
fn keep(_: &std::path::Path) -> std::io::Result<()> {
    Ok(())
}

#[cfg(target_os = "linux")]
fn main() {
    use std::path::{Path, PathBuf};

    let host = update::serve::Host {
        id: release::ID,
        format: &qsmcupdd::FORMAT,
        messages: &qsmcupdd::MESSAGES,
        post: |c, detail, about| {
            refcode::post::remote::code_about(qsmcupdd::code(c), detail, about);
        },
        share: keep,
        run_dir: release::RUN_DIR,
        dir: PathBuf::from(qsmcupdd::DIR),
        state_dir: Path::new(release::DATA_DIR).join("state"),
        keys: PathBuf::from(release::TRUSTED_KEYS),
        product: qsmcupdd::PRODUCT,
        release: release::RELEASE,
        build: release::BUILD_ID,
        control: PathBuf::from(release::CONTROL_SOCKET),
        partitions: || Box::new(update::Nothing),
        names: &release::LOAD_SOURCE,
    };
    // It returns only when it could not start.
    let e = update::serve::run(host);
    eprintln!("the update service did not start: {e}");
    std::process::exit(1);
}

#[cfg(not(target_os = "linux"))]
fn main() {
    eprintln!("qsmcupdd is a service of a QS/MC console and runs on Linux only");
}
