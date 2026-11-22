// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! Suite 7: scheduled operations run at their times as the harness moves
//! the console's clock (once, daily, weekly, monthly), in their owner's
//! name; an owner whose role was lowered is refused when the run comes;
//! a run due while the console was off is Missed after its IPL; Run now.

use std::time::Duration;

use appliance::time::{civil, from_civil, now_secs};
use qsi::host::model::PartitionState;
use qsmchook::proto::command;
use release::fact;

use super::{Lab, all_logs, asked, expect, host, lab, post_on, user};
use crate::suite::base::{channel, fact_of, power_off};
use crate::suite::mgmt::{page, until};
use crate::suite::web::{ADMIN, PASSWORD, boot, message_of, signed_on, submit};
use crate::web::Browser;

/// The first day the operations are on: Monday 1 March 2027.
const DAY: (i64, i64, i64) = (2027, 3, 1);

/// The console's clock.
fn console_now(lab: &mut Lab<'_>) -> Result<i64, String> {
    fact_of(&channel(&mut lab.c.guest)?.status()?, fact::TIME)?
        .parse()
        .map_err(|_| "the status's time is not a number".to_string())
}

/// Set the console's clock to `secs`.
fn set_clock(lab: &mut Lab<'_>, secs: i64) -> Result<(), String> {
    channel(&mut lab.c.guest)?
        .ask(&format!("{} {secs}", command::SET_CLOCK))
        .map(|_| ())
}

/// The console's display offset, minutes, as the add form names it.
pub(crate) fn offset(add: &str) -> i64 {
    let Some(z) = add
        .split("console time zone, ")
        .nth(1)
        .and_then(|r| r.split(')').next())
    else {
        return 0;
    };
    let rest = z.trim().trim_start_matches("UTC");
    let sign = if rest.starts_with('-') { -1 } else { 1 };
    let hm = rest.trim_start_matches(['+', '-']);
    let (h, m) = hm.split_once(':').unwrap_or((hm, "0"));
    sign * (h.parse::<i64>().unwrap_or(0) * 60 + m.parse::<i64>().unwrap_or(0))
}

/// A date as the console writes them, its format taken from `sample`.
pub(crate) fn date((y, m, d): (i64, i64, i64), sample: &str) -> String {
    if sample.contains('.') {
        format!("{d:02}.{m:02}.{:02}", y % 100)
    } else if sample.contains('-') {
        format!("{y:04}-{m:02}-{d:02}")
    } else {
        format!("{m:02}/{d:02}/{:02}", y % 100)
    }
}

/// The schedule list's rows: each operation's ID and its cells' text.
pub(crate) fn rows(page: &str) -> Vec<(u32, Vec<String>)> {
    use qsweb::golden::Tok;
    let mut out = Vec::new();
    let (mut id, mut cells, mut cell): (Option<u32>, Vec<String>, Option<String>) =
        (None, Vec::new(), None);
    for t in qsweb::golden::tokens(page) {
        match t {
            Tok::Open(n, _) if n == "tr" => {
                id = None;
                cells.clear();
            }
            Tok::Open(n, a) if n == "a" => {
                if let Some(h) = a.get("href")
                    && let Some(rest) = h.split("schedule/add?id=").nth(1)
                {
                    id = rest.split('&').next().and_then(|n| n.parse().ok());
                }
            }
            Tok::Open(n, _) if n == "td" => cell = Some(String::new()),
            Tok::Text(x) => {
                if let Some(c) = cell.as_mut() {
                    c.push_str(&x);
                }
            }
            Tok::Close(n) if n == "td" => {
                if let Some(c) = cell.take() {
                    cells.push(c.trim().to_string());
                }
            }
            Tok::Close(n) if n == "tr" => {
                if let Some(i) = id.take() {
                    out.push((i, std::mem::take(&mut cells)));
                }
            }
            _ => {}
        }
    }
    out
}

/// Operation `id`'s runs' statuses, newest first, and its next time.
pub(crate) fn runs_of(b: &mut Browser, id: u32) -> Result<(Vec<String>, Option<String>), String> {
    let p = page(b, "/qsmc/schedule")?;
    let mut runs = Vec::new();
    let mut next = None;
    for (i, cells) in rows(&p) {
        if i != id || cells.len() < 8 {
            continue;
        }
        if cells[7] == "Scheduled" {
            next = Some(format!("{} {}", cells[1], cells[2]));
        } else {
            runs.push(cells[7].clone());
        }
    }
    Ok((runs, next))
}

