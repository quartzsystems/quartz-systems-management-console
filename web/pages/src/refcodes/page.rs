// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! A reference code explained (`refcode.html`): its family, words, who
//! reported it and when, and Meaning, Cause and Recovery from the data
//! (`explain`); a partition's code links to its terminal window and its
//! control panel. A code the data does not know shows its text as
//! posted, with the line that this release has no explanation for it.

use qsweb::frame::{Ctx, Page};
use qsweb::html::esc;
use qsweb::http::Response;
use qsweb::msg::{Kind, Message};
use qsweb::partials::{Prop, PropGroup, mono, props_table};

use super::explain::{self, Explained, Whose};
use crate::app::routes::Rq;
use crate::systems::{system, when};

/// What reported the code.
#[derive(Clone, Debug, Default)]
pub struct Reporter {
    /// The managed system.
    pub system: String,
    /// The partition's name and ID, when a partition's.
    pub partition: Option<(String, u32)>,
    /// When it was posted.
    pub when: String,
}

/// What the page shows.
#[derive(Clone, Debug, Default)]
pub struct Explanation {
    /// The code.
    pub code: String,
    /// `A6xx - partition attention, ...`.
    pub family: String,
    /// Words 2 to 9.
    pub words: String,
    /// Who reported it, where known.
    pub by: Option<Reporter>,
    /// `Attention - ...`.
    pub severity: String,
    /// Meaning.
    pub meaning: String,
    /// Cause; empty where the data has none.
    pub cause: String,
    /// Recovery; empty where the data has none.
    pub recovery: String,
    /// The message rows.
    pub messages: Vec<Message>,
}

fn para(s: &str) -> String {
    if s.is_empty() {
        "<p class=\"grey\">The data of this release does not say.</p>".into()
    } else {
        format!("<p>{}</p>", esc(s))
    }
}

/// The page.
pub fn page(ctx: &Ctx, v: &Explanation) -> Page {
    let url = |r: &str| ctx.url(r);
    let mut rows = vec![
        Prop::new(
            "Reference code",
            format!(
                "<span class=\"mono\" style=\"font-size:12pt\">{}</span>",
                esc(&v.code)
            ),
        ),
        Prop::new("Family", esc(&v.family)),
        Prop::new("Words 2-9", mono(&v.words)),
    ];
    let mut buttons = String::new();
    if let Some(b) = &v.by {
        let sys = format!(
            "<a href=\"{}\">{}</a>",
            url(&format!("/server/{}/general", esc(&b.system))),
            esc(&b.system)
        );
        let who = match &b.partition {
            Some((name, id)) => {
                buttons = format!(
                    "<input type=\"submit\" value=\"Open terminal window\" formaction=\"{}\"><input type=\"submit\" value=\"Control panel functions\" formaction=\"{}\">",
                    url(&format!("/terminal/{}/{id}", esc(&b.system))),
                    url(&format!("/partition/{}/{id}/panel", esc(&b.system)))
                );
                format!(
                    "<a href=\"{}\">{}</a> (ID {id}) on {sys}",
                    url(&format!("/partition/{}/{id}/general", esc(&b.system))),
                    esc(name)
                )
            }
            None => sys,
        };
        rows.push(Prop::new("Reported by", format!("{who}, {}", esc(&b.when))));
    }
    rows.push(Prop::new("Severity", esc(&v.severity)));
    let body = format!(
        "{}<h2>Meaning</h2>{}<h2>Cause</h2>{}<h2>Recovery</h2>{}<form method=\"post\">{}<div class=\"buttons\">{buttons}<input type=\"submit\" value=\"&lt; Back\"></div></form>",
        props_table(ctx.product, &[PropGroup { title: None, rows }], true),
        para(&v.meaning),
        para(&v.cause),
        para(&v.recovery),
        ctx.csrf_field()
    );
    let h1 = format!("Reference code {}", v.code);
    Page {
        title: h1.clone(),
        crumb: vec![
            "Service Management".into(),
            "Reference code history".into(),
            v.code.clone(),
        ],
        h1,
        messages: v.messages.clone(),
        body,
        help: "refcode".into(),
        nav: Some(("Service Management", Some("/refcodes"))),
        ..Page::default()
    }
}

