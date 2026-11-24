// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! The window's toolbar without script, as Access for Web's 5250
//! display has it: Copy (the screen's text to select), Paste (text typed
//! at the cursor) and the keyboard map (the console's keys). With
//! script, Copy and Paste use the clipboard and never come here.

use std::fmt::Write as _;

use qsweb::frame::Page;
use qsweb::html::esc;
use qsweb::http::Response;
use qsweb::msg::Kind;
use webusers::Task;

use mgmtproto::Kept;
use qsi::host::console::In;
use qsipart::console::{FromSystem, Grid, Key};

use super::{Options, partition};
use crate::app::routes::{Rq, lacks};
use crate::systems::system;

/// The console's keyboard as the window's script has it, with no map
/// given: the key, what it is to the console.
pub const KEYS: &[(&str, &str)] = &[
    ("Enter, Keypad Enter", "Enter"),
    ("Tab", "Field advance"),
    ("Shift+Tab", "Field backspace"),
    ("Right Ctrl, Keypad +", "Field exit"),
    ("Backspace", "Backspace"),
    ("Home", "Home"),
    ("Insert", "Insert"),
    ("Delete", "Delete"),
    ("Esc", "Attention"),
    ("Shift+Esc", "System request"),
    ("Ctrl+Left", "Reset"),
    ("Left, Right, Up, Down", "Cursor left, right, up, down"),
    ("Page Up", "Roll down"),
    ("Page Down", "Roll up"),
    ("F1 to F12", "F1 to F12"),
    ("Shift+F1 to Shift+F12", "F13 to F24"),
    (
        "A click on the screen",
        "The cursor to that place; on F3=... or More..., that key",
    ),
];

/// The page under the window: its title and crumb, back to the window.
fn sub(k: &Kept, id: u32, what: &str, back: &str, body: &str) -> Page {
    let name = partition(k, id).map_or_else(|| id.to_string(), |p| p.name);
    Page {
        title: format!("{what}: {name}"),
        crumb: vec![
            "Systems Management".into(),
            "Partitions".into(),
            name.clone(),
            "Terminal window".into(),
            what.into(),
        ],
        h1: format!("{what}: {name}"),
        body: format!(
            "{body}<p><a href=\"{}\">Back to the terminal window</a></p>",
            esc(back)
        ),
        help: "terminal".into(),
        ..Page::default()
    }
}

fn back(rq: &Rq<'_>, k: &Kept, id: u32) -> String {
    rq.ctx.url(&format!(
        "/terminal/{}/{id}?x=1{}",
        k.name,
        Options::of(rq).query()
    ))
}

/// The screen's text: the held window's, or one asked of the system.
fn text(rq: &Rq<'_>, host: &str, id: u32) -> String {
    if let Some(h) = rq.app.windows.find(&rq.session.id, host, id) {
        return h.now().grid.text();
    }
    let mut g = Grid::default();
    if let Ok(Ok(Some(f))) = rq
        .app
        .console
        .mgmt()
        .screen(host.into(), id, rq.user().into())
    {
        let _ = g.apply(&FromSystem::Frame(f));
    }
    g.text()
}

/// `GET /terminal/<host>/<id>/copy`.
pub fn copy(rq: &Rq<'_>, host: &str, id: u32) -> Response {
    let Some(k) = system(rq, host) else {
        return crate::app::routes::error(rq, 404, &qmcmsg::NO_SUCH_SYSTEM, &[host]);
    };
    let body = format!(
        "<p>The screen's text, one line per row. Select it and copy it with the browser.</p><textarea rows=\"28\" cols=\"134\" class=\"conf\" readonly>{}</textarea>",
        esc(&text(rq, host, id))
    );
    rq.page(sub(&k, id, "Copy", &back(rq, &k, id), &body))
}

/// `GET`, `POST /terminal/<host>/<id>/paste`: text typed at the cursor
/// of the window this session holds.
pub fn paste(rq: &Rq<'_>, host: &str, id: u32) -> Response {
    let Some(k) = system(rq, host) else {
        return crate::app::routes::error(rq, 404, &qmcmsg::NO_SUCH_SYSTEM, &[host]);
    };
    let to = back(rq, &k, id);
    if rq.post() {
        let f = rq.req.form();
        if f.has("cancel") {
            return Response::redirect(&to);
        }
        if let Some(no) = lacks(rq, Task::Terminal) {
            return no;
        }
        let Some(h) = rq.app.windows.find(&rq.session.id, &k.name, id) else {
            let m = qmcmsg::message(
                Kind::Error,
                &qmcmsg::NOT_VALID,
                &["Connect the window first."],
            );
            rq.app.sessions.flash(&rq.session.id, m);
            return Response::redirect(&to);
        };
        for c in f.str("t").chars().filter(|c| !c.is_control()) {
            h.send(In::Key(Key::Char(c)));
        }
        // Let the system's answer come before the picture is read.
        std::thread::sleep(std::time::Duration::from_millis(150));
        return Response::redirect(&to);
    }
    let body = format!(
        "<form method=\"post\">{}<table class=\"props\" cellspacing=\"0\">\n<tr><th>Text</th><td><input type=\"text\" name=\"t\" value=\"\" size=\"60\" class=\"cmd\"> <span class=\"small grey\">typed at the cursor</span></td></tr>\n</table>\n<div class=\"buttons\"><input type=\"submit\" name=\"ok\" value=\"Paste\"><input type=\"submit\" name=\"cancel\" value=\"Cancel\"></div></form>",
        rq.ctx.csrf_field()
    );
    rq.page(sub(&k, id, "Paste", &to, &body))
}

/// `GET /terminal/<host>/<id>/keymap`: the console's keys.
pub fn keymap(rq: &Rq<'_>, host: &str, id: u32) -> Response {
    let Some(k) = system(rq, host) else {
        return crate::app::routes::error(rq, 404, &qmcmsg::NO_SUCH_SYSTEM, &[host]);
    };
    let mut rows = String::new();
    for (key, does) in KEYS {
        let _ = write!(
            rows,
            "<tr><td class=\"mono\">{}</td><td>{}</td></tr>",
            esc(key),
            esc(does)
        );
    }
    let body = format!(
        "<p>The keys the window sends to the console. The keypad beside the screen sends the same.</p><table class=\"list\" cellspacing=\"0\"><tr><th>Key</th><th>Sends</th></tr>{rows}</table>"
    );
    rq.page(sub(&k, id, "Keyboard map", &back(rq, &k, id), &body))
}
