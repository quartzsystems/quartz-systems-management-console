// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! Users and roles (`console-users.html`, `console-roles.html`): the
//! web users with their actions, the role matrix, and the forms behind
//! Properties, Change password and Create user. The prototype links
//! those to the list itself; the forms are `qsweb`'s properties under
//! the list (`docs/OPEN_QUESTIONS.md`, item 23).

use qsweb::frame::{Ctx, Page};
use qsweb::html::esc;
use qsweb::msg::Message;
use qsweb::partials::{
    Button, Cell, Col, List, Prop, PropGroup, buttons, checkbox, form, input, list_table, select,
    status, tabs,
};
use webusers::{Matrix, ROLES, Role, TASKS};

/// A user as the list shows it.
#[derive(Clone, Debug, Default)]
pub struct Row {
    /// The ID.
    pub id: String,
    /// Its description.
    pub description: String,
    /// Its role's name.
    pub role: String,
    /// When it last signed on, formatted; empty never.
    pub last: String,
    /// From where.
    pub from: String,
    /// Whether it is enabled.
    pub enabled: bool,
}

/// The form under the list.
#[derive(Clone, Debug)]
pub enum Form {
    /// None: the list alone.
    None,
    /// A user's properties.
    Properties(Row),
    /// A user's new password.
    Password(String),
    /// A new user.
    Create,
}

/// What the Users tab shows.
#[derive(Clone, Debug)]
pub struct Users {
    /// The users, sorted as asked.
    pub rows: Vec<Row>,
    /// The form, if one.
    pub form: Form,
    /// The message rows.
    pub messages: Vec<Message>,
}

fn tab_bar(ctx: &Ctx, current: usize) -> String {
    tabs(
        &[
            ("Users", ctx.url("/console/users")),
            ("Roles", ctx.url("/console/roles")),
        ],
        current,
    )
}

fn page(crumb: Vec<String>, body: String, messages: Vec<Message>, help: &str) -> Page {
    Page {
        title: "Users and roles".into(),
        crumb,
        h1: "Users and roles".into(),
        messages,
        body,
        help: help.into(),
        nav: Some(("Console Management", Some("/console/users"))),
        ..Page::default()
    }
}

fn rules() -> String {
    format!(
        "<p class=\"small grey\">{}</p>",
        esc(&[
            "A user ID is 1 to 32 letters, digits, periods, underscores and hyphens, starting with a letter.",
            users::RULES[1],
            users::RULES[2],
            users::RULES[3],
        ]
        .join(" "))
    )
}

fn role_select(chosen: &str) -> String {
    let names: Vec<&str> = ROLES.iter().map(|r| r.label()).collect();
    select("role", &names, chosen)
}

fn form_html(ctx: &Ctx, f: &Form) -> String {
    let route = ctx.url("/console/users");
    match f {
        Form::None => form(ctx, "", &buttons(&[Button::bare("Create user...")])),
        Form::Properties(r) => {
            let g = PropGroup {
                title: Some(format!("Properties of {}", r.id)),
                rows: vec![
                    Prop::new("User ID", esc(&r.id)).mono(),
                    Prop::new("Description", input("description", &r.description, 40)),
                    Prop::new("Role", role_select(&r.role)),
                    Prop::new("Enabled", checkbox("enabled", r.enabled)),
                ],
            };
            form(
                ctx,
                &route,
                &format!(
                    "<input type=\"hidden\" name=\"id\" value=\"{}\">{}{}",
                    esc(&r.id),
                    qsweb::partials::props_table(ctx.product, &[g], true),
                    buttons(&[
                        Button::new("properties", "OK"),
                        Button::new("cancel", "Cancel")
                    ])
                ),
            )
        }
        Form::Password(id) => {
            let g = PropGroup {
                title: Some(format!("New password for {id}")),
                rows: vec![
                    Prop::new(
                        "Password",
                        "<input type=\"password\" name=\"password\" size=\"30\">".into(),
                    )
                    .required(),
                    Prop::new(
                        "Again",
                        "<input type=\"password\" name=\"again\" size=\"30\">".into(),
                    )
                    .required(),
                ],
            };
            format!(
                "{}{}",
                form(
                    ctx,
                    &route,
                    &format!(
                        "<input type=\"hidden\" name=\"id\" value=\"{}\">{}{}",
                        esc(id),
                        qsweb::partials::props_table(ctx.product, &[g], true),
                        buttons(&[
                            Button::new("new-password", "OK"),
                            Button::new("cancel", "Cancel")
                        ])
                    )
                ),
                rules()
            )
        }
        Form::Create => {
            let g = PropGroup {
                title: Some("Create user".into()),
                rows: vec![
                    Prop::new("User ID", input("id", "", 32)).required(),
                    Prop::new("Description", input("description", "", 40)),
                    Prop::new("Role", role_select("Viewer")),
                    Prop::new(
                        "Password",
                        "<input type=\"password\" name=\"password\" size=\"30\">".into(),
                    )
                    .required(),
                    Prop::new(
                        "Again",
                        "<input type=\"password\" name=\"again\" size=\"30\">".into(),
                    )
                    .required(),
                ],
            };
            format!(
                "{}{}",
                form(
                    ctx,
                    &route,
                    &format!(
                        "{}{}",
                        qsweb::partials::props_table(ctx.product, &[g], true),
                        buttons(&[Button::new("create", "OK"), Button::new("cancel", "Cancel")])
                    )
                ),
                rules()
            )
        }
    }
}

