// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! Open terminal window (`terminal-config.html`): the partition, the
//! screen's size, code page and colours, the control panel strip, and
//! Take over, which takes the console from another console (not from
//! another window of this one: windows share it).

use std::fmt::Write as _;

use qsweb::frame::{Ctx, Page};
use qsweb::html::esc;
use qsweb::msg::Message;
use qsweb::partials::{Button, Prop, PropGroup, buttons, props_table};

/// A partition the window may open.
#[derive(Clone, Debug, Default)]
pub struct Choice {
    /// `QSDEV01 (ID 1) on QSHQVS1`, with its code when it waits.
    pub label: String,
    /// The managed system.
    pub host: String,
    /// The partition's ID; 0 for a standalone system.
    pub id: u32,
}

/// What the page shows.
#[derive(Clone, Debug, Default)]
#[allow(clippy::struct_excessive_bools, reason = "the page's switches")]
pub struct Config {
    /// The partitions.
    pub choices: Vec<Choice>,
    /// The one chosen.
    pub chosen: String,
    /// 27 x 132 rather than 24 x 80.
    pub wide: bool,
    /// Black on white.
    pub bw: bool,
    /// The control panel strip.
    pub strip: bool,
    /// Take over.
    pub take_over: bool,
    /// The message rows.
    pub messages: Vec<Message>,
}

/// The code pages offered.
pub const CODE_PAGES: [&str; 3] = ["37 - US English", "285 - UK English", "500 - International"];

/// The page.
pub fn page(ctx: &Ctx, v: &Config) -> Page {
    let mut select = String::from("<select name=\"p\">");
    for c in &v.choices {
        let sel = if c.label == v.chosen && v.choices.first().is_some_and(|f| f.label != c.label) {
            " selected"
        } else {
            ""
        };
        let _ = write!(select, "<option{sel}>{}</option>", esc(&c.label));
    }
    select.push_str("</select>");
    let checked = |on: bool| if on { " checked" } else { "" };
    let mut cp = String::from("<select name=\"cp\">");
    for p in CODE_PAGES {
        let _ = write!(cp, "<option>{p}</option>");
    }
    cp.push_str("</select>");
    let rows = vec![
        Prop::new("Partition", select),
        Prop::new(
            "Screen size",
            format!(
                "<input type=\"radio\" name=\"sz\" value=\"24\"{}> 24 x 80 &nbsp; <input type=\"radio\" name=\"sz\" value=\"27\"{}> 27 x 132",
                checked(!v.wide),
                checked(v.wide)
            ),
        ),
        Prop::new("Code page", cp),
        Prop::new(
            "Colours",
            format!(
                "<input type=\"radio\" name=\"c\" value=\"g\"{}> Green on black &nbsp; <input type=\"radio\" name=\"c\" value=\"w\"{}> Black on white",
                checked(!v.bw),
                checked(v.bw)
            ),
        ),
        Prop::new(
            "Control panel strip",
            format!(
                "<input type=\"checkbox\" name=\"strip\"{}> Show above the screen",
                checked(v.strip)
            ),
        ),
        Prop::new(
            "Take over",
            format!(
                "<input type=\"checkbox\" name=\"force\"{}> Take the console from another QS/MC session if one is connected",
                checked(v.take_over)
            ),
        ),
    ];
    let body = format!(
        "<form method=\"post\">{}{}\n{}</form>",
        ctx.csrf_field(),
        props_table(ctx.product, &[PropGroup { title: None, rows }], true),
        buttons(&[Button::new("open", "Open"), Button::new("cancel", "Cancel")])
    );
    Page {
        title: "Open terminal window".into(),
        crumb: vec![
            "Systems Management".into(),
            "Partitions".into(),
            "Open terminal window".into(),
        ],
        h1: "Open terminal window".into(),
        desc: Some("The console of a partition, through the QS/VS host's virtual serial adapter. This is the same 5250 page as Access for Web with a control panel strip added.".into()),
        messages: v.messages.clone(),
        body,
        help: "terminal".into(),
        nav: Some(("Systems Management", Some("/partitions"))),
        ..Page::default()
    }
}
