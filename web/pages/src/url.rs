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
    ("/updates/code", "code-updates.html"),
];

/// The prototype's file (relative to `qsmc/`) for served `url`; `None`
/// for a URL that is not the product's (a `?sort=` link, `#`).
pub fn proto(url: &str) -> Option<String> {
    let path = url.split(['?', '#']).next().unwrap_or(url);
    if path == CSS_PATH {
        return Some("../shared/qs400.css".into());
    }
    let route = path.strip_prefix(PREFIX)?;
    if route.starts_with("/help/") || route.starts_with("/msg/") || route == "/help" {
        return Some("../shared/help.html".into());
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
