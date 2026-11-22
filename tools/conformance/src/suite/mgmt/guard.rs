// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! Suite 11: this phase's routes for each role, as the matrix says.
//! Suite 12: hostile peers disturb nothing else. Suite 13: the web
//! server holds no QSI key and connects nowhere; `mgmtd` only to its
//! systems. Suite 14: a backup restored resumes the sessions without
//! pairing. Suite 15: every action audited with its user, no secret.

use std::time::{Duration, Instant};

use webusers::{Matrix, ROLES, Role, Task};

use super::{Fleet, keep, page, until};
use crate::suite::base::channel;
use crate::suite::web::{
    ADMIN, PASSWORD, boot, follow, machine, message_of, shows, signed_on, start, submit,
};

/// This phase's routes and the task each needs.
const ROUTES: [(&str, Task); 18] = [
    ("/qsmc/connectivity", Task::Events),
    ("/qsmc/servers", Task::View),
    ("/qsmc/servers/add", Task::Settings),
    ("/qsmc/servers/reset?ms=QSHQVS1", Task::Settings),
    ("/qsmc/confirm/system-remove?ms=QSHQVS1", Task::Settings),
    ("/qsmc/server/QSHQVS1/general", Task::View),
    ("/qsmc/server/QSHQVS1/processors", Task::View),
    ("/qsmc/server/QSHQVS1/memory", Task::View),
    ("/qsmc/server/QSHQVS1/io", Task::View),
    ("/qsmc/server/QSHQVS1/power", Task::View),
    ("/qsmc/server/QSHQVS1/capabilities", Task::View),
    ("/qsmc/server/QSHQVS1/refcode", Task::View),
    ("/qsmc/server/QSHQVS1/confirm?op=poweroff", Task::Power),
    ("/qsmc/refcodes?ms=QSHQVS1", Task::Events),
    ("/qsmc/refcode/A6005008?ms=QSHQVS1&p=2", Task::Events),
    ("/qsmc/terminal", Task::View),
    ("/qsmc/partition/QSHQVS1/2/panel", Task::View),
    ("/qsmc/confirm/panel?ms=QSHQVS1&p=1&f=08", Task::Terminal),
];

/// Suite 11.
pub fn roles(run: &crate::suite::base::Run<'_>) -> Result<String, String> {
    let fleet = Fleet::start(vec![sim::fixture::qshqvs1()])?;
    let mut c = start(run, "mgmt-roles")?;
    let b = &mut c.browser;
    signed_on(b, ADMIN, PASSWORD)?;
    keep(b, &fleet, 0)?;
    let matrix = Matrix::default();
    let mut pairs = 0;
    for role in ROLES {
        let mut who = if role == Role::Administrator {
            b.clone_session()
        } else {
            let id = role.label().to_lowercase();
            // A password may not hold the user ID.
            let pw = "Roles-check-42".to_string();
            submit(
                b,
                "/qsmc/console/users",
                &[
                    ("id", &id),
                    ("role", role.label()),
                    ("password", &pw),
                    ("again", &pw),
                    ("create", "OK"),
                ],
            )?;
            let mut nb = b.fresh();
            signed_on(&mut nb, &id, &pw)?;
            nb
        };
        for (route, task) in ROUTES {
            let a = who.get(route)?;
            let want = if matrix.allows(role, task) { 200 } else { 403 };
            if a.status != want {
                return Err(format!(
                    "{} on {route}: {} not {want}",
                    role.label(),
                    a.status
                ));
            }
            pairs += 1;
        }
    }
    c.guest.kill();
    Ok(format!(
        "{pairs} route and role pairs of this phase, each the page or Not authorized as the matrix says"
    ))
}

