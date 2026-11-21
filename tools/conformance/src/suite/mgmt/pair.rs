// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! Suite 2: the pages live with the fixture's systems, every link
//! answering and only the terminal window with a script. Suite 3:
//! pairing, good and refused, the password in no log. Suite 4: sessions
//! dropped, away, changed, a hundred of them. Suite 5: lists never
//! wait.

use std::collections::{BTreeSet, VecDeque};
use std::time::{Duration, Instant};

use super::{Fleet, HOST, keep, page, until};
use crate::suite::web::{ADMIN, PASSWORD, message_of, shows, signed_on, start, submit};

fn links(page: &str) -> Vec<String> {
    qsweb::golden::tokens(page)
        .into_iter()
        .filter_map(|t| match t {
            qsweb::golden::Tok::Open(n, a) if n == "a" || n == "img" => {
                a.get("href").or_else(|| a.get("src")).cloned()
            }
            _ => None,
        })
        .map(|h| h.split('#').next().unwrap_or("").replace("&amp;", "&"))
        .filter(|h| h.starts_with("/qsmc/"))
        .collect()
}

/// Whether `path` is a terminal window, the one page with a script.
fn window(path: &str) -> bool {
    let p = path.split('?').next().unwrap_or("");
    p.starts_with("/qsmc/terminal/") && p.matches('/').count() == 4
}

/// Suite 2.
#[allow(clippy::too_many_lines, reason = "one scenario, step by step")]
pub fn pages(run: &crate::suite::base::Run<'_>) -> Result<String, String> {
    let fleet = Fleet::prototype()?;
    let mut c = start(run, "mgmt-pages")?;
    let b = &mut c.browser;
    signed_on(b, ADMIN, PASSWORD)?;
    for i in 0..fleet.systems.len() {
        keep(b, &fleet, i)?;
    }
    // The branch hosts the paged list shows not connected.
    for n in 1..=25u32 {
        if sim::fixture::branch_away(n) {
            fleet.tell(fleet.index(&format!("QSBR{n:02}VS1")), "down");
        }
    }
    until(
        "the fixture's states on Welcome",
        Duration::from_secs(120),
        || {
            let w = page(b, "/qsmc/welcome")?;
            Ok(w.contains(">27</a>") && w.contains("<span class=\"err\">3</span>"))
        },
    )?;
    // The fixture's values, as the prototype shows them: the branch
    // hosts first by name, headquarters on page 2.
    let first = page(b, "/qsmc/servers")?;
    for want in [
        "QSBR01VS1",
        "QSBR25VS1",
        "10-9F1818",
        "Page 1 of 2",
        "27 entries",
    ] {
        if !first.contains(want) {
            return Err(format!("Servers lacks `{want}`"));
        }
    }
    let second = page(b, "/qsmc/servers?page=2")?;
    for want in ["QSHQVS1", "3.50", "40.0", "10-9F11A2", "9Q04-M2"] {
        if !second.contains(want) {
            return Err(format!("Servers' page 2 lacks `{want}`"));
        }
    }
    let memory = page(b, "/qsmc/server/QSHQVS1/memory")?;
    for want in [
        "131,072 MB",
        "40,960 MB",
        "65,536",
        "98,304",
        "0 (not activated)",
    ] {
        if !memory.contains(want) {
            return Err(format!("Memory lacks `{want}`"));
        }
    }
    // Every link of every page, from Welcome, answers; only a terminal
    // window carries a script.
    let mut seen = BTreeSet::new();
    let mut queue = VecDeque::from([
        "/qsmc/welcome".to_string(),
        "/qsmc/terminal/QSHQVS1/1".to_string(),
        "/qsmc/partition/QSHQVS1/2/panel".to_string(),
    ]);
    let (mut pages, mut scripts) = (0, 0);
    while let Some(path) = queue.pop_front() {
        let bare = path.split('?').next().unwrap_or("").to_string();
        if !seen.insert(path.clone()) || bare.starts_with("/qsmc/signoff") || bare.ends_with("/ws")
        {
            continue;
        }
        let a = b.get(&path)?;
        // `/qsmc/` (the help window's Close window) leads to Welcome.
        if a.status == 303 {
            match a.header("location") {
                Some(l) if l.starts_with("/qsmc/") => {
                    queue.push_back(l.to_string());
                    continue;
                }
                l => return Err(format!("{path} answered 303 to {l:?}")),
            }
        }
        if a.status != 200 {
            return Err(format!("{path} answered {}", a.status));
        }
        pages += 1;
        let text = a.text().to_ascii_lowercase();
        if text.contains("<script") {
            if !window(&path) {
                return Err(format!("{path} carries a script"));
            }
            scripts += 1;
            if !a
                .header("content-security-policy")
                .is_some_and(|p| p.contains("script-src 'self'"))
            {
                return Err(format!("{path}'s policy does not allow its script"));
            }
        } else if a
            .header("content-security-policy")
            .is_some_and(|p| p.contains("script-src"))
        {
            return Err(format!("{path} allows a script it does not have"));
        }
        if a.header("content-type")
            .is_some_and(|t| t.starts_with("text/html"))
        {
            for l in links(&a.text()) {
                // A page's other sorts and pages are the same page.
                let linked = l.split('?').next().unwrap_or("").to_string();
                if !seen
                    .iter()
                    .any(|s| s.split('?').next() == Some(linked.as_str()) && l.contains("sort="))
                {
                    queue.push_back(l);
                }
            }
        }
        if pages > 1500 {
            return Err("more than 1500 pages: the crawl does not end".into());
        }
    }
    c.guest.kill();
    Ok(format!(
        "27 systems added from the Add page; Welcome, Servers and Memory show the fixture's values; \
         {pages} pages and files reached each answered 200; {scripts} terminal windows carry the \
         script, under their own policy, and no other page does"
    ))
}

