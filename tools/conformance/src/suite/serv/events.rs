// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! Suite 3: a managed system's problems as the console shows them and
//! acts on them. Suite 4: the console's own problems, from the conditions
//! the harness can cause, and its problem service killed while codes are
//! posted.

use std::collections::BTreeMap;
use std::time::Duration;

use super::{Lab, STANDALONE, VS, kill, lab, number_of, post_code, restarted, row, set_clock};
use crate::suite::mgmt::{page, until};
use crate::suite::parts::{expect, post_on};
use crate::suite::web::{ADMIN, CERT, PASSWORD, message_of, shows, signed_on, submit};
use crate::web::Browser;

/// The detail page of event `n` once it holds `text`.
fn shows_on(b: &mut Browser, n: u64, text: &str) -> Result<String, String> {
    let path = format!("/qsmc/event/{n}");
    let mut last = String::new();
    until(
        &format!("event {n} shows `{text}`"),
        Duration::from_secs(60),
        || {
            last = page(b, &path)?;
            Ok(last.contains(text))
        },
    )?;
    Ok(last)
}

/// The number the list gives the event with `code`, once it is listed.
fn listed(b: &mut Browser, code: &str) -> Result<u64, String> {
    let mut n = 0;
    until(
        &format!("an event with {code} listed"),
        Duration::from_secs(60),
        || {
            n = number_of(b, code).unwrap_or(0);
            Ok(n > 0)
        },
    )?;
    Ok(n)
}

/// The host's own problem `theirs`, as it keeps it.
fn at_host(lab: &Lab<'_>, theirs: u64) -> Option<qsi::service::Problem> {
    sim::lock(&lab.fleet.systems[0])
        .servicing
        .problems
        .iter()
        .find(|(_, p)| p.summary.number == theirs)
        .map(|(_, p)| p.clone())
}