/// Suite 12.
pub fn hostile(run: &crate::suite::base::Run<'_>) -> Result<String, String> {
    let fleet = Fleet::start((1..=5).map(sim::fixture::branch).collect())?;
    let mut c = start(run, "mgmt-hostile")?;
    let b = &mut c.browser;
    signed_on(b, ADMIN, PASSWORD)?;
    for i in 0..5 {
        keep(b, &fleet, i)?;
    }
    let restarts = |c: &mut crate::suite::web::Console<'_>| -> Result<u32, String> {
        let s = channel(&mut c.guest)?.status()?;
        Ok(s.services
            .iter()
            .filter(|x| x.name == "mgmtd" || x.name == "webd")
            .map(|x| x.failures)
            .sum())
    };
    super::until("QSBR01VS1 connected", Duration::from_secs(30), || {
        Ok(connected(&page(&mut c.browser, "/qsmc/connectivity")?))
    })?;
    let before = restarts(&mut c)?;
    fleet.tell(1, "nonsense");
    fleet.tell(2, "oversize");
    fleet.tell(3, "flood on");
    fleet.tell(4, "no-credit on");
    for i in 1..=2 {
        fleet.tell(i, "drop");
    }
    // The good system's session stays up while the others misbehave,
    // and the pages answer.
    let t0 = Instant::now();
    let mut slowest = Duration::ZERO;
    while t0.elapsed() < Duration::from_secs(60) {
        let t = Instant::now();
        let conn = page(&mut c.browser, "/qsmc/connectivity")?;
        slowest = slowest.max(t.elapsed());
        if !connected(&conn) {
            let l = super::logs(&mut c.guest, &["QSBR0", "mgmtd", "panicked"]);
            return Err(format!(
                "the good system's session went after {} s: {}\n{l}",
                t0.elapsed().as_secs(),
                row(&conn)
            ));
        }
        std::thread::sleep(Duration::from_secs(3));
    }
    let after = restarts(&mut c)?;
    if after != before {
        return Err(format!("the services failed {} times", after - before));
    }
    c.guest.kill();
    Ok(format!(
        "frames that are no frames, a frame past every limit, a flood of events and a console \
         stream granted no credit: the good system's session stayed connected for 60 s, the \
         pages answered within {} ms, mgmtd and webd never failed",
        slowest.as_millis()
    ))
}

/// Connectivity's row of `QSBR01VS1`, the good system.
fn row(conn: &str) -> &str {
    let Some(at) = conn.find(">QSBR01VS1<") else {
        return "";
    };
    let start = conn[..at].rfind("<tr").unwrap_or(at);
    let end = conn[at..].find("</tr>").map_or(conn.len(), |e| at + e);
    &conn[start..end]
}

fn connected(conn: &str) -> bool {
    row(conn).contains(">Connected<")
}

/// The socket inodes `pid` holds.
pub(crate) fn sockets(
    ch: &mut crate::qemu::channel::Channel,
    pid: u32,
) -> Result<Vec<String>, String> {
    let mut v = Vec::new();
    for fd in ch.ls(&format!("/proc/{pid}/fd"))? {
        if let Ok(t) = ch.readlink(&format!("/proc/{pid}/fd/{fd}"))
            && let Some(inode) = t.strip_prefix("socket:[").and_then(|t| t.strip_suffix(']'))
        {
            v.push(inode.to_string());
        }
    }
    Ok(v)
}

/// The TCP connections (local port, remote address, remote port) of
/// `inodes`.
pub(crate) fn connections(tcp: &str, inodes: &[String]) -> Vec<(u16, String, u16)> {
    tcp.lines()
        .skip(1)
        .filter_map(|l| {
            let f: Vec<&str> = l.split_whitespace().collect();
            if f.len() < 10 || !inodes.iter().any(|i| i == f[9]) {
                return None;
            }
            let port = |s: &str| u16::from_str_radix(s.split(':').nth(1)?, 16).ok();
            let ip = f[2]
                .split(':')
                .next()
                .and_then(|h| u32::from_str_radix(h, 16).ok())?;
            let addr = std::net::Ipv4Addr::from(ip.swap_bytes()).to_string();
            Some((port(f[1])?, addr, port(f[2])?))
        })
        .filter(|(_, a, _)| a != "0.0.0.0")
        .collect()
}