/// Suite 3.
pub fn pair(run: &crate::suite::base::Run<'_>) -> Result<String, String> {
    let fleet = Fleet::start(vec![sim::fixture::qshqvs1()])?;
    let mut c = start(run, "mgmt-pair")?;
    let b = &mut c.browser;
    signed_on(b, ADMIN, PASSWORD)?;
    let port = fleet.ports[0].to_string();
    let mut typed = Vec::new();
    let mut try_add = |b: &mut crate::web::Browser, pw: &str| -> Result<String, String> {
        typed.push(pw.to_string());
        let r = submit(
            b,
            "/qsmc/servers/add",
            &[
                ("host", HOST),
                ("port", &port),
                ("pw", pw),
                ("connect", "Connect"),
            ],
        )?;
        Ok(r.text())
    };
    // No password out: turned away.
    let t = try_add(b, "AAAA-BBBB-CCCC-DDDD")?;
    if !shows(&t, "QMC1102") {
        return Err(format!("no password out: {}", message_of(&t)));
    }
    // A wrong one: the host's own QSI0202.
    fleet.tell(0, "pair");
    let t = try_add(b, "AAAA-BBBB-CCCC-DDDD")?;
    if !shows(&t, "QMC1111") || !shows(&t, "QSI0202") {
        return Err(format!("a wrong password: {}", message_of(&t)));
    }
    // A good one pairs; the certificate shown is the host's.
    let good = fleet.tell(0, "pair");
    let t = try_add(b, &good)?;
    if !shows(&t, "QMC1001") {
        return Err(format!("the good password: {}", message_of(&t)));
    }
    let fp = sim::lock(&fleet.systems[0])
        .identity
        .fingerprint()
        .hex()
        .to_uppercase();
    let shown = fp
        .as_bytes()
        .chunks(2)
        .map(|c| String::from_utf8_lossy(c).to_string())
        .collect::<Vec<_>>()
        .join(":");
    if !t.contains(&shown) {
        return Err("the result does not show the host's certificate".into());
    }
    // Cancel forgets it; the same password again is used: the host's
    // own QSI0201.
    submit(
        b,
        "/qsmc/servers/add",
        &[("ms", "QSHQVS1"), ("cancel", "Cancel")],
    )?;
    let t = try_add(b, &good)?;
    if !shows(&t, "QSI0201") {
        return Err(format!("a used password: {}", message_of(&t)));
    }
    // An expired one: QSI0201 again.
    let expired = fleet.tell(0, "pair");
    fleet.tell(0, "expire");
    let t = try_add(b, &expired)?;
    if !shows(&t, "QSI0201") {
        return Err(format!("an expired password: {}", message_of(&t)));
    }
    // No log holds a password typed.
    let ch = crate::suite::base::channel(&mut c.guest)?;
    let mut logs = String::new();
    for name in ch.ls("/data/log")? {
        if let Ok(bytes) = ch.cat(&format!("/data/log/{name}")) {
            logs.push_str(&String::from_utf8_lossy(&bytes));
        }
    }
    for pw in &typed {
        if logs.contains(pw.as_str()) {
            return Err(format!("a log holds the password {}...", &pw[..4]));
        }
    }
    let state =
        String::from_utf8_lossy(&ch.cat("/data/mgmt/systems.toml").unwrap_or_default()).to_string();
    if typed.iter().any(|p| state.contains(p.as_str())) {
        return Err("systems.toml holds a password".into());
    }
    c.guest.kill();
    Ok(format!(
        "no password out: QMC1102; a wrong one: QMC1111 with the host's QSI0202; a good one \
         paired and showed the host's certificate {shown}; a used one and an expired one: the \
         host's QSI0201; none of the {} passwords typed in {} bytes of logs or in systems.toml",
        typed.len(),
        logs.len()
    ))
}

