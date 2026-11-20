// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! The terminal window and the control panel (`docs/TERMINAL.md`): a
//! partition's console shared through the managed-systems service and
//! drawn by QS/400's look; the page's script is QS/400's, the only
//! script the console serves, allowed by this page's content security
//! policy and no other's.

pub mod config;
pub mod panel;
pub mod relay;
#[cfg(unix)]
pub mod socket;
pub mod window;

use qsweb::frame::render;
use qsweb::html::esc;
use qsweb::http::Response;
use qsweb::msg::{Kind, Message};
use termlook::exchange::keys;
use termlook::look::render::Scheme;
use termlook::screen::grid::View;
use webusers::Task;

use mgmtproto::console::Watch;
use mgmtproto::{Kept, Refusal};
use qsi::host::console::In;
use qsipart::console::{FromSystem, Grid};

use crate::app::routes::{Rq, lacks};
use crate::refcodes::explain;
use crate::systems::{refused, system, unreached};

/// The terminal window's content security policy: the page, its
/// stylesheet and picture, QS/400's script from the console and its
/// socket, and the style attributes the window is drawn with.
pub const CSP: &str = "default-src 'none'; script-src 'self'; connect-src 'self'; style-src 'self'; style-src-attr 'unsafe-inline'; img-src 'self'; form-action 'self'; frame-ancestors 'none'; base-uri 'none'";

/// The options a window keeps in its address.
#[derive(Clone, Debug)]
struct Options {
    scale: u32,
    strip: bool,
    take_over: bool,
    bw: bool,
}

impl Options {
    fn of(rq: &Rq<'_>) -> Self {
        Self {
            scale: if rq.q("scale") == "2" { 2 } else { 1 },
            strip: rq.q("strip") != "0",
            take_over: rq.q("take") == "1",
            bw: rq.q("c") == "w",
        }
    }

    fn query(&self) -> String {
        let mut q = String::new();
        if self.scale == 2 {
            q.push_str("&scale=2");
        }
        if !self.strip {
            q.push_str("&strip=0");
        }
        if self.bw {
            q.push_str("&c=w");
        }
        q
    }
}

/// Whether the signed-on user may type: the Open terminal window task.
fn may_type(rq: &Rq<'_>) -> bool {
    rq.app.users().matrix.allows(rq.role, Task::Terminal)
}

fn partition(k: &Kept, id: u32) -> Option<&qsi::host::partitions::Status> {
    k.partitions.iter().find(|p| p.id == id)
}

fn meaning(rq: &Rq<'_>, code: &str, text: &str) -> String {
    explain::explain(rq.app.catalogue(), code).map_or_else(|| text.to_string(), |e| e.title)
}

fn keep_windows(rq: &Rq<'_>) -> bool {
    rq.app.settings().keep_terminals
}

fn watch(rq: &Rq<'_>, host: &str, id: u32, take_over: bool) -> Watch {
    Watch {
        system: host.into(),
        id,
        user: rq.user().into(),
        take_over,
        typing: may_type(rq),
    }
}

/// The window's page as an answer: without the navigation column, with
/// this page's policy.
fn answer(rq: &Rq<'_>, p: &qsweb::frame::Page) -> Response {
    let mut p = p.clone();
    p.generic = false;
    let more = rq
        .app
        .more
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .remove(&rq.session.id)
        .unwrap_or_default();
    for m in more.into_iter().rev() {
        p.messages.insert(0, m);
    }
    if let Some(m) = rq.app.sessions.take_flash(&rq.session.id) {
        p.messages.insert(0, m);
    }
    Response::html(200, window::without_nav(&render(&rq.ctx, &p)))
        .with("Content-Security-Policy", CSP)
}

