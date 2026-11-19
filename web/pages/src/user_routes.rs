// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! Serving Users and roles: the list and its forms, the role matrix,
//! and the delete confirmation. Every change is audited; the last
//! enabled Administrator is kept (`webusers::list`).

use qsweb::frame::Page;
use qsweb::http::Response;
use qsweb::msg::{Kind, Message};
use qsweb::partials::{Cell, Col, List, list_table};
use webusers::Role;

use crate::app::routes::Rq;
use crate::users::{Form, Row, Users, matrix_of, roles_page, users_page};

fn rows(rq: &Rq<'_>, sort: &str) -> Vec<Row> {
    let app = rq.app;
    let s = app.settings();
    let zone = app.host().zone();
    let mut list = app.users().users.clone();
    match sort {
        "r" => list.sort_by_key(|u| u.role),
        "l" => list.sort_by_key(|u| std::cmp::Reverse(u.last_sign_on)),
        "u" => list.sort_by(|a, b| a.id.cmp(&b.id)),
        _ => {}
    }
    list.into_iter()
        .map(|u| {
            let at = i64::try_from(u.last_sign_on).unwrap_or(0);
            Row {
                last: if u.last_sign_on == 0 {
                    String::new()
                } else {
                    format!(
                        "{} {}",
                        crate::fmt::date(at, zone, &s.date_format),
                        crate::fmt::time(at, zone, &s.time_format, false)
                    )
                },
                id: u.id,
                description: u.description,
                role: u.role.label().into(),
                from: u.last_from,
                enabled: u.enabled,
            }
        })
        .collect()
}

fn show(rq: &Rq<'_>, form: Form, messages: Vec<Message>) -> Response {
    let sort = rq.req.query().get("sort").unwrap_or("").to_string();
    let v = Users {
        rows: rows(rq, &sort),
        form,
        messages,
    };
    rq.page(users_page(&rq.ctx, &v))
}

fn refused(text: &str) -> Message {
    if text == webusers::list::LAST_ADMIN {
        qmcmsg::message(Kind::Error, &qmcmsg::LAST_ADMIN, &[])
    } else {
        qmcmsg::message(Kind::Error, &qmcmsg::NOT_VALID, &[text])
    }
}

fn passwords(f: &qsweb::http::Form) -> Result<String, String> {
    let p = f.get("password").unwrap_or("");
    if p != f.get("again").unwrap_or("") {
        return Err("The two passwords are not the same.".into());
    }
    Ok(p.to_string())
}

/// The outcome of a change, for the audit.
pub fn outcome(done: &Result<(), String>) -> &str {
    match done {
        Ok(()) => "done",
        Err(e) => e,
    }
}

fn post(rq: &Rq<'_>) -> Response {
    let f = rq.req.form();
    let app = rq.app;
    let now = app.console.now();
    let id = f.get("id").unwrap_or("").trim().to_lowercase();
    let role = Role::parse(f.get("role").unwrap_or("")).unwrap_or(Role::Viewer);
    let description = f.get("description").unwrap_or("").to_string();
    if f.get("cancel").is_some() {
        return Response::redirect(&rq.ctx.url("/console/users"));
    }
    if f.get("create").is_some() {
        let done = passwords(&f)
            .and_then(|p| app.change_users(|u| u.add(&id, &description, role, &p, now)));
        rq.audit(
            &format!("create user {id} role {}", role.label()),
            outcome(&done),
        );
        return match done {
            Ok(()) => rq.done(
                "/console/users",
                qmcmsg::message(Kind::Done, &qmcmsg::USER_CREATED, &[&id, role.label()]),
            ),
            Err(e) => show(rq, Form::Create, vec![refused(&e)]),
        };
    }
    if f.get("new-password").is_some() {
        let done = passwords(&f).and_then(|p| app.change_users(|u| u.set_password(&id, &p, now)));
        rq.audit(&format!("change password of {id}"), outcome(&done));
        return match done {
            Ok(()) => {
                app.clear_failures(&id);
                rq.done(
                    "/console/users",
                    qmcmsg::message(Kind::Done, &qmcmsg::PASSWORD_CHANGED, &[&id]),
                )
            }
            Err(e) => show(rq, Form::Password(id), vec![refused(&e)]),
        };
    }
    if f.get("properties").is_some() {
        let enabled = f.get("enabled").is_some();
        let done = app.change_users(|u| {
            u.set_properties(&id, &description, role)?;
            u.set_enabled(&id, enabled)
        });
        rq.audit(
            &format!("change user {id}: role {}, enabled {enabled}", role.label()),
            outcome(&done),
        );
        let def = if enabled {
            &qmcmsg::USER_CHANGED
        } else {
            &qmcmsg::USER_DISABLED
        };
        return match done {
            Ok(()) => rq.done("/console/users", qmcmsg::message(Kind::Done, def, &[&id])),
            Err(e) => show(rq, Form::None, vec![refused(&e)]),
        };
    }
    // The bare Create user... button.
    show(rq, Form::Create, Vec::new())
}

