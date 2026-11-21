// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! Partitions (`docs/PARTITIONS.md`): the list across every host, a
//! partition's properties, activate and shut down, the create wizard,
//! its profiles. What a list shows is what `mgmtd` keeps; a partition's
//! own pages read its definition, its default profile with its revision
//! and its settings when they open, and put them back with that
//! revision.

#![allow(
    clippy::many_single_char_names,
    reason = "a form's fields are named as the design names them"
)]

pub mod activate;
pub mod confirm;
pub mod create;
pub mod edit;
pub mod list;
pub mod profiles;
pub mod props;
pub mod shutdown;

use qsweb::frame::{Ctx, Page};
use qsweb::http::Response;
use qsweb::msg::Message;

use mgmtproto::Kept;
use mgmtproto::kept::Kind as SystemKind;

use crate::app::routes::Rq;

/// A partition's tabs: route and label. Other is General's other half.
pub const TABS: [(&str, &str); 7] = [
    ("general", "General"),
    ("processors", "Hardware: Processors"),
    ("memory", "Hardware: Memory"),
    ("io", "Hardware: I/O"),
    ("vadapters", "Virtual adapters"),
    ("settings", "Settings"),
    ("other", "Other"),
];

/// The crumb's last word for a tab.
fn crumb_word(tab: &str) -> Option<&'static str> {
    match tab {
        "processors" => Some("Processors"),
        "memory" => Some("Memory"),
        "io" => Some("I/O"),
        "vadapters" => Some("Virtual adapters"),
        "settings" => Some("Settings"),
        "other" => Some("Other"),
        _ => None,
    }
}

/// `/qsmc/partition/<host>/<id>/<page>`.
pub fn href(ctx: &Ctx, host: &str, id: u32, page: &str) -> String {
    ctx.url(&format!("/partition/{host}/{id}/{page}"))
}

/// The tabs, `tab` current. Other is General with `?tab=other`, as the
/// design links it.
pub fn tabs(ctx: &Ctx, host: &str, id: u32, tab: &str) -> String {
    let list: Vec<(&str, String)> = TABS
        .iter()
        .map(|(r, l)| {
            let to = if *r == "other" {
                format!("{}?tab=other", href(ctx, host, id, "general"))
            } else {
                href(ctx, host, id, r)
            };
            (*l, to)
        })
        .collect();
    let current = TABS.iter().position(|(r, _)| *r == tab).unwrap_or(0);
    qsweb::partials::tabs(&list, current)
}

/// A partition's tab page.
pub fn page(
    ctx: &Ctx,
    host: &str,
    id: u32,
    name: &str,
    tab: &str,
    messages: Vec<Message>,
    body: &str,
) -> Page {
    let h1 = format!("Partition: {name}");
    let mut crumb = vec![
        "Systems Management".into(),
        "Partitions".into(),
        name.to_string(),
    ];
    if let Some(w) = crumb_word(tab) {
        crumb.push(w.into());
    }
    Page {
        title: h1.clone(),
        crumb,
        h1,
        messages,
        body: format!("{}{body}", tabs(ctx, host, id, tab)),
        help: "partition-properties".into(),
        nav: Some(("Systems Management", Some("/partitions"))),
        ..Page::default()
    }
}

/// A page under a partition that is not one of its tabs (activate, shut
/// down, profiles): its crumb through the partition to `word`, `h1`.
pub fn sub_page(name: &str, word: &str, h1: &str, messages: Vec<Message>, body: String) -> Page {
    Page {
        title: h1.into(),
        crumb: vec![
            "Systems Management".into(),
            "Partitions".into(),
            name.to_string(),
            word.into(),
        ],
        h1: h1.into(),
        messages,
        body,
        help: "partition-properties".into(),
        nav: Some(("Systems Management", Some("/partitions"))),
        ..Page::default()
    }
}

/// `/partition/<host>/<id>/<page>`: a partition's pages; the control
/// panel is the terminal's.
pub fn serve(rq: &Rq<'_>, host: &str, id: u32, page: &str) -> Response {
    let Some(k) = crate::systems::system(rq, host) else {
        return crate::app::routes::error(rq, 404, &qmcmsg::NO_SUCH_SYSTEM, &[host]);
    };
    // A standalone system is its own row: its properties are its own.
    if k.kind == SystemKind::Standalone {
        return Response::redirect(&rq.ctx.url(&format!("/server/{}/general", k.name)));
    }
    if k.part(id).is_none() {
        return crate::app::routes::error(
            rq,
            404,
            &qmcmsg::NO_SUCH_PARTITION,
            &[&id.to_string(), host],
        );
    }
    match page {
        "activate" => activate::route(rq, &k, id),
        "shutdown" => shutdown::route(rq, &k, id),
        "profiles" => profiles::route(rq, &k, id),
        "edit" => edit::route(rq, &k, id),
        "navigator" => navigator(rq, &k, id),
        p if TABS.iter().any(|(r, _)| *r == p) => props::route(rq, &k, id, p),
        _ => crate::app::routes::error(rq, 404, &qmcmsg::NOT_FOUND, &[&rq.req.path]),
    }
}

/// The partition's name in `k`.
pub fn name_of(k: &Kept, id: u32) -> String {
    k.part(id).map(|p| p.name).unwrap_or_default()
}

/// Navigator for QS/400 on the partition: the address its service
/// channel says, on Navigator's secure port; a message when it has none.
fn navigator(rq: &Rq<'_>, k: &Kept, id: u32) -> Response {
    let at = k
        .part(id)
        .and_then(|p| p.channel.addresses.first().cloned())
        .and_then(|a| a.split('/').next().map(str::to_string));
    if let Some(a) = at {
        return Response::redirect(&format!("https://{a}:2005/nav/ptfs"));
    }
    let m = qmcmsg::message(
        qsweb::msg::Kind::Error,
        &qmcmsg::NO_ADDRESS,
        &[&name_of(k, id)],
    );
    rq.done(&format!("/partition/{}/{id}/general", k.name), m)
}
