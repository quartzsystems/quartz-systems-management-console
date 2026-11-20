// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! Language, locale and session (`console-locale.html`): the formats
//! the pages show dates, times and numbers in, and the session
//! timeouts, all `web.toml`'s. English (US) is the only language; the
//! display offset is Date and time's, where the prototype has it.

use qsweb::frame::{Ctx, Page};
use qsweb::msg::Message;
use qsweb::partials::{Button, Prop, PropGroup, checkbox, input, props, select};

use crate::app::settings::{DATE_FORMATS, NUMBER_FORMATS, TIME_FORMATS};

/// The languages the page offers.
pub const LANGUAGES: [&str; 4] = ["English (US)", "English (UK)", "German", "French"];
/// The language of V1.
pub const ENGLISH: &str = LANGUAGES[0];

/// What the page shows.
#[derive(Clone, Debug, Default)]
pub struct Locale {
    /// The language chosen.
    pub language: String,
    /// The date format.
    pub date: String,
    /// The time format.
    pub time: String,
    /// The number format.
    pub number: String,
    /// Minutes idle.
    pub idle: String,
    /// Hours at most.
    pub max: String,
    /// Keep terminal windows open.
    pub terminals: bool,
    /// The message rows.
    pub messages: Vec<Message>,
}

/// The page.
pub fn page(ctx: &Ctx, v: &Locale) -> Page {
    let locale = PropGroup {
        title: Some("Language and locale".into()),
        rows: vec![
            Prop::new("Language", select("l", &LANGUAGES, &v.language)),
            Prop::new("Date format", select("df", &DATE_FORMATS, &v.date)),
            Prop::new("Time format", select("tf", &TIME_FORMATS, &v.time)),
            Prop::new("Number format", select("nf", &NUMBER_FORMATS, &v.number)),
        ],
    };
    let session = PropGroup {
        title: Some("Session".into()),
        rows: vec![
            Prop::new(
                "Idle timeout",
                format!("{} minutes", input("to", &v.idle, 3)),
            ),
            Prop::new(
                "Maximum session length",
                format!("{} hours", input("max", &v.max, 3)),
            ),
            Prop::new(
                "Terminal windows",
                format!(
                    "{} Keep terminal window sessions open after the web session times out (reconnect on next sign-on)",
                    checkbox("tw", v.terminals)
                ),
            ),
        ],
    };
    let body = props(
        ctx,
        &[locale, session],
        &[
            Button::new("apply", "Apply"),
            Button::new("ok", "OK"),
            Button::new("cancel", "Cancel"),
        ],
    );
    Page {
        title: "Language, locale and session".into(),
        crumb: vec!["Console Management".into(), "Language and locale".into()],
        h1: "Language, locale and session".into(),
        messages: v.messages.clone(),
        body,
        help: "locale".into(),
        // The prototype draws it with Console Management closed: the
        // tree has no item for it.
        nav: None,
        ..Page::default()
    }
}
