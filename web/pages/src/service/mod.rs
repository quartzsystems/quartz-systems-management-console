// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! Service Management's pages of Phase 6 (`docs/SERVICE.md`):
//! serviceable events across the fleet and the console, one in whole,
//! and the dumps every system holds.
//!
//! - [`events`]: the list, its filters, sorting and paging.
//! - [`event`]: one event: its detail, close, note, export, repair.
//! - [`dumps`]: Manage dumps.
//!
//! A managed system's problem is its own: the console keeps its row
//! under a number of the console's (`mgmtd`) and acts on it over QSI. The
//! console's own problems are its problem service's, under that log's
//! numbers, at `/event/console/<n>`.

pub mod dumps;
pub mod event;
pub mod events;

use mgmtproto::calls::{ProblemFilter, ProblemRow};
use qsweb::msg::{Kind, Message};

use crate::app::routes::Rq;

/// Whose a problem is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Owner {
    /// A managed system's, by the console's number for it.
    Fleet,
    /// The console's own, by its problem log's number.
    Console,
}

/// How grave a problem is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Grave {
    /// The system went on.
    Attention,
    /// The system stopped.
    Terminal,
    /// A record.
    Information,
    /// Its system rates none (QSI's service version 1).
    Unrated,
}

impl Grave {
    /// In a person's words.
    pub fn words(self) -> &'static str {
        match self {
            Grave::Attention => "Attention - the system continues",
            Grave::Terminal => "Terminal - the system stopped",
            Grave::Information => "Information",
            Grave::Unrated => "Not rated by the system",
        }
    }

    /// The priority of the first resource a problem names; each after is
    /// one lower. The design has a priority and QSI none: it is derived
    /// from the severity (`docs/SERVICE.md`).
    pub fn priority(self, index: usize) -> &'static str {
        const P: [&str; 3] = ["High", "Medium", "Low"];
        let first = match self {
            Grave::Terminal => 0,
            Grave::Attention | Grave::Unrated => 1,
            Grave::Information => 2,
        };
        P[(first + index).min(2)]
    }
}

/// A serviceable event as the pages show it, whoever's it is.
#[derive(Clone, Debug)]
pub struct Ev {
    /// Whose.
    pub owner: Owner,
    /// Its number on the console.
    pub number: u64,
    /// The owning system's own number for it.
    pub theirs: u64,
    /// The managed system, or the console's name.
    pub system: String,
    /// The partition's name; empty for the system itself.
    pub partition: String,
    /// The partition's ID; 0 for the system.
    pub partition_id: u32,
    /// Open.
    pub open: bool,
    /// First seen, Unix seconds.
    pub first: u64,
    /// The code as last posted for it.
    pub code: String,
    /// Its words 2 to 9.
    pub words: Vec<String>,
    /// Its description.
    pub description: String,
    /// Its message.
    pub msgid: String,
    /// The condition still holds.
    pub holds: bool,
    /// How grave.
    pub grave: Grave,
}

impl Ev {
    /// The event's page.
    pub fn route(&self) -> String {
        match self.owner {
            Owner::Fleet => format!("/event/{}", self.number),
            Owner::Console => format!("/event/console/{}", self.number),
        }
    }

    /// Its confirmation of a close.
    pub fn close_route(&self) -> String {
        match self.owner {
            Owner::Fleet => format!("/confirm/event-close?n={}", self.number),
            Owner::Console => format!("/confirm/event-close?c={}", self.number),
        }
    }

    /// A managed system's row as an event.
    pub fn fleet(r: &ProblemRow) -> Self {
        use qsi::service::{ProblemStatus, Severity, SourceKind};
        let s = &r.summary;
        Self {
            owner: Owner::Fleet,
            number: r.number,
            theirs: s.number,
            system: r.system.clone(),
            partition: if s.source.kind == SourceKind::Partition {
                s.source.name.clone()
            } else {
                String::new()
            },
            partition_id: if s.source.kind == SourceKind::Partition {
                s.source.id
            } else {
                0
            },
            open: s.status != ProblemStatus::Closed,
            first: s.first_seen,
            code: s.refcode.code.clone(),
            words: s.refcode.words.clone(),
            description: s.description.clone(),
            msgid: s.msgid.clone(),
            holds: s.holds,
            grave: match r.severity {
                Some(Severity::Terminal) => Grave::Terminal,
                Some(Severity::Information) => Grave::Information,
                Some(_) => Grave::Attention,
                None => Grave::Unrated,
            },
        }
    }

    /// One of the console's own as an event, `console` its name.
    pub fn console(r: &problem::Rated, console: &str) -> Self {
        let s = &r.summary;
        Self {
            owner: Owner::Console,
            number: s.number,
            theirs: s.number,
            system: console.into(),
            partition: String::new(),
            partition_id: 0,
            open: s.status != problem::Status::Closed,
            first: s.first_seen,
            code: s.refcode.code.clone(),
            words: s.refcode.words.clone(),
            description: s.description.clone(),
            msgid: s.msgid.clone(),
            holds: s.holds,
            grave: match r.severity {
                problem::Severity::Terminal => Grave::Terminal,
                problem::Severity::Attention => Grave::Attention,
            },
        }
    }
}

/// The most of either kind a list reads.
const MOST: u32 = 5000;

/// Every event the console knows, newest first: the fleet's as kept and
/// the console's own; why either could not be read.
pub fn all(rq: &Rq<'_>) -> (Vec<Ev>, Vec<Message>) {
    let mut out = Vec::new();
    let mut messages = Vec::new();
    match rq
        .app
        .console
        .mgmt()
        .problems(ProblemFilter::default(), 0, MOST)
    {
        Ok(p) => out.extend(p.rows.iter().map(Ev::fleet)),
        Err(e) => messages.push(crate::systems::unreached(&e)),
    }
    let console = rq.app.console.name();
    match rq
        .app
        .console
        .problems()
        .list_rated(problem::Filter::default(), None, 500)
    {
        Ok(p) => out.extend(p.problems.iter().map(|r| Ev::console(r, &console))),
        Err(e) => messages.push(qmcmsg::message(
            Kind::Error,
            &qmcmsg::SERVICE_DOWN,
            &["The console's problem service", &e.to_string()],
        )),
    }
    out.sort_by(|a, b| b.first.cmp(&a.first).then(b.number.cmp(&a.number)));
    (out, messages)
}

/// How many events are open: the fleet's and the console's.
pub fn open_count(rq: &Rq<'_>) -> usize {
    all(rq).0.iter().filter(|e| e.open).count()
}
