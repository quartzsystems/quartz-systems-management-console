// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! Scheduled operations (`schedule.html`, `schedule-add.html`): the
//! scheduler's operations with their next time and their runs; Add and
//! Change in the signed-on user's name, who owns it from then on;
//! Remove and Run now through their confirmation. The scheduler
//! (`sched/schedd`) keeps and runs them.

#![allow(
    clippy::many_single_char_names,
    reason = "a form's fields are named as the design names them"
)]

use std::fmt::Write as _;

use qsweb::frame::{Ctx, Page};
use qsweb::html::esc;
use qsweb::http::{Form, Response};
use qsweb::msg::{Kind, Message};
use qsweb::partials::{Cell, Col, List, hidden, list_table};

use mgmtproto::Kept;
use mgmtproto::kept::Kind as SystemKind;
use schedd::{Op, Outcome, Repeat, Run, Schedule, Schedules};

use crate::app::routes::{Rq, lacks};

/// One row: an operation's next time, or one of its runs.
#[derive(Clone, Debug, Default)]
pub struct Row {
    /// The operation's number.
    pub id: u32,
    /// `10/30/26`.
    pub date: String,
    /// `23:00`.
    pub time: String,
    /// `Shut down partition (delayed)`.
    pub operation: String,
    /// `QSTEST01 on QSHQVS1`.
    pub target: String,
    /// `Daily`, `Weekly (Sun)`.
    pub repeat: String,
    /// Its owner.
    pub owner: String,
    /// `Scheduled`, `Completed`, `Failed - QMC2210`, `Missed`.
    pub status: String,
}

/// The list.
pub fn page(ctx: &Ctx, rows: &[Row], messages: Vec<Message>) -> Page {
    let list = List {
        cols: vec![
            Col::plain("Actions"),
            Col::sorted("Date", "d"),
            Col::sorted("Time", "t"),
            Col::sorted("Operation", "o"),
            Col::sorted("Target", "tg"),
            Col::plain("Repeat"),
            Col::plain("Owner"),
            Col::sorted("Status", "s"),
        ],
        rows: rows
            .iter()
            .map(|r| {
                let bad = r.status.starts_with("Failed")
                    || r.status.starts_with("Refused")
                    || r.status == "Missed";
                vec![
                    Cell::act(format!(
                        "<a href=\"{}\">Change</a> | <a href=\"{}\">Remove</a> | <a href=\"{}\">Run now</a>",
                        esc(&ctx.url(&format!("/schedule/add?id={}", r.id))),
                        esc(&ctx.url(&format!("/confirm/schedule-remove?id={}", r.id))),
                        esc(&ctx.url(&format!("/schedule?run={}", r.id)))
                    )),
                    Cell::mono(&r.date),
                    Cell::mono(&r.time),
                    Cell::text(&r.operation),
                    Cell::mono(&r.target),
                    Cell::text(&r.repeat),
                    Cell::text(&r.owner),
                    Cell::html(if bad {
                        qsweb::partials::status(&r.status, Some(false))
                    } else {
                        esc(&r.status)
                    }),
                ]
            })
            .collect(),
        empty: "No operations are scheduled.".into(),
        keep: Vec::new(),
    };
    let body = format!(
        "{}<form method=\"post\" action=\"{}\">{}<div class=\"buttons\"><input type=\"submit\" name=\"add\" value=\"Add...\"><input type=\"submit\" name=\"refresh\" value=\"Refresh\"></div></form>",
        list_table(&list),
        esc(&ctx.url("/schedule/add")),
        ctx.csrf_field()
    );
    Page {
        title: "Scheduled operations".into(),
        crumb: vec!["Systems Management".into(), "Scheduled operations".into()],
        h1: "Scheduled operations".into(),
        messages,
        body,
        help: "schedule".into(),
        nav: Some(("Systems Management", Some("/partitions"))),
        ..Page::default()
    }
}

const DAYS: [&str; 7] = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];

/// How often, in the list's words.
pub fn repeat_words(r: &Repeat) -> String {
    match r.every.as_str() {
        "once" => "Once".into(),
        "daily" => "Daily".into(),
        "weekly" => {
            let d: Vec<&str> = r
                .days
                .iter()
                .filter_map(|d| DAYS.get(usize::from(*d)).copied())
                .collect();
            format!("Weekly ({})", d.join(", "))
        }
        _ => format!("Monthly (day {})", r.day),
    }
}