/// `/console/users`.
pub fn users(rq: &Rq<'_>) -> Response {
    if rq.post() {
        return post(rq);
    }
    let q = rq.req.query();
    let form = if let Some(id) = q.get("edit") {
        rq.app.users().get(id).map_or(Form::None, |u| {
            Form::Properties(Row {
                id: u.id.clone(),
                description: u.description.clone(),
                role: u.role.label().into(),
                last: String::new(),
                from: String::new(),
                enabled: u.enabled,
            })
        })
    } else if let Some(id) = q.get("password") {
        Form::Password(id.to_string())
    } else if q.get("create").is_some() {
        Form::Create
    } else {
        Form::None
    };
    show(rq, form, Vec::new())
}

/// `/console/roles`.
pub fn roles(rq: &Rq<'_>) -> Response {
    let app = rq.app;
    if rq.post() {
        let f = rq.req.form();
        if f.get("cancel").is_some() {
            return Response::redirect(&rq.ctx.url("/console/users"));
        }
        let m = matrix_of(&f);
        let done = app.change_users(|u| {
            u.matrix = m;
            Ok(())
        });
        rq.audit("change the role matrix", outcome(&done));
        let to = if f.get("ok").is_some() {
            "/console/users"
        } else {
            "/console/roles"
        };
        return match done {
            Ok(()) => rq.done(to, qmcmsg::message(Kind::Done, &qmcmsg::ROLES_CHANGED, &[])),
            Err(e) => {
                let m = app.users().matrix.clone();
                rq.page(roles_page(&rq.ctx, &m, vec![refused(&e)]))
            }
        };
    }
    let m = app.users().matrix.clone();
    rq.page(roles_page(&rq.ctx, &m, Vec::new()))
}

/// `/confirm/user-delete?u=<id>`: the user to delete, then OK.
pub fn delete(rq: &Rq<'_>) -> Response {
    let app = rq.app;
    let f = rq.req.form();
    let q = rq.req.query();
    let id = f.get("u").or_else(|| q.get("u")).unwrap_or("").to_string();
    if rq.post() {
        if f.get("ok").is_none() {
            return Response::redirect(&rq.ctx.url("/console/users"));
        }
        let done = app.change_users(|u| u.remove(&id));
        rq.audit(&format!("delete user {id}"), outcome(&done));
        return match done {
            Ok(()) => rq.done(
                "/console/users",
                qmcmsg::message(Kind::Done, &qmcmsg::USER_DELETED, &[&id]),
            ),
            Err(e) => rq.done("/console/users", refused(&e)),
        };
    }
    let Some(u) = app.users().get(&id).cloned() else {
        return crate::app::routes::error(rq, 404, &qmcmsg::NOT_FOUND, &[&id]);
    };
    let list = List {
        cols: ["User ID", "Description", "Role", "Status"]
            .iter()
            .map(|l| Col::plain(l))
            .collect(),
        rows: vec![vec![
            Cell::mono(&u.id),
            Cell::text(&u.description),
            Cell::mono(u.role.label()),
            Cell::text(if u.enabled { "*ENABLED" } else { "*DISABLED" }),
        ]],
        empty: String::new(),
        keep: Vec::new(),
    };
    let table = format!(
        "{}<p class=\"small grey\">Any session of the user ends. The last enabled Administrator cannot be deleted.</p>",
        list_table(&list)
    );
    let options = format!(
        "<input type=\"hidden\" name=\"u\" value=\"{}\">",
        qsweb::html::esc(&u.id)
    );
    let body = qsweb::pages::confirm_body(
        &rq.ctx,
        "Deleting a user removes it from the console's web users. Press OK to delete the user listed below, or Cancel to return without deleting.",
        &table,
        &options,
    );
    rq.page(Page {
        title: "Confirm Delete User".into(),
        crumb: vec![
            "Console Management".into(),
            "Users and roles".into(),
            "Confirm Delete User".into(),
        ],
        h1: "Confirm Delete User".into(),
        body,
        help: "users".into(),
        nav: Some(("Console Management", Some("/console/users"))),
        ..Page::default()
    })
}
