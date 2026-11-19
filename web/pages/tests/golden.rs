// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! Each page rendered with the prototype's sample data against its
//! prototype file. Golden corrections (`fix`) are listed in
//! `docs/WEB.md`.

use std::path::PathBuf;

use qsweb::golden::{Normalize, diff};
use webpages::url::proto;

fn prototype(file: &str) -> String {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../design/qsmc-prototype/qsmc")
        .join(file);
    std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()))
}

/// `served` against `file` after the `fix` edits of the golden text.
fn assert_golden(served: &str, file: &str, fix: &[(&str, &str)]) {
    let mut g = prototype(file);
    for (from, to) in fix {
        assert!(
            g.contains(from),
            "golden fix not applicable to {file}: {from}"
        );
        g = g.replace(from, to);
    }
    let map = |u: &str| proto(u);
    if let Some(d) = diff(
        served,
        &g,
        &Normalize {
            url: &map,
            script: None,
        },
    ) {
        panic!("{file} differs from the served page at {d}");
    }
}

#[test]
fn signon() {
    let v = webpages::signon::Signon {
        console: "QSHQMC1".into(),
        notice: "This console manages production QS/400 systems. All sessions are logged. Contact the operations desk before powering off any managed system.".into(),
        ..Default::default()
    };
    // The form posts back to the sign-on route, not to Welcome.
    assert_golden(
        &webpages::signon::render(&v),
        "signon.html",
        &[("action=\"welcome.html\"", "action=\"signon.html\"")],
    );
}

/// The frame's view for the prototype's console and user.
fn ctx(here: &str) -> qsweb::frame::Ctx {
    qsweb::frame::Ctx {
        product: &webd::product::PRODUCT,
        system: "QSHQMC1".into(),
        user: "qsadmin".into(),
        csrf: "token".into(),
        open: None,
        here: here.into(),
    }
}

fn render(here: &str, page: &qsweb::frame::Page) -> String {
    qsweb::frame::render(&ctx(here), page)
}

#[test]
fn welcome() {
    let v = webpages::welcome::Welcome {
        console: "QSHQMC1".into(),
        user: "qsadmin".into(),
        role: "Administrator".into(),
        from: "10.0.10.55".into(),
        systems: [2, 2, 0, 0, 0, 0],
        partitions: [3, 2, 1, 0, 0, 0],
        events: 1,
        dumps: 2,
        updates: "2 (QS/VS host, 1 partition)".into(),
        version: "V1R0M0".into(),
        build: "26290".into(),
        uptime: "41 days 3:12".into(),
        connected: "2 of 2".into(),
        messages: Vec::new(),
    };
    let c = ctx("/qsmc/welcome");
    assert_golden(
        &render("/qsmc/welcome", &webpages::welcome::page(&c, &v)),
        "welcome.html",
        &[],
    );
}
