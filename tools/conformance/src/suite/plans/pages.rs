// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! Suite 2: every page of this phase served with the fixture's data, each
//! link and form action answering, no script of their own (the golden comparison is
//! `webpages`' own test). Suite 10: every route for each role as the
//! matrix says; Deploy and Delete only through their confirmation and a
//! POST with its token; every capture, import, export, deployment step
//! and delete audited with its user.

use std::collections::{BTreeSet, VecDeque};
use std::time::Duration;

use webusers::{Matrix, Role, Task};

use super::{VS1, VS2, create, deploy, ended, export, import, lab};
use crate::suite::mgmt::page;
use crate::suite::parts::pages::{Crawl, check_forms, key, links};
use crate::suite::parts::{all_logs, user};
use crate::suite::web::{ADMIN, message_of, shows};
use crate::web::Browser;

/// Whether `path` is a page of this phase, whose links are followed.
fn ours(path: &str) -> bool {
    let p = path.split('?').next().unwrap_or("");
    p == "/qsmc/plans"
        || p.starts_with("/qsmc/plan/")
        || p.starts_with("/qsmc/confirm/plan-")
        || p == "/qsmc/updates/code"
        || p == "/qsmc/welcome"
}

/// Suite 2.
pub fn pages(run: &crate::suite::base::Run<'_>) -> Result<String, String> {
    let mut lab = lab(run, "plans-pages")?;
    let b = &mut lab.c.browser;
    let name = create(b, VS1)?;
    // A deployment, so the progress page has its steps.
    deploy(b, &name, VS2, &[1], false)?;
    ended(b, &name, VS2, Duration::from_secs(300))?;
    let mut queue: VecDeque<String> = [
        "/qsmc/welcome".to_string(),
        "/qsmc/plans".into(),
        "/qsmc/plans?sort=n".into(),
        "/qsmc/plans?sort=c".into(),
        format!("/qsmc/plan/{name}"),
        format!("/qsmc/plan/{name}/edit"),
        format!("/qsmc/plan/{name}/deploy/step/1"),
        format!("/qsmc/plan/{name}/deploy/step/2?t={VS2}"),
        format!("/qsmc/plan/{name}/deploy/step/3?t={VS2}"),
        format!("/qsmc/plan/{name}/deploy/progress?t={VS2}"),
        format!("/qsmc/confirm/plan-deploy?p={name}&t={VS2}&c=1,2"),
        format!("/qsmc/confirm/plan-delete?p={name}"),
    ]
    .into();
    let mut seen = BTreeSet::new();
    let mut c = Crawl::default();
    while let Some(path) = queue.pop_front() {
        let bare = path.split('?').next().unwrap_or("").to_string();
        if bare.starts_with("/qsmc/signoff") || bare.ends_with("/ws") || !seen.insert(key(&path)) {
            continue;
        }
        let a = b.get(&path)?;
        if crate::web::opened(&path, a.status)? {
            continue;
        }
        let text = a.text();
        if a.header("content-type")
            .is_some_and(|t| t.starts_with("text/html"))
            && crate::web::scripted(&text)
        {
            return Err(format!("{path} carries a script"));
        }
        if !ours(&path) || bare.ends_with("/export") {
            if a.status == 404 || a.status >= 500 {
                return Err(format!("{path} answered {}", a.status));
            }
            c.other += 1;
            continue;
        }
        if a.status != 200 {
            return Err(format!(
                "{path} answered {}: {}",
                a.status,
                message_of(&text)
            ));
        }
        c.pages += 1;
        check_forms(b, &text, &path, &mut c)?;
        queue.extend(links(&text));
        if c.pages + c.other > 2000 {
            return Err("more than 2000 pages: the crawl does not end".into());
        }
    }
    lab.c.guest.kill();
    Ok(format!(
        "{} pages of this phase (System plans and its sorts, View, Edit, the wizard's three \
         steps, both confirmations, the progress page, Code updates, Welcome) each answered 200 \
         with no script of their own, and {} pages they link to answered; {} forms each post with the token, \
         have a button and an action that answers, {} of them posted with Cancel",
        c.pages, c.other, c.forms, c.posted
    ))
}

