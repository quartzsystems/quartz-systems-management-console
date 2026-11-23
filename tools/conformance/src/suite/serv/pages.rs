// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! Suite 2: every page of this phase served with the fixture's data,
//! each link and form action answering, no script of their own, every form posting
//! without one (its golden comparison is `webpages`' own tests, run by
//! `cargo test`). Suite 16's roles: every new route for each role as the
//! matrix says.

use std::collections::{BTreeSet, VecDeque};
use std::time::Duration;

use webusers::{Matrix, Role, Task};

use super::{Lab, QSVS_NOTES, STANDALONE, VS, lab, number_of, post_code};
use crate::suite::mgmt::page;
use crate::suite::parts::pages::{Crawl, check_forms, key, links};
use crate::suite::parts::user;
use crate::suite::web::{message_of, shows};
use crate::web::Browser;

/// The dump of the fixture's `QSTEST01` the suites name.
pub const DUMP: &str = "QSHQVS1:2:SYSDUMP.QSTEST01.20261029.221405";

/// Whether `path` is a page of this phase, whose links are followed.
fn ours(path: &str) -> bool {
    let p = path.split('?').next().unwrap_or("");
    p.starts_with("/qsmc/event")
        || p.starts_with("/qsmc/dumps")
        || p.starts_with("/qsmc/updates")
        || p == "/qsmc/welcome"
        || p.ends_with("/confirm/event-close")
        || p.ends_with("/confirm/dump-delete")
        || p.ends_with("/confirm/update-install")
}

/// Whether `path` streams a file or begins a download, and is asked for
/// its answer alone.
fn streamed(path: &str) -> bool {
    let p = path.split('?').next().unwrap_or("");
    p == "/qsmc/dumps/download" || p == "/qsmc/updates/code/download"
}

/// The pages a crawl starts from: `n` the host's event, `own` the
/// console's.
fn seeds(n: u64, own: u64) -> Vec<String> {
    let mut v: Vec<String> = [
        "/qsmc/welcome",
        "/qsmc/events",
        "/qsmc/events?s=All",
        "/qsmc/events?s=Closed&ms=QSHQ400",
        "/qsmc/dumps",
        "/qsmc/updates",
        "/qsmc/updates/code",
        "/qsmc/updates/code/console/progress",
        "/qsmc/confirm/update-install?t=QSHQVS1",
    ]
    .iter()
    .map(|s| (*s).to_string())
    .collect();
    for tail in ["", "/repair", "/note", "/export"] {
        v.push(format!("/qsmc/event/{n}{tail}"));
    }
    v.push(format!("/qsmc/confirm/event-close?n={n}"));
    v.push(format!("/qsmc/event/console/{own}"));
    v.push(format!("/qsmc/confirm/event-close?c={own}"));
    v.push(format!("/qsmc/confirm/dump-delete?d={DUMP}"));
    v.push(format!("/qsmc/updates/code/{VS}/progress"));
    v.push(format!("/qsmc/updates/code/{VS}/log"));
    v.push(format!("/qsmc/updates/notes/qsvs/{QSVS_NOTES}"));
    v
}

/// A lab with a console problem of its own, the host's image downloaded:
/// the host's event's number and the console's.
fn ready_lab<'a>(
    run: &crate::suite::base::Run<'a>,
    tag: &str,
) -> Result<(Lab<'a>, u64, u64), String> {
    let mut lab = lab(run, tag, &[])?;
    post_code(
        &mut lab.c.guest,
        "A3005030",
        &format!("system:{VS}"),
        &format!("Managed system {VS} was not reached for 31 minutes."),
    )?;
    super::download(&mut lab.c.browser, VS, "qsvs", super::QSVS_IMAGE)?;
    let n = number_of(&mut lab.c.browser, "B6000100")?;
    // The problem service opens it a moment after the code is posted.
    let own = super::events::own_event(
        &mut lab.c.browser,
        "A3005030",
        "A3005030",
        Duration::from_secs(60),
    )?;
    Ok((lab, n, own))
}

