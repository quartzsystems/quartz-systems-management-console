// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! Serviceable events (`events.html`): every problem the managed
//! systems keep and the console's own, newest first, filtered by status,
//! dates and system, sorted by any column and paged.

use qsweb::frame::{Ctx, Page};
use qsweb::http::Response;
use qsweb::msg::{Kind, Message};
use qsweb::partials::{Cell, Col, FilterField, List, Paging, filter, list_table, paging, status};

use super::Ev;
use crate::app::routes::Rq;

/// One row.
#[derive(Clone, Debug, Default)]
pub struct Row {
    /// Its actions: each link and its words.
    pub actions: Vec<(String, &'static str)>,
    /// The problem's number.
    pub number: String,
    /// Open, or closed.
    pub open: bool,
    /// `10/29/26 22:14:05`.
    pub date: String,
    /// The reference code.
    pub code: String,
    /// Its explanation's link.
    pub code_link: String,
    /// The managed system.
    pub system: String,
    /// The partition; empty for the system.
    pub partition: String,
    /// The description.
    pub description: String,
}

/// What the page shows.
#[derive(Clone, Debug)]
pub struct Events {
    /// `Open`, `Closed` or `All`.
    pub status: String,
    /// From, as typed.
    pub from: String,
    /// To, as typed.
    pub to: String,
    /// The system chosen: `All` or a name.
    pub system: String,
    /// `All`, then every system and the console.
    pub systems: Vec<String>,
    /// The page's rows.
    pub rows: Vec<Row>,
    /// Where the list is.
    pub paging: Paging,
    /// The message rows.
    pub messages: Vec<Message>,
}

/// The status choices, the default first.
pub const STATUSES: [&str; 3] = ["Open", "Closed", "All"];

/// The page.
pub fn page(ctx: &Ctx, v: &Events) -> Page {
    let first = |opts: &[String], chosen: &str| {
        if opts.first().is_some_and(|f| f == chosen) {
            String::new()
        } else {
            chosen.to_string()
        }
    };
    let statuses: Vec<String> = STATUSES.iter().map(|s| (*s).to_string()).collect();
    let fields = [
        FilterField::Select(
            "Status".into(),
            "s".into(),
            statuses.clone(),
            first(&statuses, &v.status),
        ),
        FilterField::Text("From".into(), "f".into(), v.from.clone(), 8),
        FilterField::Text("To".into(), "t".into(), v.to.clone(), 8),
        FilterField::Select(
            "Managed system".into(),
            "ms".into(),
            v.systems.clone(),
            first(&v.systems, &v.system),
        ),
    ];
    let cols = vec![
        Col::plain("Actions"),
        Col::sorted("Problem", "p"),
        Col::sorted("Status", "s"),
        Col::sorted("Date", "d"),
        Col::sorted("Reference code", "rc"),
        Col::sorted("Managed system", "ms"),
        Col::sorted("Partition", "pt"),
        Col::plain("Description"),
    ];
    let rows = v
        .rows
        .iter()
        .map(|r| {
            let links: Vec<(String, &str)> =
                r.actions.iter().map(|(h, t)| (ctx.url(h), *t)).collect();
            vec![
                Cell::act(qsweb::partials::actions(&links)),
                Cell::num(&r.number),
                Cell::html(if r.open {
                    status("Open", Some(false))
                } else {
                    "Closed".into()
                }),
                Cell::mono(&r.date),
                Cell::mono_html(format!(
                    "<a class=\"mono\" href=\"{}\">{}</a>",
                    qsweb::html::esc(&ctx.url(&r.code_link)),
                    qsweb::html::esc(&r.code)
                )),
                Cell::mono(&r.system),
                Cell::mono(&r.partition),
                Cell::text(&r.description),
            ]
        })
        .collect();
    let body = format!(
        "{}{}{}",
        filter(&fields, false),
        list_table(&List {
            cols,
            rows,
            keep: v
                .paging
                .keep
                .iter()
                .filter(|(k, _)| k != "sort")
                .cloned()
                .collect(),
            ..List::default()
        }),
        paging(&v.paging)
    );
    Page {
        title: "Serviceable events".into(),
        crumb: vec!["Service Management".into(), "Serviceable events".into()],
        h1: "Serviceable events".into(),
        desc: Some(
            "Problems reported by managed systems and partitions that need attention or a record."
                .into(),
        ),
        messages: v.messages.clone(),
        body,
        help: "events".into(),
        nav: Some(("Service Management", Some("/events"))),
        ..Page::default()
    }
}

/// `text` as a date in the console's format: Unix seconds at the start
/// of that day in its zone.
fn day(rq: &Rq<'_>, text: &str) -> Option<i64> {
    let s = rq.app.settings();
    let (y, m, d) = crate::fmt::parse_date(text, &s.date_format)?;
    let zone = rq.app.host().zone();
    Some(appliance::time::from_civil(y, m, d, 0, 0, 0) - i64::from(zone.minutes) * 60)
}

/// The row of `e`.
pub fn row(rq: &Rq<'_>, e: &Ev) -> Row {
    let mut actions = vec![(e.route(), "View details")];
    if e.open {
        actions.push((e.close_route(), "Close"));
        actions.push((format!("{}/repair", e.route()), "Repair"));
    }
    let link = match e.owner {
        super::Owner::Fleet => format!("/refcode/{}?ms={}&p={}", e.code, e.system, e.partition_id),
        super::Owner::Console => format!("/refcode/{}", e.code),
    };
    Row {
        actions,
        number: e.number.to_string(),
        open: e.open,
        date: crate::systems::when(rq, e.first),
        code: e.code.clone(),
        code_link: link,
        system: e.system.clone(),
        partition: e.partition.clone(),
        description: e.description.clone(),
    }
}

/// `GET /events?s=&f=&t=&ms=&sort=&page=&rows=`.
pub fn route(rq: &Rq<'_>) -> Response {
    let (all, mut messages) = super::all(rq);
    let mut systems = vec!["All".to_string()];
    let mut names: Vec<String> = crate::systems::systems(rq)
        .unwrap_or_default()
        .into_iter()
        .map(|k| k.name)
        .collect();
    names.push(rq.app.console.name());
    for n in names {
        if !systems.contains(&n) {
            systems.push(n);
        }
    }
    let status = Some(rq.q("s"))
        .filter(|s| STATUSES.contains(&s.as_str()))
        .unwrap_or_else(|| "Open".into());
    let system = Some(rq.q("ms"))
        .filter(|s| systems.contains(s))
        .unwrap_or_else(|| "All".into());
    let (from, to) = (rq.q("f"), rq.q("t"));
    let mut bound = |text: &str, end: bool| -> Option<u64> {
        if text.trim().is_empty() {
            return None;
        }
        let Some(s) = day(rq, text) else {
            messages.push(qmcmsg::message(
                Kind::Error,
                &qmcmsg::DATE_NOT_VALID,
                &[text],
            ));
            return None;
        };
        u64::try_from(s + if end { 86_400 } else { 0 }).ok()
    };
    let (lo, hi) = (bound(&from, false), bound(&to, true));
    let mut chosen: Vec<&Ev> = all
        .iter()
        .filter(|e| match status.as_str() {
            "Open" => e.open,
            "Closed" => !e.open,
            _ => true,
        })
        .filter(|e| system == "All" || e.system.eq_ignore_ascii_case(&system))
        .filter(|e| lo.is_none_or(|l| e.first >= l) && hi.is_none_or(|h| e.first < h))
        .collect();
    match rq.q("sort").as_str() {
        "p" => chosen.sort_by_key(|e| std::cmp::Reverse(e.number)),
        "s" => chosen.sort_by_key(|e| !e.open),
        "rc" => chosen.sort_by(|a, b| a.code.cmp(&b.code)),
        "ms" => chosen.sort_by(|a, b| a.system.cmp(&b.system)),
        "pt" => chosen.sort_by(|a, b| a.partition.cmp(&b.partition)),
        // Newest first, as read.
        _ => {}
    }
    let keep = |v: &str, default: &str| {
        if v == default {
            String::new()
        } else {
            v.to_string()
        }
    };
    let paging = Paging::from_query(
        &rq.q("page"),
        &rq.q("rows"),
        chosen.len(),
        vec![
            ("s".into(), keep(&status, "Open")),
            ("f".into(), from.clone()),
            ("t".into(), to.clone()),
            ("ms".into(), keep(&system, "All")),
            ("sort".into(), rq.q("sort")),
        ],
    );
    let rows = chosen[paging.range()].iter().map(|e| row(rq, e)).collect();
    rq.page(page(
        &rq.ctx,
        &Events {
            status,
            from,
            to,
            system,
            systems,
            rows,
            paging,
            messages,
        },
    ))
}
