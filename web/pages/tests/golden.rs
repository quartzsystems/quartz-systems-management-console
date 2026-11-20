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

/// A shared page (`shared/<file>`) against the served one: URLs mapped
/// relative to `shared/`, the golden text corrected by `fix` first.
fn assert_shared(served: &str, file: &str, fix: &[(&str, &str)]) {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../design/qsmc-prototype/shared")
        .join(file);
    let mut g = std::fs::read_to_string(&p).unwrap();
    for (from, to) in fix {
        assert!(
            g.contains(from),
            "golden fix not applicable to {file}: {from}"
        );
        g = g.replace(from, to);
    }
    let map = |u: &str| {
        proto(u).map(|t| match t.strip_prefix("../shared/") {
            Some(s) => s.to_string(),
            None => format!("../qsmc/{t}"),
        })
    };
    if let Some(d) = diff(
        served,
        &g,
        &Normalize {
            url: &map,
            script: None,
        },
    ) {
        panic!("shared/{file} differs from the served page at {d}");
    }
}

/// The corrections every shared page needs: the prototype's shared
/// pages carry QS/400's frame, and QS/MC serves them in its own, as
/// its other pages (`docs/WEB.md`, golden corrections).
const QS400_FRAME: [(&str, &str); 4] = [
    (
        "<td class=\"wordmark\">QS/400</td>",
        "<td class=\"wordmark\">QS/MC Management Console</td>",
    ),
    (
        "System: <b>QSHQ400</b> &nbsp;&nbsp; User: <b>QSECOFR</b>",
        "Console: <b>QSHQMC1</b> &nbsp;&nbsp; User: <b>qsadmin</b>",
    ),
    (
        "QS/400 V1R0M0 &middot; Quartz Systems</td>",
        "QS/MC V1R0M0 &middot; Quartz Systems</td>",
    ),
    (" - QS/400</title>", " - QS/MC Management Console</title>"),
];

/// A shared page as the server renders it: in the product's frame.
fn shared_render(page: &mut qsweb::frame::Page) -> String {
    page.generic = false;
    render("/qsmc/error", page)
}

#[test]
fn shared_error() {
    let c = ctx("/qsmc/error");
    let sl = qsweb::msg::SecondLevel {
        id: "CPF2204".into(),
        text: "User profile WEBUSER not found.".into(),
        cause: "The user profile WEBUSER specified for the server user profile does not exist on system QSHQ400, or it was deleted after the instance configuration was saved.".into(),
        recovery: "Create the user profile with the CRTUSRPRF command, or change the instance to use an existing user profile (for example QTMHHTTP) on the General page of the instance, then try the request again.".into(),
        severity: 40,
    };
    let sent = qsweb::pages::Sent {
        msg_type: "Escape".into(),
        date: "10/29/26".into(),
        time: "14:02:17".into(),
        job: "123457/QTMHHTTP/ADMIN".into(),
        program: "QSYCUSRP".into(),
        technical: "Request:  POST /qs400/httpadmin/instance/WEBSTORE/general\nReturned: HTP8201 severity 30 from QHTTPADM\nJob log:  123457/QTMHHTTP/ADMIN".into(),
    };
    let mut page = qsweb::pages::second_level(&c, &sl, Some(&sent), "Error");
    let mut fix = QS400_FRAME.to_vec();
    fix.extend([
        ("HTTP Server Administration &gt; Error", "QS/MC &gt; Error"),
        // QS/MC has no job log.
        ("<input type=\"submit\" value=\"Job log\">", ""),
        (
            "<h2>Technical description</h2>",
            "<h2>Technical details</h2>",
        ),
    ]);
    assert_shared(&shared_render(&mut page), "error.html", &fix);
}

#[test]
fn shared_notauth() {
    let c = ctx("/qsmc/notauth");
    let r = qsweb::pages::Refusal {
        message: qsweb::msg::Message::new(
            qsweb::msg::Kind::Error,
            "CPF2207",
            "Not authorized to use object ADMIN in library QSYS type *JOB.",
        ),
        explain: "You are not authorized to use this function. Your user profile <span class=\"mono\">WEBUSER</span> has user class <span class=\"mono\">*USER</span> and does not have the <span class=\"mono\">*ALLOBJ</span> or <span class=\"mono\">*JOBCTL</span> special authority required to end jobs that belong to other users.".into(),
        function: "End job".into(),
        object: "123456/QTMHHTTP/ADMIN".into(),
        required: "*JOBCTL".into(),
        user: "WEBUSER".into(),
        policy: Some("Access for Web policy \"Jobs - End server jobs\" is set to <b>Deny</b> for group <span class=\"mono\">WEBUSERS</span>.".into()),
        crumb: vec!["Work Management".into(), "Active Jobs".into()],
    };
    let mut page = qsweb::pages::not_authorized(&c, &r);
    let mut fix = QS400_FRAME.to_vec();
    fix.extend([
        ("Navigator &gt; Work Management", "QS/MC &gt; Work Management"),
        (
            "Contact the security officer (QSECOFR) if you need this authority. The attempt has been recorded in the audit journal (entry type AF).",
            webd::product::BRAND.notauth_note,
        ),
    ]);
    assert_shared(&shared_render(&mut page), "notauth.html", &fix);
}

