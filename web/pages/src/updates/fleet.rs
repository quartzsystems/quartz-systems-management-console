// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! Updates (`updates.html`): one table of the console, its hosts, their
//! partitions and the standalone systems, each with its level, the
//! newest its stream offers, its status and when it was last checked;
//! Check all; links to Code updates and, for a QS/400 whose address is
//! known, to its own fixes page.

use fetchd::Stream;
use qsweb::frame::{Ctx, Page};
use qsweb::http::Response;
use qsweb::msg::{Kind, Message};
use qsweb::partials::{Cell, Col, FilterField, List, Paging, filter, list_table, paging, status};

use super::{fetched, host_level, known, level, offered};
use crate::app::routes::Rq;

/// One row.
#[derive(Clone, Debug, Default)]
pub struct Row {
    /// Its one action: the link and its words.
    pub action: (String, String),
    /// The console's, a system's or a partition's name.
    pub name: String,
    /// `QS/MC console`, `QS/VS host`, `QS/400 (standalone)`, `QS/400 partition`.
    pub kind: String,
    /// What it runs.
    pub current: String,
    /// The newest its stream offers.
    pub latest: String,
    /// `Current`, `Update available`, `Unknown`.
    pub status: String,
    /// When its stream was last checked.
    pub checked: String,
}

/// What the page shows.
#[derive(Clone, Debug, Default)]
pub struct Fleet {
    /// `All`, `Update available`, `Current`, `Unknown`.
    pub show: String,
    /// `All`, `Console`, `QS/VS hosts`, `Partitions`.
    pub kind: String,
    /// The page's rows.
    pub rows: Vec<Row>,
    /// Where the list is.
    pub paging: Option<Paging>,
    /// The message rows.
    pub messages: Vec<Message>,
}

/// The Show choices, the default first.
pub const SHOWS: [&str; 4] = ["All", "Update available", "Current", "Unknown"];
/// The Kind choices, the default first.
pub const KINDS: [&str; 4] = ["All", "Console", "QS/VS hosts", "Partitions"];

/// The page.
pub fn page(ctx: &Ctx, v: &Fleet) -> Page {
    let pick = |opts: &[&str], chosen: &str| {
        if opts.first() == Some(&chosen) {
            String::new()
        } else {
            chosen.to_string()
        }
    };
    let fields = [
        FilterField::Select(
            "Show".into(),
            "s".into(),
            SHOWS.map(String::from).to_vec(),
            pick(&SHOWS, &v.show),
        ),
        FilterField::Select(
            "Kind".into(),
            "k".into(),
            KINDS.map(String::from).to_vec(),
            pick(&KINDS, &v.kind),
        ),
    ];
    // The filter's Go, then Check all, as the design has them.
    let form = filter(&fields, false).replacen(
        "<input type=\"submit\" value=\"Go\">",
        "<input type=\"submit\" value=\"Go\"> <input type=\"submit\" name=\"check\" value=\"Check all\">",
        1,
    );
    let cols = vec![
        Col::plain("Actions"),
        Col::sorted("Name", "n"),
        Col::sorted("Kind", "k"),
        Col::sorted("Current level", "c"),
        Col::sorted("Latest available", "l"),
        Col::sorted("Status", "s"),
        Col::plain("Last checked"),
    ];
    let rows = v
        .rows
        .iter()
        .map(|r| {
            let good = match r.status.as_str() {
                "Current" => Some(true),
                "Update available" => Some(false),
                _ => None,
            };
            vec![
                Cell::act(qsweb::partials::actions(&[(
                    ctx.url(&r.action.0),
                    r.action.1.as_str(),
                )])),
                Cell::mono(&r.name),
                Cell::text(&r.kind),
                Cell::mono(&r.current),
                Cell::mono(&r.latest),
                Cell::html(status(&r.status, good)),
                Cell::text(&r.checked),
            ]
        })
        .collect();
    let body = format!(
        "{form}{}{}<p class=\"small grey\">Partition levels are read from the operating system through the service interface; a partition that is not activated reports Unknown.</p>",
        list_table(&List {
            cols,
            rows,
            ..List::default()
        }),
        v.paging.as_ref().map(paging).unwrap_or_default()
    );
    Page {
        title: "Updates".into(),
        crumb: vec!["Service Management".into(), "Updates".into()],
        h1: "Updates".into(),
        desc: Some(
            "Code levels across the fleet: console, QS/VS hosts and QS/400 partitions.".into(),
        ),
        messages: v.messages.clone(),
        body,
        help: "updates".into(),
        nav: Some(("Service Management", Some("/updates"))),
        ..Page::default()
    }
}

