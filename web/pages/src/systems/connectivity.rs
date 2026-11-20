// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! Connectivity (`connectivity.html`): each managed system's session,
//! when it last answered, how long since it was paired, Test
//! connection, Reset connection and Remove. The service password age
//! is the time since pairing: past 90 days the page warns, and Reset
//! connection pairs again.

use qsweb::frame::{Ctx, Page};
use qsweb::html::esc;
use qsweb::http::Response;
use qsweb::msg::{Kind, Message};
use qsweb::partials::{Cell, Col, List, actions, list_table, status};

use mgmtproto::Kept;
use mgmtproto::kept::Link;

use super::{refused, systems, via, when};
use crate::app::routes::Rq;

/// Days after pairing past which the page warns.
pub const WARN_DAYS: u64 = 90;

/// One system's row.
#[derive(Clone, Debug, Default)]
pub struct Row {
    /// Its name.
    pub name: String,
    /// `address:port (private)`.
    pub address: String,
    /// `Connected`, `Not connected`, `Certificate changed`.
    pub state: String,
    /// Whether that is good.
    pub good: bool,
    /// When it last answered.
    pub last: String,
    /// Days since pairing.
    pub age: u64,
    /// The release it runs.
    pub release: String,
    /// `QSI/1 over TLS 1.3`.
    pub protocol: String,
}

/// What the page shows.
#[derive(Clone, Debug, Default)]
pub struct Connectivity {
    /// The rows.
    pub rows: Vec<Row>,
    /// The last tests, a line a system.
    pub tests: Vec<String>,
    /// The message rows.
    pub messages: Vec<Message>,
}

/// The page.
pub fn page(ctx: &Ctx, v: &Connectivity) -> Page {
    let url = |r: &str| ctx.url(r);
    let rows = v
        .rows
        .iter()
        .map(|r| {
            let n = esc(&r.name);
            let age = if r.age > WARN_DAYS {
                status(&format!("{} days (rotate)", r.age), Some(false))
            } else {
                format!("{} days", r.age)
            };
            vec![
                Cell::act(actions(&[
                    (url(&format!("/connectivity?test={n}")), "Test connection"),
                    (url(&format!("/servers/reset?ms={n}")), "Reset connection"),
                    (url(&format!("/servers/reset?ms={n}")), "Change password"),
                    (url(&format!("/confirm/system-remove?ms={n}")), "Remove"),
                ])),
                Cell::mono(&r.name),
                Cell::mono(&r.address),
                Cell::html(status(&r.state, Some(r.good))),
                Cell::mono(&r.last),
                Cell::html(age),
                Cell::mono(&r.release),
                Cell::text(&r.protocol),
            ]
        })
        .collect();
    let list = List {
        cols: vec![
            Col::plain("Actions"),
            Col::sorted("Managed system", "ms"),
            Col::sorted("Address", "a"),
            Col::sorted("State", "s"),
            Col::sorted("Last contact", "l"),
            Col::plain("Service password age"),
            Col::plain("Service processor"),
            Col::plain("Protocol"),
        ],
        rows,
        empty: "This console manages no systems yet.".into(),
        keep: Vec::new(),
    };
    let tests = if v.tests.is_empty() {
        "No connection has been tested yet.".into()
    } else {
        esc(&v.tests.join("\n"))
    };
    let body = format!(
        "{}<h2>Last test</h2><pre class=\"out\">{tests}</pre><form method=\"post\">{}<div class=\"buttons\"><input type=\"submit\" value=\"Test all\"><input type=\"submit\" value=\"Add managed system...\" formaction=\"{}\"></div></form>",
        list_table(&list),
        ctx.csrf_field(),
        url("/servers/add")
    );
    Page {
        title: "Connectivity".into(),
        crumb: vec!["Service Management".into(), "Connectivity".into()],
        h1: "Connectivity".into(),
        messages: v.messages.clone(),
        body,
        help: "connectivity".into(),
        nav: Some(("Service Management", Some("/connectivity"))),
        ..Page::default()
    }
}

/// A kept system's row.
pub fn row(rq: &Rq<'_>, k: &Kept) -> Row {
    let (state, good) = match k.link {
        Link::Connected => ("Connected", true),
        Link::Changed => ("Certificate changed", false),
        Link::Never | Link::Connecting => ("Connecting", false),
        Link::Lost => ("Not connected", false),
    };
    let side = via(rq, &k.via);
    let address = if side.is_empty() {
        format!("{}:{}", k.address, k.port)
    } else {
        format!("{}:{} ({side})", k.address, k.port)
    };
    let version = k.identity.as_ref().map_or(0, |_| qsi::VERSION);
    Row {
        name: k.name.clone(),
        address,
        state: state.into(),
        good,
        last: when(rq, k.last_contact),
        age: rq.app.console.now().saturating_sub(k.paired) / 86_400,
        release: k.release().into(),
        protocol: if version == 0 {
            String::new()
        } else {
            format!("QSI/{version} over TLS 1.3")
        },
    }
}

/// Test `name`'s connection: its line for Last test, and the message.
fn test(rq: &Rq<'_>, k: &Kept, width: usize) -> (String, Message) {
    let at = format!("{}:{}", k.address, k.port);
    let now = when(rq, rq.app.console.now());
    let r = rq.app.console.mgmt().test(k.name.clone(), rq.user().into());
    let (steps, ok, m) = match r {
        Ok(Ok(t)) => {
            let steps = t.steps.join(", ");
            let m = qmcmsg::message(Kind::Info, &qmcmsg::SYSTEM_TESTED, &[&k.name, &steps]);
            (steps, t.ok, m)
        }
        Ok(Err(no)) => {
            let m = refused(&no).remove(0);
            (format!("{} {}", no.main.msgid, no.main.text), false, m)
        }
        Err(e) => (e.to_string(), false, super::unreached(&e)),
    };
    rq.audit(
        &format!("test connection {}", k.name),
        if ok { "ok" } else { "failed" },
    );
    let line = format!(
        "{now}  {}  {at:<width$}  {steps}  -  {}",
        k.name,
        if ok { "OK" } else { "FAILED" }
    );
    (line, m)
}

/// `GET /connectivity` (`?test=` a system), `POST` Test all.
pub fn route(rq: &Rq<'_>) -> Response {
    let mut messages = Vec::new();
    let kept = systems(rq).unwrap_or_else(|m| {
        messages.push(m);
        Vec::new()
    });
    let width = kept
        .iter()
        .map(|k| format!("{}:{}", k.address, k.port).len())
        .max()
        .unwrap_or(0);
    let which: Vec<&Kept> = if rq.post() {
        kept.iter().collect()
    } else {
        let t = rq.q("test");
        kept.iter()
            .filter(|k| !t.is_empty() && k.name == t)
            .collect()
    };
    if !which.is_empty() {
        let mut lines = Vec::new();
        for k in which {
            let (line, m) = test(rq, k, width);
            lines.push(line);
            messages.push(m);
        }
        *rq.app
            .tested
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = lines;
    }
    let mut rows: Vec<Row> = kept.iter().map(|k| row(rq, k)).collect();
    match rq.q("sort").as_str() {
        "a" => rows.sort_by(|a, b| a.address.cmp(&b.address)),
        "s" => rows.sort_by(|a, b| a.state.cmp(&b.state)),
        "l" => rows.sort_by(|a, b| b.last.cmp(&a.last)),
        _ => rows.sort_by(|a, b| a.name.cmp(&b.name)),
    }
    let tests = rq
        .app
        .tested
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clone();
    rq.page(page(
        &rq.ctx,
        &Connectivity {
            rows,
            tests,
            messages,
        },
    ))
}
