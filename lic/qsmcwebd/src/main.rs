// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! QS/MC's web server: `web/webd` run as a service with the console's
//! pages (`web/pages`). It listens on nothing of its own: PID 1 hands
//! it 443 and 80 on the `open` address. Its own socket answers the
//! local console.

/// Say that state files were written in this release's form.
#[cfg(target_os = "linux")]
fn migrated(files: &str) {
    refcode::post::remote::code(&refcode::STATE_MIGRATED, files);
}

#[cfg(target_os = "linux")]
fn said(line: &str) {
    eprintln!("{line}");
}

/// The web server's state files, written in this release's form once
/// the slot that runs is not on trial.
#[cfg(target_os = "linux")]
const MIGRATE: &[(&str, release::trial::Step)] = &[
    (
        "webusers.toml",
        release::trial::migrate::<webusers::WebUsers>,
    ),
    ("web.toml", release::trial::migrate::<webusers::WebSettings>),
    (
        "trusted.toml",
        release::trial::migrate::<webpages::certs_routes::Trusted>,
    ),
    (
        "backups.toml",
        release::trial::migrate::<webpages::backup::History>,
    ),
    (
        "groups.toml",
        release::trial::migrate::<webpages::groups::Groups>,
    ),
];

#[cfg(target_os = "linux")]
fn main() {
    use std::path::Path;
    use std::sync::Arc;

    let state = Path::new(release::DATA_DIR).join("state");
    release::trial::migrate_after(state.clone(), MIGRATE, migrated, said);
    let users_db = state.join(users::FILE);
    let app = webpages::App::open(&state, &users_db, Box::new(webpages::app::live::Live));
    // It returns only when it cannot go on; supervision starts it again.
    let stopped = app.map(Arc::new).and_then(|app| {
        // The local console's requests (option 4), on their own socket.
        webpages::app::control::serve(Arc::clone(&app))
            .map_err(|e| format!("{}: {e}", release::WEBD_SOCKET))?;
        webd::run::run(app)
    });
    if let Err(e) = stopped {
        eprintln!("the web server stopped: {e}");
    }
    std::process::exit(1);
}

#[cfg(not(target_os = "linux"))]
fn main() {
    eprintln!("qsmcwebd is a service of a QS/MC console and runs on Linux only");
}
