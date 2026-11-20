// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! QS/MC as `qsweb` sees it: its family ([`BRAND`]) and its one
//! product ([`PRODUCT`]) with the prototype's navigation tree. Routes
//! mirror the prototype's file names: `console-users.html` is
//! `/qsmc/console/users`.

use qsweb::brand::Brand;
use qsweb::frame::{NavGroup, Product};

/// The route prefix.
pub const PREFIX: &str = "/qsmc";

/// Where the stylesheet is served (`qsweb::CSS`, byte for byte).
pub const CSS_PATH: &str = "/qsmc/shared/qs400.css";

/// The family: QS/MC alone.
pub static BRAND: Brand = Brand {
    name: release::NAME,
    vendor: "Quartz Systems",
    release: &release::RELEASE,
    server: "QSMC",
    css_path: CSS_PATH,
    // No page carries the script ([`PRODUCT`]); the path is never served.
    nav_path: "/qsmc/shared/qsnav.js",
    cookie: "QSMCSID",
    signon_notice: "",
    notice_in_form: true,
    user_max: 32,
    notauth_note: "Ask an Administrator of this console for the task. The attempt was recorded in the console's audit log.",
    job_log: false,
};

/// The navigation tree, as the prototype has it.
pub static NAV: &[NavGroup] = &[
    NavGroup {
        label: "Welcome",
        items: &[("Welcome", "/welcome")],
    },
    NavGroup {
        label: "Systems Management",
        items: &[
            ("Servers", "/servers"),
            ("Partitions", "/partitions"),
            ("Custom groups", "/groups"),
        ],
    },
    NavGroup {
        label: "System Plans",
        items: &[("System Plans", "/plans")],
    },
    NavGroup {
        label: "Console Management",
        items: &[
            ("Console settings", "/console"),
            ("Users and roles", "/console/users"),
            ("Network settings", "/console/network"),
            ("Date and time", "/console/datetime"),
            ("Backup and restore", "/console/backup"),
            ("Certificates", "/console/certs"),
            ("Shut down or restart", "/console/shutdown"),
            ("Console version", "/console/version"),
        ],
    },
    NavGroup {
        label: "Service Management",
        items: &[
            ("Serviceable events", "/events"),
            ("Reference code history", "/refcodes"),
            ("Manage dumps", "/dumps"),
            ("Code updates", "/updates/code"),
            ("Connectivity", "/connectivity"),
        ],
    },
    NavGroup {
        label: "Updates",
        items: &[("Updates", "/updates")],
    },
];

/// The Management Console.
pub static PRODUCT: Product = Product {
    brand: &BRAND,
    wordmark: "QS/MC Management Console",
    crumb_root: "QS/MC",
    prefix: PREFIX,
    nav: NAV,
    system_label: "Console",
    nav_script: false,
};

/// The routes of the tree that later phases bring, and the phase.
/// Each answers with a page saying the function arrives later.
pub const LATER: &[(&str, u8)] = &[
    ("/partitions", 5),
    ("/groups", 5),
    ("/events", 6),
    ("/dumps", 6),
    ("/updates/code", 6),
    ("/updates", 6),
    ("/plans", 7),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_route_of_the_tree_is_live_or_a_later_phase_s() {
        let now = [
            "/welcome",
            "/console",
            "/console/users",
            "/console/network",
            "/console/datetime",
            "/console/backup",
            "/console/certs",
            "/console/shutdown",
            "/console/version",
            "/servers",
            "/refcodes",
            "/connectivity",
        ];
        for g in NAV {
            for (_, route) in g.items {
                assert!(
                    now.contains(route) || LATER.iter().any(|(r, _)| r == route),
                    "{route} is neither"
                );
            }
        }
        assert_eq!(
            BRAND.footer(),
            format!("QS/MC {} \u{b7} Quartz Systems", release::RELEASE)
        );
    }
}
