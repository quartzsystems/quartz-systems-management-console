// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! Phase 5's suites: partitions, profiles and virtual resources through
//! the web interface of a console in QEMU, against the simulator's
//! `QSHQVS1` (the whole host family, the prototype's data) and
//! `QSHQ400` (standalone), which run in the harness's process so a suite
//! reads what the host received and keeps.

mod audit;
mod confirm;
pub(crate) mod pages;
mod requests;
pub(crate) mod sched;
mod upload;
pub(crate) mod wizard;

use std::sync::MutexGuard;
use std::time::Duration;

use super::base::{Run, SuiteFn, channel};
use super::mgmt::{Fleet, keep, page, until};
use super::web::{ADMIN, Console, PASSWORD, follow, message_of, shows, signed_on, start, submit};
use crate::web::form::{Form, pick};
use crate::web::{Answer, Browser};

/// The suites, by the handoff's numbers.
pub const SUITES: &[(&str, SuiteFn)] = &[
    ("parts-pages", pages::pages),
    ("parts-requests", requests::requests),
    ("parts-stale", requests::stale),
    ("parts-wizard", wizard::wizard),
    ("parts-confirm", confirm::confirm),
    ("parts-upload", upload::upload_image),
    ("parts-sched", sched::sched),
    ("parts-groups", wizard::groups),
    ("parts-roles", pages::roles),
    ("parts-lists", pages::lists),
    ("parts-older", pages::older),
    ("parts-audit", audit::audit),
];

/// The host of the suites.
pub const VS: &str = "QSHQVS1";
/// The standalone system beside it.
pub const STANDALONE: &str = "QSHQ400";

/// A console managing `QSHQVS1` and `QSHQ400`, signed on as the first
/// Administrator.
pub struct Lab<'a> {
    /// The simulated systems: `QSHQVS1` first.
    pub fleet: Fleet,
    /// The console and its browser.
    pub c: Console<'a>,
}

/// What `QSHQVS1` keeps of `what` (`vdisks`, `profiles 1`).
pub fn kept(f: &Fleet, what: &str) -> String {
    f.tell(0, &format!("kept {what}"))
}

/// `QSHQVS1` itself, locked.
pub fn host(f: &Fleet) -> MutexGuard<'_, sim::System> {
    sim::lock(&f.systems[0])
}

/// How many requests of `family` `QSHQVS1` had from `user`.
pub fn asked(f: &Fleet, family: u16, user: &str) -> usize {
    let want = format!("request {family:04X}/");
    let by = format!("from user {user}");
    host(f)
        .lines
        .iter()
        .filter(|l| l.contains(&want) && l.ends_with(&by))
        .count()
}

/// The line of `kept` that begins with `name` and a space.
pub fn line_of<'k>(kept: &'k str, name: &str) -> Option<&'k str> {
    kept.lines().find(|l| l.starts_with(&format!("{name} ")))
}

/// Boot an installed console, add both systems and wait until what
/// `mgmtd` keeps of `QSHQVS1` is whole: its partitions, their profiles
/// and its virtual resources.
pub fn lab<'a>(run: &Run<'a>, tag: &str) -> Result<Lab<'a>, String> {
    let fleet = Fleet::start(vec![sim::fixture::qshqvs1(), sim::fixture::qshq400()])?;
    let mut c = start(run, tag)?;
    signed_on(&mut c.browser, ADMIN, PASSWORD)?;
    keep(&mut c.browser, &fleet, 0)?;
    keep(&mut c.browser, &fleet, 1)?;
    ready(&mut c.browser)?;
    Ok(Lab { fleet, c })
}

/// Wait until the pages show everything the fixture has.
pub fn ready(b: &mut Browser) -> Result<(), String> {
    until("QSHQVS1 read whole", Duration::from_secs(120), || {
        let list = page(b, "/qsmc/partitions")?;
        let profiles = page(b, "/qsmc/partition/QSHQVS1/1/profiles")?;
        let storage = page(b, "/qsmc/server/QSHQVS1/virtual/storage")?;
        let tape = page(b, "/qsmc/server/QSHQVS1/virtual/tape")?;
        Ok(list.contains("QSPROD01")
            && list.contains("(standalone)")
            && profiles.contains("big-test")
            && storage.contains("VD06")
            && tape.contains("VTAP02"))
    })
}

/// The request's page answers: `a` followed once when it is a redirect,
/// with message `id` shown; its text.
pub fn expect(b: &mut Browser, a: &Answer, id: &str) -> Result<String, String> {
    let to = follow(b, a)?;
    let t = to.text();
    if shows(&t, id) {
        Ok(t)
    } else {
        Err(format!(
            "{id} not shown ({}): {}",
            to.status,
            message_of(&t)
        ))
    }
}

/// Submit the form of `html`, served at `here`, that has the button
/// `submit` (name, value), as a browser does: its own fields with `set`
/// applied, then the button.
pub fn post_on(
    b: &mut Browser,
    html: &str,
    here: &str,
    submit: (&str, &str),
    set: &[(&str, &str)],
) -> Result<Answer, String> {
    let f: Form = pick(html, here, submit.0).ok_or_else(|| format!("{here} has no form"))?;
    let mut fields = f.with(set);
    if !submit.0.is_empty() {
        fields.push((submit.0.to_string(), submit.1.to_string()));
    }
    let pairs: Vec<(&str, &str)> = fields
        .iter()
        .map(|(k, v)| (k.as_str(), v.as_str()))
        .collect();
    b.post(&f.action, &pairs)
}

/// [`post_on`] of the page at `path`, read now.
pub fn post_form(
    b: &mut Browser,
    path: &str,
    submit: (&str, &str),
    set: &[(&str, &str)],
) -> Result<Answer, String> {
    let html = page(b, path)?;
    post_on(b, &html, path, submit, set)
}

/// A web user `id` of `role` made by the Administrator `b`, signed on in
/// a browser of its own.
pub fn user(b: &mut Browser, id: &str, role: &str, password: &str) -> Result<Browser, String> {
    let r = submit(
        b,
        "/qsmc/console/users",
        &[
            ("id", id),
            ("role", role),
            ("password", password),
            ("again", password),
            ("create", "OK"),
        ],
    )?;
    let t = follow(b, &r)?.text();
    if !t.contains(id) {
        return Err(format!("user {id} was not made: {}", message_of(&t)));
    }
    let mut nb = b.fresh();
    signed_on(&mut nb, id, password)?;
    Ok(nb)
}

/// Every log of the console, as text.
pub fn all_logs(guest: &mut crate::qemu::session::Session) -> Result<String, String> {
    let ch = channel(guest)?;
    let mut logs = String::new();
    for name in ch.ls("/data/log")? {
        if let Ok(bytes) = ch.cat(&format!("/data/log/{name}")) {
            logs.push_str(&String::from_utf8_lossy(&bytes));
        }
    }
    Ok(logs)
}

/// The value of input `name` on `html`'s first form that has it.
pub fn field(html: &str, here: &str, name: &str) -> String {
    crate::web::form::forms(html, here)
        .iter()
        .find_map(|f| f.get(name).map(str::to_string))
        .unwrap_or_default()
}

/// `html`'s text with the entities the pages write decoded: `'` for
/// `&#39;`.
pub fn plain(html: &str) -> String {
    html.replace("&#39;", "'")
        .replace("&#x27;", "'")
        .replace("&apos;", "'")
        .replace("&quot;", "\"")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
}
