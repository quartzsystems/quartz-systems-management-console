// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! The terminal window (`terminal.html`, `terminal-disconnected.html`):
//! the menu, the toolbar, the control panel strip, the screen drawn by
//! QS/400's look (`termlook`) as a picture with the keypad beside it,
//! and the status line. Without script the picture is the server's and
//! the keypad posts; with script the page's one `<script>` element is
//! QS/400's terminal script, which draws the same cells on its socket.

use std::fmt::Write as _;

use qsweb::frame::{Ctx, Page};
use qsweb::html::esc;
use qsweb::msg::Message;
use termlook::exchange::keys::KEYPAD;

/// What the window shows.
#[derive(Clone, Debug, Default)]
#[allow(clippy::struct_excessive_bools, reason = "the page's switches")]
pub struct Window {
    /// The managed system.
    pub host: String,
    /// The partition's ID.
    pub id: u32,
    /// The partition's name.
    pub partition: String,
    /// Whether its console is attached.
    pub connected: bool,
    /// The partition's current code.
    pub code: String,
    /// What it means.
    pub meaning: String,
    /// The control panel strip above the screen.
    pub strip: bool,
    /// The picture's scale, 1 or 2.
    pub scale: u32,
    /// The picture's size in pixels.
    pub size: (usize, usize),
    /// The cursor, 1-based row and column.
    pub cursor: (usize, usize),
    /// Whether the user's keys are sent.
    pub may_type: bool,
    /// The query the window's own links keep (`&scale=2`, ...).
    pub keep: String,
    /// With the page's script: its element.
    pub script: bool,
    /// Black on white.
    pub bw: bool,
    /// The message rows.
    pub messages: Vec<Message>,
}

/// The control panel strip's buttons: number and label.
pub const STRIP: [(&str, &str); 5] = [
    ("03", "IPL"),
    ("08", "Power off"),
    ("21", "Force DST"),
    ("22", "Dump"),
    ("34", "Retry"),
];

/// The strip: the current code and its meaning, the functions and the
/// function field. `class` on the code's link, as the control panel
/// page has it.
pub fn strip(ctx: &Ctx, host: &str, id: u32, code: &str, meaning: &str, class: bool) -> String {
    let mut buttons = String::new();
    for (n, label) in STRIP {
        let _ = write!(
            buttons,
            "<input type=\"submit\" name=\"f\" value=\"{n} {label}\"> "
        );
    }
    let class = if class { " class=\"mono\"" } else { "" };
    format!(
        "<table class=\"list\" style=\"width:auto; margin-bottom:4px\"><tr><th>Reference code</th><th>Meaning</th><th>Control panel functions</th></tr><tr><td class=\"mono\" style=\"font-size:12pt; width:110px\"><a{class} href=\"{}\">{}</a></td><td>{}</td><td class=\"act\">{buttons}&nbsp; Function: <input type=\"text\" name=\"fn\" value=\"\" size=\"3\"> <input type=\"submit\" value=\"Select\"></td></tr></table>",
        ctx.url(&format!("/refcode/{}?ms={}&p={id}", esc(code), esc(host))),
        esc(code),
        esc(meaning)
    )
}