/// Move the clock to ten seconds before `at` and wait for operation
/// `id`'s next run: its status.
fn run_at(lab: &mut Lab<'_>, id: u32, at: i64) -> Result<String, String> {
    let before = runs_of(&mut lab.c.browser, id)?.0.len();
    set_clock(lab, at - 10)?;
    let mut last = String::new();
    until(
        &format!("operation {id}'s run at {at}"),
        Duration::from_secs(60),
        || {
            let (runs, _) = runs_of(&mut lab.c.browser, id)?;
            if runs.len() > before {
                last.clone_from(&runs[0]);
                return Ok(true);
            }
            Ok(false)
        },
    )?;
    Ok(last)
}

/// Add an operation as `b`'s user: when (local `day` at `time`), how
/// often, what.
fn add(
    b: &mut Browser,
    day: (i64, i64, i64),
    time: &str,
    repeat: &[(&str, &str)],
    op: &str,
    target: &str,
    profile: &str,
) -> Result<(), String> {
    let form = page(b, "/qsmc/schedule/add")?;
    let sample = super::field(&form, "/qsmc/schedule/add", "d");
    let d = date(day, &sample);
    let mut set = vec![
        ("d", d.as_str()),
        ("t", time),
        ("op", op),
        ("tg", target),
        ("p", profile),
        ("delay", "60"),
    ];
    set.extend_from_slice(repeat);
    let r = post_on(b, &form, "/qsmc/schedule/add", ("ok", "OK"), &set)?;
    expect(b, &r, "QMC3401").map(|_| ())
}

/// Unix seconds of local `day` at `h`:`m`, the display offset `off`.
fn at(day: (i64, i64, i64), h: i64, m: i64, off: i64) -> i64 {
    from_civil(day.0, day.1, day.2, h, m, 0) - off * 60
}

fn plus(day: (i64, i64, i64), n: i64) -> (i64, i64, i64) {
    let c = civil(from_civil(day.0, day.1, day.2, 12, 0, 0) + n * 86_400);
    (c.year, i64::from(c.month), i64::from(c.day))
}

