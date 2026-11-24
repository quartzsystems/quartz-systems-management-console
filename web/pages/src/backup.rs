// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! Backup and restore (`console-backup.html`, `docs/BACKUP.md`): the
//! Backup tab, the Restore tab (the upload and what it holds, which the
//! prototype links and does not draw: `qsweb`'s properties, as the
//! users' forms are) and the History tab; the history of backups made,
//! kept in `backups.toml`.

use appliance::time::Zone;
use qsweb::frame::{Ctx, Page};
use qsweb::html::esc;
use qsweb::msg::Message;
use qsweb::partials::{
    Button, Cell, Col, List, Prop, PropGroup, buttons, checkbox, form, input, list_table, props,
    props_table, radio, status, tabs, upload_form,
};
use serde::{Deserialize, Serialize};

use crate::app::App;

/// One backup made.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Made {
    /// When, Unix seconds.
    pub at: u64,
    /// Where it went: `Download`.
    pub destination: String,
    /// Its size in bytes.
    pub size: u64,
    /// `Completed` or why not.
    pub status: String,
    /// Who made it.
    pub user: String,
}

/// `backups.toml`: the backups made, the newest last.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct History {
    /// The backups.
    #[serde(default, rename = "backup")]
    pub made: Vec<Made>,
}

/// How many backups the history keeps.
pub const KEPT: usize = 20;

impl state::Document for History {
    const FILE: &'static str = "backups.toml";
    const VERSION: u32 = 1;
    const MIGRATIONS: &'static [state::Migration] = &[];

    fn validate(&self) -> Result<(), String> {
        if self.made.len() > KEPT {
            return Err(format!("more than {KEPT} backups in the history"));
        }
        Ok(())
    }
}

/// The history.
pub fn history(app: &App) -> History {
    app.store()
        .load::<History>()
        .ok()
        .flatten()
        .unwrap_or_default()
}

/// The landing page's line: the last backup, or none.
pub fn last(app: &App, zone: Zone, date_format: &str) -> String {
    history(app).made.last().map_or_else(
        || "No backup made yet".to_string(),
        |m| {
            let at = i64::try_from(m.at).unwrap_or(0);
            format!(
                "Last backup {} {} to {}",
                crate::fmt::date(at, zone, date_format),
                crate::fmt::time(at, zone, "24-hour", false),
                m.destination.to_lowercase()
            )
        },
    )
}

/// One backup as the lists show it.
#[derive(Clone, Debug, Default)]
pub struct Row {
    /// When, formatted.
    pub date: String,
    /// Where it went.
    pub destination: String,
    /// `3.1 MB`.
    pub size: String,
    /// The status word and whether it is good.
    pub status: (String, Option<bool>),
}

/// What the Backup tab shows.
#[derive(Clone, Debug, Default)]
// One flag a checkbox or the radio of the page: the form's own fields.
#[allow(clippy::struct_excessive_bools)]
pub struct BackupTab {
    /// Console data chosen.
    pub data: bool,
    /// Reference code and event history chosen.
    pub history: bool,
    /// Download chosen; a remote host otherwise.
    pub download: bool,
    /// The remote host.
    pub host: String,
    /// Its path.
    pub path: String,
    /// Its user.
    pub user: String,
    /// The last backups, newest first.
    pub last: Vec<Row>,
    /// The message rows.
    pub messages: Vec<Message>,
}

/// What a backup uploaded holds, for the Restore tab.
#[derive(Clone, Debug, Default)]
pub struct Holds {
    /// The console it was made on.
    pub console: String,
    /// Its release.
    pub release: String,
    /// Its build.
    pub build: String,
    /// When it was made, formatted.
    pub made: String,
    /// How many files.
    pub files: usize,
}

fn tab_bar(ctx: &Ctx, current: usize) -> String {
    let here = ctx.url("/console/backup");
    tabs(
        &[
            ("Backup", here.clone()),
            ("Restore", format!("{here}?tab=restore")),
            ("History", format!("{here}?tab=history")),
        ],
        current,
    )
}

fn frame(crumb: Option<&str>, body: String, messages: Vec<Message>) -> Page {
    let mut c = vec![
        "Console Management".to_string(),
        "Backup and restore".into(),
    ];
    c.extend(crumb.map(String::from));
    Page {
        title: "Backup and restore".into(),
        crumb: c,
        h1: "Backup and restore".into(),
        messages,
        body,
        help: "backup".into(),
        nav: Some(("Console Management", Some("/console/backup"))),
        ..Page::default()
    }
}

