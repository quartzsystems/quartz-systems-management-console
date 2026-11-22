// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! System plans (`plans.html`): every plan with View, Deploy, Export and
//! Delete; Create plan from a managed system; Import. Delete goes
//! through its confirmation (`/confirm/plan-delete`).

use std::fmt::Write as _;

use qsweb::frame::{Ctx, Page};
use qsweb::html::esc;
use qsweb::http::Response;
use qsweb::msg::{Kind, Message};
use qsweb::partials::{Cell, Col, List, list_table};

use super::{NAV, down, issued, route_of, when};
use crate::app::routes::Rq;

/// A plan's row.
#[derive(Clone, Debug, Default)]
pub struct Row {
    /// Its name.
    pub name: String,
    /// Its description.
    pub description: String,
    /// When it was made, formatted.
    pub created: String,
    /// Its managed system; empty for a template.
    pub system: String,
    /// Its partitions.
    pub partitions: u32,
    /// Its size, `41 KB`.
    pub size: String,
}

/// The list.
#[derive(Clone, Debug, Default)]
pub struct Plans {
    /// The rows, in the page's order.
    pub rows: Vec<Row>,
    /// The managed systems Create plan offers.
    pub systems: Vec<String>,
    /// The message rows.
    pub messages: Vec<Message>,
}

/// The page.
pub fn page(ctx: &Ctx, v: &Plans) -> Page {
    let rows = v
        .rows
        .iter()
        .map(|r| {
            let at = |rest: &str| esc(&ctx.url(&route_of(&r.name, rest)));
            let del = esc(&ctx.url(&format!(
                "/confirm/plan-delete?p={}",
                qsweb::http::form::encode(&r.name)
            )));
            vec![
                Cell::act(format!(
                    "<a href=\"{}\">View</a> | <a href=\"{}\">Deploy</a> | <a href=\"{}\">Export</a> | <a href=\"{del}\">Delete</a>",
                    at(""),
                    at("/deploy/step/1"),
                    at("/export"),
                )),
                Cell::mono(&r.name),
                Cell::text(&r.description),
                Cell::mono(&r.created),
                Cell::mono(if r.system.is_empty() {
                    "(template)"
                } else {
                    &r.system
                }),
                Cell::num(&r.partitions.to_string()),
                Cell::text(&r.size),
            ]
        })
        .collect();
    let list = List {
        cols: vec![
            Col::plain("Actions"),
            Col::sorted("Plan", "n"),
            Col::plain("Description"),
            Col::sorted("Created", "c"),
            Col::plain("Managed system"),
            Col::plain("Partitions"),
            Col::plain("Size"),
        ],
        rows,
        empty: "No system plans. Create one from a managed system, or import one.".into(),
        keep: Vec::new(),
    };
    let mut options = String::new();
    for s in &v.systems {
        let _ = write!(options, "<option>{}</option>", esc(s));
    }
    let body = format!(
        "{}<form method=\"post\" enctype=\"multipart/form-data\">{}<div class=\"buttons\">Create from: <select name=\"ms\">{options}</select> <input type=\"submit\" name=\"create\" value=\"Create plan\"> &nbsp; Import: <input type=\"file\" name=\"f\" size=\"25\"> <input type=\"submit\" name=\"import\" value=\"Import\"></div></form>",
        list_table(&list),
        ctx.csrf_field()
    );
    Page {
        title: "System plans".into(),
        crumb: vec![NAV.into()],
        h1: "System plans".into(),
        desc: Some("Saved configurations of managed systems and their partitions.".into()),
        messages: v.messages.clone(),
        body,
        help: "plans".into(),
        nav: Some((NAV, Some("/plans"))),
        ..Page::default()
    }
}

/// `41 KB` of `bytes`.
pub fn size(bytes: u64) -> String {
    format!("{} KB", bytes.div_ceil(1024).max(1))
}

/// The rows of the scheduler's entries: an operator's plans first, the
/// newest first, then the automatic ones; by name with `?sort=n`, the
/// newest first with `?sort=c`.
pub fn rows(rq: &Rq<'_>, mut entries: Vec<plan::index::Entry>, sort: &str) -> Vec<Row> {
    match sort {
        "n" => entries.sort_by(|a, b| a.name.cmp(&b.name)),
        "c" => entries.sort_by_key(|e| std::cmp::Reverse(e.created)),
        _ => entries.sort_by(|a, b| {
            a.automatic
                .cmp(&b.automatic)
                .then(b.created.cmp(&a.created))
        }),
    }
    entries
        .into_iter()
        .map(|e| Row {
            created: when(rq, e.created),
            size: size(e.bytes),
            name: e.name,
            description: e.description,
            system: e.system,
            partitions: e.partitions,
        })
        .collect()
}

fn show(rq: &Rq<'_>, mut messages: Vec<Message>) -> Response {
    let mut sched = rq.app.console.sched();
    let entries = match sched.plans() {
        Ok((e, _)) => e,
        Err(e) => {
            messages.push(down(&e));
            Vec::new()
        }
    };
    let systems = crate::systems::systems(rq)
        .unwrap_or_default()
        .into_iter()
        .map(|k| k.name)
        .collect();
    let v = Plans {
        rows: rows(rq, entries, &rq.q("sort")),
        systems,
        messages,
    };
    rq.page(page(&rq.ctx, &v))
}

