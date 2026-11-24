// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! Servers (`servers.html`, `-empty`, `-error`, `-paged`): every managed
//! system as kept, filtered by type and status, sorted and paged, with
//! the grouped action list. The reference code is on every row, a link,
//! never empty. A list never waits on a managed system.

use std::fmt::Write as _;

use qsweb::frame::{Ctx, Page};
use qsweb::html::esc;
use qsweb::http::Response;
use qsweb::msg::{Kind, Message};
use qsweb::partials::{Cell, Col, List, Paging, list_table, paging, status};

use mgmtproto::Kept;
use mgmtproto::kept::Kind as SystemKind;

use super::systems;
use crate::app::routes::Rq;

/// One row.
#[derive(Clone, Debug, Default)]
pub struct Row {
    /// Its name.
    pub name: String,
    /// `QS/VS host`, `Standalone QS/400`.
    pub kind: String,
    /// `Operating`, `Power off`, `Standby`, `Error`, `No connection`.
    pub status: String,
    /// Available processors, `3.50`.
    pub processors: String,
    /// Available memory in GB, `40.0`.
    pub memory: String,
    /// Its type and model.
    pub model: String,
    /// Its serial number.
    pub serial: String,
    /// Its current code.
    pub code: String,
    /// Its partitions.
    pub partitions: usize,
}

/// What the page shows.
#[derive(Clone, Debug)]
pub struct Servers {
    /// The page's rows.
    pub rows: Vec<Row>,
    /// The type filter: `All`, `Standalone QS/400`, `QS/VS host`.
    pub kind: String,
    /// The status filter.
    pub status: String,
    /// Where the list is.
    pub paging: Paging,
    /// The number the page's first row's box takes.
    pub first: usize,
    /// The message rows.
    pub messages: Vec<Message>,
}

/// The type filter's choices.
pub const KINDS: [&str; 3] = ["All", "Standalone QS/400", "QS/VS host"];
/// The status filter's choices.
pub const STATUSES: [&str; 6] = [
    "All",
    "Operating",
    "Power off",
    "Standby",
    "Error",
    "No connection",
];

/// The action list's groups.
const ACTIONS: &[(&str, &[&str])] = &[
    (
        "Operations",
        &["Power on", "Power off", "Restart", "Schedule operations"],
    ),
    (
        "Configuration",
        &[
            "Create partition",
            "Manage partition profiles",
            "Manage system profiles",
            "Virtual resources",
        ],
    ),
    (
        "Connections",
        &["Reset or remove connection", "Add managed system"],
    ),
    (
        "Serviceability",
        &["Reference code history", "Manage dumps"],
    ),
];

fn select(name: &str, options: &[&str], chosen: &str) -> String {
    let mut s = format!("<select name=\"{name}\">");
    for o in options {
        let sel = if *o == chosen && chosen != "All" {
            " selected"
        } else {
            ""
        };
        let _ = write!(s, "<option{sel}>{}</option>", esc(o));
    }
    s.push_str("</select>");
    s
}

