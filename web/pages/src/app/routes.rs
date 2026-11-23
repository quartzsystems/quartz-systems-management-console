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

    /// A value of the query string, empty when absent.
    pub fn q(&self, name: &str) -> String {
        self.req.query().get(name).unwrap_or("").to_string()
    }

    /// Whether this is a POST.
    pub fn post(&self) -> bool {
        self.req.method == "POST"
    }

    /// A page in the frame, the waiting message first.
    pub fn page(&self, mut page: Page) -> Response {
        page.generic = false;
        let more = self
            .app
            .more
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .remove(&self.session.id)
            .unwrap_or_default();
        for m in more.into_iter().rev() {
            page.messages.insert(0, m);
        }
        if let Some(m) = self.app.sessions.take_flash(&self.session.id) {
            page.messages.insert(0, m);
        }
        if let Some(m) = self
            .app
            .announced
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
        {
            page.messages.insert(0, m);
        }
        // The page's group opens when the user arrives from another;
        // the others stay as the user left them.
        let mut ctx = self.ctx.clone();
        if let Some((group, _)) = page.nav {
            ctx.open = Some(session::nav_visit(
                &self.app.sessions,
                &self.session.id,
                group,
            ));
        }
        Response::html(200, render(&ctx, &page))
    }

    /// `303` to `route` with `m` waiting there.
    pub fn done(&self, route: &str, m: Message) -> Response {
        self.app.sessions.flash(&self.session.id, m);
        Response::redirect(&PRODUCT.url(route))
    }

    /// `303` to `route` with every one of `ms` waiting there, in order.
    pub fn done_all(&self, route: &str, ms: Vec<Message>) -> Response {
        let mut ms = ms.into_iter();
        if let Some(first) = ms.next() {
            self.app.sessions.flash(&self.session.id, first);
        }
        let rest: Vec<Message> = ms.collect();
        if !rest.is_empty() {
            self.app
                .more
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .insert(self.session.id.clone(), rest);
        }
        Response::redirect(&PRODUCT.url(route))
    }

    /// Record a change.
    pub fn audit(&self, what: &str, outcome: &str) {
        self.app.audit(self.user(), &self.from(), what, outcome);
    }
}

/// The most web sessions a console keeps at once.
pub const MAX_SESSIONS: usize = 256;

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

/// A sign-on at the address of an `open` change on trial, within its
/// window: the operator reached the console again, and it is kept.
fn reached(app: &App, req: &Request, session: &str, user: &str, from: &str) {
    let mut pending = app
        .pending
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let Some((address, until)) = pending.clone() else {
        return;
    };
    if app.console.uptime() > until {
        *pending = None;
        return;
    }
    let host = req.header("host").unwrap_or("");
    let host = host.rsplit_once(':').map_or(host, |(h, _)| h);
    if !address.is_empty() && host != address {
        return;
    }
    *pending = None;
    let done = app.console.confirm_network();
    app.audit(
        user,
        from,
        "keep the open interface's change",
        &format!("{done:?}"),
    );
    let m = match done {
        Ok(()) => qmcmsg::message(Kind::Done, &qmcmsg::NETWORK_KEPT, &[]),
        Err(e) => qmcmsg::message(Kind::Error, &qmcmsg::NETWORK_RESTORED, &[&e]),
    };
    app.sessions.flash(session, m);
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
            // The sessions a console keeps are bounded; an idle one ends
            // by its timeout and makes room.
            if app.sessions.count() >= MAX_SESSIONS {
                app.audit(
                    &id,
                    &from.to_string(),
                    "sign on",
                    "refused: the most sessions",
                );
                let n = MAX_SESSIONS.to_string();
                let m = qmcmsg::message(Kind::Error, &qmcmsg::SESSIONS_FULL, &[&n]);
                return signon_page(app, &id, vec![m]);
            }
            let s = app.sessions.create(&id);
            let now = app.console.now();
            app.sessions.update(&s.id, |s| {
                s.data.insert(ROLE.into(), role.label().into());
                s.data
                    .insert(SINCE.into(), app.console.uptime().to_string());
                s.data.insert(FROM.into(), from.to_string());
            });
            let _ = app.change_users(|u| {
                u.signed_on(&id, now, &from.to_string());
                Ok(())
            });
            app.audit(&id, &from.to_string(), "sign on", "signed on");
            reached(app, req, &s.id, &id, &from.to_string());
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
        Some(role) if still && app.console.uptime().saturating_sub(since) < max => Ok((s, role)),
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

/// Not authorized when the user's role lacks `task`: for a page whose
/// changes need more than seeing it.
pub fn lacks(rq: &Rq<'_>, task: Task) -> Option<Response> {
    let matrix = rq.app.users().matrix.clone();
    (!matrix.allows(rq.role, task)).then(|| refuse(rq, task))
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
    if route.starts_with("/console/users")
        || route.starts_with("/console/roles")
        || route == "/confirm/user-delete"
    {
        return Some(Task::Users);
    }
    if route == "/console/version" {
        return Some(Task::View);
    }
    // Adding, pairing again and removing a managed system change the
    // console's configuration.
    if route.starts_with("/servers/add") || route == "/servers/reset" {
        return Some(Task::Settings);
    }
    // Functions 08 and 22 need the task of the window that types; the
    // page checks the function's own.
    if route == "/confirm/panel" {
        return Some(Task::Terminal);
    }
    if route.starts_with("/terminal") || route.starts_with("/partition/") {
        return Some(Task::View);
    }
    // Scheduled operations: seen by every role; a change needs the task
    // of the operation, which the page checks.
    if route.starts_with("/schedule") || route == "/confirm/schedule-remove" {
        return Some(Task::View);
    }
    // Custom groups: seen by every role; changed by those that operate.
    if route == "/groups/create" || route == "/confirm/group-delete" {
        return Some(Task::Activate);
    }
    if route.starts_with("/group/") {
        return Some(Task::View);
    }
    // Partitions made, changed and deleted.
    if route.starts_with("/partitions/create")
        || crate::partitions::confirm::ROUTES.contains(&route)
    {
        return Some(Task::CreatePartitions);
    }
    // A host's virtual resources' destructive actions.
    if crate::vres::confirm::ROUTES.contains(&route) {
        return Some(Task::VirtualResources);
    }
    if route == "/confirm/event-close" || route == "/confirm/dump-delete" {
        return Some(Task::Events);
    }
    // System plans: every task of them is the one row of the matrix.
    if route.starts_with("/plan/")
        || route == "/confirm/plan-delete"
        || route == "/confirm/plan-deploy"
    {
        return Some(Task::Plans);
    }
    if route == "/confirm/update-install" {
        return Some(Task::CodeUpdates);
    }
    if route.starts_with("/console") || route.starts_with("/confirm/") {
        return Some(Task::Settings);
    }
    // The later phases' routes need the task their pages will.
    if route.starts_with("/server/") {
        // Power off, Restart and Power on need the task; a system's
        // other pages, View.
        if route.ends_with("/confirm") {
            return Some(Task::Power);
        }
        return Some(Task::View);
    }
    match route {
        "/welcome" | "/servers" | "/partitions" | "/groups" => Some(Task::View),
        "/plans" => Some(Task::Plans),
        "/events" | "/refcodes" | "/dumps" | "/connectivity" => Some(Task::Events),
        r if r.starts_with("/refcode/") => Some(Task::Events),
        r if r.starts_with("/event/") || r.starts_with("/dumps/") => Some(Task::Events),
        "/updates" | "/updates/code" => Some(Task::CodeUpdates),
        r if r.starts_with("/updates/") => Some(Task::CodeUpdates),
        r if r.starts_with("/dev/") => Some(Task::View),
        _ => None,
    }
}

/// The signed-on request of `req`, or the answer that its session is
/// not live.
fn signed_on<'a>(app: &'a App, req: &'a Request, peer: &'a Peer) -> Result<Rq<'a>, Response> {
    let (session, role) = match live(app, req) {
        Ok(s) => s,
        Err(ended) => {
            if let Some(id) = PRODUCT.session_id(req) {
                app.windows.session_ended(id, app.settings().keep_terminals);
            }
            if req.method == "GET" && !ended {
                return Err(Response::redirect(&PRODUCT.url("/signon")));
            }
            let m = qmcmsg::message(Kind::Info, &qmcmsg::SESSION_ENDED, &[]);
            return Err(signon_page(app, "", vec![m]).with("Set-Cookie", &PRODUCT.clear_cookie()));
        }
    };
    Ok(Rq {
        app,
        req,
        peer,
        ctx: Ctx {
            product: &PRODUCT,
            system: app.console.name(),
            user: session.user.clone(),
            csrf: session.csrf.clone(),
            open: Some(session::nav_open(&session)),
            here: qsweb::frame::here(req),
        },
        session,
        role,
    })
}

