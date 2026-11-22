// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! QS/MC's managed-systems service: `mgmt/mgmtd` run as a service
//! (`docs/MGMT.md`). As root it binds its two sockets; then it becomes
//! its own user for good, reads the console's QSI identity, which only
//! that user may read, and serves.

#[cfg(target_os = "linux")]
mod sys;

#[cfg(target_os = "linux")]
fn main() {
    if let Err(e) = run() {
        eprintln!("the managed-systems service stopped: {e}");
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

/// The console's QSI identity: waited for, as it is made at the first
/// IPL with a data file system.
#[cfg(target_os = "linux")]
fn identity(state: &std::path::Path) -> qsitrust::Identity {
    let mut said = false;
    loop {
        let read = |f: &str| std::fs::read_to_string(state.join(f));
        match (read("identity.crt"), read("identity.key")) {
            (Ok(c), Ok(k)) => match qsitrust::Identity::from_pem(&c, &k) {
                Ok(id) => return id,
                Err(e) if !said => eprintln!("the console's identity does not read: {e}"),
                Err(_) => {}
            },
            (Err(e), _) | (_, Err(e)) if !said => eprintln!("no console identity yet: {e}"),
            _ => {}
        }
        said = true;
        std::thread::sleep(std::time::Duration::from_secs(5));
    }
}

/// Post a code through PID 1, whose socket is shared with the service's
/// group; a busy PID 1 is tried again a few times.
#[cfg(target_os = "linux")]
fn post(code: &str, text: &str, about: &[supervise::control::About]) {
    use supervise::control::{Calls as _, Client};
    let mut c = Client::new(release::CONTROL_SOCKET);
    for _ in 0..5 {
        if c.post_about(code.to_string(), text.to_string(), about.to_vec())
            .is_ok()
        {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(200));
    }
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
    use std::path::Path;

    let calls = bind(release::MGMTD_SOCKET)?;
    let consoles = bind(release::MGMTD_CONSOLES)?;
    sys::become_user(release::MGMT_ID).map_err(|e| format!("becoming its user: {e}"))?;
    let data = Path::new(release::DATA_DIR);
    let state = data.join("state");
    let identity = identity(&state);
    // PID 1 set the host name at the IPL, from host.toml or the default.
    let name = std::fs::read_to_string("/proc/sys/kernel/hostname")
        .map_or_else(|_| "QSMC".into(), |n| n.trim().to_uppercase());
    eprintln!(
        "the console's QSI certificate is {}",
        identity.fingerprint().hex()
    );
    let mgmt = Path::new(release::MGMT_DIR);
    release::trial::migrate_after(
        mgmt.to_path_buf(),
        &[
            (
                "systems.toml",
                release::trial::migrate::<mgmtd::keep::files::Systems>,
            ),
            (
                "mgmt.toml",
                release::trial::migrate::<mgmtd::keep::files::Limits>,
            ),
        ],
        migrated,
        said,
    );
    let svc = mgmtd::Service::start(mgmtd::Config {
        identity,
        name,
        release: release::RELEASE.to_string(),
        state_dir: Some(mgmt.to_path_buf()),
        identity_dir: Some(state.clone()),
        history_dir: Some(mgmt.join("refcodes")),
        log: Box::new(|l| eprintln!("{l}")),
        post: Box::new(post),
    });
    let s2 = svc.clone();
    std::thread::spawn(move || mgmtd::service::consoles::serve(&consoles, &s2));
    supervise::ready::signal().map_err(|e| format!("ready: {e}"))?;
    mgmtproto::calls::serve(&calls, &svc);
    Err("the socket closed".into())
}

#[cfg(not(target_os = "linux"))]
fn main() {
    eprintln!("qsmcmgmtd is a service of a QS/MC console and runs on Linux only");
}
