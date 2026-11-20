// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! The pattern reference (`shared/components.html`): every `qsweb`
//! pattern once, with the prototype's own sample content. Served under
//! `/dev/` in a test image only (`cfg(test_hooks)`).

use qsweb::frame::{Ctx, Page};
use qsweb::msg::{Kind, Message};
use qsweb::partials::{
    Button, Cell, Col, FilterField, List, Paging, Prop, PropGroup, filter, list_table, messages,
    mono, paging, props, props_table, status, tabs, wizard,
};

/// The page's route.
pub const ROUTE: &str = "/dev/components";
/// The error page it points to.
pub const ERROR_ROUTE: &str = "/dev/error";

fn s(v: &[&str]) -> Vec<String> {
    v.iter().map(|x| (*x).to_string()).collect()
}

fn list(ctx: &Ctx) -> String {
    let here = ctx.url(ROUTE);
    let acts = format!(
        "<a href=\"{here}\">Hold</a> | <a href=\"{here}\">End</a> | <a href=\"{here}\">Properties</a>"
    );
    let row = |name: &str, user: &str, number: &str, st: (&str, bool), cpu: &str| {
        vec![
            Cell::act(acts.clone()),
            Cell::mono(name),
            Cell::mono(user),
            Cell::mono(number),
            Cell::html(status(st.0, Some(st.1))),
            Cell::num(cpu),
        ]
    };
    let l = List {
        cols: vec![
            Col::plain("Actions"),
            Col::sorted("Job name", "name"),
            Col::sorted("User", "user"),
            Col::sorted("Number", "number"),
            Col::sorted("Status", "status"),
            Col::sorted("CPU %", "cpu"),
        ],
        rows: vec![
            row("ADMIN", "QTMHHTTP", "123456", ("RUN", true), "0.3"),
            row("QPADEV0001", "QSECOFR", "123460", ("MSGW", false), "0.0"),
        ],
        empty: String::new(),
        keep: Vec::new(),
    };
    let empty = List {
        cols: ["Job name", "User", "Number", "Status"]
            .iter()
            .map(|c| Col::plain(c))
            .collect(),
        rows: Vec::new(),
        empty: "No active jobs match the filter.".into(),
        keep: Vec::new(),
    };
    format!(
        "{}{}{}<p class=\"small grey\">Variant: a per-row <code>&lt;select&gt;</code> of actions plus \"Go\": <select name=\"act\"><option>--Select action--</option><option>Hold</option><option>Release</option><option>End</option><option>Job log</option><option>Properties</option></select> <input type=\"submit\" value=\"Go\"></p><p class=\"small grey\">Empty state:</p>{}",
        filter(
            &[
                FilterField::Select(
                    "Subsystem".into(),
                    "sbs".into(),
                    s(&["*ALL", "QINTER", "QBATCH", "QHTTPSVR"]),
                    String::new()
                ),
                FilterField::Text("User".into(), "user".into(), String::new(), 10),
                FilterField::Select(
                    "Status".into(),
                    "sts".into(),
                    s(&["*ALL", "RUN", "DEQW", "MSGW"]),
                    String::new()
                ),
            ],
            false
        ),
        list_table(&l),
        paging(&Paging {
            page: 1,
            rows: 25,
            total: 288,
            keep: Vec::new()
        }),
        list_table(&empty)
    )
}

fn properties(ctx: &Ctx) -> String {
    let here = ctx.url(ROUTE);
    let groups = [
        PropGroup {
            title: Some("Identification".into()),
            rows: vec![
                Prop::new("Instance name", mono("WEBSTORE")),
                Prop::new(
                    "Description",
                    qsweb::partials::input("desc", "Web store front end", 40),
                )
                .help("console"),
            ],
        },
        PropGroup {
            title: Some("Startup".into()),
            rows: vec![
                Prop::new(
                    "Start with STRTCP",
                    qsweb::partials::checkbox("autostart", true),
                ),
                Prop::new(
                    "Server user profile",
                    qsweb::partials::input("usrprf", "QTMHHTTP", 10),
                ),
            ],
        },
    ];
    format!(
        "{}{}",
        tabs(
            &[
                ("General", here.clone()),
                ("Options", here.clone()),
                ("Security", here.clone()),
                ("Logs", here),
            ],
            0
        ),
        props(
            ctx,
            &groups,
            &[
                Button::new("apply", "Apply"),
                Button::new("ok", "OK"),
                Button::new("cancel", "Cancel"),
            ]
        )
    )
}

fn wizard_part(ctx: &Ctx) -> String {
    let root = PropGroup {
        title: None,
        rows: vec![Prop::new(
            "Server root",
            qsweb::partials::input_cmd("root", "/www/webstore", 40),
        )],
    };
    let body = format!(
        "<h2>Server root</h2><p class=\"small grey\">The directory that holds the configuration, logs and content of the instance.</p>{}",
        props_table(ctx.product, &[root], false)
    );
    wizard(
        ctx,
        &[
            "Name and description",
            "Server root",
            "Document root",
            "IP address and port",
            "Access log",
            "Create",
        ],
        2,
        &body,
        &[
            Button::new("back", "< Back"),
            Button::new("next", "Next >"),
            Button::new("cancel", "Cancel"),
        ],
    )
}