/// The page.
pub fn page(ctx: &Ctx, v: &Servers) -> Page {
    let url = |r: &str| ctx.url(r);
    let rows = v
        .rows
        .iter()
        .enumerate()
        .map(|(i, r)| {
            let n = esc(&r.name);
            let good = match r.status.as_str() {
                "Operating" => Some(true),
                "Error" | "No connection" => Some(false),
                _ => None,
            };
            vec![
                Cell::act(format!(
                    "<input type=\"checkbox\" name=\"s{}\">",
                    v.first + i
                )),
                Cell::mono_html(format!(
                    "<a href=\"{}\">{n}</a>",
                    url(&format!("/server/{n}/general"))
                )),
                Cell::text(&r.kind),
                Cell::html(status(&r.status, good)),
                Cell::num(&r.processors),
                Cell::num(&r.memory),
                Cell::mono(&r.model),
                Cell::mono(&r.serial),
                Cell::mono_html(format!(
                    "<a class=\"mono\" href=\"{}\">{}</a>",
                    url(&format!("/refcode/{}?ms={n}", esc(&r.code))),
                    esc(&r.code)
                )),
                Cell {
                    class: Some("num"),
                    html: format!(
                        "<a href=\"{}\">{}</a>",
                        url(&format!("/partitions?ms={n}")),
                        r.partitions
                    ),
                },
            ]
        })
        .collect();
    let list = List {
        cols: vec![
            Col::plain("Select"),
            Col::sorted("Name", "name"),
            Col::sorted("Type", "type"),
            Col::sorted("Status", "status"),
            Col::sorted("Avail. processors", "cpu"),
            Col::sorted("Avail. memory (GB)", "mem"),
            Col::sorted("Type-Model", "tm"),
            Col::sorted("Serial", "sn"),
            Col::sorted("Reference code", "rc"),
            Col::sorted("Partitions", "parts"),
        ],
        rows,
        empty: "EMPTY".into(),
        keep: Vec::new(),
    };
    let table = list_table(&list).replace(
        ">EMPTY<",
        &format!(
            ">No managed systems. <a href=\"{}\">Add managed system</a>.<",
            url("/servers/add")
        ),
    );
    let actions =
        qsweb::partials::action_select("act", "--Select action--", &["Properties"], ACTIONS);
    let body = format!(
        "<form method=\"post\">{}<table class=\"filter\" cellspacing=\"0\"><tr><td>Type:</td><td>{}</td><td>Status:</td><td>{}</td><td><input type=\"submit\" value=\"Go\"> <input type=\"submit\" name=\"refresh\" value=\"Refresh\"></td></tr></table>{table}<div class=\"toolbar\">Selected: {actions} <input type=\"submit\" value=\"Go\"> &nbsp; <input type=\"submit\" value=\"Add managed system...\" formaction=\"{}\"></div></form>{}",
        ctx.csrf_field(),
        select("t", &KINDS, &v.kind),
        select("s", &STATUSES, &v.status),
        url("/servers/add"),
        paging(&v.paging)
    );
    Page {
        title: "Servers".into(),
        crumb: vec!["Systems Management".into(), "Servers".into()],
        h1: "Servers".into(),
        desc: Some("Managed systems known to this console.".into()),
        messages: v.messages.clone(),
        body,
        help: "servers".into(),
        nav: Some(("Systems Management", Some("/servers"))),
        ..Page::default()
    }
}

/// A kept system's row.
pub fn row(k: &Kept) -> Row {
    let host = k.host.as_ref();
    let standalone = k.kind == SystemKind::Standalone;
    let (processors, memory) = match host {
        Some(h) if !standalone => (
            format!(
                "{}.{:02}",
                h.processors.available / 100,
                h.processors.available % 100
            ),
            format!(
                "{}.{}",
                h.memory.available / 1024,
                h.memory.available % 1024 * 10 / 1024
            ),
        ),
        // A standalone system's are all its own: its processors and
        // main storage, as read when it connected.
        _ => match k.usage.as_ref().filter(|_| standalone) {
            Some(r) => (
                format!("{}.00", r.processors),
                format!(
                    "{}.{}",
                    r.memory_kb >> 20,
                    ((r.memory_kb & 0xf_ffff) * 10) >> 20
                ),
            ),
            None => ("0".into(), "0".into()),
        },
    };
    Row {
        name: k.name.clone(),
        kind: k.kind.words().into(),
        status: k.status().into(),
        processors,
        memory,
        model: k.model().into(),
        serial: k
            .identity
            .as_ref()
            .map_or_else(|| k.serial.clone(), |i| i.serial.clone()),
        code: k.code(),
        partitions: k.partition_count(),
    }
}

/// The filters and sort of a request.
fn query(rq: &Rq<'_>) -> (String, String, String) {
    let pick = |v: String, of: &[&str]| {
        if of.contains(&v.as_str()) {
            v
        } else {
            "All".to_string()
        }
    };
    (
        pick(rq.q("t"), &KINDS),
        pick(rq.q("s"), &STATUSES),
        rq.q("sort"),
    )
}