/// Suite 3.
#[allow(
    clippy::too_many_lines,
    reason = "one problem's life, then the numbers"
)]
pub fn events(run: &crate::suite::base::Run<'_>) -> Result<String, String> {
    let mut lab = lab(run, "serv-events", &[])?;
    let mut said = Vec::new();
    // A problem opened at the system, seen again, cleared, closed there.
    let opened = lab.fleet.tell(
        0,
        "problem open 1 A2007020 Virtual disk VD02 is not answering",
    );
    let theirs: u64 = opened
        .split_whitespace()
        .nth(1)
        .and_then(|n| n.parse().ok())
        .ok_or_else(|| format!("the simulator said `{opened}`"))?;
    let b = &mut lab.c.browser;
    let n = listed(b, "A2007020")?;
    let t = shows_on(b, n, &format!("{theirs}"))?;
    if !t.contains("Open") || !t.contains("QSDEV01") {
        return Err(format!("event {n} does not show itself open on QSDEV01"));
    }
    lab.fleet.tell(0, &format!("problem seen {theirs}"));
    shows_on(&mut lab.c.browser, n, "Seen again")?;
    lab.fleet.tell(0, &format!("problem clear {theirs}"));
    shows_on(&mut lab.c.browser, n, "Its condition cleared")?;
    let b = &mut lab.c.browser;
    let close = page(b, &format!("/qsmc/confirm/event-close?n={n}"))?;
    if !close.contains("Cleared") {
        return Err("a cleared condition's close does not say so".into());
    }
    lab.fleet.tell(0, &format!("problem close {theirs}"));
    until(
        "the event closed at the system shown closed",
        Duration::from_secs(60),
        || {
            let t = page(&mut lab.c.browser, "/qsmc/events?s=Closed&rows=100")?;
            Ok(t.contains(&format!("/qsmc/event/{n}\"")))
        },
    )?;
    said.push(format!("event {n} (the host's {theirs}) opened, seen again, cleared and closed at the host, each shown"));
    // Event 1042 holds: closed only through its confirmation.
    let b = &mut lab.c.browser;
    let e1042 = number_of(b, "B6000100")?;
    let path = format!("/qsmc/confirm/event-close?n={e1042}");
    let html = page(b, &path)?;
    if !html.contains("Still holds") {
        return Err("event 1042's close does not say its condition holds".into());
    }
    // A POST that did not come from the page is refused by the host.
    let r = submit(b, &path, &[("n", &e1042.to_string()), ("ok", "OK")])?;
    let t = crate::suite::web::follow(b, &r)?.text();
    if shows(&t, qmcmsg::EVENT_CLOSED.id) || !t.contains("QSI1001") {
        return Err(format!(
            "a close without the page's leave: {}",
            message_of(&t)
        ));
    }
    let r = post_on(b, &html, &path, ("ok", "OK"), &[])?;
    expect(b, &r, qmcmsg::EVENT_CLOSED.id)?;
    let p = at_host(&lab, 1042).ok_or("the host has no problem 1042")?;
    if p.summary.status != qsi::service::ProblemStatus::Closed || p.summary.closed_by != ADMIN {
        return Err(format!(
            "the host keeps 1042 {:?}, closed by `{}`",
            p.summary.status, p.summary.closed_by
        ));
    }
    said.push(format!(
        "event {e1042} (1042, its condition holding) refused without the confirmation's leave \
         (QSI1001) and closed through it, by {ADMIN}"
    ));
    // A note and an export.
    let opened = lab.fleet.tell(
        0,
        "problem open 2 A2003010 Virtual Ethernet adapter 2 lost its switch",
    );
    let theirs2: u64 = opened
        .split_whitespace()
        .nth(1)
        .and_then(|n| n.parse().ok())
        .ok_or_else(|| format!("the simulator said `{opened}`"))?;
    let b = &mut lab.c.browser;
    let m = listed(b, "A2003010")?;
    let note = format!("/qsmc/event/{m}/note");
    let r = crate::suite::parts::post_form(
        b,
        &note,
        ("ok", "OK"),
        &[("text", "Switch ETHSW0 restarted by hand")],
    )?;
    expect(b, &r, qmcmsg::EVENT_NOTED.id)?;
    let noted = format!("Note by {ADMIN}: Switch ETHSW0 restarted by hand");
    shows_on(b, m, &noted)?;
    if !at_host(&lab, theirs2).is_some_and(|p| p.log.iter().any(|l| l.text == noted)) {
        return Err(format!("the host's problem {theirs2} lacks the note"));
    }
    let x = lab.c.browser.get(&format!("/qsmc/event/{m}/export"))?;
    let text = x.text();
    if x.status != 200
        || !x
            .header("content-disposition")
            .is_some_and(|d| d.contains("attachment"))
        || !text.contains(&format!("Serviceable event {m}"))
        || !text.contains(&format!("Managed system's number: {theirs2}"))
        || !text.contains("Switch ETHSW0 restarted by hand")
    {
        return Err(format!(
            "event {m}'s export: {} {}",
            x.status,
            &text[..text.len().min(200)]
        ));
    }
    said.push(format!(
        "event {m} noted (shown and at the host with {ADMIN}) and exported as text"
    ));
    // The numbers across a reconnect and a restart.
    let b = &mut lab.c.browser;
    let before: BTreeMap<&str, u64> = ["A2007020", "B6000100", "A2003010", "B1811001"]
        .iter()
        .map(|c| (*c, number_of(b, c).unwrap_or(0)))
        .collect();
    lab.fleet.tell(0, "drop");
    until("QSHQVS1 connected again", Duration::from_secs(120), || {
        let t = page(&mut lab.c.browser, "/qsmc/connectivity")?;
        Ok(row(&t, VS).contains(">Connected<"))
    })?;
    let after_drop: BTreeMap<&str, u64> = before
        .keys()
        .map(|c| (*c, number_of(&mut lab.c.browser, c).unwrap_or(0)))
        .collect();
    let Lab { fleet, c, streams } = lab;
    let c = restarted(run, c)?;
    let mut lab = Lab { fleet, c, streams };
    super::ready(&mut lab.c.browser)?;
    let after_restart: BTreeMap<&str, u64> = before
        .keys()
        .map(|c| (*c, number_of(&mut lab.c.browser, c).unwrap_or(0)))
        .collect();
    if before != after_drop || before != after_restart {
        return Err(format!(
            "the numbers changed: {before:?}, after a reconnect {after_drop:?}, after a restart {after_restart:?}"
        ));
    }
    said.push(format!(
        "the numbers {before:?} the same after a reconnect and a restart"
    ));
    // Filters and paging.
    let b = &mut lab.c.browser;
    let t = page(b, &format!("/qsmc/events?s=Closed&ms={STANDALONE}"))?;
    if !t.contains("B1811001") || t.contains("B6000100") || t.contains("A2003010") {
        return Err("Closed for QSHQ400 does not show its event alone".into());
    }
    let open = page(b, "/qsmc/events")?;
    if open.contains("B1811001") || !open.contains("A2003010") {
        return Err("the default filter is not Open".into());
    }
    for i in 0..6 {
        lab.fleet
            .tell(0, &format!("problem open 3 A2009{i:03} Paging problem {i}"));
    }
    let b = &mut lab.c.browser;
    until("six more events listed", Duration::from_secs(60), || {
        Ok(page(b, "/qsmc/events?s=All&rows=100")?
            .matches("A2009")
            .count()
            >= 6)
    })?;
    let one = page(b, "/qsmc/events?s=All&rows=10")?;
    let two = page(b, "/qsmc/events?s=All&rows=10&page=2")?;
    let rows = |t: &str| t.matches("/qsmc/event/").count();
    if rows(&one) == 0 || rows(&two) == 0 || one == two || !one.contains("page=2") {
        return Err("ten rows a page do not page".into());
    }
    let sorted = page(b, "/qsmc/events?s=All&rows=100&sort=ms")?;
    let (q, v) = (
        sorted.find(STANDALONE).unwrap_or(usize::MAX),
        sorted.find(&format!(">{VS}<")).unwrap_or(0),
    );
    if q > v {
        return Err("sorted by managed system, QSHQ400 is not before QSHQVS1".into());
    }
    said.push(
        "Closed for QSHQ400 alone, Open by default, ten rows a page paged, sorted by system".into(),
    );
    lab.c.guest.kill();
    Ok(said.join("; "))
}

