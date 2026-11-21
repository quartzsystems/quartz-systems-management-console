// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! Phase 4's suites: a console in QEMU managing simulated systems
//! (`tools/sim`) on the build machine, which the guest reaches at
//! `10.0.2.2` on QEMU's user network, through its web interface as a
//! browser does; and a real QS/VS host ([`host`]).

mod act;
mod beside;
mod guard;
pub mod host;
mod pair;
mod standalone;
mod terminal;

use std::time::{Duration, Instant};

use super::base::SuiteFn;
use super::web::{follow, shows, submit};
use crate::web::{Answer, Browser};

/// The suites, by the handoff's numbers.
pub const SUITES: &[(&str, SuiteFn)] = &[
    ("mgmt-pages", pair::pages),
    ("mgmt-pair", pair::pair),
    ("mgmt-sessions", pair::sessions),
    ("mgmt-lists", pair::lists),
    ("mgmt-power", act::power),
    ("mgmt-refcodes", act::refcodes),
    ("mgmt-terminal", terminal::terminal),
    ("mgmt-panel", act::panel),
    ("mgmt-roles", guard::roles),
    ("mgmt-hostile", guard::hostile),
    ("mgmt-outbound", guard::outbound),
    ("mgmt-backup", guard::backup),
    ("mgmt-audit", guard::audit),
];

/// The suites run by name only: suite 10 needs a QS/VS test disk.
pub const NAMED: &[(&str, SuiteFn)] = &[
    ("mgmt-host", host::host),
    ("mgmt-standalone", standalone::standalone),
];

/// The build machine as the guest reaches it on QEMU's user network.
pub const HOST: &str = "10.0.2.2";

/// Simulated managed systems on the build machine.
pub struct Fleet {
    /// The systems.
    pub systems: Vec<sim::Shared>,
    /// The port each listens on.
    pub ports: Vec<u16>,
}

impl Fleet {
    /// `systems`, each listening on a port of its own.
    pub fn start(systems: Vec<sim::Shared>) -> Result<Self, String> {
        let mut ports = Vec::new();
        for s in &systems {
            let at = sim::net::listen(s.clone(), "127.0.0.1:0").map_err(|e| e.to_string())?;
            ports.push(at.port());
        }
        Ok(Self { systems, ports })
    }

    /// The prototype's systems: the two at headquarters, then the
    /// branch hosts.
    pub fn prototype() -> Result<Self, String> {
        Self::start(sim::fixture::prototype())
    }

    /// The system named `name`.
    pub fn index(&self, name: &str) -> usize {
        self.systems
            .iter()
            .position(|s| sim::lock(s).name() == name)
            .unwrap_or(0)
    }

    /// Tell system `i` `line`.
    pub fn tell(&self, i: usize, line: &str) -> String {
        let name = sim::lock(&self.systems[i]).name().to_string();
        let (cmd, rest) = line.split_once(' ').unwrap_or((line, ""));
        let line = format!("{cmd} {name} {rest}");
        sim::control::command(std::slice::from_ref(&self.systems[i]), &line)
    }

    /// What system `i` logged.
    pub fn lines(&self, i: usize) -> String {
        sim::lock(&self.systems[i]).lines.join("\n")
    }
}

/// Add system `i` of `fleet` on the Add managed system page, with the
/// password it shows: the result page.
pub fn add(b: &mut Browser, fleet: &Fleet, i: usize) -> Result<Answer, String> {
    let pw = fleet.tell(i, "pair");
    let port = fleet.ports[i].to_string();
    let r = submit(
        b,
        "/qsmc/servers/add",
        &[
            ("host", HOST),
            ("port", &port),
            ("pw", &pw),
            ("connect", "Connect"),
        ],
    )?;
    if !shows(&r.text(), "QMC1001") {
        return Err(format!(
            "system {i} was not added: {}",
            super::web::message_of(&r.text())
        ));
    }
    Ok(r)
}

/// Add and keep system `i`: the result's Add.
pub fn keep(b: &mut Browser, fleet: &Fleet, i: usize) -> Result<(), String> {
    add(b, fleet, i)?;
    let name = sim::lock(&fleet.systems[i]).name().to_string();
    let r = submit(b, "/qsmc/servers/add", &[("ms", &name), ("add", "Add")])?;
    let page = follow(b, &r)?;
    if shows(&page.text(), "QMC1005") {
        Ok(())
    } else {
        Err(format!(
            "{name} was not kept: {}",
            super::web::message_of(&page.text())
        ))
    }
}

/// Wait until `ok`, or say what did not happen.
pub fn until(
    what: &str,
    within: Duration,
    mut ok: impl FnMut() -> Result<bool, String>,
) -> Result<(), String> {
    let end = Instant::now() + within;
    loop {
        if ok()? {
            return Ok(());
        }
        if Instant::now() > end {
            return Err(format!("{what} did not happen in {} s", within.as_secs()));
        }
        std::thread::sleep(Duration::from_millis(500));
    }
}

/// The lines of the console's logs that hold `what`, the last 40.
pub fn logs(guest: &mut crate::qemu::session::Session, what: &[&str]) -> String {
    let Ok(ch) = crate::suite::base::channel(guest) else {
        return String::new();
    };
    let mut lines = Vec::new();
    for name in ch.ls("/data/log").unwrap_or_default() {
        if let Ok(bytes) = ch.cat(&format!("/data/log/{name}")) {
            for l in String::from_utf8_lossy(&bytes).lines() {
                if what.iter().any(|w| l.contains(w)) {
                    lines.push(l.to_string());
                }
            }
        }
    }
    let from = lines.len().saturating_sub(40);
    lines[from..].join("\n")
}

/// The text of `path`, which must answer 200.
pub fn page(b: &mut Browser, path: &str) -> Result<String, String> {
    let a = b.get(path)?;
    if a.status == 200 {
        Ok(a.text())
    } else {
        Err(format!("{path} answered {}", a.status))
    }
}