/// Answer `req` whose body streams (an upload): signed on, then the
/// page, which checks its form's token as it reads.
pub fn route_stream(
    app: &App,
    req: &Request,
    body: &mut dyn std::io::Read,
    peer: &Peer,
) -> Response {
    let rq = match signed_on(app, req, peer) {
        Ok(rq) => rq,
        Err(r) => return r,
    };
    crate::vres::upload::post(&rq, body)
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
                app.windows.session_ended(id, app.settings().keep_terminals);
            }
            let m = qmcmsg::message(Kind::Info, &qmcmsg::SIGNED_OFF, &[]);
            return signon_page(app, "", vec![m]).with("Set-Cookie", &PRODUCT.clear_cookie());
        }
        "" | "/" => return Response::redirect(&PRODUCT.url("/welcome")),
        _ => {}
    }
    let rq = match signed_on(app, req, peer) {
        Ok(rq) => rq,
        Err(r) => return r,
    };
    if rq.post() {
        let f = req.form();
        if !session::csrf_ok(&rq.session, f.get(session::CSRF_FIELD).unwrap_or("")) {
            rq.audit(&format!("POST {}", req.path), "refused: no form token");
            return error(&rq, 403, &qmcmsg::FORM_EXPIRED, &[]);
        }
    }
    let matrix = app.users().matrix.clone();
    let role = rq.role;
    if let Some(task) = task_of(route).filter(|t| !matrix.allows(role, *t)) {
        return refuse(&rq, task);
    }
    if route == "/nav" {
        // `[+]` and `[-]` of the tree: the group opens or closes, the
        // page stays.
        return qsweb::frame::nav_toggle(&PRODUCT, &app.sessions, &rq.session.id, req);
    }
    if let Some(id) = route.strip_prefix("/msg/") {
        // The console's own, else a QS/VS host's from its catalogue.
        let host = || {
            app.catalogue()
                .message(id)
                .map(|(text, help)| qsweb::msg::SecondLevel {
                    id: id.into(),
                    text: text.clone(),
                    cause: "A QS/VS host issued this message.".into(),
                    recovery: help.clone(),
                    severity: 30,
                })
        };
        return match qmcmsg::Catalogue.lookup(id, &[]).or_else(host) {
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

#[cfg(test)]
mod tests {
    #[test]
    fn every_route_of_the_tree_names_its_task() {
        for g in webd::product::NAV {
            for (_, route) in g.items {
                assert!(super::task_of(route).is_some(), "{route} has no task");
            }
        }
    }
}