/// The rows a request lists, in order: every one, not only the page's.
fn listed(kept: &[Kept], kind: &str, state: &str, sort: &str) -> Vec<Row> {
    let mut rows: Vec<Row> = kept
        .iter()
        .map(row)
        .filter(|r| kind == "All" || r.kind == kind)
        .filter(|r| state == "All" || r.status == state)
        .collect();
    let num = |s: &str| s.parse::<f64>().unwrap_or(0.0);
    match sort {
        "type" => rows.sort_by(|a, b| a.kind.cmp(&b.kind).then(a.name.cmp(&b.name))),
        "status" => rows.sort_by(|a, b| a.status.cmp(&b.status).then(a.name.cmp(&b.name))),
        "cpu" => rows.sort_by(|a, b| num(&b.processors).total_cmp(&num(&a.processors))),
        "mem" => rows.sort_by(|a, b| num(&b.memory).total_cmp(&num(&a.memory))),
        "tm" => rows.sort_by(|a, b| a.model.cmp(&b.model).then(a.name.cmp(&b.name))),
        "sn" => rows.sort_by(|a, b| a.serial.cmp(&b.serial)),
        "rc" => rows.sort_by(|a, b| b.code.cmp(&a.code).then(a.name.cmp(&b.name))),
        "parts" => rows.sort_by_key(|r| std::cmp::Reverse(r.partitions)),
        _ => rows.sort_by(|a, b| a.name.cmp(&b.name)),
    }
    rows
}

/// What an action leads to for the systems selected.
fn act(rq: &Rq<'_>, action: &str, chosen: &[String]) -> Response {
    let one = chosen.first().map_or("", String::as_str);
    let needs_one = !matches!(action, "Add managed system" | "Reset or remove connection");
    if needs_one && chosen.is_empty() {
        let m = qmcmsg::message(Kind::Error, &qmcmsg::SELECT_SYSTEM, &[action]);
        return rq.done("/servers", m);
    }
    let to = match action {
        "Properties" => format!("/server/{one}/general"),
        "Power on" => format!("/server/{one}/confirm?op=poweron"),
        "Power off" => format!("/server/{one}/confirm?op=poweroff"),
        "Restart" => format!("/server/{one}/confirm?op=restart"),
        "Reset or remove connection" => "/connectivity".into(),
        "Add managed system" => "/servers/add".into(),
        "Reference code history" => format!("/refcodes?ms={one}"),
        "Schedule operations" => format!("/schedule/add?ms={one}"),
        "Create partition" => format!("/partitions/create/step/1?ms={one}"),
        "Manage partition profiles" => format!("/partitions?ms={one}"),
        "Virtual resources" => format!("/server/{one}/virtual/ethernet"),
        "Manage dumps" => "/dumps".into(),
        _ => {
            // A QS/VS host has no system profiles: not in V1.
            let m = qmcmsg::message(Kind::Info, &qmcmsg::NOT_IN_V1, &[action]);
            return rq.done("/servers", m);
        }
    };
    Response::redirect(&rq.ctx.url(&to))
}

/// `GET /servers?t=&s=&sort=&page=&rows=`, and its form's `POST`.
pub fn route(rq: &Rq<'_>) -> Response {
    let mut messages = Vec::new();
    let kept = systems(rq).unwrap_or_else(|m| {
        messages.push(m);
        Vec::new()
    });
    let (kind, state, sort) = query(rq);
    let rows = listed(&kept, &kind, &state, &sort);
    let paging = Paging::from_query(
        &rq.q("page"),
        &rq.q("rows"),
        rows.len(),
        vec![
            (
                "t".into(),
                if kind == "All" {
                    String::new()
                } else {
                    kind.clone()
                },
            ),
            (
                "s".into(),
                if state == "All" {
                    String::new()
                } else {
                    state.clone()
                },
            ),
            ("sort".into(), sort.clone()),
        ],
    );
    let range = paging.range();
    let first = range.start + 1;
    let page_rows: Vec<Row> = rows.get(range).map(<[Row]>::to_vec).unwrap_or_default();
    if rq.post() {
        let f = rq.req.form();
        let action = f.str("act");
        if !action.is_empty() && action != "--Select action--" {
            let chosen: Vec<String> = page_rows
                .iter()
                .enumerate()
                .filter(|(i, _)| f.has(&format!("s{}", first + i)))
                .map(|(_, r)| r.name.clone())
                .collect();
            return act(rq, action, &chosen);
        }
        // Go of the filter: the same list as a GET of its values.
        let mut q = Vec::new();
        for (k, v) in [("t", f.str("t")), ("s", f.str("s"))] {
            if !v.is_empty() && v != "All" {
                q.push(format!("{k}={}", qsweb::http::form::encode(v)));
            }
        }
        let to = if q.is_empty() {
            "/servers".to_string()
        } else {
            format!("/servers?{}", q.join("&"))
        };
        return Response::redirect(&rq.ctx.url(&to));
    }
    rq.page(page(
        &rq.ctx,
        &Servers {
            rows: page_rows,
            kind,
            status: state,
            paging,
            first,
            messages,
        },
    ))
}
