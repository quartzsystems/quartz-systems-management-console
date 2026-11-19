// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! Every route under `/qsmc/`: the open ones (sign-on, sign off), then
//! the session gate (a live session of a user still there and enabled,
//! within its maximum length), the CSRF check of every POST, the task
//! the route needs, and the page.

use qsweb::frame::{Ctx, Page, render};
use qsweb::http::{Peer, Request, Response};
use qsweb::msg::{Kind, Message, MessageText as _};
use qsweb::pages::{Refusal, not_authorized, second_level};
use qsweb::session::{self, Session};
use webd::product::{LATER, PREFIX, PRODUCT};
use webusers::{Refused, Role, Task};

use super::{App, FROM, ROLE, SINCE};
use crate::signon::{self, Signon};

/// What a signed-on route works with.
pub struct Rq<'a> {
    /// The pages.
    pub app: &'a App,
    /// The request.
    pub req: &'a Request,
    /// The client.
    pub peer: &'a Peer,
    /// The session.
    pub session: Session,
    /// The role it signed on with.
    pub role: Role,
    /// The frame's view.
    pub ctx: Ctx,
}

impl Rq<'_> {
    /// The address the request came from.
    pub fn from(&self) -> String {
        self.peer.addr.ip().to_string()
    }

    /// The signed-on user.
    pub fn user(&self) -> &str {
        &self.session.user
    }

    /// Whether this is a POST.
    pub fn post(&self) -> bool {
        self.req.method == "POST"
    }

    /// A page in the frame, the waiting message first.
    pub fn page(&self, mut page: Page) -> Response {
        page.generic = false;
        if let Some(m) = self.app.sessions.take_flash(&self.session.id) {
            page.messages.insert(0, m);
        }
        Response::html(200, render(&self.ctx, &page))
    }

    /// `303` to `route` with `m` waiting there.
    pub fn done(&self, route: &str, m: Message) -> Response {
        self.app.sessions.flash(&self.session.id, m);
        Response::redirect(&PRODUCT.url(route))
    }

    /// Record a change.
    pub fn audit(&self, what: &str, outcome: &str) {
        self.app.audit(self.user(), &self.from(), what, outcome);
    }
}

fn signon_page(app: &App, user: &str, messages: Vec<Message>) -> Response {
    let v = Signon {
        console: app.console.name(),
        notice: app.settings().notice,
        user: user.into(),
        messages,
    };
    Response::html(200, signon::render(&v))
}

fn refused(r: Refused, id: &str) -> Message {
    use qmcmsg::{SIGNON_DISABLED, SIGNON_LOCKED, SIGNON_NOT_CORRECT, SIGNON_TOO_MANY, message};
    match r {
        Refused::NotCorrect => message(Kind::Error, &SIGNON_NOT_CORRECT, &[]),
        Refused::Locked(s) => message(Kind::Error, &SIGNON_LOCKED, &[id, &s.to_string()]),
        Refused::Disabled => message(Kind::Error, &SIGNON_DISABLED, &[id]),
        Refused::TooMany(s) => message(Kind::Error, &SIGNON_TOO_MANY, &[&s.to_string()]),
    }
}

fn sign_on(app: &App, req: &Request, peer: &Peer) -> Response {
    if req.method != "POST" {
        return signon_page(app, "", Vec::new());
    }
    let f = req.form();
    let id = f.get("user").unwrap_or("").trim().to_string();
    let password = f.get("password").unwrap_or("");
    let from = peer.addr.ip();
    match app.sign_on(&id, password, from) {
        Ok((id, role)) => {
            let s = app.sessions.create(&id);
            let now = app.console.now();
            app.sessions.update(&s.id, |s| {
                s.data.insert(ROLE.into(), role.label().into());
                s.data.insert(SINCE.into(), now.to_string());
                s.data.insert(FROM.into(), from.to_string());
            });
            let _ = app.change_users(|u| {
                u.signed_on(&id, now, &from.to_string());
                Ok(())
            });
            app.audit(&id, &from.to_string(), "sign on", "signed on");
            Response::redirect(&PRODUCT.url("/welcome"))
                .with("Set-Cookie", &PRODUCT.set_cookie(&s.id, peer.secure))
        }
        Err(r) => {
            app.audit(
                &id,
                &from.to_string(),
                "sign on",
                &format!("refused: {r:?}"),
            );
            signon_page(app, &id, vec![refused(r, &id)])
        }
    }
}

/// The session of `req`, when it is live and its user still may sign
/// on; the reason it ended otherwise.
fn live(app: &App, req: &Request) -> Result<(Session, Role), bool> {
    let Some(id) = PRODUCT.session_id(req) else {
        return Err(false);
    };
    let Some(s) = app.sessions.get(id) else {
        return Err(true);
    };
    let max = u64::from(app.settings().max_hours) * 3600;
    let since: u64 = s.get(SINCE).parse().unwrap_or(0);
    let still = app.users().get(&s.user).is_some_and(|u| u.enabled);
    let role = Role::parse(s.get(ROLE));
    match role {
        Some(role) if still && app.console.now().saturating_sub(since) < max => Ok((s, role)),
        _ => {
            app.sessions.remove(id);
            Err(true)
        }
    }
}