/// The console's own events listed whose row holds `what`, with all
/// statuses: their numbers.
fn own_rows(b: &mut Browser, what: &str) -> Result<Vec<u64>, String> {
    let t = page(b, "/qsmc/events?s=All&rows=100")?;
    Ok(t.split("<tr")
        .filter(|r| r.contains(what) && r.contains("/qsmc/event/console/"))
        .filter_map(|r| {
            r.split("/qsmc/event/console/")
                .nth(1)?
                .split('"')
                .next()?
                .parse()
                .ok()
        })
        .collect())
}

/// Wait until the console's own event with `code` and `what` is listed:
/// its number.
fn own_event(b: &mut Browser, code: &str, what: &str, within: Duration) -> Result<u64, String> {
    let mut n = Vec::new();
    until(
        &format!("the console's {code} about {what}"),
        within,
        || {
            n = own_rows(b, what)?.into_iter().collect();
            let t = page(b, "/qsmc/events?s=All&rows=100")?;
            Ok(!n.is_empty() && row(&t, &format!("/qsmc/event/console/{}\"", n[0])).contains(code))
        },
    )?;
    Ok(n[0])
}

/// The console's own event `n`'s detail page.
fn own_page(b: &mut Browser, n: u64) -> Result<String, String> {
    page(b, &format!("/qsmc/event/console/{n}"))
}

/// How many codes are posted while the problem service is killed.
const MANY: usize = 40;