fn rows_table(rows: &[Row], empty: &str) -> String {
    list_table(&List {
        cols: vec![
            Col::plain("Date"),
            Col::plain("Destination"),
            Col::plain("Size"),
            Col::plain("Status"),
        ],
        rows: rows
            .iter()
            .map(|r| {
                vec![
                    Cell::mono(&r.date),
                    Cell::mono(&r.destination),
                    Cell::num(&r.size),
                    Cell::html(status(&r.status.0, r.status.1)),
                ]
            })
            .collect(),
        empty: empty.into(),
        keep: Vec::new(),
    })
}

/// The Backup tab.
pub fn backup_page(ctx: &Ctx, v: &BackupTab) -> Page {
    let what = PropGroup {
        title: Some("What".into()),
        rows: vec![
            Prop::new(
                "Console data",
                format!(
                    "{} Managed system connections, partition profiles, system plans, users, scheduled operations, settings",
                    checkbox("d", v.data)
                ),
            ),
            Prop::new("Reference code and event history", checkbox("h", v.history)),
        ],
    };
    let place = PropGroup {
        title: Some("Where".into()),
        rows: vec![
            Prop::new(
                "Destination",
                format!(
                    "{} Download as file<br>{} Remote host (SFTP): {} path {} user {}",
                    radio("w", "file", v.download),
                    radio("w", "remote", !v.download),
                    input("host", &v.host, 20),
                    input("path", &v.path, 16),
                    input("u", &v.user, 10)
                ),
            ),
            Prop::new(
                "Encrypt with passphrase",
                "<input type=\"password\" name=\"pp\" size=\"20\">".into(),
            ),
        ],
    };
    let body = format!(
        "{}{}<h2>Last backups</h2>{}",
        tab_bar(ctx, 0),
        props(
            ctx,
            &[what, place],
            &[
                Button::new("back_up_now", "Back up now"),
                Button::new("schedule", "Schedule..."),
            ],
        ),
        rows_table(&v.last, "No backup has been made.")
    );
    frame(None, body, v.messages.clone())
}

/// The History tab: every backup the history keeps.
pub fn history_page(ctx: &Ctx, rows: &[Row], messages: Vec<Message>) -> Page {
    let body = format!(
        "{}{}<p class=\"small grey\">The last {KEPT} backups are kept in the history.</p>",
        tab_bar(ctx, 2),
        rows_table(rows, "No backup has been made.")
    );
    frame(Some("History"), body, messages)
}

/// The Restore tab: the upload, then what the backup holds.
pub fn restore_page(ctx: &Ctx, holds: Option<&Holds>, messages: Vec<Message>) -> Page {
    let upload = PropGroup {
        title: Some("Restore from a backup file".into()),
        rows: vec![
            Prop::new(
                "Backup file",
                "<input type=\"file\" name=\"f\" size=\"40\">".into(),
            )
            .required(),
            Prop::new(
                "Passphrase",
                "<input type=\"password\" name=\"pp\" size=\"20\">".into(),
            )
            .required(),
        ],
    };
    let mut body = format!(
        "{}{}",
        tab_bar(ctx, 1),
        upload_form(
            ctx,
            "",
            &format!(
                "{}{}",
                props_table(ctx.product, &[upload], true),
                buttons(&[Button::new("open", "Open")])
            )
        )
    );
    if let Some(h) = holds {
        let g = PropGroup {
            title: Some("The backup holds".into()),
            rows: vec![
                Prop::new("Console", esc(&h.console)).mono(),
                Prop::new("Release", esc(&h.release)),
                Prop::new("Build", esc(&h.build)).mono(),
                Prop::new("Made", esc(&h.made)),
                Prop::new("Files", h.files.to_string()),
            ],
        };
        body.push_str(&form(
            ctx,
            &ctx.url("/confirm/restore"),
            &format!(
                "{}{}",
                props_table(ctx.product, &[g], true),
                buttons(&[Button::new("restore", "Restore...")])
            ),
        ));
    }
    body.push_str("<p class=\"small grey\">A restore replaces the console's state, users, network settings, identity and web certificate with the backup's. It is applied when the console restarts, and taken back if the restored console does not start its web server.</p>");
    frame(Some("Restore"), body, messages)
}

/// `3.1 MB`: a size as the lists show it.
pub fn size(bytes: u64) -> String {
    const K: u64 = 1024;
    #[allow(clippy::cast_precision_loss, reason = "one decimal shown")]
    let f = |d: u64| bytes as f64 / d as f64;
    if bytes >= K * K {
        format!("{:.1} MB", f(K * K))
    } else if bytes >= K {
        format!("{:.1} KB", f(K))
    } else {
        format!("{bytes} bytes")
    }
}
