// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! Shut down (`shutdown.html`) and its confirmation
//! (`confirm-shutdown.html`): delayed with its delay, through the
//! operating system, or immediate; restarted after or not. Restart from
//! the action list is the same page with Restart chosen. Nothing is
//! done by GET: the confirmation POSTs back with its token.

use qsweb::frame::{Ctx, Page};
use qsweb::http::{Form, Response};
use qsweb::msg::{Kind, Message};
use qsweb::partials::{Cell, Col, List, hidden, list_table, mono, status};

use mgmtproto::Kept;
use qsi::any::AnyRequest;
use qsi::host::partition::{self, ShutdownKind};
use webusers::Task;

use super::list::{good, state_word};
use crate::app::routes::{Rq, lacks};

/// The form.
#[derive(Clone, Debug, Default)]
pub struct Shutdown {
    /// The host.
    pub host: String,
    /// The partition.
    pub name: String,
    /// Its ID.
    pub id: u32,
    /// `Running`.
    pub state: String,
    /// The delay offered.
    pub delay: u32,
    /// Restart chosen.
    pub restart: bool,
    /// The messages.
    pub messages: Vec<Message>,
}

/// The page.
pub fn page(ctx: &Ctx, v: &Shutdown) -> Page {
    let restart = if v.restart { " checked" } else { "" };
    let body = format!(
        "<form method=\"post\">{}<table class=\"props\"><tr><th>Partition</th><td>{} (ID {}) on {}, {}</td></tr><tr><th>How to shut down</th><td><input type=\"radio\" name=\"how\" value=\"delayed\" checked> Delayed - <span class=\"mono\">PWRDWNSYS *CNTRLD</span> with delay <input type=\"text\" name=\"delay\" value=\"{}\" size=\"5\"> seconds<br><input type=\"radio\" name=\"how\" value=\"os\"> Operating system - send <span class=\"mono\">PWRDWNSYS OPTION(*IMMED)</span> through the service interface<br><input type=\"radio\" name=\"how\" value=\"imm\"> Immediate - end the partition without shutting down the operating system (like removing power)</td></tr><tr><th>Restart after shutdown</th><td><input type=\"checkbox\" name=\"restart\"{restart}> Yes (Restart)</td></tr></table><div class=\"buttons\"><input type=\"submit\" name=\"ok\" value=\"OK\"><input type=\"submit\" name=\"cancel\" value=\"Cancel\"></div></form>",
        ctx.csrf_field(),
        mono(&v.name),
        v.id,
        mono(&v.host),
        status(&v.state, good(&v.state)),
        v.delay,
    );
    super::sub_page(
        &v.name,
        "Shut down",
        &format!("Shut down partition: {}", v.name),
        v.messages.clone(),
        body,
    )
}

/// What the confirmation lists.
#[derive(Clone, Debug, Default)]
pub struct Confirm {
    /// The host.
    pub host: String,
    /// The partition.
    pub name: String,
    /// Its ID.
    pub id: u32,
    /// `Running`.
    pub state: String,
    /// `Delayed, 300 s`.
    pub method: String,
    /// Its active jobs, as its system says; empty when it does not.
    pub jobs: String,
    /// Its users signed on, likewise.
    pub users: String,
    /// The choices, carried.
    pub carried: String,
}

/// The confirmation page.
pub fn confirm_page(ctx: &Ctx, v: &Confirm) -> Page {
    let list = List {
        cols: [
            "Partition",
            "ID",
            "Managed system",
            "Status",
            "Method",
            "Active jobs",
            "Users signed on",
        ]
        .iter()
        .map(|c| Col::plain(c))
        .collect(),
        rows: vec![vec![
            Cell::html(mono(&v.name)),
            Cell::text(&v.id.to_string()),
            Cell::html(mono(&v.host)),
            Cell::html(status(&v.state, good(&v.state))),
            Cell::text(&v.method),
            Cell::text(&v.jobs),
            Cell::text(&v.users),
        ]],
        empty: String::new(),
        keep: Vec::new(),
    };
    let body = format!(
        "<p>Press OK to shut down the partition below. Jobs on the partition are ended by the operating system.</p>{}<form method=\"post\">{}{}<div class=\"buttons\"><input type=\"submit\" name=\"ok\" value=\"OK\"><input type=\"submit\" name=\"cancel\" value=\"Cancel\"></div></form>",
        list_table(&list),
        ctx.csrf_field(),
        v.carried,
    );
    super::sub_page(
        &v.name,
        "Confirm Shut Down",
        "Confirm Shut Down Partition",
        Vec::new(),
        body,
    )
}