/// The page saying a function arrives in a later release.
fn later(rq: &Rq<'_>, route: &str) -> Option<Response> {
    let (group, label, item) = PRODUCT.nav.iter().find_map(|g| {
        g.items
            .iter()
            .find(|(_, r)| *r == route)
            .map(|(l, r)| (g.label, *l, *r))
    })?;
    let m = qmcmsg::message(Kind::Info, &qmcmsg::LATER_RELEASE, &[label]);
    let crumb = if group == label {
        vec![label.to_string()]
    } else {
        vec![group.to_string(), label.to_string()]
    };
    Some(rq.page(Page {
        title: label.into(),
        crumb,
        h1: label.into(),
        messages: vec![m],
        body: String::new(),
        help: "later".into(),
        nav: Some((group, Some(item))),
        ..Page::default()
    }))
}

/// The error page of message `def` with `data`, with `status`.
pub fn error(rq: &Rq<'_>, status: u16, def: &qmcmsg::Def, data: &[&str]) -> Response {
    let sl = qmcmsg::Catalogue
        .lookup(def.id, data)
        .expect("a catalogue message");
    let mut page = second_level(&rq.ctx, &sl, None, "Error");
    page.generic = false;
    let mut r = Response::html(status, render(&rq.ctx, &page));
    r.status = status;
    r
}

fn refuse(rq: &Rq<'_>, task: Task) -> Response {
    let user = rq.user().to_string();
    rq.audit(
        &format!("{} {}", rq.req.method, rq.req.path),
        &format!("not authorized: {}", task.label()),
    );
    let r = Refusal {
        message: qmcmsg::message(
            Kind::Error,
            &qmcmsg::NOT_AUTHORIZED,
            &[&user, &task.label().to_lowercase()],
        ),
        explain: format!(
            "Your role, {}, does not have the task <b>{}</b>.",
            rq.role.label(),
            task.label()
        ),
        function: task.label().into(),
        object: rq.req.path.clone(),
        required: task.label().into(),
        user,
        policy: None,
        crumb: Vec::new(),
    };
    let mut page = not_authorized(&rq.ctx, &r);
    page.generic = false;
    let mut resp = Response::html(403, render(&rq.ctx, &page));
    resp.status = 403;
    resp
}

/// The task a route needs; `None` for one every signed-on user may see.
pub fn task_of(route: &str) -> Option<Task> {
    if route.starts_with("/console/users") || route.starts_with("/console/roles") {
        return Some(Task::Users);
    }
    if route == "/console/version" {
        return Some(Task::View);
    }
    if route.starts_with("/console") {
        return Some(Task::Settings);
    }
    match route {
        "/welcome" => Some(Task::View),
        _ => None,
    }
}

/// Answer `req`.
pub fn route(app: &App, req: &Request, peer: &Peer) -> Response {
    let Some(route) = req.path.strip_prefix(PREFIX) else {
        return qsweb::http::plain(404, "There is no such page.");
    };
    match route {
        "/signon" => return sign_on(app, req, peer),
        "/signoff" => {
            if let Some(id) = PRODUCT.session_id(req) {
                if let Some(s) = app.sessions.get(id) {
                    app.audit(
                        &s.user,
                        &peer.addr.ip().to_string(),
                        "sign off",
                        "signed off",
                    );
                }
                app.sessions.remove(id);
            }
            let m = qmcmsg::message(Kind::Info, &qmcmsg::SIGNED_OFF, &[]);
            return signon_page(app, "", vec![m]).with("Set-Cookie", &PRODUCT.clear_cookie());
        }
        "" | "/" => return Response::redirect(&PRODUCT.url("/welcome")),
        _ => {}
    }
    let (session, role) = match live(app, req) {
        Ok(s) => s,
        Err(ended) => {
            if req.method == "GET" && !ended {
                return Response::redirect(&PRODUCT.url("/signon"));
            }
            let m = qmcmsg::message(Kind::Info, &qmcmsg::SESSION_ENDED, &[]);
            return signon_page(app, "", vec![m]).with("Set-Cookie", &PRODUCT.clear_cookie());
        }
    };
    let rq = Rq {
        app,
        req,
        peer,
        ctx: Ctx {
            product: &PRODUCT,
            system: app.console.name(),
            user: session.user.clone(),
            csrf: session.csrf.clone(),
            open: None,
            here: qsweb::frame::here(req),
        },
        session,
        role,
    };
    if rq.post() {
        let f = req.form();
        if !session::csrf_ok(&rq.session, f.get(session::CSRF_FIELD).unwrap_or("")) {
            rq.audit(&format!("POST {}", req.path), "refused: no form token");
            return error(&rq, 403, &qmcmsg::FORM_EXPIRED, &[]);
        }
    }
    let matrix = app.users().matrix.clone();
    if let Some(task) = task_of(route).filter(|t| !matrix.allows(role, *t)) {
        return refuse(&rq, task);
    }
    if let Some(id) = route.strip_prefix("/msg/") {
        return match qmcmsg::Catalogue.lookup(id, &[]) {
            Some(sl) => {
                let mut page = second_level(&rq.ctx, &sl, None, "Message");
                page.generic = false;
                Response::html(200, render(&rq.ctx, &page))
            }
            None => error(&rq, 404, &qmcmsg::NOT_FOUND, &[&req.path]),
        };
    }
    if let Some(topic) = route.strip_prefix("/help/") {
        return Response::html(200, crate::help::window(topic));
    }
    if LATER.iter().any(|(r, _)| *r == route) {
        if !matrix.allows(role, Task::View) {
            return refuse(&rq, Task::View);
        }
        if let Some(r) = later(&rq, route) {
            return r;
        }
    }
    crate::pages::serve(&rq, route)
        .unwrap_or_else(|| error(&rq, 404, &qmcmsg::NOT_FOUND, &[&req.path]))
}
