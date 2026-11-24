// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! A standalone QS/400's own tabs: Status, what it has and uses as its
//! `WRKSYSSTS` counts them (QSI base's `Resources`, asked when the tab
//! is shown, so the processor used is since the last look), and Dumps,
//! its own dumps as `mgmtd` keeps them. A host has neither: its
//! processors and memory are the host family's tabs, its dumps are its
//! partitions'.

use qsweb::frame::Ctx;
use qsweb::html::esc;
use qsweb::msg::{Kind, Message};
use qsweb::partials::{Cell, Col, List, Prop, PropGroup, list_table};

use mgmtproto::Kept;
use qsi::any::{AnyReply, AnyRequest};
use qsi::base::{self, Resources};
use qsi::{Empty, caps};

use crate::app::routes::Rq;

/// The Status tab.
#[derive(Clone, Debug, Default)]
pub struct Status {
    /// Processors, `4`.
    pub processors: String,
    /// Processor used, `12.5 %`.
    pub cpu: String,
    /// Main storage, `32,768 MB`.
    pub memory: String,
    /// In use, `21,504 MB (65.6 %)`.
    pub memory_used: String,
    /// The system ASP, `1,200,000 MB`.
    pub asp: String,
    /// Its use, `62.15 %`.
    pub asp_used: String,
    /// Temporary storage, `2,310 MB`.
    pub temp: String,
    /// Jobs in the system, active, signed-on users, batch running,
    /// threads.
    pub jobs: [String; 5],
    /// Since the IPL, `12 days 3 hours`.
    pub up: String,
}

/// One of the system's own dumps.
#[derive(Clone, Debug, Default)]
pub struct DumpRow {
    /// Its name.
    pub name: String,
    /// Its kind's words.
    pub kind: String,
    /// When it was taken.
    pub taken: String,
    /// Its size, `120 MB`.
    pub size: String,
    /// Who took it.
    pub by: String,
}

/// Hundredths of a percent as `12.5 %`.
fn percent(h: u32) -> String {
    let p = format!("{}.{:02}", h / 100, h % 100);
    format!("{} %", p.trim_end_matches('0').trim_end_matches('.'))
}

/// Seconds as `12 days 3 hours`, `2 hours 5 minutes`.
fn elapsed(s: u64) -> String {
    let (d, h, m) = (s / 86_400, s % 86_400 / 3_600, s % 3_600 / 60);
    let unit = |n: u64, w: &str| format!("{n} {w}{}", if n == 1 { "" } else { "s" });
    match (d, h) {
        (0, 0) => unit(m, "minute"),
        (0, _) => format!("{} {}", unit(h, "hour"), unit(m, "minute")),
        _ => format!("{} {}", unit(d, "day"), unit(h, "hour")),
    }
}

/// `r` as the tab shows it, numbers in the console's format.
pub fn status(r: &Resources, number_format: &str) -> Status {
    let n = |v: u64| crate::fmt::number(v, number_format);
    let mb = |v: u64| format!("{} MB", n(v));
    let used = if r.memory_used_kb == 0 || r.memory_kb == 0 {
        "not reported".to_string()
    } else {
        let h =
            u32::try_from(r.memory_used_kb.saturating_mul(10_000) / r.memory_kb).unwrap_or(10_000);
        format!("{} ({})", mb(r.memory_used_kb / 1024), percent(h))
    };
    Status {
        processors: r.processors.to_string(),
        cpu: percent(r.cpu_used),
        memory: mb(r.memory_kb / 1024),
        memory_used: used,
        asp: mb(r.asp_mb),
        asp_used: percent(r.asp_used),
        temp: mb(r.temp_mb),
        jobs: [
            n(u64::from(r.total_jobs)),
            n(u64::from(r.active_jobs)),
            n(u64::from(r.signed_on)),
            n(u64::from(r.batch_running)),
            n(u64::from(r.threads)),
        ],
        up: if r.up_secs == 0 {
            "not reported".into()
        } else {
            elapsed(r.up_secs)
        },
    }
}