#[test]
fn shared_confirm() {
    use qsweb::partials::{Cell, Col, List, list_table, mono, status};
    let c = ctx("/qsmc/confirm");
    let list = List {
        cols: ["Instance name", "Status", "Server root", "Job", "Log files"]
            .iter()
            .map(|l| Col::plain(l))
            .collect(),
        rows: vec![vec![
            Cell::text("WEBSTORE"),
            Cell::html(status("Stopped", Some(false))),
            Cell::html(mono("/www/webstore")),
            Cell::html(mono("(not active)")),
            Cell::text("14 files, 212 MB"),
        ]],
        empty: String::new(),
        keep: Vec::new(),
    };
    let table = format!(
        "{}<p class=\"small grey\">The instance must be stopped before it can be deleted. Running instances are skipped and message HTP8103 is issued.</p>",
        list_table(&list)
    );
    let body = qsweb::pages::confirm_body(
        &c,
        "Deleting an instance removes its configuration directory and all log files. Press OK to delete the instances listed below, or Cancel to return without deleting.",
        &table,
        "",
    );
    let mut page = qsweb::frame::Page {
        title: "Confirm Delete Instance".into(),
        crumb: vec![
            "Manage".into(),
            "Server instances".into(),
            "Confirm Delete Instance".into(),
        ],
        h1: "Confirm Delete Instance".into(),
        body,
        help: "confirm".into(),
        ..Default::default()
    };
    let mut fix = QS400_FRAME.to_vec();
    fix.push((
        "HTTP Server Administration &gt; Manage",
        "QS/MC &gt; Manage",
    ));
    assert_shared(&shared_render(&mut page), "confirm.html", &fix);
}

#[test]
fn shared_help() {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../design/qsmc-prototype/shared/help.html");
    let g = std::fs::read_to_string(p).unwrap();
    // The window's body is the topic's own text; the window is QS/MC's.
    let body = g
        .split_once("<h1>Active jobs</h1>\n")
        .and_then(|(_, rest)| rest.split_once("\n<hr>"))
        .map(|(b, _)| b.to_string())
        .unwrap();
    let served = qsweb::frame::help(&webd::product::PRODUCT, "Active jobs", &body);
    assert_shared(
        &served,
        "help.html",
        &[
            (
                "<td class=\"wordmark\">QS/400 Help</td>",
                "<td class=\"wordmark\">QS/MC Help</td>",
            ),
            (" - QS/400</title>", " - QS/MC</title>"),
            (
                "QS/400 V1R0M0 &middot; Quartz Systems &middot;",
                "QS/MC V1R0M0 &middot; Quartz Systems &middot;",
            ),
            (
                "<a href=\"#\" >Contents</a>",
                "<a href=\"help.html\" >Contents</a>",
            ),
            ("<a href=\"#\">Index</a>", "<a href=\"help.html\">Index</a>"),
            // qsweb's window closes by an inline handler, which QS/MC's
            // content security policy blocks (`docs/OPEN_QUESTIONS.md`).
            (
                "<a href=\"#\">Close window</a>",
                "<a href=\"#\" onclick=\"if (history.length > 1) { history.back(); } else { window.close(); } return false;\">Close window</a>",
            ),
        ],
    );
}

#[test]
fn console_settings() {
    let v = webpages::console::Settings {
        console: "QSHQMC1".into(),
        values: [
            "4 users, 4 roles",
            "Public 10.0.10.40/24, private 192.168.250.1/24",
            "10/30/26 07:14:02 EDT, NTP 10.0.10.1",
            "qshqmc1.quartz.example, expires 03/14/27",
            "Last backup 10/26/26 02:00 to backup.quartz.example",
            "QS/MC V1R0M0 build 26290 - current",
            "Uptime 41 days 3:12",
            "English (US), MM/DD/YY, 24-hour",
            "30 minutes idle",
            "V1R0M0 build 26290",
        ]
        .map(String::from)
        .to_vec(),
        messages: Vec::new(),
    };
    let c = ctx("/qsmc/console");
    assert_golden(
        &render("/qsmc/console", &webpages::console::page(&c, &v)),
        "console.html",
        &[],
    );
}

