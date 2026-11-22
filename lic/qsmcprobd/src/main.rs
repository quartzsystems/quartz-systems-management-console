// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! QS/MC's problem service (`docs/SERVICE.md`): `base/problem` run as
//! QS/MC, with the table of the console's own conditions. It reads the
//! reference code history the log service writes and keeps the
//! console's problem log in `/data/problems`. Its socket is root's: the
//! web server and the local console, root without capabilities, ask it.

/// The data file system nearly full (`A3006010`), or with room again
/// (`C3006010`).
#[cfg(target_os = "linux")]
fn post_data(full: bool, detail: &str, about: &[supervise::control::About]) {
    let code = if full {
        &refcode::DATA_NEARLY_FULL
    } else {
        &refcode::DATA_HAS_ROOM
    };
    refcode::post::remote::code_about(code, detail, about);
}

/// Nobody else's: every client is root.
#[cfg(target_os = "linux")]
#[allow(clippy::unnecessary_wraps, reason = "the base crate's share hook")]
fn keep(_: &std::path::Path) -> std::io::Result<()> {
    Ok(())
}

#[cfg(target_os = "linux")]
fn main() {
    use std::path::PathBuf;

    let appliance = release::appliance(|_| {}, |_, _| {});
    let host = problem::serve::Host {
        rules: qsmcprobd::RULES,
        post_data,
        full_code: refcode::DATA_NEARLY_FULL.code,
        id: release::ID,
        run_dir: release::RUN_DIR,
        dir: PathBuf::from(release::DATA_DIR).join("problems"),
        history: appliance.log_dir().join(logd::files::HISTORY),
        data: PathBuf::from(release::DATA_DIR),
        share: keep,
    };
    // It returns only when it could not start.
    let e = problem::serve::run(host);
    eprintln!("the problem service did not start: {e}");
    std::process::exit(1);
}

#[cfg(not(target_os = "linux"))]
fn main() {
    eprintln!("qsmcprobd is a service of a QS/MC console and runs on Linux only");
}