/// An operation in the list's words.
pub fn op_words(s: &Schedule) -> String {
    match s.op {
        Op::Activate if !s.profile.is_empty() => {
            format!("Activate partition (profile {})", s.profile)
        }
        Op::Activate => "Activate partition (default profile)".into(),
        Op::PowerOff if s.delay > 0 => "Power off managed system".into(),
        op => op.words().into(),
    }
}

/// A run's status word.
pub fn outcome_words(r: &Run) -> String {
    match r.outcome {
        Outcome::Completed => "Completed".into(),
        Outcome::Missed => "Missed".into(),
        Outcome::Failed => format!("Failed - {}", r.msgid),
        Outcome::Refused => format!("Refused - {}", r.msgid),
    }
}

/// The list's rows: each operation's next time first, then the runs,
/// newest first.
pub fn rows(rq: &Rq<'_>, doc: &Schedules) -> Vec<Row> {
    let s = rq.app.settings();
    let zone = rq.app.host().zone();
    let date = |t: i64| crate::fmt::date(t, zone, &s.date_format);
    let time = |t: i64| crate::fmt::time(t, zone, &s.time_format, false);
    let mut out: Vec<Row> = doc
        .schedules
        .iter()
        .filter(|x| x.next != 0)
        .map(|x| Row {
            id: x.id,
            date: date(x.next),
            time: time(x.next),
            operation: op_words(x),
            target: schedd::service::target(x),
            repeat: repeat_words(&x.repeat),
            owner: x.owner.clone(),
            status: "Scheduled".into(),
        })
        .collect();
    out.sort_by(|a, b| (&a.date, &a.time).cmp(&(&b.date, &b.time)));
    for r in doc.runs.iter().rev() {
        let Some(x) = doc.schedules.iter().find(|x| x.id == r.id) else {
            continue;
        };
        out.push(Row {
            id: r.id,
            date: date(r.due),
            time: time(r.due),
            operation: op_words(x),
            target: schedd::service::target(x),
            repeat: repeat_words(&x.repeat),
            owner: r.owner.clone(),
            status: outcome_words(r),
        });
    }
    out
}

fn down(e: &ipc::WireError) -> Message {
    qmcmsg::message(Kind::Error, &qmcmsg::SCHEDD_DOWN, &[&e.to_string()])
}

/// `GET /schedule?sort=&run=`, and Run now's confirmation.
pub fn route(rq: &Rq<'_>) -> Response {
    let f = rq.req.form();
    let run = if rq.post() {
        f.str("run").to_string()
    } else {
        rq.q("run")
    };
    if let Ok(id) = run.parse::<u32>() {
        return run_now(rq, id);
    }
    let mut messages = Vec::new();
    let doc = rq.app.console.sched().list().unwrap_or_else(|e| {
        messages.push(down(&e));
        Schedules::default()
    });
    let mut list = rows(rq, &doc);
    match rq.q("sort").as_str() {
        "o" => list.sort_by(|a, b| a.operation.cmp(&b.operation)),
        "tg" => list.sort_by(|a, b| a.target.cmp(&b.target)),
        "s" => list.sort_by(|a, b| a.status.cmp(&b.status)),
        _ => {}
    }
    rq.page(page(&rq.ctx, &list, messages))
}