fn user(id: &str, d: &str, role: &str, last: &str, from: &str, on: bool) -> webpages::users::Row {
    webpages::users::Row {
        id: id.into(),
        description: d.into(),
        role: role.into(),
        last: last.into(),
        from: from.into(),
        enabled: on,
    }
}

#[test]
fn console_users() {
    let v = webpages::users::Users {
        rows: vec![
            user(
                "qsadmin",
                "Console administrator",
                "Administrator",
                "10/30/26 06:58",
                "10.0.10.55",
                true,
            ),
            user(
                "operator1",
                "Operations desk",
                "Operator",
                "10/29/26 22:10",
                "10.0.10.61",
                true,
            ),
            user(
                "viewer",
                "Read-only monitoring",
                "Viewer",
                "10/28/26 09:00",
                "10.0.22.7",
                true,
            ),
            user(
                "qsservice",
                "Quartz service access",
                "Service",
                "10/02/26 14:30",
                "172.16.0.9",
                false,
            ),
        ],
        form: webpages::users::Form::None,
        messages: Vec::new(),
    };
    let c = ctx("/qsmc/console/users");
    assert_golden(
        &render("/qsmc/console/users", &webpages::users::users_page(&c, &v)),
        "console-users.html",
        &[],
    );
}

#[test]
fn console_roles() {
    let c = ctx("/qsmc/console/roles");
    let page = webpages::users::roles_page(&c, &webusers::Matrix::default(), Vec::new());
    assert_golden(
        &render("/qsmc/console/roles", &page),
        "console-roles.html",
        &[],
    );
}

#[test]
fn console_network() {
    use webpages::network::{Link, Net, page};
    let v = Net {
        open: Link {
            name: "eth0".into(),
            state: "1 GbE, link up".into(),
        },
        ip: "10.0.10.40".into(),
        mask: "255.255.255.0".into(),
        gateway: "10.0.10.1".into(),
        dns: "10.0.10.1, 10.0.10.2".into(),
        host: "qshqmc1".into(),
        domain: "quartz.example".into(),
        private: Link {
            name: "eth1".into(),
            state: "1 GbE, link up".into(),
        },
        pip: "192.168.250.1".into(),
        pmask: "255.255.255.0".into(),
        range: "192.168.250.100-199".into(),
        dhcp: true,
        reached: "QSHQVS1 (192.168.250.101)".into(),
        allow: "10.0.0.0/8, 172.16.0.0/12".into(),
        ssh: false,
        messages: Vec::new(),
    };
    let c = ctx("/qsmc/console/network");
    assert_golden(
        &render("/qsmc/console/network", &page(&c, &v)),
        "console-network.html",
        &[],
    );
}

#[test]
fn console_datetime() {
    use webpages::datetime::{DateTime, page};
    let v = DateTime {
        now: "10/30/26 07:14:02 EDT".into(),
        zones: vec![
            "America/New_York (Eastern)".into(),
            "UTC".into(),
            "America/Chicago".into(),
        ],
        zone: String::new(),
        from_servers: true,
        servers: "10.0.10.1, time.quartz.example".into(),
        date: "10/30/26".into(),
        time: "07:14".into(),
        status: ("Active".into(), Some(true)),
        detail: ", last sync 07:12:00, offset +0.002 s".into(),
        serve: true,
        messages: Vec::new(),
    };
    let c = ctx("/qsmc/console/datetime");
    assert_golden(
        &render("/qsmc/console/datetime", &page(&c, &v)),
        "console-datetime.html",
        &[],
    );
}

#[test]
fn console_certs() {
    use webpages::certs::{Ca, Certs, page};
    let v = Certs {
        subject: "CN=qshqmc1.quartz.example, O=Quartz Systems, C=US".into(),
        issuer: "Quartz Internal CA".into(),
        valid: "03/14/26 to 03/14/27".into(),
        sha256: "7A:11:C0:3F:...:B2:90".into(),
        key: "RSA 2048".into(),
        status: ("Active".into(), Some(true)),
        note: " (expires in 135 days)".into(),
        cas: vec![
            Ca {
                label: "Quartz Internal CA".into(),
                subject: "CN=Quartz Internal Certificate Authority, O=Quartz Systems, C=US".into(),
                expires: "01/01/36".into(),
            },
            Ca {
                label: "Quartz Update Server CA".into(),
                subject: "CN=Quartz Update Services CA, O=Quartz Systems, C=US".into(),
                expires: "01/01/34".into(),
            },
        ],
        details: None,
        messages: Vec::new(),
    };
    let c = ctx("/qsmc/console/certs");
    assert_golden(
        &render("/qsmc/console/certs", &page(&c, &v)),
        "console-certs.html",
        &[],
    );
}