/// Suite 2.
pub fn pages(run: &crate::suite::base::Run<'_>) -> Result<String, String> {
    let (mut lab, n, own) = ready_lab(run, "serv-pages")?;
    let b = &mut lab.c.browser;
    let mut seen = BTreeSet::new();
    let mut queue: VecDeque<String> = seeds(n, own).into();
    let mut c = Crawl::default();
    let mut asked = 0;
    while let Some(path) = queue.pop_front() {
        let bare = path.split('?').next().unwrap_or("").to_string();
        if bare.starts_with("/qsmc/signoff") || bare.ends_with("/ws") || !seen.insert(key(&path)) {
            continue;
        }
        if streamed(&path) {
            // A download's answer, none of its body; a download begun
            // is the stream's, which every link names already.
            if bare == "/qsmc/dumps/download" {
                let got = b.stream_get(&path, Some(0), |_| {})?;
                if got.status != 200 {
                    return Err(format!("{path} answered {}", got.status));
                }
            }
            asked += 1;
            continue;
        }
        let a = b.get(&path)?;
        if crate::web::opened(&path, a.status)? {
            continue;
        }
        let text = a.text();
        if crate::web::scripted(&text) {
            return Err(format!("{path} carries a script"));
        }
        if !ours(&path) {
            if a.status == 404 || a.status >= 500 {
                return Err(format!("{path} answered {}", a.status));
            }
            c.other += 1;
            continue;
        }
        // A note's text and an export are files of their own.
        if a.status != 200 {
            // A redirect's message is on the page it leads to.
            let said = crate::suite::web::follow(b, &a)?.text();
            return Err(format!(
                "{path} answered {}: {}",
                a.status,
                message_of(&said)
            ));
        }
        c.pages += 1;
        if a.header("content-type")
            .is_some_and(|t| t.starts_with("text/html"))
        {
            check_forms(b, &text, &path, &mut c)?;
            queue.extend(links(&text));
        }
        if c.pages + c.other > 2000 {
            return Err("more than 2000 pages: the crawl does not end".into());
        }
    }
    lab.c.guest.kill();
    Ok(format!(
        "{} pages of this phase (events, event {n} and the console's own with Repair, Add note, \
         Export and Close, dumps, Code updates, the install confirmation, progress and log, \
         Updates, the notes, Welcome) each answered 200 with no script of their own, and {} pages they link \
         to answered; {asked} downloads answered without their bodies; {} forms each post with \
         the token, have a button and an action that answers, {} of them posted with Cancel",
        c.pages, c.other, c.forms, c.posted
    ))
}