/// The choice of a form: kind, delay, restart.
fn choice(f: &Form) -> (ShutdownKind, u32, bool) {
    let kind = match f.str("how") {
        "os" => ShutdownKind::System,
        "imm" => ShutdownKind::Immediate,
        _ => ShutdownKind::Delayed,
    };
    let delay = f.str("delay").trim().parse().unwrap_or(300).min(99_999);
    (kind, delay, f.has("restart"))
}

/// A method in the confirmation's words.
pub fn method(kind: ShutdownKind, delay: u32, restart: bool) -> String {
    let m = match kind {
        ShutdownKind::System => "Operating system".to_string(),
        ShutdownKind::Immediate => "Immediate".to_string(),
        _ => format!("Delayed, {delay} s"),
    };
    if restart {
        format!("{m}, then restart")
    } else {
        m
    }
}

/// Shut partition `id` down as `kind`: the messages to show.
pub fn shut_down(
    rq: &Rq<'_>,
    k: &Kept,
    id: u32,
    kind: ShutdownKind,
    delay: u32,
    restart: bool,
) -> Result<Message, Vec<Message>> {
    let name = super::name_of(k, id);
    let req = AnyRequest::Partition(partition::Request::ShutDown(partition::ShutDown {
        id,
        kind,
        delay,
        restart,
    }));
    let what = format!(
        "shut down {name} on {}: {}",
        k.name,
        method(kind, delay, restart)
    );
    match crate::systems::ask(rq, &k.name, "Shut down", &req) {
        Ok(_) => {
            rq.audit(&what, "accepted");
            Ok(qmcmsg::message(
                Kind::Done,
                if restart {
                    &qmcmsg::RESTARTING
                } else {
                    &qmcmsg::SHUTTING_DOWN
                },
                &[&name],
            ))
        }
        Err(ms) => {
            rq.audit(&what, "refused");
            Err(ms)
        }
    }
}

/// `GET`, `POST /partition/<host>/<id>/shutdown?restart=`: the form,
/// its confirmation by GET, the request by the confirmation's POST.
pub fn route(rq: &Rq<'_>, k: &Kept, id: u32) -> Response {
    let f = rq.req.form();
    let back = "/partitions";
    let Some(st) = k.part(id) else {
        return crate::app::routes::error(rq, 404, &qmcmsg::NOT_FOUND, &[&rq.req.path]);
    };
    if rq.post() && f.has("cancel") {
        return Response::redirect(&rq.ctx.url(back));
    }
    if rq.post() {
        if let Some(no) = lacks(rq, Task::Activate) {
            return no;
        }
        if f.has("confirmed") {
            let (kind, delay, restart) = choice(&f);
            return match shut_down(rq, k, id, kind, delay, restart) {
                Ok(m) => rq.done(back, m),
                Err(ms) => rq.done_all(back, ms),
            };
        }
        let mut q = format!(
            "how={}&delay={}",
            qsweb::http::form::encode(f.str("how")),
            qsweb::http::form::encode(f.str("delay"))
        );
        if f.has("restart") {
            q.push_str("&restart=on");
        }
        return Response::redirect(&rq.ctx.url(&format!(
            "/partition/{}/{id}/shutdown?confirm=1&{q}",
            k.name
        )));
    }
    if rq.q("confirm") == "1" {
        let qf = rq.req.query();
        let (kind, delay, restart) = choice(&qf);
        let mut carried = hidden("how", qf.str("how")) + &hidden("delay", &delay.to_string());
        if restart {
            carried.push_str(&hidden("restart", "on"));
        }
        carried.push_str(&hidden("confirmed", "1"));
        let v = Confirm {
            host: k.name.clone(),
            name: st.name.clone(),
            id,
            state: state_word(st.state).into(),
            method: method(kind, delay, restart),
            jobs: String::new(),
            users: String::new(),
            carried,
        };
        return rq.page(confirm_page(&rq.ctx, &v));
    }
    let delay = k.power_on.map_or(300, |p| p.delay).clamp(1, 99_999);
    let v = Shutdown {
        host: k.name.clone(),
        name: st.name.clone(),
        id,
        state: state_word(st.state).into(),
        delay,
        restart: rq.q("restart") == "1",
        messages: Vec::new(),
    };
    rq.page(page(&rq.ctx, &v))
}
