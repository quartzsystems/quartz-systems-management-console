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
pub mod hw;
pub mod power;
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

/// The console's message `def` as a refusal, for what the web server
/// refuses on a managed system's behalf.
pub fn own(def: &qmcmsg::Def, data: &[&str]) -> Refusal {
    let m = qmcmsg::message(Kind::Error, def, data);
    Refusal::new(def.id, &m.text)
}

/// Send `req` to `name` for the signed-on user through `mgmtd`: its
/// reply, or the messages to show (the console's first, `action`
/// naming what was asked, then the system's own).
pub fn ask(
    rq: &Rq<'_>,
    name: &str,
    action: &str,
    req: &qsi::any::AnyRequest,
) -> Result<qsi::any::AnyReply, Vec<Message>> {
    let asked = mgmtproto::Asked::of(req);
    match rq
        .app
        .console
        .mgmt()
        .ask(name.to_string(), asked, rq.user().into())
    {
        Ok(Ok(a)) => a
            .reply()
            .ok_or_else(|| vec![qmcmsg::message(Kind::Error, &qmcmsg::NO_REPLY, &[name])]),
        Ok(Err(no)) => {
            let mut ms = Vec::new();
            // A refusal of the console's own is shown alone.
            if !no.main.msgid.starts_with("QMC") {
                ms.push(qmcmsg::message(
                    Kind::Error,
                    &qmcmsg::POWER_REFUSED,
                    &[action, name],
                ));
            }
            ms.extend(refused(&no));
            Err(ms)
        }
        Err(e) => Err(vec![unreached(&e)]),
    }
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