/// The window, attached or not.
fn show(rq: &Rq<'_>, k: &Kept, id: u32, o: &Options, mut messages: Vec<Message>) -> Response {
    let Some(p) = partition(k, id) else {
        return crate::app::routes::error(rq, 404, &qmcmsg::NOT_FOUND, &[&rq.req.path]);
    };
    let held = rq.app.windows.find(&rq.session.id, &k.name, id);
    let shown = held.as_ref().map(|h| h.now());
    let connected = held.as_ref().is_some_and(|h| h.open());
    if let Some(s) = &shown
        && let Some(why) = &s.ended
        && messages.is_empty()
    {
        messages.push(qmcmsg::message(
            Kind::Error,
            &qmcmsg::CONSOLE_CLOSED,
            &[&p.name, why],
        ));
    }
    let grid = shown.as_ref().map(|s| s.grid.clone()).unwrap_or_default();
    let (rows, cols) = if grid.ready() {
        (usize::from(grid.rows), usize::from(grid.cols))
    } else {
        (24, 80)
    };
    let code = if p.code.is_empty() {
        explain::NONE.to_string()
    } else {
        p.code.clone()
    };
    let v = window::Window {
        host: k.name.clone(),
        id,
        partition: p.name.clone(),
        connected,
        meaning: meaning(rq, &code, &p.text),
        code,
        strip: o.strip,
        scale: o.scale,
        size: termlook::screen::size(rows, cols, o.scale as usize),
        cursor: (
            usize::from(grid.status.row) + 1,
            usize::from(grid.status.col) + 1,
        ),
        may_type: may_type(rq),
        keep: o.query(),
        script: true,
        bw: o.bw,
        messages,
    };
    answer(rq, &window::page(&rq.ctx, &v))
}

/// Open the window this session holds on the partition.
#[cfg(unix)]
fn attach(rq: &Rq<'_>, host: &str, id: u32, take_over: bool) -> Result<(), Refusal> {
    let Some(path) = rq.app.console.consoles() else {
        return Err(crate::systems::own(
            &qmcmsg::MGMT_DOWN,
            &["no console socket"],
        ));
    };
    rq.app
        .windows
        .open(&path, &rq.session.id, &watch(rq, host, id, take_over))
        .map(|_| ())
}

#[cfg(not(unix))]
fn attach(_rq: &Rq<'_>, _host: &str, _id: u32, _take_over: bool) -> Result<(), Refusal> {
    Err(crate::systems::own(
        &qmcmsg::MGMT_DOWN,
        &["not on this platform"],
    ))
}

/// `GET`, `POST /terminal/<host>/<id>`.
fn window_route(rq: &Rq<'_>, host: &str, id: u32) -> Response {
    let Some(k) = system(rq, host) else {
        return crate::app::routes::error(rq, 404, &qmcmsg::NO_SUCH_SYSTEM, &[host]);
    };
    let o = Options::of(rq);
    let back = format!("/terminal/{}/{id}?x=1{}", k.name, o.query());
    if rq.post() {
        let f = rq.req.form();
        if let Some(label) = f.get("k") {
            if let Some(no) = lacks(rq, Task::Terminal) {
                return no;
            }
            if let (Some(h), Some(key)) = (
                rq.app.windows.find(&rq.session.id, &k.name, id),
                keys::keypad(label).and_then(keys::console_key),
            ) {
                h.send(In::Key(key));
                // Let the system's answer come before the picture is read.
                std::thread::sleep(std::time::Duration::from_millis(150));
            }
            return Response::redirect(&rq.ctx.url(&back));
        }
        if f.has("f") || f.has("fn") {
            return function(rq, &k, id, &f, &back);
        }
        match f.str("tb") {
            "Disconnect" => {
                rq.app.windows.close(&rq.session.id, &k.name, id);
                rq.audit(
                    &format!("disconnect console of {} partition {id}", k.name),
                    "closed",
                );
                let m = qmcmsg::message(Kind::Info, &qmcmsg::CONSOLE_DISCONNECTED, &[&k.name]);
                return show(rq, &k, id, &o, vec![m]);
            }
            "Print Screen" => {
                return Response::redirect(
                    &rq.ctx.url(&format!("/terminal/{}/{id}/print", k.name)),
                );
            }
            "Connect" => {}
            other if !other.is_empty() => {
                let m = qmcmsg::message(Kind::Info, &qmcmsg::NOT_IN_V1, &[other]);
                return rq.done(&back, m);
            }
            _ => {}
        }
    }
    let connect = rq.post() && rq.req.form().str("tb") == "Connect";
    if !connect && let Some(why) = rq.app.windows.ended(&rq.session.id, &k.name, id) {
        let name = partition(&k, id)
            .map(|p| p.name.clone())
            .unwrap_or_default();
        let m = qmcmsg::message(Kind::Error, &qmcmsg::CONSOLE_CLOSED, &[&name, &why]);
        return show(rq, &k, id, &o, vec![m]);
    }
    if rq.app.windows.find(&rq.session.id, &k.name, id).is_none() {
        let what = format!("open console of {} partition {id}", k.name);
        match attach(rq, &k.name, id, o.take_over) {
            Ok(()) => rq.audit(&what, "opened"),
            Err(no) => {
                rq.audit(
                    &what,
                    &format!("refused: {} {}", no.main.msgid, no.main.text),
                );
                let name = partition(&k, id)
                    .map(|p| p.name.clone())
                    .unwrap_or_default();
                let mut ms = vec![qmcmsg::message(
                    Kind::Error,
                    &qmcmsg::CONSOLE_CLOSED,
                    &[&name, &no.main.text],
                )];
                ms.extend(refused(&no));
                return show(rq, &k, id, &o, ms);
            }
        }
    }
    show(rq, &k, id, &o, Vec::new())
}

