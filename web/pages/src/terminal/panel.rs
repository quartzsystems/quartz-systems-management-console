// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! Control panel functions (`control-panel.html`): the list with its
//! texts as the design has them; the function chosen is sent to the
//! host, which carries out what it has and refuses the rest with its own
//! message; the result shows what it displays and the code before and
//! after. Functions 08 and 22 go through the confirmation page; 50 to
//! 70 need the task of their own.

use std::fmt::Write as _;

use qsweb::frame::{Ctx, Page};
use qsweb::html::esc;
use qsweb::msg::Message;

/// The functions, as the design lists them.
pub const FUNCTIONS: [(u8, &str); 30] = [
    (1, "Display selected IPL type, side and keylock"),
    (2, "Select IPL type, side and keylock"),
    (3, "Start IPL (restart partition)"),
    (4, "Lamp test"),
    (5, "Display reference code (SRC) words 1-9"),
    (7, "Display SPCN status"),
    (8, "Fast power off"),
    (11, "Display SRC word 1"),
    (12, "Display SRC words 2-5"),
    (13, "Display SRC words 6-9"),
    (20, "Display system type, model, feature and serial"),
    (21, "Force dedicated service tools (DST) on the console"),
    (22, "Force main storage dump"),
    (25, "Service switch 1 (enable 50-70)"),
    (26, "Service switch 2"),
    (33, "Reorder SPCN addressing"),
    (34, "Retry main storage dump IPL"),
    (50, "Service processor stop"),
    (51, "Service processor status"),
    (52, "Service processor start"),
    (53, "Path switch"),
    (55, "Display service processor log"),
    (63, "Display system status SRC trace"),
    (64, "Display service processor diagnostic SRC trace"),
    (65, "Deactivate remote service"),
    (66, "Activate remote service"),
    (67, "Disk unit IOP reset"),
    (68, "Concurrent maintenance power domain power off"),
    (69, "Concurrent maintenance power domain power on"),
    (70, "Dump service processor control storage"),
];

/// Functions that end or interrupt the system: through the confirmation
/// page only.
pub const CONFIRMED: [u8; 2] = [8, 22];

/// What the page shows.
#[derive(Clone, Debug, Default)]
pub struct Panel {
    /// The managed system.
    pub host: String,
    /// The partition's ID.
    pub id: u32,
    /// The partition's name.
    pub partition: String,
    /// `Running`, `Not activated`.
    pub state: String,
    /// `Manual`, `Normal`.
    pub keylock: String,
    /// Its code.
    pub code: String,
    /// What the code means.
    pub meaning: String,
    /// The function chosen.
    pub chosen: u8,
    /// The result's lines.
    pub result: Vec<String>,
    /// The messages under the strip.
    pub messages: Vec<Message>,
}

/// The page.
pub fn page(ctx: &Ctx, v: &Panel) -> Page {
    let mut list = String::from("<select name=\"func\" size=\"12\">");
    for (n, text) in FUNCTIONS {
        let sel = if n == v.chosen { " selected" } else { "" };
        let _ = write!(list, "<option{sel}>{n:02} - {}</option>", esc(text));
    }
    list.push_str("</select>");
    let state_good = v.state == "Running";
    let msgs = if v.messages.is_empty() {
        String::new()
    } else {
        qsweb::partials::messages(ctx.product, &v.messages)
    };
    let result = if v.result.is_empty() {
        String::new()
    } else {
        format!(
            "<tr><th>Result</th><td><pre class=\"out\">{}</pre></td></tr>",
            esc(&v.result.join("\n"))
        )
    };
    let body = format!(
        "<form method=\"post\">{}{}{msgs}<table class=\"props\"><tr><th>Partition</th><td><span class=\"mono\">{}</span> (ID {}) on <span class=\"mono\">{}</span>, {} &nbsp; Keylock: {}</td></tr><tr><th>Function</th><td>{list}</td></tr>{result}</table><div class=\"buttons\"><input type=\"submit\" name=\"select\" value=\"Select\"><input type=\"submit\" name=\"refresh\" value=\"Refresh\"></div></form><p class=\"small grey\">Functions 50-70 require service switch 25 then 26 first. Functions 08 and 22 go through the confirmation page.</p>",
        ctx.csrf_field(),
        super::window::strip(ctx, &v.host, v.id, &v.code, &v.meaning, true),
        esc(&v.partition),
        v.id,
        esc(&v.host),
        qsweb::partials::status(&v.state, Some(state_good)),
        esc(&v.keylock),
    );
    Page {
        title: format!("Control panel functions: {}", v.partition),
        crumb: vec![
            "Systems Management".into(),
            "Partitions".into(),
            v.partition.clone(),
            "Control panel functions".into(),
        ],
        h1: format!("Control panel functions: {}", v.partition),
        body,
        help: "control-panel".into(),
        nav: Some(("Systems Management", Some("/partitions"))),
        ..Page::default()
    }
}

/// The function a form chose: a strip button (`08 Power off`), the
/// function field, or the list (`21 - Force ...`).
pub fn chosen(f: &qsweb::http::Form) -> Option<u8> {
    let first = |s: &str| {
        s.split([' ', '-'])
            .next()
            .and_then(|n| n.trim().parse::<u8>().ok())
    };
    if let Some(b) = f.get("f") {
        return first(b);
    }
    if let Some(n) = f.get("fn").filter(|n| !n.trim().is_empty()) {
        return first(n);
    }
    f.get("func").and_then(first)
}

/// The function's short name, for the result: `21 - Force DST`.
pub fn label(n: u8) -> String {
    let strip = super::window::STRIP
        .iter()
        .find(|(s, _)| s.parse::<u8>().ok() == Some(n))
        .map(|(_, l)| (*l).to_string());
    let text = strip.unwrap_or_else(|| {
        FUNCTIONS.iter().find(|(f, _)| *f == n).map_or_else(
            || "not a function of the list".into(),
            |(_, t)| (*t).to_string(),
        )
    });
    format!("Function {n:02} - {text}")
}