/// Suite 4.
#[allow(clippy::too_many_lines, reason = "one condition a step")]
pub fn own(run: &crate::suite::base::Run<'_>) -> Result<String, String> {
    let mut lab = lab(run, "serv-own", &[])?;
    let mut said = Vec::new();
    // A download that fails its check.
    lab.streams.set_mode(crate::streams::server::Mode::Wrong);
    let b = &mut lab.c.browser;
    let a = b.get(&format!(
        "/qsmc/updates/code/download?s=qsvs&f={}",
        super::QSVS_IMAGE
    ))?;
    expect(b, &a, qmcmsg::DOWNLOAD_BEGUN.id)?;
    let n = own_event(b, "A3007110", super::QSVS_IMAGE, Duration::from_secs(120))?;
    let t = own_page(b, n)?;
    if !t.contains("A3007110") || !t.contains("Open") {
        return Err(format!(
            "the bad download's problem {n}: {}",
            message_of(&t)
        ));
    }
    lab.streams.set_mode(crate::streams::server::Mode::Normal);
    said.push(format!(
        "a download of the wrong hash opened problem {n} (A3007110)"
    ));
    // A managed system not reached for longer than the setting.
    lab.fleet.tell(0, "down");
    let now = super::clock(&mut lab.c.guest)?;
    until("QSHQVS1 not reached", Duration::from_secs(60), || {
        let t = page(&mut lab.c.browser, "/qsmc/connectivity")?;
        Ok(!row(&t, VS).contains(">Connected<"))
    })?;
    set_clock(&mut lab.c.guest, now + 31 * 60)?;
    lab.c.browser = lab.c.browser.fresh();
    signed_on(&mut lab.c.browser, ADMIN, PASSWORD)?;
    let n = own_event(&mut lab.c.browser, "A3005030", VS, Duration::from_secs(180))?;
    let t = own_page(&mut lab.c.browser, n)?;
    if !t.contains("Managed system") || !t.contains(VS) {
        return Err(format!("problem {n} does not name {VS} as its resource"));
    }
    lab.fleet.tell(0, "up");
    until(
        "the reached system's problem cleared",
        Duration::from_secs(180),
        || {
            let t = own_page(&mut lab.c.browser, n)?;
            Ok(t.contains("C3005040") || t.contains("Closed") || t.contains("cleared"))
        },
    )?;
    set_clock(&mut lab.c.guest, now + 32 * 60)?;
    said.push(format!(
        "QSHQVS1 down past 30 minutes opened problem {n} (A3005030) naming it; reached again, cleared"
    ));
    // The web certificate within 30 days of its end.
    let pem = super::super::base::channel(&mut lab.c.guest)?.text(CERT)?;
    let cert = x509kit::bundle::read_pem(&pem, None).map_err(|e| e.to_string())?;
    let not_after = cert.chain.first().ok_or("no certificate")?.info.not_after;
    let back = super::clock(&mut lab.c.guest)?;
    set_clock(
        &mut lab.c.guest,
        i64::try_from(not_after).unwrap_or(i64::MAX) - 10 * 86_400,
    )?;
    kill(&mut lab.c.guest, "webd")?;
    crate::suite::web::webd_ready(&mut lab.c.guest, Duration::from_secs(60))?;
    lab.c.browser = lab.c.browser.fresh();
    signed_on(&mut lab.c.browser, ADMIN, PASSWORD)?;
    let n = own_event(
        &mut lab.c.browser,
        "A3006050",
        "A3006050",
        Duration::from_secs(120),
    )?;
    set_clock(&mut lab.c.guest, back)?;
    kill(&mut lab.c.guest, "webd")?;
    crate::suite::web::webd_ready(&mut lab.c.guest, Duration::from_secs(60))?;
    lab.c.browser = lab.c.browser.fresh();
    signed_on(&mut lab.c.browser, ADMIN, PASSWORD)?;
    until(
        "the certificate's problem cleared",
        Duration::from_secs(120),
        || {
            let t = own_page(&mut lab.c.browser, n)?;
            Ok(t.contains("C3006050") || t.contains("cleared"))
        },
    )?;
    said.push(format!(
        "the clock ten days before the certificate's end opened problem {n} (A3006050); back, cleared"
    ));
    // The problem service killed while codes are posted.
    for i in 0..MANY {
        if i == 10 || i == 25 {
            kill(&mut lab.c.guest, "probd")?;
        }
        post_code(
            &mut lab.c.guest,
            "A3005030",
            &format!("system:LOST{i:02}"),
            &format!("LOST{i:02} was not reached."),
        )?;
    }
    let mut counts = BTreeMap::new();
    until(
        "every posted code's problem",
        Duration::from_secs(120),
        || {
            let t = page(&mut lab.c.browser, "/qsmc/events?s=All&rows=100")?;
            counts = (0..MANY)
                .map(|i| {
                    let name = format!("LOST{i:02}");
                    let k = t.split("<tr").filter(|r| r.contains(&name)).count();
                    (name, k)
                })
                .collect();
            Ok(counts.values().all(|k| *k >= 1))
        },
    )?;
    if let Some((name, k)) = counts.iter().find(|(_, k)| **k != 1) {
        return Err(format!("{name}'s problem is listed {k} times"));
    }
    said.push(format!(
        "{MANY} codes posted while the problem service was killed twice: {MANY} problems, none lost or doubled"
    ));
    // A service stopped after its retries: the next boot's flaky service.
    let Lab { fleet, c, streams } = lab;
    let crate::suite::web::Console {
        mut guest,
        mut machine,
        ..
    } = c;
    crate::suite::base::power_off(&mut guest)?;
    drop(guest);
    machine.oem = vec![qsmchook::oem_string(qsmchook::FLAKY_FAILS)];
    // A service stopped: the base is never ready, the web server is.
    let mut guest = run.boot(&machine)?;
    let deadline = std::time::Instant::now() + Duration::from_secs(300);
    guest.seen(&refcode::SERVICE_STOPPED.tag(), deadline)?;
    guest.channel(deadline)?;
    let mut c = crate::suite::web::attach(guest, machine)?;
    signed_on(&mut c.browser, ADMIN, PASSWORD)?;
    let n = own_event(
        &mut c.browser,
        "A3001080",
        "A3001080",
        Duration::from_secs(180),
    )?;
    let t = own_page(&mut c.browser, n)?;
    if !t.contains("Service") {
        return Err(format!("problem {n} does not name the service"));
    }
    let dumps = page(&mut c.browser, "/qsmc/dumps")?;
    if !dumps.contains(consdump::KIND.prefix) {
        return Err("no console dump was taken when the service stopped".into());
    }
    said.push(format!(
        "the flaky service stopped after its retries: problem {n} (A3001080) naming it, and a dump"
    ));
    c.guest.kill();
    drop((fleet, streams));
    Ok(format!(
        "{}; the other conditions of the table by qsmcprobd's tests",
        said.join("; ")
    ))
}