/// A control panel function from a window's strip or the control
/// panel: 08 and 22 through the confirmation page.
fn function(rq: &Rq<'_>, k: &Kept, id: u32, f: &qsweb::http::Form, back: &str) -> Response {
    let Some(n) = panel::chosen(f) else {
        let m = qmcmsg::message(
            Kind::Error,
            &qmcmsg::NOT_VALID,
            &["Choose a control panel function."],
        );
        return rq.done(back, m);
    };
    let task = if n >= 50 {
        Task::Panel50
    } else {
        Task::Terminal
    };
    if let Some(no) = lacks(rq, task) {
        return no;
    }
    if panel::CONFIRMED.contains(&n) {
        return Response::redirect(
            &rq.ctx
                .url(&format!("/confirm/panel?ms={}&p={id}&f={n:02}", k.name)),
        );
    }
    let (ms, _) = carry_out(rq, k, id, n);
    rq.done_all(back, ms)
}

/// Carry out function `n`: the messages and the result's lines.
fn carry_out(rq: &Rq<'_>, k: &Kept, id: u32, n: u8) -> (Vec<Message>, Vec<String>) {
    let name = partition(k, id).map(|p| p.name.clone()).unwrap_or_default();
    let words = |code: &str| {
        let r = rq.app.console.mgmt().history(
            mgmtproto::calls::Filter {
                system: k.name.clone(),
                partition: Some(id),
                code: code.into(),
                ..Default::default()
            },
            0,
            1,
        );
        r.ok().and_then(|p| p.rows.into_iter().next()).map_or_else(
            || ["00000000"; 8].join(" "),
            |r| {
                let mut w = r.posted.refcode.words;
                w.resize(8, "00000000".into());
                w.join(" ")
            },
        )
    };
    let done = rq
        .app
        .console
        .mgmt()
        .panel(k.name.clone(), id, n, rq.user().into());
    let what = format!("control panel function {n:02} of {} partition {id}", k.name);
    match done {
        Ok(Ok(p)) => {
            rq.audit(&what, &format!("done: {}", p.text));
            let lines = vec![
                panel::label(n),
                format!(
                    "Reference code before: {}  Words 2-9: {}",
                    p.before,
                    words(&p.before)
                ),
                format!("Reference code after:  {}  ({})", p.after, p.text),
            ];
            let m = qmcmsg::message(
                Kind::Done,
                &qmcmsg::FUNCTION_ACCEPTED,
                &[&format!("{n:02}"), &name],
            );
            (vec![m], lines)
        }
        Ok(Err(no)) => {
            rq.audit(
                &what,
                &format!("refused: {} {}", no.main.msgid, no.main.text),
            );
            let mut ms = vec![qmcmsg::message(
                Kind::Error,
                &qmcmsg::FUNCTION_REFUSED,
                &[&format!("{n:02}"), &name],
            )];
            ms.extend(refused(&no));
            (ms, Vec::new())
        }
        Err(e) => (vec![unreached(&e)], Vec::new()),
    }
}

