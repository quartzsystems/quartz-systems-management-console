// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! The managed systems' pages: Connectivity and Add managed system,
//! Servers, a system's properties, the power confirmations. Every value
//! comes from what the managed-systems service keeps (`mgmtproto`); a
//! list never waits on a managed system, and an action's refusal shows
//! the managed system's own messages.

pub mod add;
pub mod connectivity;
pub mod props;
pub mod servers;

use mgmtproto::{Kept, Refusal};
use qsweb::msg::{Kind, Message};

use crate::app::routes::Rq;

/// What the service keeps of every system, by name; none when it does
/// not answer.
pub fn systems(rq: &Rq<'_>) -> Result<Vec<Kept>, Message> {
    rq.app.console.mgmt().systems().map_err(|e| unreached(&e))
}

/// One system as kept.
pub fn system(rq: &Rq<'_>, name: &str) -> Option<Kept> {
    rq.app
        .console
        .mgmt()
        .system(name.to_string())
        .ok()
        .flatten()
}

/// The service did not answer.
pub fn unreached(e: &ipc::WireError) -> Message {
    qmcmsg::message(Kind::Error, &qmcmsg::MGMT_DOWN, &[&e.to_string()])
}

/// A refusal's messages, the managed system's as it sent them.
pub fn refused(r: &Refusal) -> Vec<Message> {
    r.notes()
        .into_iter()
        .map(|n| Message::new(Kind::Error, &n.msgid, &n.text))
        .collect()
}

/// `secs` as the console shows a date and time.
pub fn when(rq: &Rq<'_>, secs: u64) -> String {
    if secs == 0 {
        return "never".into();
    }
    let s = rq.app.settings();
    let zone = rq.app.host().zone();
    let t = i64::try_from(secs).unwrap_or(0);
    format!(
        "{} {}",
        crate::fmt::date(t, zone, &s.date_format),
        crate::fmt::time(t, zone, &s.time_format, true)
    )
}

/// The console's interface `local` (the address a session leaves from)
/// is on: `private` or `open`, as the network service has them.
pub fn via(rq: &Rq<'_>, local: &str) -> String {
    if local.is_empty() {
        return String::new();
    }
    let ifs = rq.app.console.interfaces();
    let on = ifs
        .iter()
        .find(|i| i.address.split('/').next() == Some(local));
    let role = on.map_or("open", |i| i.role.as_str());
    if role == "private" {
        "private"
    } else {
        "public"
    }
    .to_string()
}
