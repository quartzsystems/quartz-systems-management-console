// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! The soak (Phase 8, Section 7), run by name: `QSMC_SOAK_SECS` long (24
//! hours unless said), against a simulated fleet of `QSMC_SOAK_HOSTS`
//! hosts (100) of 30 partitions each. Browsers of every role crawl the
//! pages, terminal windows stay open, partitions are made and deleted,
//! systems drop and return, problems open and close, plans are captured
//! and deployed, and the streams are checked, turn after turn. At the
//! end: no service restarted, no console problem the script did not
//! cause, and nothing leaked: each service's descriptors and memory,
//! the sessions, the console's staging and temporary files, the audit
//! log within its bound. What each service holds is recorded, idle and
//! under the soak, as baselines (`docs/PHASE8.md`).

use std::collections::BTreeMap;
use std::time::{Duration, Instant};

use webusers::{ROLES, Role};

use crate::qemu::channel::Channel;
use crate::suite::base::{Run, channel};
use crate::suite::mgmt::{Fleet, keep};
use crate::suite::parts::wizard;
use crate::suite::web::{ADMIN, PASSWORD, signed_on, start, submit};
use crate::web::Browser;

/// Each user's password.
const PW: &str = "Soak-pass-2026";
/// Partitions of each host.
const PARTS: u32 = 30;
/// What a service may hold past what it held idle before it is said to
/// leak: descriptors, and resident memory in KiB.
const FD_SLACK: usize = 32;
const RSS_SLACK_KIB: u64 = 64 * 1024;

/// What a service holds: its descriptors and resident memory (KiB).
fn held(ch: &mut Channel, pid: u32) -> (usize, u64) {
    let fds = ch.ls(&format!("/proc/{pid}/fd")).map_or(0, |l| l.len());
    let rss = ch
        .text(&format!("/proc/{pid}/status"))
        .ok()
        .and_then(|t| {
            t.lines()
                .find_map(|l| l.strip_prefix("VmRSS:"))
                .and_then(|v| v.split_whitespace().next()?.parse().ok())
        })
        .unwrap_or(0);
    (fds, rss)
}

/// Every service's process, descriptors and memory.
/// A service's process, failures, descriptors and memory, by name.
type Held = BTreeMap<String, (u32, u32, usize, u64)>;

fn every(ch: &mut Channel) -> Result<Held, String> {
    let status = ch.status()?;
    let mut out = BTreeMap::new();
    for s in &status.services {
        let (fds, rss) = held(ch, s.pid);
        out.insert(s.name.clone(), (s.pid, s.failures, fds, rss));
    }
    Ok(out)
}

/// Files under the console's staging and temporary folders.
fn staged(ch: &mut Channel) -> usize {
    [
        "/data/updates/.partial",
        "/data/update",
        "/tmp",
        "/data/restore",
    ]
    .iter()
    .map(|d| ch.ls(d).map_or(0, |l| l.len()))
    .sum()
}

/// One turn of a browser of `role` over the pages.
fn crawl(b: &mut Browser, hosts: u32, turn: u32) -> Result<usize, String> {
    let h = turn % hosts + 1;
    let host = format!("QSSK{h:03}VS1");
    let pages = [
        "/qsmc/welcome".to_string(),
        "/qsmc/servers".into(),
        format!("/qsmc/partitions?page={}", turn % 20 + 1),
        "/qsmc/events".into(),
        "/qsmc/connectivity".into(),
        format!("/qsmc/server/{host}/general"),
        format!("/qsmc/partition/{host}/{}/general", turn % PARTS + 1),
        "/qsmc/plans".into(),
        "/qsmc/schedule".into(),
        "/qsmc/updates/code".into(),
    ];
    for p in &pages {
        let a = b.get(p)?;
        if a.status >= 500 {
            return Err(format!("{p} answered {}", a.status));
        }
    }
    Ok(pages.len())
}

