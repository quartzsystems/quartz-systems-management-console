// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! Serving Date and time. The time servers and the display offset go
//! to PID 1 (`host.toml`) and the network service reads them again; a
//! clock set by hand goes to the network service, which writes the RTC
//! too, and leaves no time server to step it back.

use appliance::time::{Zone, from_civil};
use qsweb::http::Response;
use qsweb::msg::{Kind, Message};

use crate::app::routes::Rq;
use crate::datetime::{DateTime, page, zones};
use crate::user_routes::outcome;

fn view(rq: &Rq<'_>, messages: Vec<Message>) -> DateTime {
    let app = rq.app;
    let s = app.settings();
    let host = app.host();
    let zone = host.zone();
    let now = i64::try_from(app.console.now()).unwrap_or(0);
    let mut list = zones();
    let current = zone.to_string();
    if !list.contains(&current) {
        list.push(current.clone());
    }
    let synced = app.console.fact(release::fact::SYNCED);
    let (status, detail) = match (host.ntp_servers.is_empty(), synced.split_once(' ')) {
        (true, _) => (("Not used".into(), None), String::new()),
        (false, Some((at, server))) => {
            let at = at.parse().unwrap_or(0);
            (
                ("Active".into(), Some(true)),
                format!(
                    ", last synchronized {} {} with {server}",
                    crate::fmt::date(at, zone, &s.date_format),
                    crate::fmt::time(at, zone, &s.time_format, true)
                ),
            )
        }
        (false, None) => (
            ("Not synchronized".into(), Some(false)),
            ", no time server has answered since the IPL".into(),
        ),
    };
    DateTime {
        now: format!(
            "{} {} {zone}",
            crate::fmt::date(now, zone, &s.date_format),
            crate::fmt::time(now, zone, &s.time_format, true)
        ),
        zones: list,
        zone: current,
        from_servers: !host.ntp_servers.is_empty(),
        servers: host.ntp_servers.join(", "),
        date: crate::fmt::date(now, zone, &s.date_format),
        time: crate::fmt::time(now, zone, &s.time_format, false),
        status,
        detail,
        serve: false,
        messages,
    }
}

fn show(rq: &Rq<'_>, messages: Vec<Message>) -> Response {
    rq.page(page(&rq.ctx, &view(rq, messages)))
}

fn not_valid(text: &str) -> Message {
    qmcmsg::message(Kind::Error, &qmcmsg::NOT_VALID, &[text])
}

/// The clock the form sets by hand, in seconds since the epoch.
fn by_hand(rq: &Rq<'_>, date: &str, time: &str, zone: Zone) -> Result<i64, String> {
    let format = rq.app.settings().date_format;
    let (y, mo, d) = crate::fmt::parse_date(date, &format)
        .ok_or_else(|| format!("`{date}` is not a date in the form {format}."))?;
    let (h, mi) = crate::fmt::parse_time(time).ok_or_else(|| format!("`{time}` is not a time."))?;
    Ok(from_civil(y, mo, d, h, mi, 0) - i64::from(zone.minutes) * 60)
}

fn post(rq: &Rq<'_>) -> Response {
    let f = rq.req.form();
    let app = rq.app;
    if f.get("cancel").is_some() {
        return Response::redirect(&rq.ctx.url("/console"));
    }
    if f.get("serve").is_some() {
        return show(
            rq,
            vec![qmcmsg::message(
                Kind::Error,
                &qmcmsg::NOT_IN_V1,
                &["Serving time to the managed systems"],
            )],
        );
    }
    let zone: Zone = match f.get("tz").unwrap_or("UTC").parse() {
        Ok(z) => z,
        Err(e) => return show(rq, vec![not_valid(&e)]),
    };
    let manual = f.get("s") == Some("manual");
    let servers: Vec<String> = if manual {
        Vec::new()
    } else {
        f.get("ntp")
            .unwrap_or("")
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(String::from)
            .collect()
    };
    if !manual && servers.is_empty() {
        return show(
            rq,
            vec![not_valid(
                "Give at least one time server, or choose Manually and give the date and time.",
            )],
        );
    }
    let clock = if manual {
        match by_hand(
            rq,
            f.get("md").unwrap_or(""),
            f.get("mt").unwrap_or(""),
            zone,
        ) {
            Ok(c) => Some(c),
            Err(e) => return show(rq, vec![not_valid(&e)]),
        }
    } else {
        None
    };
    let host = app.host();
    let name = host.name.clone().unwrap_or_else(|| app.console.name());
    let mut messages = Vec::new();
    if zone != host.zone() || servers != host.ntp_servers {
        let done = app.console.set_host(&name, &zone.to_string(), &servers);
        rq.audit(
            &format!(
                "change the display offset to {zone}, time servers {}",
                if servers.is_empty() {
                    "none".to_string()
                } else {
                    servers.join(", ")
                }
            ),
            outcome(&done),
        );
        if let Err(e) = done {
            return show(rq, vec![not_valid(&e)]);
        }
        messages.push(qmcmsg::message(Kind::Done, &qmcmsg::TIME_SAVED, &[]));
    }
    if let Some(at) = clock {
        let done = app.console.set_clock(at);
        let s = app.settings();
        let said = format!(
            "{} {} {zone}",
            crate::fmt::date(at, zone, &s.date_format),
            crate::fmt::time(at, zone, &s.time_format, false)
        );
        rq.audit(&format!("set the clock to {said}"), outcome(&done));
        if let Err(e) = done {
            return show(rq, vec![not_valid(&e)]);
        }
        messages.push(qmcmsg::message(Kind::Done, &qmcmsg::CLOCK_SET, &[&said]));
    }
    if f.get("ok").is_some() {
        if let Some(m) = messages.pop() {
            return rq.done("/console", m);
        }
        return Response::redirect(&rq.ctx.url("/console"));
    }
    show(rq, messages)
}

/// `/console/datetime`.
pub fn datetime(rq: &Rq<'_>) -> Response {
    if rq.post() {
        return post(rq);
    }
    show(rq, Vec::new())
}