/// `GET`, `POST /partition/<host>/<id>/panel`.
fn panel_route(rq: &Rq<'_>, host: &str, id: u32) -> Response {
    let Some(k) = system(rq, host) else {
        return crate::app::routes::error(rq, 404, &qmcmsg::NO_SUCH_SYSTEM, &[host]);
    };
    let Some(p) = partition(&k, id).cloned() else {
        return crate::app::routes::error(rq, 404, &qmcmsg::NOT_FOUND, &[&rq.req.path]);
    };
    let f = rq.req.form();
    let mut chosen = 1;
    let (mut messages, mut result) = (Vec::new(), Vec::new());
    if rq.post()
        && !f.has("refresh")
        && let Some(n) = panel::chosen(&f)
    {
        chosen = n;
        let task = if n >= 50 {
            Task::Panel50
        } else {
            Task::Terminal
        };
        if let Some(no) = lacks(rq, task) {
            return no;
        }
        if panel::CONFIRMED.contains(&n) {
            return Response::redirect(
                &rq.ctx
                    .url(&format!("/confirm/panel?ms={}&p={id}&f={n:02}", k.name)),
            );
        }
        (messages, result) = carry_out(rq, &k, id, n);
    }
    let k = system(rq, host).unwrap_or(k);
    let p = partition(&k, id).cloned().unwrap_or(p);
    let keylock = rq
        .app
        .console
        .mgmt()
        .partition(k.name.clone(), id, rq.user().into())
        .ok()
        .and_then(Result::ok)
        .map_or_else(
            || "not reported".to_string(),
            |d| format!("{:?}", d.definition.keylock),
        );
    let code = if p.code.is_empty() {
        explain::NONE.to_string()
    } else {
        p.code.clone()
    };
    let v = panel::Panel {
        host: k.name.clone(),
        id,
        partition: p.name.clone(),
        state: state_word(p.state).into(),
        keylock,
        meaning: meaning(rq, &code, &p.text),
        code,
        chosen,
        result,
        messages,
    };
    rq.page(panel::page(&rq.ctx, &v))
}

fn state_word(s: qsi::host::model::PartitionState) -> &'static str {
    use qsi::host::model::PartitionState as P;
    match s {
        P::Running => "Running",
        P::Starting => "Starting",
        P::ShuttingDown => "Shutting down",
        P::Error => "Error",
        _ => "Not activated",
    }
}

