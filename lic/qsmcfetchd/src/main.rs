// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! QS/MC's download service: `upd/fetchd` run as a service
//! (`docs/UPDATES.md`). As root it binds its socket and makes its two
//! folders its own; then it becomes its own user for good, which reads
//! no key of the console's, and serves. It is the only process of the
//! console that connects outward to anything but managed systems.

#[cfg(target_os = "linux")]
mod sys;

#[cfg(target_os = "linux")]
fn main() {
    if let Err(e) = run() {
        eprintln!("the download service stopped: {e}");
    }
    std::process::exit(1);
}

/// Bind `path` afresh, for root's alone: the web server and the local
/// console, root without capabilities, are its clients.
#[cfg(target_os = "linux")]
fn bind(path: &str) -> Result<std::os::unix::net::UnixListener, String> {
    use std::os::unix::fs::PermissionsExt as _;
    let _ = std::fs::remove_file(path);
    let l = std::os::unix::net::UnixListener::bind(path).map_err(|e| format!("{path}: {e}"))?;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
        .map_err(|e| format!("{path}: {e}"))?;
    Ok(l)
}

/// Make `dir` and everything in it the service's: what a restore or
/// an update's state copy wrote as root is its again.
#[cfg(target_os = "linux")]
fn own(dir: &std::path::Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    std::os::unix::fs::chown(dir, Some(release::FETCH_ID), Some(release::FETCH_ID))?;
    for e in std::fs::read_dir(dir)? {
        let p = e?.path();
        if p.is_dir() {
            own(&p)?;
        } else {
            std::os::unix::fs::chown(&p, Some(release::FETCH_ID), Some(release::FETCH_ID))?;
        }
    }
    Ok(())
}

/// Post a code through PID 1, whose socket's group this service has.
#[cfg(target_os = "linux")]
fn post(code: &'static refcode::RefCode, detail: &str) {
    refcode::post::remote::code(code, detail);
}

/// The console's offset from UTC now, from `host.toml`, which every
/// service may read.
#[cfg(target_os = "linux")]
fn offset() -> i64 {
    state::Store::at(std::path::Path::new(release::DATA_DIR).join("state"))
        .load::<state::Host>()
        .ok()
        .flatten()
        .map_or(0, |h| i64::from(h.zone().minutes) * 60)
}

#[cfg(target_os = "linux")]
fn run() -> Result<(), String> {
    use std::path::{Path, PathBuf};
    use std::time::Duration;

    let listener = bind(release::FETCHD_SOCKET)?;
    // No data file system yet: nothing to keep a download on.
    while !Path::new(release::DATA_DIR).join("state").is_dir() {
        std::thread::sleep(Duration::from_secs(5));
    }
    for dir in [release::FETCH_DIR, release::UPDATES_DIR] {
        own(Path::new(dir)).map_err(|e| format!("{dir}: {e}"))?;
    }
    sys::become_user(release::FETCH_ID, release::MGMT_ID)
        .map_err(|e| format!("becoming its user: {e}"))?;
    release::trial::migrate_after(
        PathBuf::from(release::FETCH_DIR),
        &[(
            "updates.toml",
            release::trial::migrate::<fetchd::settings::Settings>,
        )],
        |files| refcode::post::remote::code(&refcode::STATE_MIGRATED, files),
        |line| eprintln!("{line}"),
    );
    let roots = certs::roots::roots()
        .iter()
        .map(|r| r.der.clone())
        .collect();
    let svc = fetchd::Service::start(fetchd::Config {
        dir: PathBuf::from(release::FETCH_DIR),
        files: PathBuf::from(release::UPDATES_DIR),
        keys: PathBuf::from(fetchd::streams::KEYS_DIR),
        limit: fetchd::service::LIMIT,
        running: release::RELEASE,
        clock: Box::new(|| u64::try_from(appliance::time::now_secs()).unwrap_or(0)),
        offset: Box::new(offset),
        post: Box::new(post),
        log: Box::new(|l| eprintln!("{l}")),
        get: Box::new(fetchd::net::Http {
            roots,
            agent: format!("QSMC/{} {}", release::RELEASE, release::BUILD_ID),
        }),
    });
    // The automatic check, by the settings.
    let timer = svc.clone();
    std::thread::spawn(move || {
        loop {
            std::thread::sleep(Duration::from_mins(1));
            let now = u64::try_from(appliance::time::now_secs()).unwrap_or(0);
            if timer.due(now) {
                timer.check("the automatic check");
            }
        }
    });
    supervise::ready::signal().map_err(|e| format!("ready: {e}"))?;
    fetchd::proto::serve(&listener, &svc);
    Err("the socket closed".into())
}

#[cfg(not(target_os = "linux"))]
fn main() {
    eprintln!("qsmcfetchd is a service of a QS/MC console and runs on Linux only");
}