/// `GET`, `POST /plans`: the list; Create plan; Import.
pub fn route(rq: &Rq<'_>) -> Response {
    if !rq.post() {
        return show(rq, Vec::new());
    }
    let parts = rq.req.multipart().unwrap_or_default();
    let field = |n: &str| parts.iter().find(|p| p.name == n);
    let mut sched = rq.app.console.sched();
    if field("import").is_some() {
        let bytes = field("f").map(|p| p.data.clone()).unwrap_or_default();
        return match sched.import(bytes, rq.user().to_string()) {
            Ok(Ok(e)) => rq.done(
                "/plans",
                qmcmsg::message(Kind::Done, &qmcmsg::PLAN_IMPORTED, &[&e.name]),
            ),
            Ok(Err(i)) => show(rq, vec![issued(Kind::Error, &i)]),
            Err(e) => show(rq, vec![down(&e)]),
        };
    }
    let ms = field("ms")
        .map(|p| String::from_utf8_lossy(&p.data).trim().to_string())
        .unwrap_or_default();
    if ms.is_empty() {
        return show(
            rq,
            vec![qmcmsg::message(
                Kind::Error,
                &qmcmsg::SELECT_SYSTEM,
                &["Create plan"],
            )],
        );
    }
    match sched.capture(
        ms.clone(),
        String::new(),
        String::new(),
        rq.user().to_string(),
    ) {
        Ok(Ok(e)) => rq.done(
            "/plans",
            qmcmsg::message(Kind::Done, &qmcmsg::PLAN_CAPTURED, &[&e.name, &ms]),
        ),
        Ok(Err(i)) => show(rq, vec![issued(Kind::Error, &i)]),
        Err(e) => show(rq, vec![down(&e)]),
    }
}

/// `GET /plan/<name>/export`: the plan's document, downloaded.
pub fn export(rq: &Rq<'_>, name: &str) -> Response {
    match rq.app.console.sched().plan(name.to_string()) {
        Ok(Some(text)) => {
            rq.audit(&format!("export system plan {name}"), "exported");
            Response::new(200, "application/toml", text.into_bytes()).with(
                "Content-Disposition",
                &format!("attachment; filename=\"{name}.{}\"", plan::doc::EXTENSION),
            )
        }
        Ok(None) => crate::app::routes::error(rq, 404, &qmcmsg::NO_SUCH_PLAN, &[name]),
        Err(e) => show(rq, vec![down(&e)]),
    }
}

/// `GET`, `POST /confirm/plan-delete?p=`.
pub fn delete(rq: &Rq<'_>) -> Response {
    let f = rq.req.form();
    let name = if rq.post() {
        f.str("p").to_string()
    } else {
        rq.q("p")
    };
    let mut sched = rq.app.console.sched();
    let entry = match sched.plans() {
        Ok((e, _)) => e.into_iter().find(|e| e.name == name),
        Err(e) => return show(rq, vec![down(&e)]),
    };
    let Some(entry) = entry else {
        return crate::app::routes::error(rq, 404, &qmcmsg::NO_SUCH_PLAN, &[&name]);
    };
    if rq.post() {
        if f.get("ok").is_none() {
            return Response::redirect(&rq.ctx.url("/plans"));
        }
        return match sched.delete_plan(name.clone(), rq.user().to_string()) {
            Ok(Ok(())) => rq.done(
                "/plans",
                qmcmsg::message(Kind::Done, &qmcmsg::PLAN_DELETED, &[&name]),
            ),
            Ok(Err(i)) => rq.done("/plans", issued(Kind::Error, &i)),
            Err(e) => rq.done("/plans", down(&e)),
        };
    }
    let system = if entry.system.is_empty() {
        "(template)".to_string()
    } else {
        entry.system.clone()
    };
    let body = qsweb::pages::confirm_body(
        &rq.ctx,
        "The plan is deleted; no managed system is changed. Press OK to delete it, or Cancel to return without deleting it.",
        &format!(
            "<table class=\"props\" cellspacing=\"0\"><tr><th>Plan</th><td>{}</td></tr><tr><th>Description</th><td>{}</td></tr><tr><th>Managed system</th><td>{}</td></tr><tr><th>Created</th><td>{}</td></tr></table>",
            qsweb::partials::mono(&name),
            esc(&entry.description),
            qsweb::partials::mono(&system),
            esc(&when(rq, entry.created)),
        ),
        &qsweb::partials::hidden("p", &name),
    );
    rq.page(Page {
        title: "Confirm Delete System Plan".into(),
        crumb: vec![NAV.into(), name.clone(), "Delete".into()],
        h1: "Confirm Delete System Plan".into(),
        body,
        help: "plans".into(),
        nav: Some((NAV, None)),
        ..Page::default()
    })
}