/// `/confirm/panel?ms=&p=&f=`: functions 08 and 22, confirmed.
fn confirm(rq: &Rq<'_>) -> Response {
    let f = rq.req.form();
    let get = |n: &str| {
        if rq.post() {
            f.str(n).to_string()
        } else {
            rq.q(n)
        }
    };
    let (host, id, n) = (
        get("ms"),
        get("p").parse::<u32>().unwrap_or(0),
        get("f").parse::<u8>().unwrap_or(0),
    );
    let Some(k) = system(rq, &host) else {
        return crate::app::routes::error(rq, 404, &qmcmsg::NO_SUCH_SYSTEM, &[&host]);
    };
    if !panel::CONFIRMED.contains(&n) {
        return crate::app::routes::error(rq, 404, &qmcmsg::NOT_FOUND, &[&rq.req.path]);
    }
    if let Some(no) = lacks(rq, Task::Terminal) {
        return no;
    }
    let back = format!("/partition/{}/{id}/panel", k.name);
    if rq.post() {
        if f.get("ok").is_none() {
            return Response::redirect(&rq.ctx.url(&back));
        }
        let (ms, _) = carry_out(rq, &k, id, n);
        return rq.done_all(&back, ms);
    }
    let name = partition(&k, id)
        .map(|p| p.name.clone())
        .unwrap_or_default();
    let what = if n == 8 {
        "Function 08 powers the partition off at once: its system ends without shutting down."
    } else {
        "Function 22 dumps the partition's memory, then restarts its IPL."
    };
    let table = format!(
        "<table class=\"props\" cellspacing=\"0\"><tr><th>Partition</th><td><span class=\"mono\">{}</span> (ID {id}) on <span class=\"mono\">{}</span></td></tr><tr><th>Function</th><td>{}</td></tr></table>",
        esc(&name),
        esc(&k.name),
        esc(&panel::label(n))
    );
    let options = format!(
        "<input type=\"hidden\" name=\"ms\" value=\"{}\"><input type=\"hidden\" name=\"p\" value=\"{id}\"><input type=\"hidden\" name=\"f\" value=\"{n:02}\">",
        esc(&k.name)
    );
    let body = qsweb::pages::confirm_body(
        &rq.ctx,
        &format!(
            "{what} Press OK to carry it out on the partition listed below, or Cancel to return without it."
        ),
        &table,
        &options,
    );
    let title = format!("Confirm Function {n:02}");
    rq.page(qsweb::frame::Page {
        title: title.clone(),
        crumb: vec![
            "Systems Management".into(),
            "Partitions".into(),
            name,
            title.clone(),
        ],
        h1: title,
        body,
        help: "control-panel".into(),
        nav: Some(("Systems Management", Some("/partitions"))),
        ..qsweb::frame::Page::default()
    })
}

/// The picture of `host`'s partition `id`: the window's latest whole
/// screen, or one asked of the host; greyed when `off`.
fn picture(rq: &Rq<'_>, host: &str, id: u32, off: bool) -> Response {
    let o = Options::of(rq);
    let (grid, system) = if let Some(h) = rq.app.windows.find(&rq.session.id, host, id) {
        let s = h.now();
        (s.grid, s.system)
    } else {
        let mut g = Grid::default();
        if let Ok(Ok(Some(f))) = rq
            .app
            .console
            .mgmt()
            .screen(host.into(), id, rq.user().into())
        {
            let _ = g.apply(&FromSystem::Frame(f));
        }
        let sys = system(rq, host)
            .and_then(|k| {
                partition(&k, id).map(|p| {
                    if p.channel.system.is_empty() {
                        p.name.clone()
                    } else {
                        p.channel.system.clone()
                    }
                })
            })
            .unwrap_or_default();
        (g, sys)
    };
    let view = View {
        grid,
        system,
        connected: !off,
        tls: true,
        scheme: if o.bw {
            Scheme::BlackOnWhite
        } else {
            Scheme::Console
        },
    };
    if rq.req.path.ends_with("/print") {
        let title = format!("{} on {host}", view.system);
        let pdf = view
            .shown()
            .print_screen(&title, "QS/MC Management Console");
        rq.audit(&format!("print screen of {host} partition {id}"), "printed");
        return Response::new(200, "application/pdf", pdf).with(
            "Content-Disposition",
            &format!("attachment; filename=\"{host}-{id}.pdf\""),
        );
    }
    Response::new(200, "image/png", view.png(o.scale as usize))
}