/// Whether a QS/400 level `current` is the catalog's `latest`: the same
/// name, or no older by the date in a cumulative package's name.
fn at_least(current: &str, latest: &str) -> bool {
    if current == latest {
        return true;
    }
    match (
        current.parse::<updcat::Cumulative>(),
        latest.parse::<updcat::Cumulative>(),
    ) {
        (Ok(c), Ok(l)) => (c.year, c.day) >= (l.year, l.day),
        _ => false,
    }
}

/// Every row, the console first.
#[allow(clippy::too_many_lines, reason = "a row of each kind")]
fn rows(rq: &Rq<'_>, messages: &mut Vec<Message>) -> Vec<Row> {
    let f = fetched(rq).unwrap_or_else(|m| {
        messages.push(m);
        fetchd::proto::Status::default()
    });
    let s = rq.app.settings();
    let zone = rq.app.host().zone();
    let when = |secs: u64| {
        if secs == 0 {
            return "never".to_string();
        }
        let t = i64::try_from(secs).unwrap_or(0);
        format!(
            "{} {}",
            crate::fmt::date(t, zone, &s.date_format),
            crate::fmt::time(t, zone, &s.time_format, false)
        )
    };
    let checked = |st: Stream| when(known(&f, st).map_or(0, |k| k.checked));
    let state = |offer: bool, known: bool| {
        if !known {
            "Unknown"
        } else if offer {
            "Update available"
        } else {
            "Current"
        }
    };
    let code = ("/updates/code".to_string(), "Code updates".to_string());
    let mut out = Vec::new();
    let mine = level(&release::RELEASE.to_string(), release::BUILD_ID);
    let offer = offered(&f, Stream::Qsmc, &release::RELEASE.to_string());
    out.push(Row {
        action: code.clone(),
        name: rq.app.console.name(),
        kind: "QS/MC console".into(),
        current: mine.clone(),
        latest: offer.map_or(mine, |e| level(&e.release, &e.build)),
        status: state(offer.is_some(), known(&f, Stream::Qsmc).is_some()).into(),
        checked: checked(Stream::Qsmc),
    });
    let kept = crate::systems::systems(rq).unwrap_or_else(|m| {
        messages.push(m);
        Vec::new()
    });
    let catalog = known(&f, Stream::Qs400);
    let latest = |release: &str| {
        catalog.and_then(|k| {
            k.levels
                .iter()
                .find(|l| l.release == release)
                .map(|l| l.name.clone())
        })
    };
    let mut parts = Vec::new();
    for k in &kept {
        if k.kind == mgmtproto::kept::Kind::Standalone {
            let current = k
                .fix_level
                .as_ref()
                .map(|l| l.level.clone())
                .unwrap_or_default();
            let release = k
                .identity
                .as_ref()
                .map(|i| i.release.clone())
                .unwrap_or_default();
            let newest = latest(&release);
            out.push(Row {
                action: (
                    format!("/partition/{}/0/navigator", k.name),
                    "Navigator PTFs".into(),
                ),
                name: k.name.clone(),
                kind: "QS/400 (standalone)".into(),
                // A system that answered with no level has no cumulative
                // package installed; one not asked is not known.
                current: match (&k.fix_level, current.is_empty()) {
                    (_, false) => current.clone(),
                    (Some(_), true) => "(none installed)".into(),
                    (None, true) => "(not known)".into(),
                },
                latest: newest.clone().unwrap_or_else(|| "(not known)".into()),
                status: match &newest {
                    Some(n) if !current.is_empty() => state(!at_least(&current, n), true),
                    _ => "Unknown",
                }
                .into(),
                checked: checked(Stream::Qs400),
            });
            continue;
        }
        let (release, build) = host_level(k);
        let offer = offered(&f, Stream::Qsvs, &release);
        let current = level(&release, &build);
        out.push(Row {
            action: code.clone(),
            name: k.name.clone(),
            kind: "QS/VS host".into(),
            current: current.clone(),
            latest: offer.map_or(current, |e| level(&e.release, &e.build)),
            status: state(
                offer.is_some(),
                known(&f, Stream::Qsvs).is_some() && !release.is_empty(),
            )
            .into(),
            checked: checked(Stream::Qsvs),
        });
        for p in &k.partitions {
            let running = p.channel.up && !p.channel.fix_level.is_empty();
            let newest = latest(&p.channel.release);
            parts.push(Row {
                action: (
                    format!("/partition/{}/{}/navigator", k.name, p.id),
                    "Navigator PTFs".into(),
                ),
                name: p.name.clone(),
                kind: "QS/400 partition".into(),
                current: if running {
                    p.channel.fix_level.clone()
                } else if matches!(p.state, qsi::host::model::PartitionState::NotActivated) {
                    "(not activated)".into()
                } else {
                    "(not known)".into()
                },
                latest: newest.clone().unwrap_or_else(|| "(not known)".into()),
                status: match &newest {
                    Some(n) if running => state(!at_least(&p.channel.fix_level, n), true),
                    _ => "Unknown",
                }
                .into(),
                checked: checked(Stream::Qs400),
            });
        }
    }
    out.extend(parts);
    out
}

