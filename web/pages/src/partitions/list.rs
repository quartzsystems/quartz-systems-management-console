// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! Partitions (`partitions.html`, `-empty`, `-error`, `-paged`): every
//! partition of every host and each standalone system as one row with
//! no ID, filtered, sorted and paged, with the grouped action list. The
//! reference code is on every row. A list never waits on a managed
//! system: a host not connected keeps its last rows, marked.

use std::fmt::Write as _;

use qsweb::frame::{Ctx, Page};
use qsweb::html::esc;
use qsweb::http::Response;
use qsweb::msg::{Kind, Message};
use qsweb::partials::{Cell, Col, List, Paging, list_table, paging, status};

use mgmtproto::Kept;
use mgmtproto::kept::NO_CODE;
use mgmtproto::kept::{Kind as SystemKind, Link};
use qsi::host::model::{PartitionState, ProcessorMode};
use qsi::host::partitions::Status;

use crate::app::routes::Rq;
use crate::systems::props::units;

/// One row.
#[derive(Clone, Debug, Default)]
pub struct Row {
    /// The partition's name; a standalone system's own.
    pub name: String,
    /// Its ID; 0 for a standalone system, shown `-`.
    pub id: u32,
    /// `Running`, `Not activated`, `Shutting down`, `Error`.
    pub status: String,
    /// Its managed system.
    pub system: String,
    /// `QS/400`.
    pub environment: String,
    /// Its processing units: `1.00`.
    pub units: String,
    /// Its memory in GB: `16.0`.
    pub memory: String,
    /// Its profile activated with; `(standalone)`.
    pub profile: String,
    /// Its current code, never empty.
    pub code: String,
    /// Its system's release.
    pub os: String,
}

/// What the page shows.
#[derive(Clone, Debug)]
pub struct Partitions {
    /// The page's rows.
    pub rows: Vec<Row>,
    /// The systems the filter offers.
    pub systems: Vec<String>,
    /// The system filter.
    pub system: String,
    /// The status filter.
    pub status: String,
    /// The name filter.
    pub name: String,
    /// Where the list is.
    pub paging: Paging,
    /// The number the page's first row's box takes.
    pub first: usize,
    /// The message rows.
    pub messages: Vec<Message>,
    /// The custom group shown, when it is a group's page.
    pub group: Option<String>,
}

/// The status filter's choices.
pub const STATUSES: [&str; 6] = [
    "All",
    "Running",
    "Not activated",
    "Shutting down",
    "Error",
    "Open firmware",
];