/// The Users tab.
pub fn users_page(ctx: &Ctx, v: &Users) -> Page {
    let here = ctx.url("/console/users");
    let list = List {
        cols: vec![
            Col::plain("Actions"),
            Col::sorted("User ID", "u"),
            Col::plain("Description"),
            Col::sorted("Role", "r"),
            Col::sorted("Last login", "l"),
            Col::plain("From"),
            Col::plain("Status"),
        ],
        rows: v
            .rows
            .iter()
            .map(|r| {
                let q = qsweb::html::query(&[("edit", &r.id)]);
                let p = qsweb::html::query(&[("password", &r.id)]);
                let d = qsweb::html::query(&[("u", &r.id)]);
                vec![
                    Cell::act(format!(
                        "<a href=\"{here}{q}\">Properties</a> | <a href=\"{here}{p}\">Change password</a> | <a href=\"{}{d}\">Delete</a>",
                        ctx.url("/confirm/user-delete")
                    )),
                    Cell::mono(&r.id),
                    Cell::text(&r.description),
                    Cell::mono(&r.role),
                    Cell::text(&r.last),
                    Cell::text(&r.from),
                    Cell::html(if r.enabled {
                        status("*ENABLED", Some(true))
                    } else {
                        status("*DISABLED", Some(false))
                    }),
                ]
            })
            .collect(),
        empty: "There are no web users. Make an Administrator at the local console (option 4).".into(),
        keep: Vec::new(),
    };
    let body = format!(
        "{}{}{}",
        tab_bar(ctx, 0),
        list_table(&list),
        form_html(ctx, &v.form)
    );
    page(
        vec!["Console Management".into(), "Users and roles".into()],
        body,
        v.messages.clone(),
        "users",
    )
}

/// The Roles tab.
pub fn roles_page(ctx: &Ctx, m: &Matrix, messages: Vec<Message>) -> Page {
    let list = List {
        cols: std::iter::once(Col::plain("Task"))
            .chain(ROLES.iter().map(|r| Col::plain(r.label())))
            .collect(),
        rows: TASKS
            .iter()
            .enumerate()
            .map(|(t, task)| {
                std::iter::once(Cell::text(task.label()))
                    .chain(ROLES.iter().enumerate().map(|(r, role)| Cell {
                        class: Some("radio"),
                        html: checkbox(&format!("r{t}_{r}"), m.allows(*role, *task)),
                    }))
                    .collect()
            })
            .collect(),
        empty: String::new(),
        keep: Vec::new(),
    };
    let inner = format!(
        "{}{}",
        list_table(&list),
        buttons(&[
            Button::new("apply", "Apply"),
            Button::new("ok", "OK"),
            Button::new("cancel", "Cancel"),
        ])
    );
    let body = format!(
        "{}{}<p class=\"small grey\">Administrator cannot be changed. Changes apply at the user's next sign-on.</p>",
        tab_bar(ctx, 1),
        form(ctx, "", &inner)
    );
    page(
        vec![
            "Console Management".into(),
            "Users and roles".into(),
            "Roles".into(),
        ],
        body,
        messages,
        "roles",
    )
}

/// The matrix a Roles form asks for: `r<task>_<role>` checked.
pub fn matrix_of(form: &qsweb::http::Form) -> Matrix {
    let mut m = Matrix::default();
    for (t, task) in TASKS.iter().enumerate() {
        for (r, role) in ROLES.iter().enumerate() {
            if *role != Role::Administrator {
                m.set(*role, *task, form.get(&format!("r{t}_{r}")).is_some());
            }
        }
    }
    m
}
