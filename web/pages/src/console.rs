// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! Console settings (`console.html`): each setting of the console with
//! its current value.

use qsweb::frame::{Ctx, Page};
use qsweb::msg::Message;
use qsweb::partials::{Cell, Col, List, link, list_table};

/// The settings' rows: route, label. The values are the view's.
pub const ROWS: [(&str, &str); 10] = [
    ("/console/users", "Users and roles"),
    ("/console/network", "Network settings"),
    ("/console/datetime", "Date and time"),
    ("/console/certs", "Certificates"),
    ("/console/backup", "Backup and restore"),
    ("/updates/code", "Updates"),
    ("/console/shutdown", "Shut down or restart console"),
    ("/console/locale", "Language and locale"),
    ("/console/locale", "Session timeout"),
    ("/console/version", "Console version"),
];

/// What Console settings shows.
#[derive(Clone, Debug, Default)]
pub struct Settings {
    /// The console's name.
    pub console: String,
    /// A value for each of [`ROWS`].
    pub values: Vec<String>,
    /// The message rows.
    pub messages: Vec<Message>,
}

/// The page.
pub fn page(ctx: &Ctx, v: &Settings) -> Page {
    let list = List {
        cols: vec![
            Col::plain("Setting"),
            Col::plain("Current value"),
            Col::plain(""),
        ],
        rows: ROWS
            .iter()
            .zip(&v.values)
            .map(|((route, label), value)| {
                vec![
                    Cell::html(link(&ctx.url(route), label)),
                    Cell::text(value),
                    Cell::text(""),
                ]
            })
            .collect(),
        empty: String::new(),
        keep: Vec::new(),
    };
    Page {
        title: "Console settings".into(),
        crumb: vec!["Console Management".into()],
        h1: "Console settings".into(),
        desc: Some(format!("Settings of this console, {}.", v.console)),
        messages: v.messages.clone(),
        body: list_table(&list),
        help: "console".into(),
        nav: Some(("Console Management", Some("/console"))),
        ..Page::default()
    }
}
