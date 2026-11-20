// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! Reference code history (`refcode-history.html`): the codes a managed
//! system and its partitions posted, newest first, as the
//! managed-systems service keeps them, read by their cursor and from
//! the events; filtered by system, partition and dates, and paged.

use std::fmt::Write as _;

use qsweb::frame::{Ctx, Page};
use qsweb::html::esc;
use qsweb::http::Response;
use qsweb::msg::{Kind, Message};
use qsweb::partials::{FilterField, Paging, filter, paging};

use mgmtproto::calls::Filter;
use mgmtproto::kept::Kind as SystemKind;

use crate::app::routes::Rq;
use crate::systems::{systems, when};

/// One row of the history.
#[derive(Clone, Debug, Default)]
pub struct Row {
    /// `10/30/26 06:02:14`.
    pub when: String,
    /// The code.
    pub code: String,
    /// Words 2 to 9.
    pub words: Vec<String>,
    /// The partition's name, `(host)` for the host itself.
    pub partition: String,
    /// The explanation's link: the code's route and query.
    pub link: String,
}

/// What the page shows.
#[derive(Clone, Debug)]
pub struct History {
    /// The system chosen.
    pub system: String,
    /// Every system, in the list's order.
    pub systems: Vec<String>,
    /// The partition chosen: `All`, `(host)` or a name.
    pub partition: String,
    /// The choices: `All`, `(host)`, then the partitions.
    pub partitions: Vec<String>,
    /// From, as typed.
    pub from: String,
    /// To, as typed.
    pub to: String,
    /// The page's rows.
    pub rows: Vec<Row>,
    /// Where the list is.
    pub paging: Paging,
    /// The message rows.
    pub messages: Vec<Message>,
}

/// The history as the console shows it, a line a code, in columns.
pub fn pre(ctx: &Ctx, rows: &[Row]) -> String {
    let mut s = format!("{:<21}{:<10}", "TIMESTAMP", "REFCODE");
    for n in 2..=9 {
        let _ = write!(s, "{:<9}", format!("WORD{n}"));
    }
    s.push_str("PARTITION");
    for r in rows {
        let mut words = r.words.clone();
        words.resize(8, "00000000".into());
        let _ = write!(
            s,
            "\n{:<21}<a href=\"{}\">{}</a>  {} {}",
            esc(&r.when),
            ctx.url(&r.link),
            esc(&r.code),
            esc(&words.join(" ")),
            esc(&r.partition)
        );
    }
    s
}

/// The page.
pub fn page(ctx: &Ctx, v: &History) -> Page {
    let first = |opts: &[String], chosen: &str| {
        if opts.first().is_some_and(|f| f == chosen) {
            String::new()
        } else {
            chosen.to_string()
        }
    };
    let fields = [
        FilterField::Select(
            "Managed system".into(),
            "ms".into(),
            v.systems.clone(),
            first(&v.systems, &v.system),
        ),
        FilterField::Select(
            "Partition".into(),
            "p".into(),
            v.partitions.clone(),
            first(&v.partitions, &v.partition),
        ),
        FilterField::Text("From".into(), "f".into(), v.from.clone(), 8),
        FilterField::Text("To".into(), "t".into(), v.to.clone(), 8),
    ];
    let body = format!(
        "{}<pre class=\"out\">{}\n</pre>{}<p class=\"small grey\">Codes link to their explanation. Words 2-9 are shown as the service processor reports them.</p>",
        filter(&fields, true),
        pre(ctx, &v.rows),
        paging(&v.paging)
    );
    let h1 = if v.system.is_empty() {
        "Reference code history".to_string()
    } else {
        format!("Reference code history: {}", v.system)
    };
    Page {
        title: h1.clone(),
        crumb: vec!["Service Management".into(), "Reference code history".into()],
        h1,
        desc: Some(
            "Reference codes reported by the managed system and its partitions, newest first."
                .into(),
        ),
        messages: v.messages.clone(),
        body,
        help: "refcodes".into(),
        nav: Some(("Service Management", Some("/refcodes"))),
        ..Page::default()
    }
}

