// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! The power confirmations (`confirm-poweroff.html`,
//! `confirm-poweron.html`): Power off and Restart of a host list every
//! partition they end and act only on the page's POST with its token,
//! through QSI's `power`; the host shuts its partitions down first
//! unless told to end them at once. Power on is drawn as the design has
//! it, and answers that this release cannot: a powered-off machine does
//! not answer QSI, and the console has no way to its BMC.

use qsweb::frame::{Ctx, Page};
use qsweb::html::esc;
use qsweb::http::Response;
use qsweb::msg::Kind;
use qsweb::partials::{Cell, Col, List, list_table, status};

use mgmtproto::kept::Kind as SystemKind;
use qsi::host::model::PartitionState;

use super::{refused, system, unreached};
use crate::app::routes::Rq;

/// The delay a host gives its partitions to shut down, in seconds.
pub const DELAY: u32 = 300;

/// A partition a power off ends.
#[derive(Clone, Debug, Default)]
pub struct Ended {
    /// Its name.
    pub name: String,
    /// Its ID.
    pub id: u32,
    /// `Running`, `Not activated`, ...
    pub state: String,
    /// Its current code.
    pub code: String,
    /// What the power off does to it.
    pub action: String,
}

/// Power off or Restart.
#[derive(Clone, Debug, Default)]
pub struct Off {
    /// The managed system.
    pub name: String,
    /// Restart rather than power off.
    pub restart: bool,
    /// The partitions.
    pub partitions: Vec<Ended>,
}

/// Power on.
#[derive(Clone, Debug, Default)]
pub struct On {
    /// The managed system.
    pub name: String,
    /// `QS/VS host`.
    pub kind: String,
    /// `Power off`, ...
    pub state: String,
    /// The partitions it starts.
    pub auto: String,
}

fn crumb(last: &str) -> Vec<String> {
    vec!["Systems Management".into(), "Servers".into(), last.into()]
}

fn ok_cancel(ctx: &Ctx, name: &str, inner: &str) -> String {
    format!(
        "<form method=\"post\">{}<input type=\"hidden\" name=\"ms\" value=\"{}\">{inner}<div class=\"buttons\"><input type=\"submit\" name=\"ok\" value=\"OK\"><input type=\"submit\" name=\"cancel\" value=\"Cancel\"></div></form>",
        ctx.csrf_field(),
        esc(name)
    )
}

/// Power off or Restart's confirmation.
pub fn off_page(ctx: &Ctx, v: &Off) -> Page {
    let rows = v
        .partitions
        .iter()
        .map(|p| {
            let good = Some(p.state == "Running");
            vec![
                Cell::html(format!("<span class=\"mono\">{}</span>", esc(&p.name))),
                Cell::text(&p.id.to_string()),
                Cell::html(status(&p.state, good)),
                Cell::html(format!(
                    "<a class=\"mono\" href=\"{}\">{}</a>",
                    ctx.url(&format!(
                        "/refcode/{}?ms={}&p={}",
                        esc(&p.code),
                        esc(&v.name),
                        p.id
                    )),
                    esc(&p.code)
                )),
                Cell::text(&p.action),
            ]
        })
        .collect();
    let list = List {
        cols: ["Partition", "ID", "Status", "Reference code", "Action"]
            .iter()
            .map(|c| Col::plain(c))
            .collect(),
        rows,
        empty: "The system has no partitions.".into(),
        keep: Vec::new(),
    };
    let (title, verb, how) = if v.restart {
        ("Confirm Restart", "Restarting", "Restart")
    } else {
        ("Confirm Power Off", "Powering off", "Power off")
    };
    let choice = format!(
        "<p class=\"small grey\">{how}: <input type=\"radio\" name=\"how\" value=\"delayed\" checked> Shut down partitions first (PWRDWNSYS *CNTRLD through the service interface)<br><input type=\"radio\" name=\"how\" value=\"imm\"> Immediate - partitions are ended without shutdown</p>"
    );
    let body = format!(
        "<p>{verb} managed system {} ends every partition listed below. Press OK to continue.</p>{}{}",
        esc(&v.name),
        list_table(&list),
        ok_cancel(ctx, &v.name, &choice)
    );
    Page {
        title: title.into(),
        crumb: crumb(title),
        h1: title.into(),
        body,
        help: "power".into(),
        nav: Some(("Systems Management", Some("/servers"))),
        ..Page::default()
    }
}

