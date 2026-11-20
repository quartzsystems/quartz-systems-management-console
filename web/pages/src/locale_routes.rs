// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! Serving Language, locale and session: `web.toml`'s formats and
//! timeouts. A new idle timeout applies to every session at once; a new
//! maximum length at each session's next request.

use qsweb::http::Response;
use qsweb::msg::{Kind, Message};

use crate::app::routes::Rq;
use crate::locale::{ENGLISH, Locale, page};
use crate::user_routes::outcome;

fn view(rq: &Rq<'_>, messages: Vec<Message>) -> Locale {
    let s = rq.app.settings();
    Locale {
        language: ENGLISH.into(),
        date: s.date_format,
        time: s.time_format,
        number: s.number_format,
        idle: s.idle_minutes.to_string(),
        max: s.max_hours.to_string(),
        terminals: s.keep_terminals,
        messages,
    }
}

fn error(def: &qmcmsg::Def, data: &[&str]) -> Message {
    qmcmsg::message(Kind::Error, def, data)
}

/// `/console/locale`.
pub fn locale(rq: &Rq<'_>) -> Response {
    if !rq.post() {
        return rq.page(page(&rq.ctx, &view(rq, Vec::new())));
    }
    let f = rq.req.form();
    if f.get("cancel").is_some() {
        return Response::redirect(&rq.ctx.url("/console"));
    }
    let field = |k: &str| f.get(k).unwrap_or("").trim().to_string();
    let mut asked = Locale {
        language: field("l"),
        date: field("df"),
        time: field("tf"),
        number: field("nf"),
        idle: field("to"),
        max: field("max"),
        terminals: f.get("tw").is_some(),
        messages: Vec::new(),
    };
    if !asked.language.is_empty() && asked.language != ENGLISH {
        let what = format!("The language {}", asked.language);
        asked.messages.push(error(&qmcmsg::NOT_IN_V1, &[&what]));
    }
    let mut next = rq.app.settings();
    next.keep_terminals = asked.terminals;
    next.date_format.clone_from(&asked.date);
    next.time_format.clone_from(&asked.time);
    next.number_format.clone_from(&asked.number);
    match (asked.idle.parse(), asked.max.parse()) {
        (Ok(i), Ok(m)) => {
            next.idle_minutes = i;
            next.max_hours = m;
        }
        _ => asked.messages.push(error(
            &qmcmsg::NOT_VALID,
            &["The idle timeout and the maximum session length are whole numbers."],
        )),
    }
    if !asked.messages.is_empty() {
        return rq.page(page(&rq.ctx, &asked));
    }
    let done = rq.app.change_settings(next.clone());
    rq.audit(
        &format!(
            "change the locale and session: {}, {}, {}, idle {} minutes, at most {} hours",
            next.date_format,
            next.time_format,
            next.number_format,
            next.idle_minutes,
            next.max_hours
        ),
        outcome(&done),
    );
    if let Err(e) = done {
        asked.messages.push(error(&qmcmsg::NOT_VALID, &[&e]));
        return rq.page(page(&rq.ctx, &asked));
    }
    let m = qmcmsg::message(Kind::Done, &qmcmsg::SETTINGS_SAVED, &[]);
    if f.get("ok").is_some() {
        return rq.done("/console", m);
    }
    rq.page(page(&rq.ctx, &view(rq, vec![m])))
}