/// The terminal's routes; `None` for one that is not.
pub fn serve(rq: &Rq<'_>, route: &str) -> Option<Response> {
    if route == "/terminal" {
        return Some(config_route(rq));
    }
    if route == window::SCRIPT {
        return Some(
            Response::new(
                200,
                "text/javascript; charset=utf-8",
                termlook::script::text(keys::KEYPAD).into_bytes(),
            )
            .with("Cache-Control", "max-age=3600"),
        );
    }
    if route == "/confirm/panel" {
        return Some(confirm(rq));
    }
    if let Some(rest) = route.strip_prefix("/partition/") {
        let mut parts = rest.split('/');
        let (host, id, page) = (parts.next()?, parts.next()?.parse().ok()?, parts.next()?);
        return (page == "panel").then(|| panel_route(rq, host, id));
    }
    let rest = route.strip_prefix("/terminal/")?;
    let mut parts = rest.split('/');
    let (host, id) = (parts.next()?, parts.next()?.parse::<u32>().ok()?);
    Some(match parts.next() {
        None | Some("disconnected") => window_route(rq, host, id),
        Some("screen.png") => picture(rq, host, id, false),
        Some("screen-off.png") => picture(rq, host, id, true),
        Some("print") => picture(rq, host, id, false),
        #[cfg(unix)]
        Some("ws") => socket_route(rq, host, id),
        _ => return None,
    })
}

/// `GET /terminal/<host>/<id>/ws`: the script's socket.
#[cfg(unix)]
fn socket_route(rq: &Rq<'_>, host: &str, id: u32) -> Response {
    let Some(path) = rq.app.console.consoles() else {
        return qsweb::http::plain(503, "The managed-systems service has no console socket.");
    };
    let keep = keep_windows(rq);
    let idle = rq.app.settings().idle_minutes;
    let relay = socket::Relay {
        path,
        watch: watch(rq, host, id, Options::of(rq).take_over),
        bw: Options::of(rq).bw,
        ended: rq.app.windows.socket(&rq.session.id),
        idle: (!keep).then(|| std::time::Duration::from_secs(u64::from(idle) * 60)),
    };
    rq.audit(
        &format!("open console socket of {host} partition {id}"),
        "opened",
    );
    qsweb::ws::upgrade(rq.req, move |ws| socket::run(&relay, ws))
}

/// `GET`, `POST /terminal`: Open terminal window.
fn config_route(rq: &Rq<'_>) -> Response {
    let kept = crate::systems::systems(rq).unwrap_or_default();
    let mut choices = Vec::new();
    for k in &kept {
        if k.kind == mgmtproto::kept::Kind::Standalone {
            choices.push(config::Choice {
                label: format!("{} (standalone)", k.name),
                host: k.name.clone(),
                id: 0,
            });
            continue;
        }
        for p in &k.partitions {
            let waiting = p.code.starts_with('A') && p.code.len() == 8;
            let label = if waiting {
                format!(
                    "{} (ID {}) on {} - {} waiting for console",
                    p.name, p.id, k.name, p.code
                )
            } else {
                format!("{} (ID {}) on {}", p.name, p.id, k.name)
            };
            choices.push(config::Choice {
                label,
                host: k.name.clone(),
                id: p.id,
            });
        }
    }
    let f = rq.req.form();
    if rq.post() {
        if f.has("cancel") {
            return Response::redirect(&rq.ctx.url("/servers"));
        }
        let Some(c) = choices.iter().find(|c| c.label == f.str("p")) else {
            let m = qmcmsg::message(Kind::Error, &qmcmsg::NOT_VALID, &["Choose a partition."]);
            return rq.done("/terminal", m);
        };
        if c.id == 0 {
            let m = qmcmsg::message(Kind::Error, &qmcmsg::STANDALONE_CONSOLE, &[&c.host]);
            return rq.done("/terminal", m);
        }
        let mut q = format!("/terminal/{}/{}?x=1", c.host, c.id);
        if !f.has("strip") {
            q.push_str("&strip=0");
        }
        if f.has("force") {
            q.push_str("&take=1");
        }
        if f.str("c") == "w" {
            q.push_str("&c=w");
        }
        return Response::redirect(&rq.ctx.url(&q));
    }
    let v = config::Config {
        chosen: choices.first().map(|c| c.label.clone()).unwrap_or_default(),
        choices,
        wide: false,
        bw: false,
        strip: true,
        take_over: false,
        messages: Vec::new(),
    };
    rq.page(config::page(&rq.ctx, &v))
}