/// How many times `code` is in the history page of `system`.
fn times(b: &mut crate::web::Browser, system: &str, code: &str) -> Result<usize, String> {
    let t = page(b, &format!("/qsmc/refcodes?ms={system}&rows=100"))?;
    Ok(t.matches(&format!(">{code}</a>")).count())
}

/// Suite 4.
pub fn sessions(run: &crate::suite::base::Run<'_>) -> Result<String, String> {
    let mut systems = vec![sim::fixture::qshqvs1()];
    systems.extend((1..=100).map(sim::fixture::branch));
    let fleet = Fleet::start(systems)?;
    let mut c = start(run, "mgmt-sessions")?;
    let b = &mut c.browser;
    signed_on(b, ADMIN, PASSWORD)?;
    keep(b, &fleet, 0)?;
    // Dropped, and a code posted meanwhile: replayed, once.
    fleet.tell(0, "drop");
    fleet.tell(0, "code C2002030 Time synchronized");
    until("the code replayed", Duration::from_secs(30), || {
        Ok(times(b, "QSHQVS1", "C2002030")? == 1)
    })?;
    std::thread::sleep(Duration::from_secs(3));
    if times(b, "QSHQVS1", "C2002030")? != 1 {
        return Err("the replayed code is in the history twice".into());
    }
    // Away past what its log keeps: read again.
    fleet.tell(0, "down");
    for i in 0..5000 {
        fleet.tell(0, &format!("code C2001005 Host running {i}"));
    }
    fleet.tell(0, "code A2002010 Management interface eth0 has no carrier");
    fleet.tell(0, "up");
    until("the system read again", Duration::from_secs(90), || {
        Ok(page(b, "/qsmc/servers")?.contains(">A2002010</a>"))
    })?;
    // Another certificate under the same name: refused, and shown.
    fleet.tell(0, "renew");
    until(
        "the changed certificate shown",
        Duration::from_secs(60),
        || Ok(page(b, "/qsmc/connectivity")?.contains("Certificate changed")),
    )?;
    // A hundred systems connect within the limits.
    let t0 = Instant::now();
    for i in 1..=100 {
        keep(b, &fleet, i)?;
    }
    let added = t0.elapsed().as_secs();
    until("a hundred connected", Duration::from_secs(300), || {
        Ok(page(b, "/qsmc/connectivity")?
            .matches(">Connected<")
            .count()
            >= 100)
    })?;
    c.guest.kill();
    Ok(format!(
        "a dropped session resumed and replayed the code posted meanwhile once; a system away \
         past its log of 4,096 events was read again (its last code shown); a new certificate \
         under the same name was shown as changed and not trusted; 100 more systems added in \
         {added} s, all connected"
    ))
}

/// Suite 5.
pub fn lists(run: &crate::suite::base::Run<'_>) -> Result<String, String> {
    let fleet = Fleet::start((1..=20).map(sim::fixture::branch).collect())?;
    let mut c = start(run, "mgmt-lists")?;
    let b = &mut c.browser;
    signed_on(b, ADMIN, PASSWORD)?;
    for i in 0..20 {
        keep(b, &fleet, i)?;
    }
    for i in (0..20).step_by(2) {
        fleet.tell(i, "stall on");
    }
    let mut slowest = Duration::ZERO;
    let t0 = Instant::now();
    let mut away = 0;
    while t0.elapsed() < Duration::from_secs(120) {
        for path in ["/qsmc/servers", "/qsmc/welcome", "/qsmc/connectivity"] {
            let t = Instant::now();
            let text = page(b, path)?;
            slowest = slowest.max(t.elapsed());
            if path == "/qsmc/servers" {
                away = text.matches("No connection").count();
            }
        }
        if away >= 10 {
            break;
        }
        std::thread::sleep(Duration::from_secs(2));
    }
    if slowest > Duration::from_secs(2) {
        return Err(format!("a list took {} ms", slowest.as_millis()));
    }
    if away < 10 {
        return Err(format!(
            "{away} of the 10 stalled systems shown not connected"
        ));
    }
    c.guest.kill();
    Ok(format!(
        "with 10 of 20 systems stalled, Servers, Welcome and Connectivity each answered within \
         {} ms throughout, and showed the 10 as No connection after {} s",
        slowest.as_millis(),
        t0.elapsed().as_secs()
    ))
}