fn run_now(rq: &Rq<'_>, id: u32) -> Response {
    let doc = rq.app.console.sched().list().unwrap_or_default();
    let Some(s) = doc.schedules.iter().find(|x| x.id == id).cloned() else {
        return crate::app::routes::error(rq, 404, &qmcmsg::NOT_FOUND, &[&rq.req.path]);
    };
    if let Some(no) = lacks(rq, schedd::service::task(s.op)) {
        return no;
    }
    if rq.post() {
        if rq.req.form().get("ok").is_none() {
            return Response::redirect(&rq.ctx.url("/schedule"));
        }
        let r = rq.app.console.sched().run_now(id, rq.user().into());
        return match r {
            Ok(Ok(run)) => rq.done(
                "/schedule",
                qmcmsg::message(
                    if run.outcome == Outcome::Completed {
                        Kind::Done
                    } else {
                        Kind::Error
                    },
                    &qmcmsg::RAN_NOW,
                    &[&id.to_string(), &outcome_words(&run)],
                ),
            ),
            Ok(Err(e)) => rq.done(
                "/schedule",
                qmcmsg::message(Kind::Error, &qmcmsg::SCHEDULE_REFUSED, &[&e]),
            ),
            Err(e) => rq.done("/schedule", down(&e)),
        };
    }
    let body = qsweb::pages::confirm_body(
        &rq.ctx,
        &format!(
            "{} is run now in its owner's name, {}. Press OK to run it, or Cancel to return without it.",
            op_words(&s),
            s.owner
        ),
        &format!(
            "<table class=\"props\" cellspacing=\"0\"><tr><th>Operation</th><td>{}</td></tr><tr><th>Target</th><td>{}</td></tr></table>",
            esc(&op_words(&s)),
            qsweb::partials::mono(&schedd::service::target(&s))
        ),
        &hidden("run", &id.to_string()),
    );
    rq.page(Page {
        title: "Confirm Run Now".into(),
        crumb: vec![
            "Systems Management".into(),
            "Scheduled operations".into(),
            "Run now".into(),
        ],
        h1: "Confirm Run Now".into(),
        body,
        help: "schedule".into(),
        nav: Some(("Systems Management", Some("/partitions"))),
        ..Page::default()
    })
}

/// The add form.
#[derive(Clone, Debug, Default)]
pub struct Add {
    /// `10/31/26`.
    pub date: String,
    /// `06:00`.
    pub time: String,
    /// The console's zone as the form names it.
    pub zone: String,
    /// `once`, `daily`, `weekly`, `monthly`.
    pub every: String,
    /// The weekdays chosen, 0 for Sunday.
    pub days: Vec<u8>,
    /// The day of the month.
    pub day: String,
    /// The end date; empty for none.
    pub until: String,
    /// The operation chosen.
    pub op: String,
    /// The targets: `QSTEST01 on QSHQVS1`, `QSHQVS1`.
    pub targets: Vec<String>,
    /// The target chosen.
    pub target: String,
    /// The profiles offered.
    pub profiles: Vec<String>,
    /// The profile chosen.
    pub profile: String,
    /// The delay, seconds.
    pub delay: String,
    /// It changes an operation, not adds one.
    pub id: u32,
    /// The messages.
    pub messages: Vec<Message>,
}

fn select(name: &str, options: &[&str], chosen: &str) -> String {
    let mut s = format!("<select name=\"{name}\">");
    for o in options {
        let sel = if *o == chosen && Some(o) != options.first() {
            " selected"
        } else {
            ""
        };
        let _ = write!(s, "<option{sel}>{}</option>", esc(o));
    }
    s.push_str("</select>");
    s
}

