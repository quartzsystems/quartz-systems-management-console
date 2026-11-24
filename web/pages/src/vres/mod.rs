// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! A host's virtual resources (`vr-ethernet.html` and its four tabs,
//! `create-vdisk.html`, `docs/VRES.md`): what the managed-systems
//! service keeps of the host's switches, pools, virtual disks, media,
//! tape and processor pools, and the forms that change them. A list
//! never waits on the host; an action is a request to it, and a
//! refusal shows its message. Each tab is a module with its view and
//! its route.

pub mod confirm;
pub mod ethernet;
pub mod forms;
pub mod optical;
pub mod pools;
pub mod storage;
pub mod tape;
pub mod upload;
pub mod vdisk;

use qsweb::frame::{Ctx, Page};
use qsweb::http::Response;
use qsweb::msg::Message;

use crate::app::routes::Rq;

/// The tabs: route and label.
pub const TABS: [(&str, &str); 5] = [
    ("ethernet", "Virtual Ethernet"),
    ("storage", "Virtual Storage"),
    ("optical", "Virtual Optical"),
    ("tape", "Virtual Tape"),
    ("pools", "Shared processor pools"),
];

/// `/qsmc/server/<host>/virtual/<tab>`.
pub fn href(ctx: &Ctx, host: &str, tab: &str) -> String {
    ctx.url(&format!("/server/{host}/virtual/{tab}"))
}

/// The tabs above every page, `tab` current.
pub fn tabs(ctx: &Ctx, host: &str, tab: &str) -> String {
    let list: Vec<(&str, String)> = TABS.iter().map(|(r, l)| (*l, href(ctx, host, r))).collect();
    let current = TABS.iter().position(|(r, _)| *r == tab).unwrap_or(0);
    qsweb::partials::tabs(&list, current)
}

/// A tab's page: `Virtual resources: <host>`, its tab in the crumb (as
/// the design has it, Virtual Ethernet's crumb ends at the resources).
pub fn page(ctx: &Ctx, host: &str, tab: &str, messages: Vec<Message>, body: &str) -> Page {
    let h1 = format!("Virtual resources: {host}");
    let mut crumb = vec![
        "Systems Management".into(),
        "Servers".into(),
        host.to_string(),
        "Virtual resources".into(),
    ];
    if tab != "ethernet"
        && let Some((_, label)) = TABS.iter().find(|(r, _)| *r == tab)
    {
        crumb.push((*label).into());
    }
    Page {
        title: h1.clone(),
        crumb,
        h1,
        messages,
        body: format!(
            "{}{}{body}",
            crate::systems::props::tabs(ctx, host, crate::systems::props::VIRTUAL.0, true),
            tabs(ctx, host, tab)
        ),
        help: "virtual-resources".into(),
        nav: Some(("Systems Management", Some("/servers"))),
        ..Page::default()
    }
}

/// A form page under a tab: its title, the crumb through the tab.
pub fn form_page(host: &str, tab: &str, title: &str, messages: Vec<Message>, body: String) -> Page {
    let mut crumb = vec![
        "Systems Management".into(),
        "Servers".into(),
        host.to_string(),
        "Virtual resources".into(),
    ];
    if let Some((_, label)) = TABS.iter().find(|(r, _)| *r == tab) {
        crumb.push((*label).into());
    }
    crumb.push(title.into());
    Page {
        title: title.into(),
        crumb,
        h1: title.into(),
        messages,
        body,
        help: "virtual-resources".into(),
        nav: Some(("Systems Management", Some("/servers"))),
        ..Page::default()
    }
}

/// Bytes as the design's `GB`: whole gigabytes, 1,000,000,000 bytes,
/// with the console's number format.
pub fn gb(rq: &Rq<'_>, bytes: u64) -> String {
    crate::fmt::number(bytes / 1_000_000_000, &rq.app.settings().number_format)
}

/// Bytes as the design's `MB`.
pub fn mb(rq: &Rq<'_>, bytes: u64) -> String {
    crate::fmt::number(bytes / 1_000_000, &rq.app.settings().number_format)
}

/// `/server/<host>/virtual/<rest>`: the tab or form `rest` names.
pub fn serve(rq: &Rq<'_>, host: &str, rest: &str) -> Response {
    let mut parts = rest.split('/');
    let tab = parts.next().filter(|t| !t.is_empty()).unwrap_or("ethernet");
    let sub = parts.next().unwrap_or("");
    let Some(k) = crate::systems::system(rq, host) else {
        return crate::app::routes::error(rq, 404, &qmcmsg::NO_SUCH_SYSTEM, &[host]);
    };
    match (tab, sub) {
        ("ethernet", "") => ethernet::route(rq, &k),
        ("ethernet", "switch") => forms::switch(rq, &k),
        ("storage", "") => storage::route(rq, &k),
        ("storage", "create") => vdisk::create(rq, &k),
        ("storage", "resize") => vdisk::resize(rq, &k),
        ("storage", "pool") => forms::pool(rq, &k),
        ("optical", "") => optical::route(rq, &k),
        ("optical", "load") => optical::load(rq, &k),
        ("tape", "") => tape::route(rq, &k),
        ("tape", "volume") => forms::volume(rq, &k),
        ("pools", "") => pools::route(rq, &k),
        ("pools", "change") => forms::proc_pool(rq, &k),
        _ => crate::app::routes::error(rq, 404, &qmcmsg::NOT_FOUND, &[&rq.req.path]),
    }
}

/// Processing units typed as `2.00` or `2`: hundredths of a core.
pub fn parse_units(text: &str) -> Option<u32> {
    let t = text.trim();
    let (whole, part) = t.split_once('.').unwrap_or((t, ""));
    if part.len() > 2 || (whole.is_empty() && part.is_empty()) {
        return None;
    }
    let w: u32 = if whole.is_empty() {
        0
    } else {
        whole.parse().ok()?
    };
    let p: u32 = if part.is_empty() {
        0
    } else {
        format!("{part:0<2}").parse().ok()?
    };
    w.checked_mul(100)?.checked_add(p)
}

/// Whether `k` is a host whose hello offers virtual resources: the
/// message for one that does not.
pub fn lacking(k: &mgmtproto::Kept, bits: u64) -> Option<Message> {
    mgmtproto::ask::lacks(k.caps, bits)
        .map(|what| qmcmsg::message(qsweb::msg::Kind::Error, &qmcmsg::LACKS, &[&k.name, what]))
}

/// Ask the host `req` for `action`; on success `done` is shown on the
/// tab `back`, otherwise the messages are.
pub fn act(
    rq: &Rq<'_>,
    k: &mgmtproto::Kept,
    action: &str,
    req: &qsi::any::AnyRequest,
) -> Result<qsi::any::AnyReply, Vec<Message>> {
    if let Some(m) = lacking(k, qsi::caps::of_request(req.family(), req.op())) {
        rq.audit(action, "refused: not offered");
        return Err(vec![m]);
    }
    let r = crate::systems::ask(rq, &k.name, action, req);
    rq.audit(action, if r.is_ok() { "done" } else { "refused" });
    r
}
