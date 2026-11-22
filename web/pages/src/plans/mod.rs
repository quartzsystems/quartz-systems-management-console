// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! System plans (`docs/PLANS.md`): steps 21 and 22 of the prototype's
//! build order. The scheduler owns `/data/plans/`; every page here asks
//! it on its socket, and holds nothing of a plan itself.
//!
//! - [`list`]: System plans: the list, Create plan, Import, Export,
//!   Delete with its confirmation.
//! - [`view`]: View system plan: the managed system and each
//!   partition's profiles.
//! - [`edit`]: a plan's description, Save as template (built from the
//!   shared patterns).
//! - [`deploy`]: Deploy system plan: the target, the validation, the
//!   partitions, the confirmation and the progress.

pub mod deploy;
pub mod edit;
pub mod list;
pub mod view;

use qsweb::http::Response;
use qsweb::msg::{Kind, Message};

use crate::app::routes::Rq;

/// The navigation group of every plan page.
pub const NAV: &str = "System Plans";

/// A refusal of the scheduler as a message row.
pub fn issued(kind: Kind, i: &msg::Issued) -> Message {
    Message::new(kind, &i.id, &i.text)
}

/// The scheduler not answering, as a message row.
pub fn down(e: &ipc::WireError) -> Message {
    qmcmsg::message(Kind::Error, &qmcmsg::SCHEDD_DOWN, &[&e.to_string()])
}

/// `10/02/26 10:12` in the console's zone and formats.
pub fn when(rq: &Rq<'_>, secs: u64) -> String {
    let s = rq.app.settings();
    let zone = rq.app.host().zone();
    let t = i64::try_from(secs).unwrap_or(0);
    format!(
        "{} {}",
        crate::fmt::date(t, zone, &s.date_format),
        crate::fmt::time(t, zone, &s.time_format, false)
    )
}

/// `07:12:03` in the console's zone.
pub fn clock(rq: &Rq<'_>, secs: u64) -> String {
    if secs == 0 {
        return String::new();
    }
    let s = rq.app.settings();
    let zone = rq.app.host().zone();
    crate::fmt::time(i64::try_from(secs).unwrap_or(0), zone, &s.time_format, true)
}

/// `/plan/<name>` of `name`, its name URL-encoded.
pub fn route_of(name: &str, rest: &str) -> String {
    format!("/plan/{}{rest}", qsweb::http::form::encode(name))
}

/// A plan's routes: `/plan/<name>[/export|/edit|/deploy/step/<n>|/deploy/progress]`.
pub fn serve(rq: &Rq<'_>, rest: &str) -> Option<Response> {
    let mut parts = rest.splitn(2, '/');
    let name = parts.next().filter(|n| plan::doc::check::plan_name_ok(n))?;
    Some(match parts.next().unwrap_or("") {
        "" => view::route(rq, name),
        "export" => list::export(rq, name),
        "edit" => edit::route(rq, name),
        "deploy/progress" => deploy::progress(rq, name),
        r => {
            let n = r.strip_prefix("deploy/step/")?.parse().ok()?;
            deploy::step(rq, name, n)
        }
    })
}