const CONTROLS: &str = r#"<form method="post">
<table class="props" cellspacing="0">
<tr><th>Text</th><td><input type="text" name="t" value="QGPL" size="10"> &nbsp; Command string: <input type="text" name="c" value="WRKACTJOB SBS(QHTTPSVR)" size="40" class="cmd"></td></tr>
<tr><th>Select</th><td><select name="s"><option>*ALL</option><option>*SYSTEM</option><option>*OBJECTSIGNING</option></select></td></tr>
<tr><th>Checkbox / radio</th><td><input type="checkbox" name="a" checked> Start automatically &nbsp; <input type="radio" name="r" value="*CNTRLD" checked> *CNTRLD <input type="radio" name="r" value="*IMMED"> *IMMED</td></tr>
<tr><th>File</th><td><input type="file" name="f"> <input type="submit" value="Browse..."></td></tr>
<tr><th>Textarea</th><td><textarea name="x" rows="3" cols="60" class="conf">Listen *:80
DocumentRoot /www/webstore/htdocs</textarea></td></tr>
<tr><th>Buttons</th><td><input type="submit" value="OK"> <input type="submit" value="Cancel"> <input type="submit" value="Apply"> <input type="submit" value="Refresh"> <input type="submit" value="Start"> <input type="submit" value="Stop"> <input type="submit" value="Restart"> <input type="submit" value="Delete"> <input type="submit" value="Add"> <input type="submit" value="Remove"> <input type="submit" value="Browse..."> <input type="submit" value="&lt; Back"> <input type="submit" value="Next &gt;"> <input type="submit" value="Finish"></td></tr>
<tr><th>Status words</th><td><span class="ok">Running</span> <span class="ok">Active</span> &nbsp; <span class="err">Stopped</span> <span class="err">Ended</span> <span class="err">Error</span> &nbsp; (text colour only; everything else black)</td></tr>
<tr><th>Utilisation bar</th><td><table class="bar" cellspacing="0" cellpadding="0"><tr><td class="fill" width="37%"></td><td width="63%"></td></tr></table> 37% &nbsp; <table class="bar" cellspacing="0" cellpadding="0"><tr><td class="hot" width="91%"></td><td width="9%"></td></tr></table> 91%</td></tr>
<tr><th>Glyphs</th><td><span class="glyph">[+]</span> collapsed &nbsp; <span class="glyph">[-]</span> expanded &nbsp; (text glyphs; a 16&times;16 GIF may replace them)</td></tr>
</table>
</form>"#;

const OUTPUT: &str = "<pre class=\"out\">Job . . :   ADMIN          User . . :   QTMHHTTP     Number . . . . . . :   123456
  MSGID      TYPE                    SEV  DATE      TIME             FROM PGM     LIBRARY
  CPF1124    Information             00   10/29/26  06:00:01.117332  QWTPIIPP     QSYS
             Message . . . . :   Job 123456/QTMHHTTP/ADMIN started on 10/29/26 at 06:00:01 in subsystem QHTTPSVR in QSYS.
  HTP8001    Information             00   10/29/26  06:00:03.004210  QZHBMAIN     QHTTPSVR
             Message . . . . :   HTTP server instance ADMIN started, listening on *:2001.</pre>";

/// The page.
pub fn page(ctx: &Ctx) -> Page {
    let area = messages(
        ctx.product,
        &[
            Message::new(Kind::Info, "CPF1321", "Job 123456/QUSER/QPADEV0001 ended."),
            Message::new(
                Kind::Done,
                "HTP8001",
                "HTTP server instance WEBSTORE started.",
            ),
            Message::new(
                Kind::Error,
                "SQL0204",
                "TABLE CUSTMAST in QGPL type *FILE not found.",
            ),
        ],
    );
    let body = format!(
        "<p class=\"small grey\">Every pattern the three products use, once. Use this page to check a new page against the standard before adding it.</p>\
<h2>1. Message area</h2>{area}\
<h2>2. List pattern (filter row, sortable headers, per-row actions, paging)</h2>{}\
<h2>3. Properties pattern (tabs, grouped label/value rows, buttons)</h2>{}\
<h2>4. Wizard pattern</h2>{}\
<h2>5. Confirmation and second-level text</h2><p class=\"small grey\">Confirmations are separate pages (see <a href=\"{}\">Confirm Delete Instance</a>). Second-level text is Message, Cause, Recovery, in that order (see <a href=\"{}\">Error page</a>).</p>\
<h2>6. Controls (browser defaults, never restyled)</h2>{CONTROLS}\
<h2>7. System output</h2>{OUTPUT}",
        list(ctx),
        properties(ctx),
        wizard_part(ctx),
        ctx.url("/confirm/cert-new"),
        ctx.url(ERROR_ROUTE),
    );
    Page {
        title: "Pattern reference".into(),
        crumb: vec!["Shared".into(), "Pattern reference".into()],
        h1: "Pattern reference".into(),
        body,
        help: "components".into(),
        nav: None,
        ..Page::default()
    }
}
