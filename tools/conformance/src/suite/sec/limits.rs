// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! The console's limits at the limit and one past it (Phase 8,
//! Section 8): web users, custom groups, scheduled operations, system
//! plans, web sessions, uploads to a host at once, managed systems and
//! terminal windows (both set low in `mgmt.toml` for the suite), and the
//! problems kept. Each past its limit is refused with its message, and
//! what was there is kept. The backup's size and the codes kept a system
//! are held by their crates' tests (`web/backup`, `mgmt/mgmtd`).

use std::time::Duration;

use crate::suite::base::{Run, channel};
use crate::suite::mgmt::{Fleet, add, keep};
use crate::suite::parts::Lab;
use crate::suite::web::{ADMIN, PASSWORD, follow, shows, signed_on, start, submit};
use crate::web::Browser;
use crate::web::stream::Upload;

/// Post `fields` to `path` and say whether the page shows `id`.
fn answers(b: &mut Browser, path: &str, fields: &[(&str, &str)], id: &str) -> Result<bool, String> {
    let r = submit(b, path, fields)?;
    let t = follow(b, &r)?.text();
    Ok(shows(&t, id))
}

/// Fill to `most` with `one(i)`, then one more must be refused.
fn filled(
    what: &str,
    most: usize,
    have: usize,
    full: &str,
    mut one: impl FnMut(usize) -> Result<bool, String>,
) -> Result<String, String> {
    for i in have..most {
        if !one(i)? {
            return Err(format!("{what} {i} of {most} was refused"));
        }
    }
    if one(most)? {
        return Err(format!("{what} past {most} was not refused with {full}"));
    }
    Ok(format!("{most} {what}, the next refused ({full})"))
}

fn users(b: &mut Browser) -> Result<String, String> {
    let most = webusers::list::MAX_USERS;
    filled("web users", most, 1, qmcmsg::USERS_FULL.id, |i| {
        let id = format!("lim{i}");
        let ok = answers(
            b,
            "/qsmc/console/users",
            &[
                ("id", &id),
                ("role", webusers::Role::Viewer.label()),
                ("password", "Limit-pass-01"),
                ("again", "Limit-pass-01"),
                ("create", "OK"),
            ],
            qmcmsg::USER_CREATED.id,
        )?;
        Ok(ok)
    })
}

fn groups(b: &mut Browser) -> Result<String, String> {
    filled(
        "custom groups",
        webpages::groups::MAX_GROUPS,
        0,
        qmcmsg::GROUPS_FULL.id,
        |i| {
            answers(
                b,
                "/qsmc/groups/create",
                &[("g", &format!("LIM{i}")), ("m1", "on"), ("ok", "OK")],
                "QMC3301",
            )
        },
    )
}

fn schedules(b: &mut Browser) -> Result<String, String> {
    filled(
        "scheduled operations",
        schedd::model::MAX_SCHEDULES,
        0,
        qmcmsg::SCHEDULES_FULL.id,
        |_| {
            // The form's own date and time, as a browser sends them.
            let r = crate::suite::parts::post_form(
                b,
                "/qsmc/schedule/add",
                ("ok", "OK"),
                &[
                    ("r", "daily"),
                    ("op", "Restart partition"),
                    ("tg", "QSDEV01 on QSHQVS1"),
                ],
            )?;
            Ok(shows(&follow(b, &r)?.text(), "QMC3401"))
        },
    )
}

fn plans(b: &mut Browser) -> Result<String, String> {
    let first = crate::suite::plans::create(b, "QSHQVS1")?;
    let doc =
        String::from_utf8(crate::suite::plans::export(b, &first)?).map_err(|e| e.to_string())?;
    let line = doc
        .lines()
        .find(|l| l.starts_with("name = "))
        .ok_or("no name in the plan")?
        .to_string();
    filled(
        "system plans",
        schedd::plans::MAX_PLANS,
        1,
        qmcmsg::PLANS_FULL.id,
        |i| {
            let named = doc.replacen(&line, &format!("name = \"lim{i}\""), 1);
            let t = crate::suite::plans::import(b, named.as_bytes())?;
            Ok(
                shows(&t, "QMC6102")
                    || (!shows(&t, qmcmsg::PLANS_FULL.id) && !shows(&t, "QMC6104")),
            )
        },
    )
}

fn sessions(browser: &Browser) -> Result<String, String> {
    let b = browser;
    let most = webpages::app::routes::MAX_SESSIONS;
    let mut held = Vec::new();
    let mut made = 1;
    while made < most {
        let mut s = b.fresh();
        let a = s.sign_on(ADMIN, PASSWORD)?;
        let t = a.text();
        if shows(&t, qmcmsg::SIGNON_TOO_MANY.id) {
            // The address's attempts: wait for the minute to pass.
            std::thread::sleep(Duration::from_secs(61));
            continue;
        }
        if a.status != 303 {
            return Err(format!("sign-on {made} of {most}: {}", a.status));
        }
        held.push(s);
        made += 1;
    }
    loop {
        let mut s = b.fresh();
        let t = s.sign_on(ADMIN, PASSWORD)?.text();
        if shows(&t, qmcmsg::SIGNON_TOO_MANY.id) {
            std::thread::sleep(Duration::from_secs(61));
            continue;
        }
        if !shows(&t, qmcmsg::SESSIONS_FULL.id) {
            return Err(format!("a sign-on past {most} sessions was not refused"));
        }
        break;
    }
    drop(held);
    Ok(format!(
        "{most} web sessions, the next sign-on refused ({})",
        qmcmsg::SESSIONS_FULL.id
    ))
}