/// The add form's page.
pub fn add_page(ctx: &Ctx, v: &Add) -> Page {
    let radio = |value: &str| {
        format!(
            "<input type=\"radio\" name=\"r\" value=\"{value}\"{}>",
            if v.every == value { " checked" } else { "" }
        )
    };
    let mut days = String::new();
    for (i, d) in DAYS.iter().enumerate() {
        let on = v.days.contains(&u8::try_from(i).unwrap_or(0));
        let _ = write!(
            days,
            " <input type=\"checkbox\" name=\"w{d}\"{}> {d}",
            if on { " checked" } else { "" }
        );
    }
    let ops: Vec<&str> = Op::ALL.iter().map(|o| o.words()).collect();
    let targets: Vec<&str> = v.targets.iter().map(String::as_str).collect();
    let profiles: Vec<&str> = v.profiles.iter().map(String::as_str).collect();
    let id = if v.id == 0 {
        String::new()
    } else {
        hidden("id", &v.id.to_string())
    };
    let body = format!(
        "<form method=\"post\">{}{id}<table class=\"props\" cellspacing=\"0\">\n<tr class=\"subrow\"><td colspan=\"2\">When</td></tr>\n<tr><th>Date</th><td><input type=\"text\" name=\"d\" value=\"{}\" size=\"8\"></td></tr>\n<tr><th>Time</th><td><input type=\"text\" name=\"t\" value=\"{}\" size=\"5\"> (console time zone, {})</td></tr>\n<tr><th>Repeat</th><td>{} Once<br>{} Daily<br>{} Weekly on:{days}<br>{} Monthly on day <input type=\"text\" name=\"md\" value=\"{}\" size=\"3\"></td></tr>\n<tr><th>Until</th><td><input type=\"text\" name=\"u\" value=\"{}\" size=\"8\"> <span class=\"small grey\">blank: no end</span></td></tr>\n<tr class=\"subrow\"><td colspan=\"2\">What</td></tr>\n<tr><th>Operation</th><td>{}</td></tr>\n<tr><th>Target</th><td>{}</td></tr>\n<tr><th>Profile (activate)</th><td>{}</td></tr>\n<tr><th>Delay (shut down)</th><td><input type=\"text\" name=\"delay\" value=\"{}\" size=\"5\"> seconds</td></tr>\n</table>\n<div class=\"buttons\"><input type=\"submit\" name=\"ok\" value=\"OK\"><input type=\"submit\" name=\"cancel\" value=\"Cancel\"></div></form>",
        ctx.csrf_field(),
        esc(&v.date),
        esc(&v.time),
        esc(&v.zone),
        radio("once"),
        radio("daily"),
        radio("weekly"),
        radio("monthly"),
        esc(&v.day),
        esc(&v.until),
        select("op", &ops, &v.op),
        select("tg", &targets, &v.target),
        select("p", &profiles, &v.profile),
        esc(&v.delay),
    );
    Page {
        title: if v.id == 0 {
            "Add scheduled operation".into()
        } else {
            "Change scheduled operation".into()
        },
        crumb: vec![
            "Systems Management".into(),
            "Scheduled operations".into(),
            if v.id == 0 { "Add" } else { "Change" }.into(),
        ],
        h1: if v.id == 0 {
            "Add scheduled operation".into()
        } else {
            "Change scheduled operation".into()
        },
        messages: v.messages.clone(),
        body,
        help: "schedule".into(),
        nav: Some(("Systems Management", Some("/partitions"))),
        ..Page::default()
    }
}

/// The console as a target: Install code update's alone, named as the
/// scheduler names it.
pub const CONSOLE_TARGET: &str = "the console";

/// Every target: each partition of each host, then each system, then
/// the console.
fn targets(kept: &[Kept]) -> Vec<(String, String, u32, String)> {
    let mut out = Vec::new();
    for k in kept.iter().filter(|k| k.kind == SystemKind::Host) {
        for p in &k.partitions {
            out.push((
                format!("{} on {}", p.name, k.name),
                k.name.clone(),
                p.id,
                p.name.clone(),
            ));
        }
    }
    for k in kept {
        out.push((k.name.clone(), k.name.clone(), 0, String::new()));
    }
    out.push((CONSOLE_TARGET.into(), String::new(), 0, String::new()));
    out
}

/// The form's schedule; an error's words.
fn schedule_of(rq: &Rq<'_>, kept: &[Kept], f: &Form) -> Result<Schedule, String> {
    let s = rq.app.settings();
    let zone = rq.app.host().zone();
    let op = Op::of(f.str("op")).ok_or("Choose an operation.")?;
    let all = targets(kept);
    let (_, system, partition, name) = all
        .into_iter()
        .find(|t| t.0 == f.str("tg"))
        .ok_or("Choose a target.")?;
    if op.on_partition() && partition == 0 {
        return Err("A partition's operation needs a partition as its target.".into());
    }
    if !op.on_partition() && partition != 0 {
        return Err("A managed system's operation needs a managed system as its target.".into());
    }
    if system.is_empty() && op != Op::Update {
        return Err("Only Install code update has the console as its target.".into());
    }
    if op == Op::Update
        && !system.is_empty()
        && !kept
            .iter()
            .any(|k| k.name == system && k.kind == SystemKind::Host)
    {
        return Err("Install code update is for the console or a QS/VS host.".into());
    }
    let date = crate::fmt::parse_date(f.str("d"), &s.date_format)
        .ok_or("Type the date as the console writes dates.")?;
    let (h, m) = crate::fmt::parse_time(f.str("t")).ok_or("Type the time as HH:MM.")?;
    let first = schedd::when::at(
        date,
        h,
        m,
        appliance::time::Zone {
            minutes: zone.minutes,
        },
    );
    let until = match f.str("u").trim() {
        "" => 0,
        u => {
            let d = crate::fmt::parse_date(u, &s.date_format)
                .ok_or("Type the end date as the console writes dates.")?;
            schedd::when::at(
                d,
                23,
                59,
                appliance::time::Zone {
                    minutes: zone.minutes,
                },
            )
        }
    };
    let every = match f.str("r") {
        r @ ("once" | "weekly" | "monthly") => r.to_string(),
        _ => "daily".into(),
    };
    let days: Vec<u8> = DAYS
        .iter()
        .enumerate()
        .filter(|(_, d)| f.has(&format!("w{d}")))
        .map(|(i, _)| u8::try_from(i).unwrap_or(0))
        .collect();
    if every == "weekly" && days.is_empty() {
        return Err("Choose the weekdays of a weekly operation.".into());
    }
    let day: u8 = f.str("md").trim().parse().unwrap_or(1);
    let delay: u32 = f.str("delay").trim().parse().unwrap_or(300).min(99_999);
    Ok(Schedule {
        id: f.str("id").parse().unwrap_or(0),
        owner: rq.user().into(),
        op,
        system,
        partition,
        partition_name: name,
        profile: if op == Op::Activate {
            f.str("p").to_string()
        } else {
            String::new()
        },
        delay,
        first,
        repeat: Repeat {
            every,
            days,
            day: day.clamp(1, 31),
        },
        until,
        next: 0,
    })
}