#[test]
fn console_backup() {
    use webpages::backup::{BackupTab, Row, backup_page};
    let row = |date: &str, file: &str, size: &str| Row {
        date: date.into(),
        destination: format!("backup.quartz.example:/backups/qsmc/{file}"),
        size: size.into(),
        status: ("Complete".into(), Some(true)),
    };
    let v = BackupTab {
        data: true,
        history: true,
        media: false,
        download: false,
        host: "backup.quartz.example".into(),
        path: "/backups/qsmc/".into(),
        user: "qsmcbackup".into(),
        last: vec![
            row("10/26/26 02:00", "qshqmc1-20261026.qmcb", "3.1 MB"),
            row("10/19/26 02:00", "qshqmc1-20261019.qmcb", "3.0 MB"),
            row("10/12/26 02:00", "qshqmc1-20261012.qmcb", "3.0 MB"),
        ],
        messages: Vec::new(),
    };
    let c = ctx("/qsmc/console/backup");
    assert_golden(
        &render("/qsmc/console/backup", &backup_page(&c, &v)),
        "console-backup.html",
        &[],
    );
}

#[test]
fn console_shutdown() {
    use webpages::shutdown::{Shutdown, page};
    let v = Shutdown {
        console: "QSHQMC1".into(),
        uptime: "41 days 3:12".into(),
        systems: "2".into(),
        terminals: "1 (qsadmin -> QSDEV01)".into(),
        scheduled: "0".into(),
        restart: true,
        message: "Console restart for update 26300, back in 5 minutes".into(),
        messages: Vec::new(),
    };
    let c = ctx("/qsmc/console/shutdown");
    // The choice and the message are in the form, so they are sent.
    assert_golden(
        &render("/qsmc/console/shutdown", &page(&c, &v)),
        "console-shutdown.html",
        &[
            (
                "</table><p class=\"small grey\"><input type=\"radio\" name=\"how\"",
                "</table><form method=\"post\"><p class=\"small grey\"><input type=\"radio\" name=\"how\"",
            ),
            (
                "size=\"50\"></p><form method=\"post\"><div",
                "size=\"50\"></p><div",
            ),
        ],
    );
}

#[test]
fn console_locale() {
    use webpages::locale::{Locale, page};
    let v = Locale {
        language: String::new(),
        date: String::new(),
        time: String::new(),
        number: String::new(),
        idle: "30".into(),
        max: "12".into(),
        terminals: true,
        messages: Vec::new(),
    };
    let c = ctx("/qsmc/console/locale");
    assert_golden(
        &render("/qsmc/console/locale", &page(&c, &v)),
        "console-locale.html",
        &[],
    );
}

#[test]
fn console_version() {
    use webpages::version::{Version, page};
    let v = Version {
        release: "V1R0M0".into(),
        build: "26290".into(),
        built: "10/17/26".into(),
        fixes: "QMC10102, QMC10115".into(),
        kernel: "qsmc-kernel 1.0.4".into(),
        web: "qsmc-web 1.0.4".into(),
        uptime: "41 days 3:12 (since 09/19/26 04:02)".into(),
        platform: "Virtual machine (KVM on PVE-01)".into(),
        processors: "2 vCPU".into(),
        memory: "4,096 MB".into(),
        disk: "40 GB (18% used)".into(),
        interfaces: "eth0 (public), eth1 (private management)".into(),
        serial: "<span class=\"mono\">ttyS0</span>, 115200, login enabled".into(),
        messages: Vec::new(),
    };
    let c = ctx("/qsmc/console/version");
    assert_golden(
        &render("/qsmc/console/version", &page(&c, &v)),
        "console-version.html",
        &[],
    );
}

#[test]
fn shared_components() {
    let c = ctx("/qsmc/dev/components");
    let mut fix = QS400_FRAME.to_vec();
    fix.extend([
        (
            "QS/400 &gt; Shared &gt; Pattern reference",
            "QS/MC &gt; Shared &gt; Pattern reference",
        ),
        // QS/MC has no job pages: the job actions lead to the page itself.
        ("../nav/active-jobs.html", "components.html"),
        ("../nav/end-job-confirm.html", "components.html"),
        ("../nav/job-props-general.html", "components.html"),
    ]);
    assert_shared(
        &render("/qsmc/dev/components", &webpages::components::page(&c)),
        "components.html",
        &fix,
    );
}