/// The action list's groups.
pub const ACTIONS: &[(&str, &[&str])] = &[
    (
        "Operations",
        &[
            "Activate",
            "Shut down",
            "Restart",
            "Open terminal window",
            "Schedule operations",
        ],
    ),
    (
        "Configuration",
        &[
            "Manage profiles",
            "Save current configuration",
            "Delete partition",
        ],
    ),
    (
        "Serviceability",
        &[
            "Reference code history",
            "Manage dumps",
            "Control panel functions",
        ],
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

/// Whether a status word is good, bad or neither.
pub fn good(word: &str) -> Option<bool> {
    match word {
        "Running" => Some(true),
        "Not activated" | "Error" | "No connection" => Some(false),
        _ => None,
    }
}

/// The table and its toolbar, inside the page's form.
pub fn table(ctx: &Ctx, v: &Partitions) -> String {
    let url = |r: &str| ctx.url(r);
    let rows = v
        .rows
        .iter()
        .enumerate()
        .map(|(i, r)| {
            let n = esc(&r.name);
            vec![
                Cell::act(format!(
                    "<input type=\"checkbox\" name=\"p{}\">",
                    v.first + i
                )),
                Cell::mono_html(format!(
                    "<a href=\"{}\">{n}</a>",
                    url(&format!("/partition/{}/{}/general", esc(&r.system), r.id))
                )),
                Cell::num(&if r.id == 0 {
                    "-".to_string()
                } else {
                    r.id.to_string()
                }),
                Cell::html(status(&r.status, good(&r.status))),
                Cell::mono(&r.system),
                Cell::mono(&r.environment),
                Cell::num(&r.units),
                Cell::num(&r.memory),
                Cell::mono(&r.profile),
                Cell::mono_html(format!(
                    "<a class=\"mono\" href=\"{}\">{}</a>",
                    url(&format!(
                        "/refcode/{}?ms={}&p={}",
                        esc(&r.code),
                        esc(&r.system),
                        r.id
                    )),
                    esc(&r.code)
                )),
                Cell::mono(&r.os),
            ]
        })
        .collect();
    let list = List {
        cols: vec![
            Col::plain("Select"),
            Col::sorted("Name", "name"),
            Col::sorted("ID", "id"),
            Col::sorted("Status", "status"),
            Col::sorted("Managed system", "ms"),
            Col::plain("Environment"),
            Col::sorted("Proc. units", "pu"),
            Col::sorted("Memory (GB)", "mem"),
            Col::sorted("Active profile", "prof"),
            Col::sorted("Reference code", "rc"),
            Col::sorted("OS version", "os"),
        ],
        rows,
        empty: "No partitions on the selected managed systems. Use Create partition or deploy a system plan.".into(),
        keep: Vec::new(),
    };
    let actions =
        qsweb::partials::action_select("act", "--Select action--", &["Properties"], ACTIONS);
    let mut systems = vec!["All"];
    systems.extend(v.systems.iter().map(String::as_str));
    format!(
        "<table class=\"filter\" cellspacing=\"0\"><tr><td>Managed system:</td><td>{}</td><td>Status:</td><td>{}</td><td>Name:</td><td><input type=\"text\" name=\"n\" value=\"{}\" size=\"10\"></td><td><input type=\"submit\" value=\"Go\"> <input type=\"submit\" name=\"refresh\" value=\"Refresh\"></td></tr></table>{}<div class=\"toolbar\">Selected: {actions} <input type=\"submit\" value=\"Go\"> &nbsp; <input type=\"submit\" value=\"Create partition...\" formaction=\"{}\"></div>",
        select("ms", &systems, &v.system),
        select("s", &STATUSES, &v.status),
        esc(&v.name),
        list_table(&list),
        url("/partitions/create/step/1"),
    )
}

/// The page.
pub fn page(ctx: &Ctx, v: &Partitions) -> Page {
    let body = format!(
        "<form method=\"post\">{}{}</form>{}",
        ctx.csrf_field(),
        table(ctx, v),
        paging(&v.paging)
    );
    Page {
        title: "Partitions".into(),
        crumb: vec!["Systems Management".into(), "Partitions".into()],
        h1: "Partitions".into(),
        desc: Some(
            "All partitions across all managed systems. A standalone QS/400 appears as one row with no ID."
                .into(),
        ),
        messages: v.messages.clone(),
        body,
        help: "partitions".into(),
        nav: Some(("Systems Management", Some("/partitions"))),
        ..Page::default()
    }
}

/// A partition's state in the list's words.
pub fn state_word(s: PartitionState) -> &'static str {
    match s {
        PartitionState::Running => "Running",
        PartitionState::Starting => "Starting",
        PartitionState::ShuttingDown => "Shutting down",
        PartitionState::Error => "Error",
        _ => "Not activated",
    }
}

/// Memory in MiB as the list's GB: `16.0`.
pub fn gb(mib: u64) -> String {
    format!("{}.{}", mib / 1024, mib % 1024 * 10 / 1024)
}

/// The processing units a partition holds now: `1.00`; `0.00` when not
/// activated.
pub fn held(p: &Status) -> (String, String) {
    match &p.current {
        Some(c) => {
            let u = if c.mode == ProcessorMode::Dedicated {
                u64::try_from(c.cores.len()).unwrap_or(0) * 100
            } else {
                u64::from(c.units)
            };
            (units(u), gb(c.memory_mib))
        }
        None => ("0.00".into(), "0.0".into()),
    }
}

/// The rows of `k`: its partitions, or itself for a standalone system.
pub fn rows_of(k: &Kept) -> Vec<Row> {
    let away = k.link != Link::Connected;
    if k.kind == SystemKind::Standalone {
        let me = k.itself();
        return vec![Row {
            name: k.name.clone(),
            id: 0,
            status: if away {
                "No connection".into()
            } else {
                state_word(me.state).into()
            },
            system: k.name.clone(),
            environment: "QS/400".into(),
            units: String::new(),
            memory: String::new(),
            profile: "(standalone)".into(),
            code: me.code,
            os: k.release().into(),
        }];
    }
    k.partitions
        .iter()
        .map(|p| {
            let (units, memory) = held(p);
            Row {
                name: p.name.clone(),
                id: p.id,
                status: if away {
                    "No connection".into()
                } else {
                    state_word(p.state).into()
                },
                system: k.name.clone(),
                environment: "QS/400".into(),
                units,
                memory,
                profile: p.profile.clone(),
                code: if p.code.is_empty() {
                    NO_CODE.into()
                } else {
                    p.code.clone()
                },
                os: p.channel.release.clone(),
            }
        })
        .collect()
}

/// Every row of `kept` that passes the filters, in `sort`'s order.
pub fn listed(kept: &[Kept], system: &str, state: &str, name: &str, sort: &str) -> Vec<Row> {
    let name = name.trim().to_uppercase();
    let mut rows: Vec<Row> = kept
        .iter()
        .filter(|k| system == "All" || k.name == system)
        .flat_map(rows_of)
        .filter(|r| state == "All" || r.status == state)
        .filter(|r| name.is_empty() || r.name.to_uppercase().contains(&name))
        .collect();
    let num = |s: &str| s.parse::<f64>().unwrap_or(0.0);
    match sort {
        "id" => rows.sort_by(|a, b| (&a.system, a.id).cmp(&(&b.system, b.id))),
        "status" => rows.sort_by(|a, b| a.status.cmp(&b.status).then(a.name.cmp(&b.name))),
        "ms" => rows.sort_by(|a, b| (&a.system, a.id).cmp(&(&b.system, b.id))),
        "pu" => rows.sort_by(|a, b| num(&b.units).total_cmp(&num(&a.units))),
        "mem" => rows.sort_by(|a, b| num(&b.memory).total_cmp(&num(&a.memory))),
        "prof" => rows.sort_by(|a, b| a.profile.cmp(&b.profile).then(a.name.cmp(&b.name))),
        "rc" => rows.sort_by(|a, b| b.code.cmp(&a.code).then(a.name.cmp(&b.name))),
        "os" => rows.sort_by(|a, b| a.os.cmp(&b.os).then(a.name.cmp(&b.name))),
        "name" => rows.sort_by(|a, b| a.name.cmp(&b.name)),
        _ => rows.sort_by(|a, b| (&a.system, a.id).cmp(&(&b.system, b.id))),
    }
    rows
}

/// The messages the list carries of what waits: a partition waiting
/// for a console.
pub fn waiting(rows: &[Row]) -> Vec<Message> {
    rows.iter()
        .filter(|r| r.code == "A6005008")
        .map(|r| {
            qmcmsg::message(
                Kind::Info,
                &qmcmsg::WAITING_FOR_CONSOLE,
                &[&r.name, &r.code],
            )
        })
        .collect()
}

/// Where an action leads for the partitions chosen (system, ID).
pub fn act(rq: &Rq<'_>, back: &str, action: &str, chosen: &[(String, u32)]) -> Response {
    let Some((ms, id)) = chosen.first() else {
        let m = qmcmsg::message(Kind::Error, &qmcmsg::SELECT_PARTITION, &[action]);
        return rq.done(back, m);
    };
    let to = match action {
        "Properties" => format!("/partition/{ms}/{id}/general"),
        "Activate" => format!("/partition/{ms}/{id}/activate"),
        "Shut down" => format!("/partition/{ms}/{id}/shutdown"),
        "Restart" => format!("/partition/{ms}/{id}/shutdown?restart=1"),
        "Open terminal window" => format!("/terminal/{ms}/{id}"),
        "Schedule operations" => format!("/schedule/add?ms={ms}&p={id}"),
        "Manage profiles" => format!("/partition/{ms}/{id}/profiles"),
        "Save current configuration" => format!("/partition/{ms}/{id}/profiles?save=1"),
        "Delete partition" => format!("/confirm/partition-delete?ms={ms}&p={id}"),
        "Reference code history" => format!("/refcodes?ms={ms}&p={id}"),
        "Control panel functions" => format!("/partition/{ms}/{id}/panel"),
        _ => {
            let m = qmcmsg::message(Kind::Info, &qmcmsg::LATER_RELEASE, &[action]);
            return rq.done(back, m);
        }
    };
    Response::redirect(&rq.ctx.url(&to))
}

/// The list of `kept`'s rows by the request's filters, for the route at
/// `here` (the Partitions list, or a group's page): the view, and every
/// row it pages over.
pub fn view(rq: &Rq<'_>, kept: &[Kept], messages: Vec<Message>) -> (Partitions, Vec<Row>) {
    let systems: Vec<String> = kept.iter().map(|k| k.name.clone()).collect();
    let pick = |v: String, of: &[String]| {
        if of.contains(&v) {
            v
        } else {
            "All".to_string()
        }
    };
    let system = pick(rq.q("ms"), &systems);
    let state = {
        let s = rq.q("s");
        if STATUSES.contains(&s.as_str()) {
            s
        } else {
            "All".into()
        }
    };
    let (name, sort) = (rq.q("n"), rq.q("sort"));
    let rows = listed(kept, &system, &state, &name, &sort);
    let keep = |k: &str, v: &str| {
        (
            k.to_string(),
            if v == "All" {
                String::new()
            } else {
                v.to_string()
            },
        )
    };
    let paging = Paging::from_query(
        &rq.q("page"),
        &rq.q("rows"),
        rows.len(),
        vec![
            keep("ms", &system),
            keep("s", &state),
            keep("n", &name),
            keep("sort", &sort),
        ],
    );
    let range = paging.range();
    let first = range.start + 1;
    let page_rows: Vec<Row> = rows.get(range).map(<[Row]>::to_vec).unwrap_or_default();
    let mut messages = messages;
    messages.extend(waiting(&page_rows));
    (
        Partitions {
            rows: page_rows,
            systems,
            system,
            status: state,
            name,
            paging,
            first,
            messages,
            group: None,
        },
        rows,
    )
}

/// A post of the list's form at `here`: an action on the rows chosen,
/// or the filter's Go as a GET of its values.
pub fn post(rq: &Rq<'_>, here: &str, v: &Partitions) -> Response {
    let f = rq.req.form();
    let action = f.str("act");
    if !action.is_empty() && action != "--Select action--" {
        let chosen: Vec<(String, u32)> = v
            .rows
            .iter()
            .enumerate()
            .filter(|(i, _)| f.has(&format!("p{}", v.first + i)))
            .map(|(_, r)| (r.system.clone(), r.id))
            .collect();
        return act(rq, here, action, &chosen);
    }
    let mut q = Vec::new();
    for (k, val) in [("ms", f.str("ms")), ("s", f.str("s")), ("n", f.str("n"))] {
        if !val.is_empty() && val != "All" {
            q.push(format!("{k}={}", qsweb::http::form::encode(val)));
        }
    }
    let to = if q.is_empty() {
        here.to_string()
    } else {
        format!("{here}?{}", q.join("&"))
    };
    Response::redirect(&rq.ctx.url(&to))
}

/// `GET /partitions?ms=&s=&n=&sort=&page=&rows=`, and its form's `POST`.
pub fn route(rq: &Rq<'_>) -> Response {
    let mut messages = Vec::new();
    let kept = crate::systems::systems(rq).unwrap_or_else(|m| {
        messages.push(m);
        Vec::new()
    });
    let (v, _) = view(rq, &kept, messages);
    if rq.post() {
        return post(rq, "/partitions", &v);
    }
    rq.page(page(&rq.ctx, &v))
}