/// One route: a GET's task, and a POST's harmless fields and task.
struct R {
    path: String,
    post: Option<Vec<(&'static str, String)>>,
}

fn routes(name: &str) -> Vec<R> {
    let r = |path: String, post: Option<Vec<(&'static str, String)>>| R { path, post };
    let cancel = || Some(vec![("cancel", "Cancel".to_string())]);
    vec![
        r("/qsmc/plans".into(), None),
        r(format!("/qsmc/plan/{name}"), None),
        r(format!("/qsmc/plan/{name}/export"), None),
        r(format!("/qsmc/plan/{name}/edit"), cancel()),
        r(format!("/qsmc/plan/{name}/deploy/step/1"), cancel()),
        r(format!("/qsmc/plan/{name}/deploy/step/2?t={VS2}"), cancel()),
        r(format!("/qsmc/plan/{name}/deploy/step/3?t={VS2}"), cancel()),
        r(
            format!("/qsmc/plan/{name}/deploy/progress?t={VS2}"),
            cancel(),
        ),
        r(
            format!("/qsmc/confirm/plan-deploy?p={name}&t={VS2}&c=1"),
            Some(vec![
                ("p", name.to_string()),
                ("t", VS2.to_string()),
                ("c", "1".into()),
                ("cancel", "Cancel".into()),
            ]),
        ),
        r(
            format!("/qsmc/confirm/plan-delete?p={name}"),
            Some(vec![("p", name.to_string()), ("cancel", "Cancel".into())]),
        ),
    ]
}

fn as_matrix(want: bool, status: u16, text: &str) -> bool {
    if want {
        status != 403 && status != 404 && status < 500
    } else {
        status == 403 && shows(text, "QMC9010")
    }
}

/// Suite 10.
#[allow(
    clippy::too_many_lines,
    reason = "the roles, the confirmations, the audit"
)]
pub fn roles(run: &crate::suite::base::Run<'_>) -> Result<String, String> {
    let mut lab = lab(run, "plans-roles")?;
    let b = &mut lab.c.browser;
    let name = create(b, VS1)?;
    let matrix = Matrix::default();
    let mut who: Vec<(Role, Browser)> = vec![(Role::Administrator, b.clone_session())];
    for role in [Role::Operator, Role::Viewer, Role::Service] {
        let id = role.label().to_lowercase();
        who.push((role, user(b, &id, role.label(), "Plan-roles-42")?));
    }
    let (mut pairs, mut refused) = (0, 0);
    let mut wrong = Vec::new();
    for (role, nb) in &mut who {
        let want = matrix.allows(*role, Task::Plans);
        for route in routes(&name) {
            let a = nb.get(&route.path)?;
            if !as_matrix(want, a.status, &a.text()) {
                wrong.push(format!("{} GET {}: {}", role.label(), route.path, a.status));
            }
            pairs += 1;
            refused += usize::from(!want);
            let Some(fields) = route.post else {
                continue;
            };
            let token = nb.csrf("/qsmc/partitions")?;
            let mut all = vec![("csrf", token.as_str())];
            all.extend(fields.iter().map(|(k, v)| (*k, v.as_str())));
            let a = nb.post(&route.path, &all)?;
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
    }
    if !wrong.is_empty() {
        return Err(format!(
            "{} of {pairs} pairs not as the matrix says: {}",
            wrong.len(),
            wrong.join("; ")
        ));
    }
    // Deploy and Delete: a GET of the confirmation does neither; a POST
    // without the token is refused; step 3's Deploy only leads to the
    // confirmation.
    let deploy_path = format!("/qsmc/confirm/plan-deploy?p={name}&t={VS2}&c=1&act=1");
    let _ = page(b, &deploy_path)?;
    let r = b.post(
        &deploy_path,
        &[("p", &name), ("t", VS2), ("c", "1"), ("ok", "OK")],
    )?;
    if r.status != 403 || !shows(&r.text(), "QMC9011") {
        return Err(format!("Deploy without its token answered {}", r.status));
    }
    let token = b.csrf(&format!("/qsmc/plan/{name}/deploy/step/3?t={VS2}"))?;
    let r = b.post(
        &format!("/qsmc/plan/{name}/deploy/step/3?t={VS2}"),
        &[("csrf", &token), ("p1", "on"), ("finish", "Deploy")],
    )?;
    if !r
        .header("location")
        .is_some_and(|l| l.contains("/confirm/plan-deploy"))
    {
        return Err("step 3's Deploy did not lead to its confirmation".into());
    }
    let delete_path = format!("/qsmc/confirm/plan-delete?p={name}");
    let _ = page(b, &delete_path)?;
    let r = b.post(&delete_path, &[("p", &name), ("ok", "OK")])?;
    if r.status != 403 || !shows(&r.text(), "QMC9011") {
        return Err(format!("Delete without its token answered {}", r.status));
    }
    std::thread::sleep(Duration::from_secs(2));
    let progress = page(b, &super::progress_path(&name, VS2))?;
    if !shows(&progress, qmcmsg::NO_DEPLOYMENT.id)
        || !sim::lock(&lab.fleet.systems[1]).parts.is_empty()
    {
        return Err("a refused or unconfirmed request deployed".into());
    }
    if !page(b, "/qsmc/plans")?.contains(&name) {
        return Err("a refused or unconfirmed request deleted the plan".into());
    }
    // The audit: capture, import, export, each step, delete.
    let bytes = export(b, &name)?;
    let token = b.csrf(&delete_path)?;
    let r = b.post(
        &delete_path,
        &[("csrf", &token), ("p", &name), ("ok", "OK")],
    )?;
    crate::suite::parts::expect(b, &r, qmcmsg::PLAN_DELETED.id)?;
    let t = import(b, &bytes)?;
    if !shows(&t, qmcmsg::PLAN_IMPORTED.id) {
        return Err(format!("the import: {}", message_of(&t)));
    }
    deploy(b, &name, VS2, &[1], false)?;
    let (_, steps) = ended(b, &name, VS2, Duration::from_secs(300))?;
    let logs = all_logs(&mut lab.c.guest)?;
    lab.c.guest.kill();
    let audit: Vec<&str> = logs
        .lines()
        .filter(|l| l.contains(&format!("audit: user {ADMIN} ")))
        .collect();
    let has = |what: &str| audit.iter().any(|l| l.contains(what));
    let mut lacking = Vec::new();
    for what in [
        format!("create system plan {name} from {VS1}"),
        format!("import system plan {name}"),
        format!("export system plan {name}"),
        format!("delete system plan {name}"),
        format!("deploy system plan {name} to {VS2}: begun"),
    ] {
        if !has(&what) {
            lacking.push(what);
        }
    }
    let made = steps.iter().filter(|r| r[1] == "Complete").count();
    let step_lines = audit
        .iter()
        .filter(|l| l.contains(&format!("from deployment of system plan {name}")))
        .count();
    if step_lines < made {
        lacking.push(format!("{made} steps, {step_lines} audited"));
    }
    if audit
        .iter()
        .any(|l| l.contains("password") || l.contains("BEGIN"))
    {
        return Err("an audit line holds a secret".into());
    }
    if !lacking.is_empty() {
        return Err(format!("not audited: {}", lacking.join("; ")));
    }
    Ok(format!(
        "{pairs} route, method and role pairs as the matrix says ({refused} Not authorized, \
         QMC9010); Deploy and Delete without their token refused (QMC9011), step 3's Deploy led \
         to its confirmation, nothing deployed or deleted until confirmed; the capture, export, \
         delete, import and each of {made} steps audited with {ADMIN} ({} lines)",
        audit.len()
    ))
}