/// Suite 13.
pub fn outbound(run: &crate::suite::base::Run<'_>) -> Result<String, String> {
    let fleet = Fleet::start((1..=3).map(sim::fixture::branch).collect())?;
    let mut c = start(run, "mgmt-outbound")?;
    let b = &mut c.browser;
    signed_on(b, ADMIN, PASSWORD)?;
    for i in 0..3 {
        keep(b, &fleet, i)?;
    }
    page(b, "/qsmc/servers")?;
    let ch = channel(&mut c.guest)?;
    // The key is mgmtd's user's alone; the web server, root without a
    // capability, cannot read it.
    let (mode, uid, _) = ch.stat("/data/state/identity.key")?;
    if mode != "600" || uid != release::MGMT_ID {
        return Err(format!("the key is mode {mode}, user {uid}"));
    }
    let status = ch.status()?;
    let pid = |name: &str| {
        status
            .service(name)
            .map(|s| s.pid)
            .ok_or(format!("no {name}"))
    };
    let (web, mgmt) = (pid("webd")?, pid("mgmtd")?);
    let web_status = ch.text(&format!("/proc/{web}/status"))?;
    if !web_status.contains("CapEff:\t0000000000000000") {
        return Err("the web server has a capability".into());
    }
    let mgmt_status = ch.text(&format!("/proc/{mgmt}/status"))?;
    if !mgmt_status.contains(&format!("Uid:\t{0}\t{0}\t{0}", release::MGMT_ID)) {
        return Err("mgmtd does not run as its own user".into());
    }
    let tcp = ch.text("/proc/net/tcp")?;
    let webs = connections(&tcp, &sockets(ch, web)?);
    if let Some(out) = webs.iter().find(|(l, _, _)| *l != 443 && *l != 80) {
        return Err(format!("the web server connects out: {out:?}"));
    }
    let mgmts = connections(&tcp, &sockets(ch, mgmt)?);
    for (_, addr, port) in &mgmts {
        if addr != super::HOST || !fleet.ports.contains(port) {
            return Err(format!(
                "mgmtd connects to {addr}:{port}, no system it paired"
            ));
        }
    }
    if mgmts.len() < 3 {
        return Err(format!("mgmtd holds {} sessions, not 3", mgmts.len()));
    }
    c.guest.kill();
    Ok(format!(
        "identity.key mode 600, user {}; the web server root with no capability, {} connections \
         all to 443 or 80; mgmtd as user {} with {} connections, each to a paired system",
        release::MGMT_ID,
        webs.len(),
        release::MGMT_ID,
        mgmts.len()
    ))
}

/// Suite 14.
pub fn backup(run: &crate::suite::base::Run<'_>) -> Result<String, String> {
    const PASS: &str = "suite backup passphrase";
    let fleet = Fleet::start(vec![sim::fixture::qshqvs1()])?;
    let mut a = boot(run, machine(run, "mgmt-backup-a")?)?;
    signed_on(&mut a.browser, ADMIN, PASSWORD)?;
    keep(&mut a.browser, &fleet, 0)?;
    let r = submit(
        &mut a.browser,
        "/qsmc/console/backup",
        &[
            ("d", "on"),
            ("w", "file"),
            ("pp", PASS),
            ("back_up_now", "x"),
        ],
    )?;
    let file = r.body.clone();
    let opened = qmcbackup::seal::open(&file, PASS).map_err(|e| e.to_string())?;
    for p in ["mgmt/systems.toml", "state/identity.key"] {
        if !opened.entries.iter().any(|e| e.path == p) {
            return Err(format!("the backup lacks {p}"));
        }
    }
    let _ = crate::suite::base::power_off(&mut a.guest);
    drop(a);
    let mut b = boot(run, machine(run, "mgmt-backup-b")?)?;
    signed_on(&mut b.browser, ADMIN, PASSWORD)?;
    let token = b.browser.csrf("/qsmc/console/backup?tab=restore")?;
    let page_ = b.browser.upload(
        "/qsmc/console/backup?tab=restore",
        &[("csrf", &token), ("pp", PASS), ("open", "Open")],
        &file,
    )?;
    if !page_.text().contains("The backup holds") {
        return Err(format!(
            "the backup did not open: {}",
            message_of(&page_.text())
        ));
    }
    let _ = submit(&mut b.browser, "/qsmc/confirm/restore", &[("ok", "OK")]);
    b.guest
        .wait_exit(Instant::now() + Duration::from_secs(60))
        .map_err(|e| format!("the console did not restart: {e}"))?;
    let m = b.machine.clone();
    drop(b);
    let pairings = fleet.lines(0).matches("pairing from user").count();
    let mut r = boot(run, m)?;
    let sha = crate::suite::web::served(&mut r.guest)?;
    r.browser.pin(&sha)?;
    signed_on(&mut r.browser, ADMIN, PASSWORD)?;
    until(
        "the restored console's session",
        Duration::from_secs(90),
        || Ok(page(&mut r.browser, "/qsmc/connectivity")?.contains(">Connected<")),
    )?;
    if fleet.lines(0).matches("pairing from user").count() != pairings {
        return Err("the restored console paired again".into());
    }
    let _ = crate::suite::base::power_off(&mut r.guest);
    Ok(format!(
        "the backup holds mgmt/systems.toml and the console's QSI key; restored on a second \
         console, which then connected to QSHQVS1 without pairing ({pairings} pairing at the host \
         in all)"
    ))
}