/// `GET`, `POST /schedule/add?id=`.
#[allow(
    clippy::too_many_lines,
    reason = "the form, its checks and its request"
)]
pub fn add(rq: &Rq<'_>) -> Response {
    let f = rq.req.form();
    if rq.post() && f.has("cancel") || rq.post() && f.has("refresh") {
        return Response::redirect(&rq.ctx.url("/schedule"));
    }
    let kept = crate::systems::systems(rq).unwrap_or_default();
    let doc = rq.app.console.sched().list().unwrap_or_default();
    let id: u32 = if rq.post() {
        f.str("id").parse().unwrap_or(0)
    } else {
        rq.q("id").parse().unwrap_or(0)
    };
    let had = doc.schedules.iter().find(|x| x.id == id).cloned();
    let mut messages = Vec::new();
    if rq.post() && f.has("ok") {
        let s = match schedule_of(rq, &kept, &f) {
            Ok(s) => s,
            Err(e) => {
                messages.push(qmcmsg::message(Kind::Error, &qmcmsg::NOT_VALID, &[&e]));
                Schedule::default()
            }
        };
        if messages.is_empty() {
            if let Some(no) = lacks(rq, schedd::service::task(s.op)) {
                return no;
            }
            let sched = rq.app.console.sched();
            let r = if had.is_some() {
                let mut sched = sched;
                sched
                    .change(s.clone(), rq.user().into())
                    .map(|r| r.map(|()| s.id))
            } else {
                let mut sched = sched;
                sched.add(s.clone(), rq.user().into())
            };
            match r {
                Ok(Ok(n)) => {
                    rq.audit(
                        &format!("schedule {} {}", op_words(&s), schedd::service::target(&s)),
                        "scheduled",
                    );
                    let set = rq.app.settings();
                    let zone = rq.app.host().zone();
                    let first = format!(
                        "{} {}",
                        crate::fmt::date(s.first, zone, &set.date_format),
                        crate::fmt::time(s.first, zone, &set.time_format, false)
                    );
                    let m = if had.is_some() {
                        qmcmsg::message(Kind::Done, &qmcmsg::SCHEDULE_CHANGED, &[&n.to_string()])
                    } else {
                        qmcmsg::message(Kind::Done, &qmcmsg::SCHEDULED, &[&op_words(&s), &first])
                    };
                    return rq.done("/schedule", m);
                }
                Ok(Err(e)) => messages.push(qmcmsg::message(
                    Kind::Error,
                    &qmcmsg::SCHEDULE_REFUSED,
                    &[&e],
                )),
                Err(e) => messages.push(down(&e)),
            }
        }
    }
    let set = rq.app.settings();
    let zone = rq.app.host().zone();
    let start = had.as_ref().map_or_else(
        || i64::try_from(rq.app.console.now()).unwrap_or(0) + 86_400,
        |x| x.first,
    );
    let all = targets(&kept);
    let mut profiles: Vec<String> = kept
        .iter()
        .flat_map(|k| {
            k.profile_lists
                .iter()
                .flat_map(|p| p.entries.iter().map(|e| e.name.clone()))
        })
        .collect();
    profiles.sort();
    profiles.dedup();
    if profiles.is_empty() {
        profiles.push("default".into());
    }
    let posted = |n: &str, d: String| if rq.post() { f.str(n).to_string() } else { d };
    // Code updates' Schedule: the install of its row.
    let update = had.is_none() && rq.q("op") == "update";
    let ms = rq.q("ms");
    let v = Add {
        date: posted("d", crate::fmt::date(start, zone, &set.date_format)),
        time: posted("t", crate::fmt::time(start, zone, &set.time_format, false)),
        zone: zone.to_string(),
        every: posted(
            "r",
            had.as_ref().map_or_else(
                || if update { "once" } else { "daily" }.into(),
                |x| x.repeat.every.clone(),
            ),
        ),
        days: had
            .as_ref()
            .map(|x| x.repeat.days.clone())
            .unwrap_or_default(),
        day: posted(
            "md",
            had.as_ref()
                .map_or_else(|| "1".into(), |x| x.repeat.day.to_string()),
        ),
        until: posted("u", String::new()),
        op: posted(
            "op",
            had.as_ref().map_or_else(
                || {
                    if update {
                        Op::Update.words().into()
                    } else {
                        String::new()
                    }
                },
                |x| x.op.words().into(),
            ),
        ),
        targets: all.iter().map(|t| t.0.clone()).collect(),
        target: posted(
            "tg",
            had.as_ref().map_or_else(
                || match (update, ms.is_empty()) {
                    (true, true) => CONSOLE_TARGET.into(),
                    (true, false) => ms.clone(),
                    _ => String::new(),
                },
                schedd::service::target,
            ),
        ),
        profiles,
        profile: posted(
            "p",
            had.as_ref().map(|x| x.profile.clone()).unwrap_or_default(),
        ),
        delay: posted(
            "delay",
            had.as_ref()
                .map_or_else(|| "300".into(), |x| x.delay.to_string()),
        ),
        id,
        messages,
    };
    rq.page(add_page(&rq.ctx, &v))
}