/// `GET /updates?s=&k=&check=&sort=&page=&rows=`.
pub fn route(rq: &Rq<'_>) -> Response {
    let mut messages = Vec::new();
    if !rq.q("check").is_empty() {
        match rq.app.console.fetch().check(rq.user().into()) {
            Ok(_) => rq.audit("product streams", "checked"),
            Err(e) => messages.push(qmcmsg::message(
                Kind::Error,
                &qmcmsg::SERVICE_DOWN,
                &["The download service", &e.to_string()],
            )),
        }
    }
    let show = Some(rq.q("s"))
        .filter(|s| SHOWS.contains(&s.as_str()))
        .unwrap_or_else(|| "All".into());
    let kind = Some(rq.q("k"))
        .filter(|s| KINDS.contains(&s.as_str()))
        .unwrap_or_else(|| "All".into());
    let mut all: Vec<Row> = rows(rq, &mut messages)
        .into_iter()
        .filter(|r| show == "All" || r.status == show)
        .filter(|r| match kind.as_str() {
            "Console" => r.kind == "QS/MC console",
            "QS/VS hosts" => r.kind == "QS/VS host",
            "Partitions" => r.kind.starts_with("QS/400"),
            _ => true,
        })
        .collect();
    match rq.q("sort").as_str() {
        "n" => all.sort_by(|a, b| a.name.cmp(&b.name)),
        "k" => all.sort_by(|a, b| a.kind.cmp(&b.kind)),
        "c" => all.sort_by(|a, b| a.current.cmp(&b.current)),
        "l" => all.sort_by(|a, b| a.latest.cmp(&b.latest)),
        "s" => all.sort_by(|a, b| a.status.cmp(&b.status)),
        _ => {}
    }
    let keep = |v: &str| {
        if v == "All" {
            String::new()
        } else {
            v.to_string()
        }
    };
    let paging = Paging::from_query(
        &rq.q("page"),
        &rq.q("rows"),
        all.len(),
        vec![
            ("s".into(), keep(&show)),
            ("k".into(), keep(&kind)),
            ("sort".into(), rq.q("sort")),
        ],
    );
    let rows = all[paging.range()].to_vec();
    rq.page(page(
        &rq.ctx,
        &Fleet {
            show,
            kind,
            rows,
            paging: Some(paging),
            messages,
        },
    ))
}

/// How many updates are available, and of what, for Welcome:
/// `2 (QS/VS host, 1 partition)`.
pub fn available(rq: &Rq<'_>) -> String {
    let mut messages = Vec::new();
    let all = rows(rq, &mut messages);
    let of = |k: &str| {
        all.iter()
            .filter(|r| r.status == "Update available" && r.kind == k)
            .count()
    };
    let (console, hosts, standalone, parts) = (
        of("QS/MC console"),
        of("QS/VS host"),
        of("QS/400 (standalone)"),
        of("QS/400 partition"),
    );
    let n = console + hosts + standalone + parts;
    let mut what = Vec::new();
    let mut say = |n: usize, one: &str, many: &str| match n {
        0 => {}
        1 => what.push(one.to_string()),
        n => what.push(format!("{n} {many}")),
    };
    say(console, "the console", "consoles");
    say(hosts, "QS/VS host", "QS/VS hosts");
    say(standalone, "1 standalone system", "standalone systems");
    say(parts, "1 partition", "partitions");
    if n == 0 {
        "0".into()
    } else {
        format!("{n} ({})", what.join(", "))
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_cumulative_level_is_compared_by_its_date() {
        assert!(super::at_least("C26290110", "C26290110"));
        assert!(super::at_least("C26300110", "C26290110"));
        assert!(!super::at_least("C26270110", "C26290110"));
        assert!(!super::at_least("QS400-CUM-26270", "QS400-CUM-26290"));
    }
}
