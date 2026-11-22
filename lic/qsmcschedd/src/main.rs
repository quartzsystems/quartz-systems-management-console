// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! QS/MC's scheduler: `sched/schedd` run as a service (`docs/SCHED.md`).
//! PID 1 starts it with no capability, as it starts the web server: it
//! reads the users' roles and writes its own file in the state
//! directory, binds its socket for the web server, and asks `mgmtd` on
//! its socket in each operation's owner's name.

#[cfg(target_os = "linux")]
fn main() {
    if let Err(e) = run() {
        eprintln!("the scheduler stopped: {e}");
    }
    std::process::exit(1);
}

/// Bind `path` afresh, for root's alone: the web server, root without
/// capabilities, is its owner.
#[cfg(target_os = "linux")]
fn bind(path: &str) -> Result<std::os::unix::net::UnixListener, String> {
    use std::os::unix::fs::PermissionsExt as _;
    let _ = std::fs::remove_file(path);
    let l = std::os::unix::net::UnixListener::bind(path).map_err(|e| format!("{path}: {e}"))?;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
        .map_err(|e| format!("{path}: {e}"))?;
    Ok(l)
}

/// Say that state files were written in this release's form.
#[cfg(target_os = "linux")]
fn migrated(files: &str) {
    refcode::post::remote::code(&refcode::STATE_MIGRATED, files);
}

#[cfg(target_os = "linux")]
fn said(line: &str) {
    eprintln!("{line}");
}

#[cfg(target_os = "linux")]
fn run() -> Result<(), String> {
    let listener = bind(release::SCHEDD_SOCKET)?;
    let state = std::path::Path::new(release::DATA_DIR).join("state");
    // No data file system yet: nothing to schedule until there is one.
    while !state.is_dir() {
        std::thread::sleep(std::time::Duration::from_secs(5));
    }
    release::trial::migrate_after(
        state.clone(),
        &[(
            "schedules.toml",
            release::trial::migrate::<schedd::model::Schedules>,
        )],
        migrated,
        said,
    );
    let svc = schedd::Service::start(schedd::Config {
        store: state::Store::at(&state),
        mgmt: Box::new(|| Box::new(mgmtproto::calls::Client::new(release::MGMTD_SOCKET))),
        clock: Box::new(appliance::time::now_secs),
        log: Box::new(|l| eprintln!("{l}")),
    });
    supervise::ready::signal().map_err(|e| format!("ready: {e}"))?;
    schedd::calls::serve(&listener, &svc);
    Err("the socket closed".into())
}

#[cfg(not(target_os = "linux"))]
fn main() {
    eprintln!("qsmcschedd is a service of a QS/MC console and runs on Linux only");
}