fn kind_word(code: &str) -> &'static str {
    match code.as_bytes().first() {
        Some(b'A') => "Attention",
        Some(b'B') => "Fatal",
        Some(b'C') => "Progress",
        Some(b'D') => "Power down",
        _ => "None",
    }
}

/// A code's family in words.
pub fn family(code: &str, e: Option<&Explained>) -> String {
    let head = code.get(..2).map(|h| format!("{h}xx")).unwrap_or_default();
    match explain::whose(code) {
        Whose::None => "None - the system is operating".into(),
        Whose::System => match srcdata::Family::of(code) {
            Some(f) => format!("{head} - {}", f.description()),
            None => format!("{head} - the system's"),
        },
        Whose::Host => format!(
            "{head} - QS/VS host {}",
            e.map_or("code", |e| e.family.as_str())
        ),
        Whose::Console => format!("{head} - this console's"),
    }
}

/// `GET /refcode/<code>?ms=&p=&seq=`; its form's Back.
pub fn route(rq: &Rq<'_>, code: &str) -> Response {
    let (ms, p, seq) = (rq.q("ms"), rq.q("p"), rq.q("seq"));
    if rq.post() {
        let back = if ms.is_empty() {
            "/refcodes".to_string()
        } else {
            format!("/refcodes?ms={ms}")
        };
        return Response::redirect(&rq.ctx.url(&back));
    }
    let id: u32 = p.parse().unwrap_or(0);
    let k = system(rq, &ms);
    let posted = k.as_ref().and_then(|k| {
        let page = rq
            .app
            .console
            .mgmt()
            .history(
                mgmtproto::calls::Filter {
                    system: k.name.clone(),
                    partition: Some(id),
                    code: code.into(),
                    ..Default::default()
                },
                0,
                1000,
            )
            .ok()?;
        let n: u64 = seq.parse().unwrap_or(0);
        page.rows
            .into_iter()
            .find(|r| n == 0 || r.posted.refcode.seq == n)
    });
    let e = explain::explain(rq.app.catalogue(), code);
    let mut messages = Vec::new();
    let (meaning, cause, recovery, title) = if let Some(e) = &e {
        (
            e.meaning.clone(),
            e.cause.clone(),
            e.recovery.clone(),
            e.title.clone(),
        )
    } else {
        messages.push(qmcmsg::message(
            Kind::Info,
            &qmcmsg::NO_EXPLANATION,
            &[code],
        ));
        let text = posted
            .as_ref()
            .map(|r| r.posted.refcode.text.clone())
            .unwrap_or_default();
        (text.clone(), String::new(), String::new(), text)
    };
    let words = posted.as_ref().map_or_else(
        || ["00000000"; 8].join(" "),
        |r| {
            let mut w = r.posted.refcode.words.clone();
            w.resize(8, "00000000".into());
            w.join(" ")
        },
    );
    let by = k.as_ref().map(|k| Reporter {
        system: k.name.clone(),
        partition: (id != 0).then(|| {
            let name = k
                .partitions
                .iter()
                .find(|p| p.id == id)
                .map(|p| p.name.clone())
                .unwrap_or_default();
            (name, id)
        }),
        when: posted.as_ref().map_or_else(
            || "not in the history".into(),
            |r| when(rq, r.posted.refcode.time_ms / 1000),
        ),
    });
    let v = Explanation {
        code: code.into(),
        family: family(code, e.as_ref()),
        words,
        by,
        severity: if title.is_empty() {
            kind_word(code).into()
        } else {
            format!("{} - {title}", kind_word(code))
        },
        meaning,
        cause,
        recovery,
        messages,
    };
    rq.page(page(&rq.ctx, &v))
}