/// `GET`, `POST /confirm/schedule-remove?id=`.
pub fn remove(rq: &Rq<'_>) -> Response {
    let f = rq.req.form();
    let id: u32 = if rq.post() {
        f.str("id").parse().unwrap_or(0)
    } else {
        rq.q("id").parse().unwrap_or(0)
    };
    let doc = rq.app.console.sched().list().unwrap_or_default();
    let Some(s) = doc.schedules.iter().find(|x| x.id == id).cloned() else {
        return crate::app::routes::error(rq, 404, &qmcmsg::NOT_FOUND, &[&rq.req.path]);
    };
    if let Some(no) = lacks(rq, schedd::service::task(s.op)) {
        return no;
    }
    if rq.post() {
        if f.get("ok").is_none() {
            return Response::redirect(&rq.ctx.url("/schedule"));
        }
        return match rq.app.console.sched().remove(id, rq.user().into()) {
            Ok(Ok(())) => {
                rq.audit(&format!("remove scheduled operation {id}"), "removed");
                rq.done(
                    "/schedule",
                    qmcmsg::message(Kind::Done, &qmcmsg::SCHEDULE_REMOVED, &[&id.to_string()]),
                )
            }
            Ok(Err(e)) => rq.done(
                "/schedule",
                qmcmsg::message(Kind::Error, &qmcmsg::SCHEDULE_REFUSED, &[&e]),
            ),
            Err(e) => rq.done("/schedule", down(&e)),
        };
    }
    let body = qsweb::pages::confirm_body(
        &rq.ctx,
        "The operation is not run again. Press OK to remove it, or Cancel to return without it.",
        &format!(
            "<table class=\"props\" cellspacing=\"0\"><tr><th>Operation</th><td>{}</td></tr><tr><th>Target</th><td>{}</td></tr><tr><th>Owner</th><td>{}</td></tr></table>",
            esc(&op_words(&s)),
            qsweb::partials::mono(&schedd::service::target(&s)),
            esc(&s.owner)
        ),
        &hidden("id", &id.to_string()),
    );
    rq.page(Page {
        title: "Confirm Remove Scheduled Operation".into(),
        crumb: vec![
            "Systems Management".into(),
            "Scheduled operations".into(),
            "Remove".into(),
        ],
        h1: "Confirm Remove Scheduled Operation".into(),
        body,
        help: "schedule".into(),
        nav: Some(("Systems Management", Some("/partitions"))),
        ..Page::default()
    })
}