/// Power on's confirmation.
pub fn on_page(ctx: &Ctx, v: &On) -> Page {
    let list = List {
        cols: [
            "Managed system",
            "Type",
            "Status",
            "Partitions to auto-start",
        ]
        .iter()
        .map(|c| Col::plain(c))
        .collect(),
        rows: vec![vec![
            Cell::html(format!("<span class=\"mono\">{}</span>", esc(&v.name))),
            Cell::text(&v.kind),
            Cell::html(status(&v.state, Some(v.state == "Operating"))),
            Cell::html(format!("<span class=\"mono\">{}</span>", esc(&v.auto))),
        ]],
        empty: String::new(),
        keep: Vec::new(),
    };
    let choice = "<p class=\"small grey\">IPL side: <select name=\"side\"><option>A (permanent)</option><option>B (temporary)</option></select> &nbsp; Keylock: <select name=\"key\"><option>Normal</option><option>Manual</option></select> &nbsp; Boot mode: <select name=\"boot\"><option>Normal</option><option>Service - partition standby</option></select></p>";
    let body = format!(
        "<p>Press OK to power on managed system {} with the parameters below.</p>{}{}",
        esc(&v.name),
        list_table(&list),
        ok_cancel(ctx, &v.name, choice)
    );
    Page {
        title: "Confirm Power On".into(),
        crumb: crumb("Confirm Power On"),
        h1: "Confirm Power On".into(),
        body,
        help: "power".into(),
        nav: Some(("Systems Management", Some("/servers"))),
        ..Page::default()
    }
}

fn state_word(s: PartitionState) -> &'static str {
    match s {
        PartitionState::Running => "Running",
        PartitionState::Starting => "Starting",
        PartitionState::ShuttingDown => "Shutting down",
        PartitionState::Error => "Error",
        _ => "Not activated",
    }
}

/// `GET`, `POST /server/<name>/confirm?op=poweron|poweroff|restart`.
#[allow(
    clippy::too_many_lines,
    reason = "the confirmation and its POST, top to bottom"
)]
pub fn route(rq: &Rq<'_>, name: &str) -> Response {
    let Some(k) = system(rq, name) else {
        return crate::app::routes::error(rq, 404, &qmcmsg::NO_SUCH_SYSTEM, &[name]);
    };
    let op = rq.q("op");
    let f = rq.req.form();
    if rq.post() {
        if !f.has("ok") {
            return Response::redirect(&rq.ctx.url("/servers"));
        }
        if op == "poweron" {
            rq.audit(
                &format!("power on {}", k.name),
                "refused: not in this release",
            );
            return rq.done(
                "/servers",
                qmcmsg::message(Kind::Error, &qmcmsg::POWER_ON_NOT_HERE, &[&k.name]),
            );
        }
        let restart = op == "restart";
        let immediate = f.str("how") == "imm";
        let what = format!(
            "{} {}{}",
            if restart { "restart" } else { "power off" },
            k.name,
            if immediate {
                ", immediate"
            } else {
                ", partitions shut down first"
            }
        );
        let done = rq.app.console.mgmt().power(
            k.name.clone(),
            restart,
            immediate,
            DELAY,
            rq.user().into(),
        );
        let verb = if restart { "Restart" } else { "Power off" };
        return match done {
            Ok(Ok(())) => {
                rq.audit(&what, "started");
                let def = if restart {
                    &qmcmsg::RESTART_STARTED_SYSTEM
                } else {
                    &qmcmsg::POWER_OFF_STARTED
                };
                rq.done("/servers", qmcmsg::message(Kind::Done, def, &[&k.name]))
            }
            Ok(Err(no)) => {
                rq.audit(
                    &what,
                    &format!("refused: {} {}", no.main.msgid, no.main.text),
                );
                let mut ms = vec![qmcmsg::message(
                    Kind::Error,
                    &qmcmsg::POWER_REFUSED,
                    &[verb, &k.name],
                )];
                ms.extend(refused(&no));
                rq.done_all("/servers", ms)
            }
            Err(e) => rq.done("/servers", unreached(&e)),
        };
    }
    if op == "poweron" {
        let v = On {
            name: k.name.clone(),
            kind: k.kind.words().into(),
            state: k.status().into(),
            auto: "set at the system".into(),
        };
        return rq.page(on_page(&rq.ctx, &v));
    }
    let delayed = format!("Shut down (delayed, {DELAY} s) then power off");
    let partitions = if k.kind == SystemKind::Host {
        k.partitions
            .iter()
            .map(|p| Ended {
                name: p.name.clone(),
                id: p.id,
                state: state_word(p.state).into(),
                code: if p.code.is_empty() {
                    "00000000".into()
                } else {
                    p.code.clone()
                },
                action: if p.state == PartitionState::NotActivated {
                    "None".into()
                } else {
                    delayed.clone()
                },
            })
            .collect()
    } else {
        Vec::new()
    };
    rq.page(off_page(
        &rq.ctx,
        &Off {
            name: k.name.clone(),
            restart: op == "restart",
            partitions,
        },
    ))
}
