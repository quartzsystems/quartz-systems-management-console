// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! Phase 3's suites: the web interface reached from outside, as a
//! browser reaches it, on a console installed from the test image's
//! ISO. QEMU forwards a host port to the guest's 443 and one to its 80;
//! the browser trusts only the certificate the console has, read
//! through the test hooks' channel ([`crate::web`]).

mod backup;
mod console;
mod hostile;
mod pages;
mod settings;
mod signon;

use std::time::{Duration, Instant};

use super::base::{Guest, Run, SuiteFn, channel};
use super::install::{self, Kind};
use crate::qemu::session::Machine;
use crate::web::{Browser, free_port, sha256_of_pem};

/// The suites, by the handoff's numbers (1 is Phase 0 to 2's).
pub const SUITES: &[(&str, SuiteFn)] = &[
    ("web-pages", pages::pages),
    ("web-first", console::first),
    ("web-roles", pages::roles),
    ("web-signon", signon::rules),
    ("web-users", signon::users),
    ("web-network", settings::network),
    ("web-certs", settings::certs),
    ("web-backup", backup::backup),
    ("web-slot", console::slot),
    ("web-listeners", settings::listeners),
    ("web-option4", console::option4),
    ("web-hostile", hostile::hostile),
    ("web-secrets", hostile::secrets),
    ("web-processor", console::processor),
];

/// The first Administrator: the installation's user, whose ID the web
/// interface keeps in lower case.
pub const ADMIN: &str = "admin";
/// Its password, the installation's.
pub const PASSWORD: &str = install::PASSWORD;

/// The web certificate on the guest.
pub const CERT: &str = "/data/web/certificate.pem";

/// A console and a browser of it.
pub struct Console<'a> {
    /// The guest.
    pub guest: Guest<'a>,
    /// A browser that trusts its certificate.
    pub browser: Browser,
    /// The machine, for a boot again.
    pub machine: Machine,
}

/// The machine of an installed console with the browser's forwards.
pub fn machine(run: &Run<'_>, tag: &str) -> Result<Machine, String> {
    let disks = install::installed(run, Kind::Single, tag)?;
    let mut m = install::installed_machine(run, &disks);
    m.forwards = vec![(free_port()?, 443), (free_port()?, 80)];
    Ok(m)
}

/// The SHA-256 of the certificate the guest serves now.
pub fn served(guest: &mut Guest<'_>) -> Result<String, String> {
    sha256_of_pem(&channel(guest)?.cat(CERT)?)
}

/// Wait until the web server is ready.
pub fn webd_ready(guest: &mut Guest<'_>, within: Duration) -> Result<(), String> {
    let until = Instant::now() + within;
    loop {
        let status = channel(guest)?.status()?;
        if status
            .services
            .iter()
            .any(|s| s.name == "webd" && s.state == "ready")
        {
            return Ok(());
        }
        if Instant::now() > until {
            return Err(format!(
                "the web server is not ready: {:?}",
                status.services.iter().find(|s| s.name == "webd")
            ));
        }
        std::thread::sleep(Duration::from_millis(500));
    }
}

/// Wait until 443 listens: the web server is ready before DHCP has
/// given the console its address, and listens from then.
pub fn listening_443(guest: &mut Guest<'_>, within: Duration) -> Result<(), String> {
    let until = Instant::now() + within;
    loop {
        let tcp = channel(guest)?.text("/proc/net/tcp")?;
        if crate::suite::base::network::listening(&tcp, false)
            .iter()
            .any(|(_, p)| *p == release::HTTPS_PORT)
        {
            return Ok(());
        }
        if Instant::now() > until {
            return Err("nothing listens on 443".into());
        }
        std::thread::sleep(Duration::from_millis(500));
    }
}

/// Boot `machine` to a ready web server and a browser of it.
pub fn boot<'a>(run: &Run<'a>, machine: Machine) -> Result<Console<'a>, String> {
    let mut guest = install::boot_installed(run, &machine, run.launch.timeout)?;
    webd_ready(&mut guest, Duration::from_secs(60))?;
    listening_443(&mut guest, Duration::from_secs(60))?;
    let (https, http) = (machine.forwards[0].0, machine.forwards[1].0);
    // PID 1 binds 443 before the web server has the set, and the web
    // server may make its certificate again for the address first: the
    // certificate read is the one served once a request is answered.
    let until = Instant::now() + Duration::from_secs(30);
    loop {
        let sha = served(&mut guest)?;
        let mut browser = Browser::new(https, http, &sha)?;
        match browser.get(webd::product::CSS_PATH) {
            Ok(_) => {
                return Ok(Console {
                    guest,
                    browser,
                    machine,
                });
            }
            Err(e) if Instant::now() > until => {
                return Err(format!("the certificate read is not the one served: {e}"));
            }
            Err(_) => std::thread::sleep(Duration::from_millis(500)),
        }
    }
}

/// An installed console, booted, with a browser.
pub fn start<'a>(run: &Run<'a>, tag: &str) -> Result<Console<'a>, String> {
    let m = machine(run, tag)?;
    boot(run, m)
}

/// Sign `b` on as `user`; the error says what the page said instead.
pub fn signed_on(b: &mut Browser, user: &str, password: &str) -> Result<(), String> {
    let r = b.sign_on(user, password)?;
    if r.status == 303 && r.header("location") == Some("/qsmc/welcome") {
        Ok(())
    } else {
        Err(format!(
            "{user} was not signed on ({}): {}",
            r.status,
            message_of(&r.text())
        ))
    }
}

/// The message rows' identifiers and texts of a page, for a report.
pub fn message_of(page: &str) -> String {
    page.split("class=\"msgid\"")
        .skip(1)
        .filter_map(|rest| {
            let (_, after) = rest.split_once('>')?;
            let (id, tail) = after.split_once("</a>")?;
            let text = tail.trim_start_matches(" - ").split('<').next()?;
            Some(format!("{id} {text}"))
        })
        .collect::<Vec<_>>()
        .join("; ")
}

/// Whether `page` shows the message `id`.
pub fn shows(page: &str, id: &str) -> bool {
    page.contains(&format!(">{id}</a>"))
}

/// A form's POST with its token, the page's, first.
pub fn submit(
    b: &mut Browser,
    path: &str,
    fields: &[(&str, &str)],
) -> Result<crate::web::Answer, String> {
    let token = b.csrf(path)?;
    let mut all = vec![("csrf", token.as_str())];
    all.extend_from_slice(fields);
    b.post(path, &all)
}

/// The page a redirect leads to, followed once.
pub fn follow(b: &mut Browser, a: &crate::web::Answer) -> Result<crate::web::Answer, String> {
    match a.header("location") {
        Some(to) if a.status == 303 || a.status == 301 => {
            let to = to.to_string();
            b.get(&to)
        }
        _ => Ok(a.clone()),
    }
}