/// The Status tab's body.
pub fn status_body(ctx: &Ctx, s: &Status) -> String {
    let row = |l: &str, v: &str| Prop::new(l, esc(v));
    let groups = [
        PropGroup {
            title: Some("Processors".into()),
            rows: vec![
                row("Processors", &s.processors),
                row("Processor used", &s.cpu),
            ],
        },
        PropGroup {
            title: Some("Main storage".into()),
            rows: vec![row("Installed", &s.memory), row("In use", &s.memory_used)],
        },
        PropGroup {
            title: Some("Storage".into()),
            rows: vec![
                row("System ASP", &s.asp),
                row("System ASP used", &s.asp_used),
                row("Temporary storage", &s.temp),
            ],
        },
        PropGroup {
            title: Some("Jobs".into()),
            rows: vec![
                row("Jobs in system", &s.jobs[0]),
                row("Active jobs", &s.jobs[1]),
                row("Signed-on users", &s.jobs[2]),
                row("Batch jobs running", &s.jobs[3]),
                row("Threads", &s.jobs[4]),
            ],
        },
        PropGroup {
            title: Some("Since the IPL".into()),
            rows: vec![row("Up", &s.up)],
        },
    ];
    format!(
        "{}<p class=\"small grey\">As the system's own Work with System Status counts them; the processor used is since this tab was last shown.</p>",
        super::props::plain(ctx, &groups, &[])
    )
}

/// What `k` has and uses, asked now; what is kept when it cannot be
/// asked, with why.
pub fn status_of(rq: &Rq<'_>, k: &Kept) -> (Status, Vec<Message>) {
    let format = rq.app.settings().number_format;
    if let Some(what) = mgmtproto::ask::lacks(k.caps, caps::RESOURCES) {
        let m = qmcmsg::message(Kind::Error, &qmcmsg::LACKS, &[&k.name, what]);
        return (none(), vec![m]);
    }
    let req = AnyRequest::Base(base::Request::Resources(Empty));
    match super::ask(rq, &k.name, "Read status", &req) {
        Ok(AnyReply::Base(base::Reply::Resources(r))) => (status(&r, &format), Vec::new()),
        Ok(_) => (none(), Vec::new()),
        Err(ms) => match &k.usage {
            Some(r) => (status(r, &format), ms),
            None => (none(), ms),
        },
    }
}

fn none() -> Status {
    let n = || "not reported".to_string();
    Status {
        processors: n(),
        cpu: n(),
        memory: n(),
        memory_used: n(),
        asp: n(),
        asp_used: n(),
        temp: n(),
        jobs: [n(), n(), n(), n(), n()],
        up: n(),
    }
}

/// The Dumps tab's body.
pub fn dumps_body(ctx: &Ctx, rows: &[DumpRow]) -> String {
    let list = list_table(&List {
        cols: vec![
            Col::plain("Dump"),
            Col::plain("Kind"),
            Col::plain("Taken"),
            Col::plain("Size"),
            Col::plain("Taken by"),
        ],
        rows: rows
            .iter()
            .map(|d| {
                vec![
                    Cell::mono(&d.name),
                    Cell::text(&d.kind),
                    Cell::mono(&d.taken),
                    Cell::num(&d.size),
                    Cell::mono(&d.by),
                ]
            })
            .collect(),
        empty: "The system has no dumps.".into(),
        keep: Vec::new(),
    });
    format!(
        "{list}<p><a href=\"{}\">Manage dumps</a> to take, download or delete one.</p>",
        esc(&ctx.url("/dumps"))
    )
}

/// `k`'s own dumps, newest first.
pub fn dumps_of(rq: &Rq<'_>, k: &Kept) -> Vec<DumpRow> {
    let format = rq.app.settings().number_format;
    k.dumps
        .iter()
        .rev()
        .map(|d| DumpRow {
            name: d.name.clone(),
            kind: if d.kind_name.is_empty() {
                "Service data".into()
            } else {
                d.kind_name.clone()
            },
            taken: super::when(rq, d.time),
            size: format!("{} MB", crate::fmt::number(d.bytes >> 20, &format)),
            by: d.by.clone(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn figures_in_words() {
        assert_eq!(percent(1250), "12.5 %");
        assert_eq!(percent(6215), "62.15 %");
        assert_eq!(percent(0), "0 %");
        assert_eq!(percent(10_000), "100 %");
        assert_eq!(elapsed(86_400 * 12 + 3 * 3_600), "12 days 3 hours");
        assert_eq!(elapsed(3_600 + 300), "1 hour 5 minutes");
        assert_eq!(elapsed(59), "0 minutes");
        let r = Resources {
            processors: 4,
            cpu_used: 1250,
            memory_kb: 32 * 1024 * 1024,
            memory_used_kb: 16 * 1024 * 1024,
            asp_mb: 1_200_000,
            asp_used: 6215,
            temp_mb: 2_310,
            total_jobs: 342,
            active_jobs: 287,
            signed_on: 12,
            batch_running: 9,
            threads: 1_104,
            up_secs: 0,
        };
        let s = status(&r, "1,234.56");
        assert_eq!(s.memory, "32,768 MB");
        assert_eq!(s.memory_used, "16,384 MB (50 %)");
        assert_eq!(s.asp, "1,200,000 MB");
        assert_eq!(s.jobs[4], "1,104");
        assert_eq!(s.up, "not reported");
    }
}