/// Suite 15.
pub fn audit(run: &crate::suite::base::Run<'_>) -> Result<String, String> {
    let fleet = Fleet::start(vec![sim::fixture::qshqvs1()])?;
    let mut c = start(run, "mgmt-audit")?;
    let b = &mut c.browser;
    signed_on(b, ADMIN, PASSWORD)?;
    keep(b, &fleet, 0)?;
    let pw = fleet.tell(0, "pair");
    submit(
        b,
        "/qsmc/servers/reset",
        &[("ms", "QSHQVS1"), ("pw", &pw), ("connect", "Connect")],
    )?;
    b.get("/qsmc/connectivity?test=QSHQVS1")?;
    submit(
        b,
        "/qsmc/server/QSHQVS1/general",
        &[("n", "QSHQVS1"), ("d", "Lab host"), ("apply", "Apply")],
    )?;
    submit(
        b,
        "/qsmc/server/QSHQVS1/power",
        &[("side", "B"), ("apply", "Apply")],
    )?;
    fleet.tell(0, "refuse-power QVS1011 Two partitions are running.");
    let r = submit(
        b,
        "/qsmc/server/QSHQVS1/confirm?op=poweroff",
        &[("ms", "QSHQVS1"), ("ok", "OK")],
    )?;
    let _ = follow(b, &r)?;
    submit(
        b,
        "/qsmc/partition/QSHQVS1/1/panel",
        &[("func", "11 - x"), ("select", "Select")],
    )?;
    b.get("/qsmc/terminal/QSHQVS1/1")?;
    b.get("/qsmc/terminal/QSHQVS1/1/print")?;
    submit(
        b,
        "/qsmc/confirm/system-remove?ms=QSHQVS1",
        &[("ms", "QSHQVS1"), ("ok", "OK")],
    )?;
    std::thread::sleep(Duration::from_secs(3));
    let ch = channel(&mut c.guest)?;
    let mut logs = String::new();
    for name in ch.ls("/data/log")? {
        if let Ok(bytes) = ch.cat(&format!("/data/log/{name}")) {
            logs.push_str(&String::from_utf8_lossy(&bytes));
        }
    }
    let wants = [
        "add managed system at 10.0.2.2",
        "reset connection QSHQVS1: paired again",
        "test connection QSHQVS1",
        "describe QSHQVS1",
        "IPL side, keylock or boot mode of QSHQVS1",
        "power off QSHQVS1",
        "refused: QVS1011",
        "control panel function 11 of QSHQVS1 partition 1",
        "open console of QSHQVS1 partition 1",
        "print screen of QSHQVS1 partition 1",
        "remove managed system QSHQVS1",
    ];
    for want in wants {
        if !logs
            .lines()
            .any(|l| l.contains(&format!("audit: user {ADMIN} from ")) && l.contains(want))
        {
            return Err(format!("no audit record of `{want}`"));
        }
    }
    if logs.contains(&pw) {
        return Err("a log holds the pairing password".into());
    }
    let key = ch.cat("/data/state/identity.key").unwrap_or_default();
    let key_line = String::from_utf8_lossy(&key)
        .lines()
        .nth(1)
        .unwrap_or("")
        .to_string();
    if !key_line.is_empty() && logs.contains(&key_line) {
        return Err("a log holds the QSI key".into());
    }
    let _ = shows;
    c.guest.kill();
    Ok(format!(
        "{} kinds of this phase's action each audited with the user; {} bytes of logs hold no \
         pairing password and no key",
        wants.len(),
        logs.len()
    ))
}