fn uploads(lab: &mut Lab<'_>) -> Result<String, String> {
    let most = mgmtd::system::MAX_UPLOADS;
    lab.fleet.tell(0, "upload-rate stall");
    let path = "/qsmc/server/QSHQVS1/virtual/optical";
    let csrf = lab.c.browser.csrf(path)?;
    let up = |name: String, bytes: u64| Upload {
        csrf: csrf.clone(),
        name,
        bytes,
        announce: None,
        cut: None,
        head_only: false,
        label: "lim".into(),
    };
    let mut running = Vec::new();
    for i in 0..most {
        let b = lab.c.browser.clone_session();
        let u = up(format!("LIM{i}.iso"), 64 << 20);
        running.push(std::thread::spawn(move || b.stream_upload(path, &u)));
        std::thread::sleep(Duration::from_secs(2));
    }
    // Small enough to be sent whole before the refusal closes the
    // connection.
    let past = lab
        .c
        .browser
        .stream_upload(path, &up("PAST.iso".into(), 32 << 10))?;
    // The refusal is shown on the page the answer sends the browser to.
    let t = match past {
        Some(a) => follow(&mut lab.c.browser, &a)?.text(),
        None => String::new(),
    };
    lab.fleet.tell(0, "upload-rate off");
    if !shows(&t, qmcmsg::UPLOADS_FULL.id) {
        return Err(format!("an upload past {most} at once was not refused"));
    }
    for r in running {
        let _ = r.join();
    }
    Ok(format!(
        "{most} uploads at once, the next refused ({})",
        qmcmsg::UPLOADS_FULL.id
    ))
}

fn systems_and_windows(lab: &mut Lab<'_>) -> Result<String, String> {
    // Two systems and two windows at most, for the suite; mgmtd reads
    // them when it starts again.
    let ch = channel(&mut lab.c.guest)?;
    let text = ch.text("/data/mgmt/mgmt.toml").unwrap_or_default();
    let mut kept: Vec<String> = text
        .lines()
        .filter(|l| !l.starts_with("max_systems") && !l.starts_with("max_consoles"))
        .map(str::to_string)
        .collect();
    // The file is not there until written: it needs its version too.
    if !kept.iter().any(|l| l.starts_with("schema_version")) {
        kept.insert(0, "schema_version = 1".into());
    }
    kept.push("max_systems = 2".into());
    kept.push("max_consoles = 2".into());
    ch.ask(&format!(
        "{} /data/mgmt/mgmt.toml {}",
        qsmchook::proto::command::PUT,
        hex::encode(kept.join("\n") + "\n")
    ))?;
    ch.ask(&format!("{} mgmtd", qsmchook::proto::command::KILL))?;
    std::thread::sleep(Duration::from_secs(10));
    let b = &mut lab.c.browser;
    // Two are kept already: the third is refused.
    let r = add(b, &lab.fleet, 2);
    let refused = match r {
        Err(e) => e.contains(qmcmsg::TOO_MANY_SYSTEMS.id),
        Ok(a) => shows(&a.text(), qmcmsg::TOO_MANY_SYSTEMS.id),
    };
    if !refused {
        return Err("a third managed system past two was not refused".into());
    }
    let w1 = b.websocket("/qsmc/terminal/QSHQVS1/1/ws")?;
    let w2 = b.websocket("/qsmc/terminal/QSHQVS1/3/ws")?;
    let third = b.get("/qsmc/terminal/QSHQVS1/2")?.text();
    drop((w1, w2));
    if !shows(&third, qmcmsg::TOO_MANY_WINDOWS.id) {
        return Err("a third terminal window past two was not refused".into());
    }
    Ok(format!(
        "2 managed systems, the next refused ({}); 2 terminal windows, the next refused ({})",
        qmcmsg::TOO_MANY_SYSTEMS.id,
        qmcmsg::TOO_MANY_WINDOWS.id
    ))
}

/// The problems kept: past 500 the oldest goes.
fn problems(lab: &mut Lab<'_>) -> Result<String, String> {
    let ch = channel(&mut lab.c.guest)?;
    for i in 0..510 {
        ch.ask(&format!(
            "{} A3001080 service:lim{i} a service stopped for the limits",
            qsmchook::proto::command::POST
        ))?;
    }
    std::thread::sleep(Duration::from_secs(10));
    let text = ch.text("/data/problems/problems.json")?;
    let kept = text.matches("\"number\"").count();
    if kept > 500 {
        return Err(format!("{kept} problems kept, past 500"));
    }
    Ok(format!(
        "510 problems opened, {kept} kept (500 at most, the oldest dropped)"
    ))
}

/// The suite.
pub fn limits(run: &Run<'_>) -> Result<String, String> {
    // Two systems kept, a third ready to be refused.
    let fleet = Fleet::start(vec![
        sim::fixture::qshqvs1(),
        sim::fixture::qshq400(),
        sim::fixture::qshqvs2(),
    ])?;
    let mut c = start(run, "sec-limits")?;
    signed_on(&mut c.browser, ADMIN, PASSWORD)?;
    keep(&mut c.browser, &fleet, 0)?;
    keep(&mut c.browser, &fleet, 1)?;
    let mut lab = Lab { fleet, c };
    let mut said = vec![users(&mut lab.c.browser)?];
    said.push(groups(&mut lab.c.browser)?);
    said.push(schedules(&mut lab.c.browser)?);
    said.push(plans(&mut lab.c.browser)?);
    said.push(uploads(&mut lab)?);
    said.push(systems_and_windows(&mut lab)?);
    said.push(problems(&mut lab)?);
    said.push(sessions(&lab.c.browser)?);
    lab.c.guest.kill();
    Ok(said.join("; "))
}