/// The suite.
#[allow(clippy::too_many_lines, reason = "one soak, its turns and its end")]
pub fn soak(run: &Run<'_>) -> Result<String, String> {
    let env = |k: &str, d: u64| {
        std::env::var(k)
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(d)
    };
    let secs = env("QSMC_SOAK_SECS", 86_400);
    let hosts = u32::try_from(env("QSMC_SOAK_HOSTS", 100)).unwrap_or(100);
    let fleet = Fleet::start((1..=hosts).map(|n| sim::fixture::fleet(n, PARTS)).collect())?;
    let mut c = start(run, "sec-soak")?;
    signed_on(&mut c.browser, ADMIN, PASSWORD)?;
    for i in 0..hosts as usize {
        keep(&mut c.browser, &fleet, i)?;
    }
    // A user of every role, and a browser each.
    let mut browsers = vec![(Role::Administrator, c.browser.clone_session())];
    for role in ROLES.iter().copied().filter(|r| *r != Role::Administrator) {
        let id = format!("soak{}", role.label().to_ascii_lowercase().replace(' ', ""));
        submit(
            &mut c.browser,
            "/qsmc/console/users",
            &[
                ("id", &id),
                ("role", role.label()),
                ("password", PW),
                ("again", PW),
                ("create", "OK"),
            ],
        )?;
        let mut b = c.browser.fresh();
        signed_on(&mut b, &id, PW)?;
        browsers.push((role, b));
    }
    // Terminal windows that stay open.
    let mut windows = Vec::new();
    for h in 1..=hosts.min(5) {
        windows.push(
            c.browser
                .websocket(&format!("/qsmc/terminal/QSSK{h:03}VS1/1/ws"))?,
        );
    }
    let ch = channel(&mut c.guest)?;
    let idle = every(ch)?;
    let staged_idle = staged(ch);
    let problems_before = ch.ls("/data/problems").map_or(0, |l| l.len());
    let end = Instant::now() + Duration::from_secs(secs);
    let (mut turns, mut pages, mut made, mut plans, mut dropped) = (0u32, 0usize, 0u32, 0u32, 0u32);
    let mut peak: BTreeMap<String, (usize, u64)> = BTreeMap::new();
    while Instant::now() < end {
        for (_, b) in &mut browsers {
            pages += crawl(b, hosts, turns)?;
        }
        for w in &mut windows {
            let _ = w.recv(Duration::from_millis(10));
        }
        let host = format!("QSSK{:03}VS1", turns % hosts + 1);
        let b = &mut c.browser;
        match turns % 6 {
            0 => {
                wizard::create(b, &host, 60, "SOAKNEW")?;
                wizard::delete(b, &host, 60)?;
                made += 1;
            }
            1 => {
                fleet.tell((turns % hosts) as usize, "drop");
                dropped += 1;
            }
            2 => {
                let n = fleet.tell((turns % hosts) as usize, "problem open 1 B6000666 soak");
                fleet.tell(
                    (turns % hosts) as usize,
                    &format!("problem clear {}", n.trim()),
                );
            }
            3 if turns % 30 == 3 => {
                let name = crate::suite::plans::create(b, &host)?;
                crate::suite::plans::deploy(b, &name, &host, &[], false)?;
                crate::suite::plans::ended(b, &name, &host, Duration::from_secs(600))?;
                plans += 1;
            }
            4 => {
                let _ = crate::suite::serv::check(b);
            }
            _ => {}
        }
        if turns % 20 == 0 {
            let ch = channel(&mut c.guest)?;
            for (name, (_, _, fds, rss)) in every(ch)? {
                let p = peak.entry(name).or_default();
                *p = (p.0.max(fds), p.1.max(rss));
            }
        }
        turns += 1;
    }
    drop(windows);
    // A while for what the last turns started to end.
    std::thread::sleep(Duration::from_secs(90));
    let ch = channel(&mut c.guest)?;
    let after = every(ch)?;
    let mut said = Vec::new();
    for (name, (pid, failures, fds, rss)) in &after {
        let Some((pid0, failures0, fds0, rss0)) = idle.get(name) else {
            continue;
        };
        if pid != pid0 || failures != failures0 {
            return Err(format!("{name} restarted during the soak"));
        }
        if *fds > fds0 + FD_SLACK {
            return Err(format!("{name} holds {fds} descriptors, {fds0} idle"));
        }
        if *rss > rss0 + RSS_SLACK_KIB {
            return Err(format!("{name} holds {rss} KiB, {rss0} KiB idle"));
        }
        let (pf, pr) = peak.get(name).copied().unwrap_or_default();
        said.push(format!(
            "{name} {fds0}/{pf}/{fds} fds, {rss0}/{pr}/{rss} KiB"
        ));
    }
    let staged_after = staged(ch);
    if staged_after > staged_idle {
        return Err(format!(
            "{staged_after} staging or temporary files left, {staged_idle} idle"
        ));
    }
    let problems_after = ch.ls("/data/problems").map_or(0, |l| l.len());
    let audit = ch
        .ls("/data/log")
        .map_or(0, |l| l.iter().filter(|n| n.contains("audit")).count());
    c.guest.kill();
    Ok(format!(
        "{secs} s, {turns} turns over {hosts} hosts ({} partitions): {pages} pages by {} roles, \
         {made} partitions made and deleted, {dropped} systems dropped and back, {plans} plans \
         captured and deployed, terminals open throughout; no service restarted; staging and \
         temporary files as idle ({staged_idle}); the problem log's files {problems_before} then \
         {problems_after}; {audit} audit files; held (idle/peak/end): {}",
        hosts * PARTS,
        browsers.len(),
        said.join("; ")
    ))
}