/// `text` as a date in the console's format, Unix seconds at the start
/// of that day in its zone.
fn day(rq: &Rq<'_>, text: &str) -> Option<i64> {
    let s = rq.app.settings();
    let (y, m, d) = crate::fmt::parse_date(text, &s.date_format)?;
    let zone = rq.app.host().zone();
    Some(appliance::time::from_civil(y, m, d, 0, 0, 0) - i64::from(zone.minutes) * 60)
}

/// `GET /refcodes?ms=&p=&f=&t=&page=&rows=`.
#[allow(
    clippy::too_many_lines,
    reason = "the filters, the page and its rows, top to bottom"
)]
pub fn route(rq: &Rq<'_>) -> Response {
    let mut messages = Vec::new();
    let kept = systems(rq).unwrap_or_else(|m| {
        messages.push(m);
        Vec::new()
    });
    let names: Vec<String> = kept.iter().map(|k| k.name.clone()).collect();
    let system = Some(rq.q("ms"))
        .filter(|n| names.contains(n))
        .or_else(|| names.first().cloned())
        .unwrap_or_default();
    let k = kept.iter().find(|k| k.name == system);
    let own = if k.is_some_and(|k| k.kind == SystemKind::Standalone) {
        "(system)"
    } else {
        "(host)"
    };
    let mut partitions = vec!["All".to_string(), own.to_string()];
    if let Some(k) = k {
        partitions.extend(k.partitions.iter().map(|p| p.name.clone()));
    }
    let chosen = Some(rq.q("p"))
        .filter(|p| partitions.contains(p))
        .unwrap_or_else(|| "All".into());
    let partition = match chosen.as_str() {
        "All" => None,
        p if p == own => Some(0),
        name => k
            .and_then(|k| k.partitions.iter().find(|p| p.name == name))
            .map(|p| p.id),
    };
    let (from, to) = (rq.q("f"), rq.q("t"));
    let mut bound = |text: &str, end: bool| -> u64 {
        if text.trim().is_empty() {
            return 0;
        }
        if let Some(s) = day(rq, text) {
            u64::try_from(s + if end { 86_400 } else { 0 }).unwrap_or(0) * 1000
        } else {
            messages.push(qmcmsg::message(
                Kind::Error,
                &qmcmsg::DATE_NOT_VALID,
                &[text],
            ));
            0
        }
    };
    let filter_ = Filter {
        system: system.clone(),
        partition,
        from_ms: bound(&from, false),
        to_ms: bound(&to, true),
        code: String::new(),
    };
    let wanted = Paging::from_query(&rq.q("page"), &rq.q("rows"), usize::MAX, Vec::new());
    let skip = u32::try_from((wanted.page.max(1) - 1) * wanted.rows.max(1)).unwrap_or(0);
    let page_of = rq
        .app
        .console
        .mgmt()
        .history(filter_, skip, u32::try_from(wanted.rows).unwrap_or(25))
        .unwrap_or_else(|e| {
            messages.push(crate::systems::unreached(&e));
            mgmtproto::calls::Page::default()
        });
    let paging = Paging::from_query(
        &rq.q("page"),
        &rq.q("rows"),
        page_of.total as usize,
        vec![
            ("ms".into(), system.clone()),
            (
                "p".into(),
                if chosen == "All" {
                    String::new()
                } else {
                    chosen.clone()
                },
            ),
            ("f".into(), from.clone()),
            ("t".into(), to.clone()),
        ],
    );
    let rows = page_of
        .rows
        .iter()
        .map(|r| {
            let code = &r.posted.refcode.code;
            Row {
                when: when(rq, r.posted.refcode.time_ms / 1000),
                code: code.clone(),
                words: r.posted.refcode.words.clone(),
                partition: if r.partition == 0 {
                    own.into()
                } else {
                    r.partition_name.clone()
                },
                link: format!(
                    "/refcode/{code}?ms={}&p={}&seq={}",
                    r.system, r.partition, r.posted.refcode.seq
                ),
            }
        })
        .collect();
    rq.page(page(
        &rq.ctx,
        &History {
            system,
            systems: names,
            partition: chosen,
            partitions,
            from,
            to,
            rows,
            paging,
            messages,
        },
    ))
}
