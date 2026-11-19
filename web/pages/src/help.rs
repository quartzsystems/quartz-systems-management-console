// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! The help window (`shared/help.html`) for each page's help key.

use qsweb::html::esc;
use webd::product::PRODUCT;

/// The topics: key, title, text.
const TOPICS: &[(&str, &str, &str)] = &[
    (
        "welcome",
        "Welcome",
        "Welcome shows the console's sections and a status strip: managed systems and partitions by state, open serviceable events, dumps, updates, and the console's version and uptime.",
    ),
    (
        "console",
        "Console settings",
        "Console settings lists each setting of this console with its current value. Choose a setting to change it.",
    ),
    (
        "users",
        "Users and roles",
        "Web users sign on to this console's web interface. Each has one role; the role's tasks are on the Roles tab. The local console's service tools users are a separate list.",
    ),
    (
        "roles",
        "Roles",
        "Which tasks each role has. Administrator always has every task. A change applies at each user's next sign-on.",
    ),
    (
        "network",
        "Network settings",
        "The open network is where operators reach the console; the private network faces the managed systems. A change to the address you came in on is kept only if you reach the console again within the trial.",
    ),
    (
        "datetime",
        "Date and time",
        "The console's clock is kept by its time servers; without one it can be set by hand. Times are shown at the display offset.",
    ),
    (
        "certs",
        "Certificates",
        "The certificate the web interface presents. Make a new self-signed one, or import a certificate and its key. It is not the console's QSI identity.",
    ),
    (
        "backup",
        "Backup and restore",
        "A backup holds the console's state, its identity and its users, encrypted under a passphrase you type. It is restored at the next restart.",
    ),
    (
        "shutdown",
        "Shut down or restart",
        "Shutting down the console does not affect managed systems or partitions.",
    ),
    (
        "locale",
        "Language, locale and session",
        "The formats of dates, times and numbers, and how long a session may be idle or last.",
    ),
    (
        "version",
        "Console version",
        "The console's release, build, slot and fixes, and the machine it runs on.",
    ),
    (
        "error",
        "Messages",
        "A message's identifier links to its second-level text: what happened and what to do.",
    ),
    (
        "later",
        "Later functions",
        "Functions of the navigation tree that arrive in a later release say so when chosen.",
    ),
];

/// The help window for `key`.
pub fn window(key: &str) -> String {
    let (title, text) = TOPICS.iter().find(|(k, _, _)| *k == key).map_or(
        ("Help", "There is no help for this page yet."),
        |(_, t, x)| (*t, *x),
    );
    qsweb::frame::help(&PRODUCT, title, &format!("<p>{}</p>", esc(text)))
}
