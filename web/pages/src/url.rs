// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! Served routes and the prototype's file names. A route mirrors its
//! file (`console-users.html` is `/qsmc/console/users`); this maps a
//! served URL back, for the golden comparison and the link check.

use webd::product::{CSS_PATH, PREFIX};

/// The routes whose files are not their route's name.
const RENAMED: &[(&str, &str)] = &[
    ("/signon", "signon.html"),
    ("/signoff", "signon.html"),
    ("/refcodes", "refcode-history.html"),
    ("/servers/add", "add-system.html"),
    ("/servers/add/result", "add-system-result.html"),
    ("/servers/reset", "add-system.html"),
    ("/updates/code", "code-updates.html"),
    ("/groups/create", "group-create.html"),
    ("/schedule/add", "schedule-add.html"),
    // A test image's pattern reference, and the error page it shows.
    ("/dev/components", "../shared/components.html"),
    ("/dev/error", "../shared/error.html"),
];

/// The prototype's file (relative to `qsmc/`) for served `url`; `None`
/// for a URL that is not the product's (a `?sort=` link, `#`).
pub fn proto(url: &str) -> Option<String> {
    let path = url.split(['?', '#']).next().unwrap_or(url);
    if path == CSS_PATH {
        return Some("../shared/qs400.css".into());
    }
    let route = path.strip_prefix(PREFIX)?;
    if route.starts_with("/confirm/") {
        return Some("../shared/confirm.html".into());
    }
    if route.starts_with("/refcode/") {
        return Some("refcode.html".into());
    }
    // A partition's pages: `/partition/<host>/<id>/<page>` is
    // `partition-<page>.html`, its control panel `control-panel.html`.
    if let Some(rest) = route.strip_prefix("/partition/") {
        let page = rest.split('/').nth(2).unwrap_or("general");
        return Some(match page {
            "panel" => "control-panel.html".into(),
            "activate" => "activate.html".into(),
            "shutdown" => "shutdown.html".into(),
            "profiles" => "manage-profiles.html".into(),
            // An adapter's form is the adapters' tab's.
            "edit" => "partition-vadapters.html".into(),
            "navigator" => "../nav/ptfs.html".into(),
            _ => format!("partition-{page}.html"),
        });
    }
    // The wizard: `/partitions/create/step/<n>` is `create-<n>.html`.
    if let Some(n) = route.strip_prefix("/partitions/create/step/") {
        return Some(format!("create-{n}.html"));
    }
    // The terminal: its configuration, the window, its picture, the
    // page a window shows when its console went.
    if let Some(rest) = route.strip_prefix("/terminal") {
        let parts: Vec<&str> = rest.split('/').filter(|p| !p.is_empty()).collect();
        return Some(match parts.get(2).copied() {
            _ if parts.len() < 2 => "terminal-config.html".into(),
            Some("screen.png") => "../shared/img/5250-24x80.png".into(),
            Some("screen-off.png") => "../shared/img/5250-24x80-off.png".into(),
            Some("disconnected") => "terminal-disconnected.html".into(),
            _ => "terminal.html".into(),
        });
    }
    // A managed system's pages: `/server/<name>/<tab>` is
    // `system-<tab>.html`; its power confirmation by the operation.
    if let Some(rest) = route.strip_prefix("/server/") {
        let mut parts = rest.split('/').skip(1);
        let tab = parts.next().unwrap_or("general");
        if tab == "virtual" {
            let page = parts.next().unwrap_or("ethernet");
            // A disk's create, properties and resize are the design's
            // one form; the other forms are their tab's.
            if page == "storage" && matches!(parts.next(), Some("create" | "resize")) {
                return Some("create-vdisk.html".into());
            }
            return Some(format!("vr-{page}.html"));
        }
        if tab == "confirm" {
            let op = url.split("op=").nth(1).unwrap_or("poweroff");
            let file = if op.starts_with("poweron") {
                "poweron"
            } else {
                "poweroff"
            };
            return Some(format!("confirm-{file}.html"));
        }
        return Some(format!("system-{tab}.html"));
    }
    // A serviceable event and the pages of it built from the patterns.
    if route.starts_with("/event/") {
        return Some("event-detail.html".into());
    }
    // An install's progress and its log.
    if let Some(rest) = route.strip_prefix("/updates/code/")
        && (rest.ends_with("/progress") || rest.ends_with("/log"))
    {
        return Some("code-update-progress.html".into());
    }
    // A release's notes: the design has none, and links `#`.
    if route.starts_with("/updates/notes/") {
        return Some("#".into());
    }
    if route.starts_with("/help/") || route.starts_with("/msg/") || route == "/help" {
        return Some("../shared/help.html".into());
    }
    if route.starts_with("/group/") {
        return Some("group-view.html".into());
    }
    if let Some((_, file)) = RENAMED.iter().find(|(r, _)| *r == route) {
        return Some((*file).into());
    }
    let name = route.trim_start_matches('/').replace('/', "-");
    (!name.is_empty()).then(|| format!("{name}.html"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn routes_mirror_the_files() {
        assert_eq!(
            proto("/qsmc/console/users").as_deref(),
            Some("console-users.html")
        );
        assert_eq!(proto("/qsmc/welcome").as_deref(), Some("welcome.html"));
        assert_eq!(proto("/qsmc/signoff").as_deref(), Some("signon.html"));
        assert_eq!(
            proto("/qsmc/msg/QMC9001").as_deref(),
            Some("../shared/help.html")
        );
        assert_eq!(
            proto("/qsmc/shared/qs400.css").as_deref(),
            Some("../shared/qs400.css")
        );
        assert_eq!(
            proto("/qsmc/updates/code").as_deref(),
            Some("code-updates.html")
        );
        assert_eq!(proto("?sort=name"), None);
    }
}
