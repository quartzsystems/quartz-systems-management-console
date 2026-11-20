// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! Shut down or restart console (`console-shutdown.html`): the page is
//! its own confirmation, `qsweb`'s, with the choice and the message in
//! its form (the prototype has them before it: `docs/WEB.md`).

use qsweb::frame::{Ctx, Page};
use qsweb::msg::Message;
use qsweb::partials::{Cell, Col, List, input, list_table, mono, radio};

/// What the page shows.
#[derive(Clone, Debug, Default)]
pub struct Shutdown {
    /// The console's name.
    pub console: String,
    /// Its uptime, as the pages say it.
    pub uptime: String,
    /// Managed systems connected.
    pub systems: String,
    /// Terminal windows open.
    pub terminals: String,
    /// Scheduled operations in the next hour.
    pub scheduled: String,
    /// Restart chosen; power off otherwise.
    pub restart: bool,
    /// The message to signed-on users.
    pub message: String,
    /// The message rows.
    pub messages: Vec<Message>,
}

/// The page.
pub fn page(ctx: &Ctx, v: &Shutdown) -> Page {
    let list = List {
        cols: [
            "Console",
            "Uptime",
            "Connected systems",
            "Open terminal windows",
            "Scheduled operations in next hour",
        ]
        .iter()
        .map(|l| Col::plain(l))
        .collect(),
        rows: vec![vec![
            Cell::html(mono(&v.console)),
            Cell::text(&v.uptime),
            Cell::text(&v.systems),
            Cell::text(&v.terminals),
            Cell::text(&v.scheduled),
        ]],
        empty: String::new(),
        keep: Vec::new(),
    };
    let options = format!(
        "<p class=\"small grey\">{} Restart (reboot) &nbsp; {} Power off &nbsp; Message to signed-on users: {}</p>",
        radio("how", "reboot", v.restart),
        radio("how", "off", !v.restart),
        input("m", &v.message, 50)
    );
    let body = qsweb::pages::confirm_body(
        ctx,
        "Shutting down the console does not affect managed systems or partitions; they keep running. Terminal windows and scheduled operations stop until the console is back.",
        &list_table(&list),
        &options,
    );
    Page {
        title: "Shut down or restart console".into(),
        crumb: vec![
            "Console Management".into(),
            "Shut down or restart console".into(),
        ],
        h1: "Shut down or restart console".into(),
        messages: v.messages.clone(),
        body,
        help: "shutdown".into(),
        nav: Some(("Console Management", Some("/console/shutdown"))),
        ..Page::default()
    }
}
