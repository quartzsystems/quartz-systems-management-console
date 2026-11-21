// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! Suite 2: the pages as served. The stylesheet is `qsweb`'s byte for
//! byte; every link of every page reached from Welcome answers; port 80
//! answers only with the redirect. (That each page equals its prototype
//! file is `cargo test -p webpages --test golden`.) Suite 4: every
//! route, for each role, is the page or Not authorized as the matrix
//! says.

use std::collections::{BTreeSet, VecDeque};

use webusers::{Matrix, ROLES, Role};

use super::{ADMIN, PASSWORD, follow, shows, signed_on, start, submit};
use crate::qemu::session::Session;
use crate::web::Browser;

/// The links of a page that lead into the product.
fn links(page: &str) -> Vec<String> {
    qsweb::golden::tokens(page)
        .into_iter()
        .filter_map(|t| match t {
            qsweb::golden::Tok::Open(n, a) if n == "a" || n == "link" => a.get("href").cloned(),
            _ => None,
        })
        .map(|h| h.split('#').next().unwrap_or("").to_string())
        .filter(|h| h.starts_with("/qsmc/"))
        .collect()
}

/// The suite of the served pages.
pub fn pages(run: &super::Run<'_>) -> Result<String, String> {
    let mut c = start(run, "web-pages")?;
    let b = &mut c.browser;
    let css = b.get(webd::product::CSS_PATH)?;
    if css.status != 200 || css.body != qsweb::CSS {
        return Err(format!(
            "the stylesheet is not qsweb's: {} bytes, status {}",
            css.body.len(),
            css.status
        ));
    }
    let redirect = b.plain("/qsmc/welcome")?;
    if redirect.status != 301
        || redirect.header("location") != Some("https://10.0.2.15/qsmc/welcome")
    {
        return Err(format!(
            "port 80 answered {} to {:?}",
            redirect.status,
            redirect.header("location")
        ));
    }
    signed_on(b, ADMIN, PASSWORD)?;
    let mut seen = BTreeSet::new();
    let mut queue = VecDeque::from(["/qsmc/welcome".to_string()]);
    let mut pages = 0;
    let mut redirects = 0;
    while let Some(path) = queue.pop_front() {
        // Signing off would end the crawl's session.
        if !seen.insert(path.clone()) || path.starts_with("/qsmc/signoff") {
            continue;
        }
        let a = b.get(&path)?;
        // `/qsmc/` (the help window's Close window) leads to Welcome.
        if a.status == 303 {
            match a.header("location") {
                Some(l) if l.starts_with("/qsmc/") => {
                    redirects += 1;
                    queue.push_back(l.to_string());
                    continue;
                }
                l => return Err(format!("{path} answered 303 to {l:?}")),
            }
        }
        if a.status != 200 {
            return Err(format!("{path} answered {}", a.status));
        }
        pages += 1;
        // No page carries a script or an inline handler.
        let text = a.text().to_ascii_lowercase();
        if text.contains("<script") || text.contains("javascript:") || text.contains(" onclick=") {
            return Err(format!("{path} carries a script"));
        }
        if a.header("content-type")
            .is_some_and(|t| t.starts_with("text/html"))
        {
            for l in links(&a.text()) {
                if !seen.contains(&l) {
                    queue.push_back(l);
                }
            }
        }
        if pages > 400 {
            return Err("more than 400 pages: the crawl does not end".into());
        }
    }
    c.guest.kill();
    Ok(format!(
        "the stylesheet is qsweb's, {} bytes; {pages} pages and files reached from Welcome \
         each answered 200 ({redirects} redirect(s) to a page of the console), none with a \
         script or an inline handler; port 80 answers 301 to https://10.0.2.15/qsmc/welcome",
        qsweb::CSS.len()
    ))
}

/// The routes a role is checked on: the tree's, Roles, and a
/// confirmation of each task's.
fn routes() -> Vec<String> {
    let mut r: Vec<String> = webd::product::NAV
        .iter()
        .flat_map(|g| g.items.iter().map(|(_, r)| (*r).to_string()))
        .collect();
    r.extend(
        [
            "/console/roles",
            "/confirm/user-delete?u=admin",
            "/confirm/cert-new",
        ]
        .map(String::from),
    );
    r
}

/// The password of each role's user: none of their IDs is in it.
const ROLE_PASSWORD: &str = "Matrix-check-pass-4";

fn make_user(b: &mut Browser, id: &str, role: Role) -> Result<(), String> {
    let pw = ROLE_PASSWORD;
    let r = submit(
        b,
        "/qsmc/console/users",
        &[
            ("id", id),
            ("description", "suite"),
            ("role", role.label()),
            ("password", pw),
            ("again", pw),
            ("create", "OK"),
        ],
    )?;
    let page = follow(b, &r)?.text();
    if shows(&page, "QMC8001") {
        Ok(())
    } else {
        Err(format!("{id} was not made: {}", super::message_of(&page)))
    }
}

/// The suite of the role matrix.
pub fn roles(run: &super::Run<'_>) -> Result<String, String> {
    let mut c = start(run, "web-roles")?;
    signed_on(&mut c.browser, ADMIN, PASSWORD)?;
    let matrix = Matrix::default();
    let mut checked = 0;
    for role in ROLES {
        let id = role.label().to_lowercase();
        let mut b = if role == Role::Administrator {
            c.browser.clone_session()
        } else {
            make_user(&mut c.browser, &format!("r{id}"), role)?;
            let mut b = c.browser.fresh();
            signed_on(&mut b, &format!("r{id}"), ROLE_PASSWORD)?;
            b
        };
        for path in routes() {
            let route = path
                .strip_prefix(webd::product::PREFIX)
                .unwrap_or(&path)
                .split('?')
                .next()
                .unwrap_or("")
                .to_string();
            let task = webpages::app::routes::task_of(&route)
                .ok_or_else(|| format!("{route} is missing from the matrix"))?;
            let a = b.get(&format!("/qsmc{path}"))?;
            let allowed = matrix.allows(role, task);
            let ok = if allowed {
                a.status == 200
            } else {
                a.status == 403 && shows(&a.text(), "QMC9010")
            };
            if !ok {
                return Err(format!(
                    "{} on {path}: {} (the matrix says {})",
                    role.label(),
                    a.status,
                    if allowed { "allowed" } else { "not authorized" }
                ));
            }
            checked += 1;
        }
    }
    let n = routes().len();
    finish(&mut c.guest);
    Ok(format!(
        "{checked} route and role pairs as the matrix says: {n} routes for each of the four \
         roles, every route naming its task"
    ))
}

fn finish(guest: &mut Session) {
    let _ = super::super::base::power_off(guest);
}