/// The page.
#[allow(clippy::too_many_lines, reason = "one page, top to bottom")]
pub fn page(ctx: &Ctx, v: &Window) -> Page {
    let base = format!("/terminal/{}/{}", esc(&v.host), v.id);
    let here = |extra: &str| ctx.url(&format!("{base}{extra}"));
    let config = ctx.url("/terminal");
    let mut body = format!(
        "<table class=\"list\" cellspacing=\"0\" style=\"margin-bottom:3px\"><tr><th><b>File</b> &nbsp;&nbsp; <a href=\"{e}\">Edit</a> &nbsp;&nbsp; <a href=\"{vw}\">View</a> &nbsp;&nbsp; <a href=\"{c}\">Communication</a> &nbsp;&nbsp; <a href=\"{a}\">Actions</a> &nbsp;&nbsp; <a href=\"{config}\">Macro</a> &nbsp;&nbsp; <a href=\"{h}\">Help</a></th></tr><tr><td class=\"small\"><a href=\"{config}\">Open session...</a> &nbsp;|&nbsp; <a href=\"{s}\">Save session</a> &nbsp;|&nbsp; <a href=\"{config}\">Save session as...</a> &nbsp;|&nbsp; <a href=\"{config}\">Recent sessions</a> &nbsp;|&nbsp; <a href=\"{config}\">Properties...</a> &nbsp;|&nbsp; <a href=\"{p}\">Print screen</a> &nbsp;|&nbsp; <a href=\"{config}\">Close</a></td></tr></table>",
        e = here(&format!("?menu=edit{}", v.keep)),
        vw = here(&format!("?menu=view{}", v.keep)),
        c = here(&format!("?menu=communication{}", v.keep)),
        a = here(&format!("?menu=actions{}", v.keep)),
        h = ctx.url("/help/terminal"),
        s = here(&format!("?menu=file{}", v.keep)),
        p = here("/print"),
    );
    let (connect, disconnect) = if v.connected {
        (" disabled", "")
    } else {
        ("", " disabled")
    };
    let scale = if v.scale == 2 {
        format!("<a href=\"{}\">1x</a> <b>2x</b>", here("?scale=1"))
    } else {
        format!("<b>1x</b> <a href=\"{}\">2x</a>", here("?scale=2"))
    };
    let _ = write!(
        body,
        "<form method=\"post\">{}<div class=\"toolbar\"><input type=\"submit\" name=\"tb\" value=\"Connect\"{connect}><input type=\"submit\" name=\"tb\" value=\"Disconnect\"{disconnect}> &nbsp;|&nbsp; <input type=\"submit\" name=\"tb\" value=\"Copy\"><input type=\"submit\" name=\"tb\" value=\"Paste\"><input type=\"submit\" name=\"tb\" value=\"Print Screen\"> &nbsp;|&nbsp; <input type=\"submit\" name=\"tb\" value=\"Keypad\"><input type=\"submit\" name=\"tb\" value=\"Keyboard Map\"> &nbsp;|&nbsp; <span class=\"small\">Scale: {scale}</span></div></form>",
        ctx.csrf_field()
    );
    if v.strip {
        let _ = write!(
            body,
            "<form method=\"post\">{}{}</form>",
            ctx.csrf_field(),
            strip(ctx, &v.host, v.id, &v.code, &v.meaning, false)
        );
    }
    let off = if v.connected { "" } else { "-off" };
    let (w, h) = v.size;
    let mut keypad = String::from(
        "<table cellspacing=\"0\" cellpadding=\"2\" style=\"margin:2px\" class=\"keypad\">",
    );
    let disabled = if v.connected && v.may_type {
        ""
    } else {
        " disabled"
    };
    for (row, keys) in KEYPAD.chunks(3).enumerate() {
        keypad.push_str("<tr>");
        for (label, _) in keys {
            let td = if row == 8 {
                "<td style=\"padding-top:8px\">"
            } else {
                "<td>"
            };
            let _ = write!(
                keypad,
                "{td}<input type=\"submit\" name=\"k\" value=\"{}\" style=\"width:54px;height:22px;font-size:8pt;padding:0\"{disabled}></td>",
                esc(label)
            );
        }
        keypad.push_str("</tr>");
    }
    keypad.push_str("</table>");
    let _ = write!(
        body,
        "<form method=\"post\">{}<table cellspacing=\"0\" cellpadding=\"0\" style=\"border:1px solid #808080\"><tr><td style=\"padding:0;vertical-align:top;background:#000000\"><img id=\"term\" src=\"{}\" width=\"{w}\" height=\"{h}\" alt=\"5250 screen\" style=\"display:block;image-rendering:pixelated;image-rendering:crisp-edges\"></td><td style=\"padding:0;vertical-align:top;background:#e8e8e8;border-left:1px solid #c0c0c0;width:178px\">{keypad}</td></tr></table></form>",
        ctx.csrf_field(),
        here(&format!("/screen{off}.png?scale={}", v.scale.max(1))),
    );
    let name = format!("{} on {}", v.partition, v.host);
    let (left, mid, right) = if v.connected {
        (
            String::new(),
            format!("TLS &nbsp; {:02}/{:03}", v.cursor.0, v.cursor.1),
            format!(
                "<td class=\"ok\" style=\"width:200px\">{}: Connected</td>",
                esc(&name)
            ),
        )
    } else {
        (
            "Not connected. Communication &gt; Connect, or Reconnect.".to_string(),
            String::new(),
            format!(
                "<td class=\"err\" style=\"width:200px\">{}: Not connected</td>",
                esc(&name)
            ),
        )
    };
    let _ = write!(
        body,
        "<table class=\"list\" cellspacing=\"0\" style=\"margin-top:2px\"><tr><td style=\"width:60%\">{left}</td><td class=\"num\" style=\"width:120px\">{mid}</td>{right}</tr></table>"
    );
    if v.script && v.connected {
        let scheme = if v.bw { " data-scheme=\"bw\"" } else { "" };
        let _ = write!(
            body,
            "<script src=\"{}\" data-socket=\"{}\" data-image=\"term\" data-scale=\"{}\"{scheme}></script>",
            ctx.url(SCRIPT),
            here("/ws"),
            v.scale.max(1)
        );
    }
    Page {
        title: format!("Terminal window: {}", v.partition),
        crumb: vec![
            "Systems Management".into(),
            "Partitions".into(),
            v.partition.clone(),
            "Terminal window".into(),
        ],
        h1: format!("Terminal window: {}", v.partition),
        messages: v.messages.clone(),
        body,
        help: "terminal".into(),
        nav: None,
        ..Page::default()
    }
}

/// The script's route.
pub const SCRIPT: &str = "/terminal/script.js";

/// `html`, a page rendered in the frame, without the navigation column
/// or the navigation script: the content spans the frame, as the
/// terminal window has it, and its own script is its one.
pub fn without_nav(html: &str) -> String {
    let tag = format!(
        "<script type=\"text/javascript\" src=\"{}\" defer=\"defer\"></script>\n",
        webd::product::NAV_PATH
    );
    let html = html.replacen(&tag, "", 1);
    let Some(start) = html.find("<td class=\"nav\"") else {
        return html;
    };
    let Some(end) = html[start..]
        .find("</td>")
        .map(|e| start + e + "</td>".len())
    else {
        return html;
    };
    let mut out = String::with_capacity(html.len());
    out.push_str(&html[..start]);
    out.push_str(html[end..].trim_start_matches('\n'));
    out.replacen(
        "<td class=\"content\" valign=\"top\">",
        "<td class=\"content\" valign=\"top\" colspan=\"2\">",
        1,
    )
}