/// Suite 7.
#[allow(clippy::too_many_lines, reason = "one calendar, step by step")]
pub fn sched(run: &crate::suite::base::Run<'_>) -> Result<String, String> {
    let mut lab = lab(run, "parts-sched")?;
    let form = page(&mut lab.c.browser, "/qsmc/schedule/add")?;
    let off = offset(&form);
    let mut oper = user(&mut lab.c.browser, "oper", "Operator", "Sched-run-42")?;
    let mut oper2 = user(&mut lab.c.browser, "oper2", "Operator", "Sched-run2-42")?;
    let (d0, d1, d2) = (DAY, plus(DAY, 1), plus(DAY, 2));
    let p = |name: &str| format!("{name} on QSHQVS1");
    // 1 once, 2 daily, 3 weekly on Tuesdays, 4 monthly on the 3rd; 5 by
    // the user whose role is lowered; 6 due while the console is off.
    add(
        &mut oper,
        d0,
        "06:00",
        &[("r", "once")],
        "Activate partition",
        &p("QSTEST01"),
        "default",
    )?;
    add(
        &mut oper,
        d0,
        "07:00",
        &[("r", "daily")],
        "Restart partition",
        &p("QSDEV01"),
        "default",
    )?;
    add(
        &mut oper,
        d0,
        "08:00",
        &[("r", "weekly"), ("wTue", "on")],
        "Shut down partition (immediate)",
        &p("QSPROD01"),
        "default",
    )?;
    add(
        &mut oper,
        d0,
        "09:00",
        &[("r", "monthly"), ("md", "3")],
        "Shut down partition (delayed)",
        &p("QSDEV01"),
        "default",
    )?;
    add(
        &mut oper2,
        d2,
        "10:00",
        &[("r", "once")],
        "Activate partition",
        &p("QSPROD01"),
        "prod",
    )?;
    // Due two minutes on, by the console's clock.
    let now = console_now(&mut lab)?;
    let skew = now - now_secs();
    let local = now + off * 60;
    let due_local = (local / 60 + 3) * 60;
    let c = civil(due_local);
    let hhmm = format!("{:02}:{:02}", c.hour, c.minute);
    let due = due_local - off * 60;
    add(
        &mut oper,
        (c.year, i64::from(c.month), i64::from(c.day)),
        &hhmm,
        &[("r", "once")],
        "Activate partition",
        &p("QSTEST01"),
        "default",
    )?;
    // Off past its time, and on again.
    power_off(&mut lab.c.guest)?;
    let machine = lab.c.machine.clone();
    let wake = due - skew + 75;
    while now_secs() < wake {
        std::thread::sleep(Duration::from_secs(1));
    }
    let Lab { fleet, c } = lab;
    drop(c);
    let mut lab = Lab {
        fleet,
        c: boot(run, machine)?,
    };
    signed_on(&mut lab.c.browser, ADMIN, PASSWORD)?;
    until("QSHQVS1 connected again", Duration::from_secs(90), || {
        Ok(page(&mut lab.c.browser, "/qsmc/connectivity")?.contains(">Connected<"))
    })?;
    until("operation 6 missed", Duration::from_secs(30), || {
        Ok(runs_of(&mut lab.c.browser, 6)?
            .0
            .first()
            .is_some_and(|s| s.contains("Missed")))
    })?;
    // oper2 lowered to Viewer before its run.
    let r = submit(
        &mut lab.c.browser,
        "/qsmc/console/users",
        &[
            ("id", "oper2"),
            ("description", ""),
            ("role", "Viewer"),
            ("enabled", "on"),
            ("properties", "OK"),
        ],
    )?;
    let _ = crate::suite::web::follow(&mut lab.c.browser, &r)?;
    let by_oper = asked(&lab.fleet, qsi::family::PARTITION, "oper");
    let mut said = Vec::new();
    let once = run_at(&mut lab, 1, at(d0, 6, 0, off))?;
    until("QSTEST01 activated", Duration::from_secs(20), || {
        Ok(host(&lab.fleet)
            .part_ref(2)
            .is_some_and(|p| p.status.state != PartitionState::NotActivated))
    })?;
    said.push(format!("once at 06:00: {once}"));
    let mut daily = Vec::new();
    daily.push(run_at(&mut lab, 2, at(d0, 7, 0, off))?);
    let next2 = runs_of(&mut lab.c.browser, 2)?.1.unwrap_or_default();
    daily.push(run_at(&mut lab, 2, at(d1, 7, 0, off))?);
    let weekly = run_at(&mut lab, 3, at(d1, 8, 0, off))?;
    let next3 = runs_of(&mut lab.c.browser, 3)?.1.unwrap_or_default();
    daily.push(run_at(&mut lab, 2, at(d2, 7, 0, off))?);
    // Run now: operation 2 in its owner's name, after its confirmation.
    let confirm = page(&mut lab.c.browser, "/qsmc/schedule?run=2")?;
    if !super::plain(&confirm).contains("in its owner's name, oper") {
        return Err("Run now's confirmation does not name the owner".into());
    }
    let r = post_on(
        &mut lab.c.browser,
        &confirm,
        "/qsmc/schedule?run=2",
        ("ok", "OK"),
        &[],
    )?;
    let now_page = expect(&mut lab.c.browser, &r, "QMC3404")?;
    let monthly = run_at(&mut lab, 4, at(d2, 9, 0, off))?;
    let next4 = runs_of(&mut lab.c.browser, 4)?.1.unwrap_or_default();
    let refused = run_at(&mut lab, 5, at(d2, 10, 0, off))?;
    let sent = asked(&lab.fleet, qsi::family::PARTITION, "oper") - by_oper;
    let by_oper2 = asked(&lab.fleet, qsi::family::PARTITION, "oper2");
    let fine = |s: &str| s == "Completed";
    if !fine(&once) || !daily.iter().all(|s| fine(s)) || !fine(&weekly) || !fine(&monthly) {
        return Err(format!(
            "runs: once {once}, daily {daily:?}, weekly {weekly}, monthly {monthly}"
        ));
    }
    if !refused.contains("QMC3406") || by_oper2 != 0 {
        return Err(format!(
            "the lowered owner's run: {refused}, {by_oper2} requests from oper2"
        ));
    }
    let sample = super::field(&form, "/qsmc/schedule/add", "d");
    let want = [
        (next2.as_str(), date(d1, &sample)),
        (next3.as_str(), date(plus(d1, 7), &sample)),
        (next4.as_str(), date((2027, 4, 3), &sample)),
    ];
    for (got, day) in &want {
        if !got.starts_with(day.as_str()) {
            return Err(format!("a next time is `{got}`, not on {day}"));
        }
    }
    if sent < 6 {
        return Err(format!(
            "the host had {sent} requests from user oper, not 6 or more"
        ));
    }
    let logs = all_logs(&mut lab.c.guest)?;
    for want in [
        "audit: user oper from schedule 1: Activate partition QSTEST01 on QSHQVS1: done",
        "audit: user oper2 from schedule 5: Activate partition QSPROD01 on QSHQVS1: refused",
        "scheduled operation 6",
    ] {
        if !logs.contains(want) {
            return Err(format!("no log line `{want}`"));
        }
    }
    lab.c.guest.kill();
    Ok(format!(
        "{}; daily at 07:00 three days running: {daily:?}, next {next2} after the first; weekly on \
         Tuesdays at 08:00: {weekly}, next {next3}; monthly on the 3rd at 09:00: {monthly}, next \
         {next4}; the owner lowered to Viewer: {refused}, nothing sent; due while the console was \
         off: Missed after its IPL; Run now: {}; {sent} requests to the host named user oper",
        said.join("; "),
        message_of(&now_page)
    ))
}