/// One route of the roles' table: a GET's task, and a POST's fields,
/// which change nothing, and its task.
struct R {
    path: String,
    get: Task,
    post: Option<(Task, Vec<(&'static str, String)>)>,
}

/// The routes of this phase and the tasks `docs/WEBUSERS.md` gives them.
fn routes(n: u64, own: u64) -> Vec<R> {
    use Task::{CodeUpdates as U, Events as E};
    let r = |path: String, get: Task, post: Option<(Task, Vec<(&'static str, String)>)>| R {
        path,
        get,
        post,
    };
    vec![
        r("/qsmc/events".into(), E, None),
        r(
            format!("/qsmc/event/{n}"),
            E,
            Some((E, vec![("repair", "Repair".into())])),
        ),
        r(format!("/qsmc/event/{n}/repair"), E, None),
        r(
            format!("/qsmc/event/{n}/note"),
            E,
            Some((E, vec![("cancel", "Cancel".into())])),
        ),
        r(format!("/qsmc/event/{n}/export"), E, None),
        r(format!("/qsmc/event/console/{own}"), E, None),
        r(
            format!("/qsmc/confirm/event-close?n={n}"),
            E,
            Some((E, vec![("n", n.to_string()), ("cancel", "Cancel".into())])),
        ),
        r(
            "/qsmc/dumps".into(),
            E,
            Some((E, vec![("copy", "Copy".into())])),
        ),
        r(
            format!("/qsmc/confirm/dump-delete?d={DUMP}"),
            E,
            Some((E, vec![("d", DUMP.into()), ("cancel", "Cancel".into())])),
        ),
        r("/qsmc/dumps/take".into(), E, None),
        r("/qsmc/updates".into(), U, None),
        r(
            "/qsmc/updates/code".into(),
            U,
            Some((U, vec![("s", String::new()), ("apply", "Apply".into())])),
        ),
        r(
            format!("/qsmc/confirm/update-install?t={VS}"),
            U,
            Some((U, vec![("t", VS.into()), ("cancel", "Cancel".into())])),
        ),
        r(
            format!("/qsmc/updates/code/{VS}/progress"),
            U,
            Some((U, vec![("log", "View log".into())])),
        ),
        r(format!("/qsmc/updates/code/{VS}/log"), U, None),
        r(format!("/qsmc/updates/notes/qsvs/{QSVS_NOTES}"), U, None),
    ]
}

/// Whether `a` is what the matrix says for a request it allows or not.
fn as_matrix(want: bool, status: u16, text: &str) -> bool {
    if want {
        status != 403 && status != 404 && status < 500
    } else {
        status == 403 && shows(text, "QMC9010")
    }
}

/// Suite 16's roles.
pub fn roles(run: &crate::suite::base::Run<'_>) -> Result<String, String> {
    let (mut lab, n, own) = ready_lab(run, "serv-roles")?;
    let b = &mut lab.c.browser;
    let matrix = Matrix::default();
    let mut who: Vec<(Role, Browser)> = vec![(Role::Administrator, b.clone_session())];
    for role in [Role::Operator, Role::Viewer, Role::Service] {
        let id = role.label().to_lowercase();
        who.push((role, user(b, &id, role.label(), "Serv-roles-42")?));
    }
    let (mut pairs, mut refused) = (0, 0);
    let mut wrong = Vec::new();
    for (role, nb) in &mut who {
        for route in routes(n, own) {
            let a = nb.get(&route.path)?;
            let want = matrix.allows(*role, route.get);
            if !as_matrix(want, a.status, &a.text()) {
                wrong.push(format!("{} GET {}: {}", role.label(), route.path, a.status));
            }
            pairs += 1;
            refused += usize::from(!want);
            let Some((task, fields)) = route.post else {
                continue;
            };
            let token = nb.csrf("/qsmc/partitions")?;
            let mut all = vec![("csrf", token.as_str())];
            all.extend(fields.iter().map(|(k, v)| (*k, v.as_str())));
            let a = nb.post(&route.path, &all)?;
            let want = matrix.allows(*role, task);
            if !as_matrix(want, a.status, &a.text()) {
                wrong.push(format!(
                    "{} POST {}: {} ({})",
                    role.label(),
                    route.path,
                    a.status,
                    message_of(&a.text())
                ));
            }
            pairs += 1;
            refused += usize::from(!want);
        }
        // A dump's download, its answer alone.
        let got = nb.stream_get(&format!("/qsmc/dumps/download?d={DUMP}"), Some(0), |_| {})?;
        let want = matrix.allows(*role, Task::Events);
        if (got.status == 403) == want {
            wrong.push(format!("{} dump download: {}", role.label(), got.status));
        }
        pairs += 1;
        refused += usize::from(!want);
    }
    // Nothing a refused request asked was done: the event is open, the
    // dump listed.
    let events = page(b, "/qsmc/events")?;
    let dumps = page(b, "/qsmc/dumps")?;
    lab.c.guest.kill();
    if !wrong.is_empty() {
        return Err(format!(
            "{} of {pairs} route and role pairs not as the matrix says: {}",
            wrong.len(),
            wrong.join("; ")
        ));
    }
    if !events.contains("B6000100")
        || !dumps.contains("SYSDUMP.QSTEST01")
        || !dumps.contains(STANDALONE)
    {
        return Err("a refused request changed what is listed".into());
    }
    Ok(format!(
        "{pairs} route, method and role pairs of this phase for Administrator, Operator, Viewer \
         and Service, each as the matrix says: {refused} Not authorized (403, QMC9010), the rest \
         served; event 1042 still open and the dumps still listed"
    ))
}
