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
    assert_golden_script(served, file, fix, None);
}

/// The same, the server's `script` element (by its source) dropped.
fn assert_golden_script(served: &str, file: &str, fix: &[(&str, &str)], script: Option<&str>) {
    let mut g = prototype(file);
    for (from, to) in fix {
        assert!(
            g.contains(from),
            "golden fix not applicable to {file}: {from}"
        );
        g = g.replace(from, to);
    }
    let map = |u: &str| proto(u);
    if let Some(d) = diff(served, &g, &Normalize { url: &map, script }) {
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

#[test]
fn connectivity() {
    use webpages::systems::connectivity::{Connectivity, Row, page};
    let row = |name: &str, address: &str, last: &str, age: u64, sp: &str| Row {
        name: name.into(),
        address: address.into(),
        state: "Connected".into(),
        good: true,
        last: last.into(),
        age,
        release: sp.into(),
        protocol: "QSMP/2 over TLS 1.3".into(),
    };
    let v = Connectivity {
        rows: vec![
            row("QSHQVS1", "192.168.250.101:30000 (private)", "10/30/26 07:14:00", 42, "1.0.4"),
            row("QSHQ400", "10.0.10.20:30000 (public)", "10/30/26 07:13:58", 121, "1.0.2"),
        ],
        tests: vec![
            "10/30/26 07:10:02  QSHQVS1  192.168.250.101:30000  TCP connect 1 ms, TLS handshake 9 ms, auth OK, heartbeat 12 ms  -  OK".into(),
            "10/30/26 07:10:03  QSHQ400  10.0.10.20:30000        TCP connect 1 ms, TLS handshake 11 ms, auth OK, heartbeat 14 ms  -  OK (password older than 90 days)".into(),
        ],
        messages: Vec::new(),
    };
    // Reset connection, and Change password with it, pair again on the
    // add form.
    assert_golden(
        &render("/qsmc/connectivity", &page(&ctx("/qsmc/connectivity"), &v)),
        "connectivity.html",
        &[(
            "<a href=\"connectivity.html\">Reset connection</a> | <a href=\"connectivity.html\">Change password</a>",
            "<a href=\"add-system.html\">Reset connection</a> | <a href=\"add-system.html\">Change password</a>",
        )],
    );
}

#[test]
fn add_system() {
    use webpages::systems::add::{AddForm, form_page};
    let v = AddForm {
        host: "10.0.10.30".into(),
        port: "30000".into(),
        ..AddForm::default()
    };
    // QSI trusts a system by the certificate pinned when it paired, not
    // by an authority.
    assert_golden(
        &render(
            "/qsmc/servers/add",
            &form_page(&ctx("/qsmc/servers/add"), &v),
        ),
        "add-system.html",
        &[(
            "<input type=\"checkbox\" name=\"tls\" checked> Require a certificate signed by the Quartz Internal CA",
            "<span class=\"grey\">The system's own certificate, pinned when paired: compare its fingerprint with the one the system shows</span>",
        )],
    );
}

#[test]
fn add_system_result() {
    use webpages::systems::add::{Found, result_page};
    let v = Found {
        name: "QSHQVS2".into(),
        kind: "QS/VS host".into(),
        model: "9Q10-V1".into(),
        serial: "10-9F11B7".into(),
        version_label: "QS/VS version".into(),
        release: "V1R0M0".into(),
        build: "26280".into(),
        state: "Operating".into(),
        processors: "8 installed, 8 configurable".into(),
        memory: "131,072 MB installed".into(),
        partitions: "0 defined".into(),
        code: "00000000".into(),
        fingerprint: "a1b2c3".into(),
        messages: vec![qmcmsg::message(
            qsweb::msg::Kind::Done,
            &qmcmsg::SYSTEM_CONNECTED,
            &["QSHQVS2", "1.0.4"],
        )],
    };
    // The system's name goes with Add and Cancel; the certificate's
    // fingerprint is shown to compare with the system's own.
    assert_golden(
        &render(
            "/qsmc/servers/add",
            &result_page(&ctx("/qsmc/servers/add"), &v),
        ),
        "add-system-result.html",
        &[
            (
                "<h2>Found</h2><form method=\"post\"><table",
                "<h2>Found</h2><form method=\"post\"><input type=\"hidden\" name=\"ms\" value=\"QSHQVS2\"><table",
            ),
            (
                "00000000</a></td></tr>\n</table>",
                "00000000</a></td></tr>\n<tr><th>Certificate SHA-256</th><td><span class=\"mono\">A1:B2:C3</span></td></tr>\n</table>",
            ),
        ],
    );
}

/// The prototype's two headquarters systems as Servers lists them.
fn servers_hq() -> Vec<webpages::systems::servers::Row> {
    use webpages::systems::servers::Row;
    vec![
        Row {
            name: "QSHQ400".into(),
            kind: "Standalone QS/400".into(),
            status: "Operating".into(),
            processors: "0".into(),
            memory: "0".into(),
            model: "9Q04-M2".into(),
            serial: "10-7A2B9C".into(),
            code: "00000000".into(),
            partitions: 1,
        },
        Row {
            name: "QSHQVS1".into(),
            kind: "QS/VS host".into(),
            status: "Operating".into(),
            processors: "3.50".into(),
            memory: "40.0".into(),
            model: "9Q10-V1".into(),
            serial: "10-9F11A2".into(),
            code: "00000000".into(),
            partitions: 3,
        },
    ]
}

fn servers_view(
    rows: Vec<webpages::systems::servers::Row>,
    total: usize,
    messages: Vec<qsweb::msg::Message>,
) -> webpages::systems::servers::Servers {
    webpages::systems::servers::Servers {
        rows,
        kind: "All".into(),
        status: "All".into(),
        paging: qsweb::partials::Paging {
            page: 1,
            rows: 25,
            total,
            keep: Vec::new(),
        },
        first: 1,
        messages,
    }
}

/// The prototype's Servers states other than the first draw Systems
/// Management closed; the console opens it on every state of the page.
const SERVERS_NAV: (&str, &str) = (
    "<a class=\"glyph\" href=\"servers.html\">[+]</a> <b>Systems Management</b></div>",
    "<a class=\"glyph\" href=\"servers.html\">[-]</a> <b>Systems Management</b></div>\n<div class=\"navitem\"><a href=\"servers.html\" class=\"navcur\">Servers</a></div>\n<div class=\"navitem\"><a href=\"partitions.html\">Partitions</a></div>\n<div class=\"navitem\"><a href=\"groups.html\">Custom groups</a></div>",
);

fn servers_render(v: &webpages::systems::servers::Servers) -> String {
    render(
        "/qsmc/servers",
        &webpages::systems::servers::page(&ctx("/qsmc/servers"), v),
    )
}

#[test]
fn servers() {
    let m = qmcmsg::message(
        qsweb::msg::Kind::Done,
        &qmcmsg::SYSTEM_CONNECTED,
        &["QSHQVS1", "1.0.4"],
    );
    let v = servers_view(servers_hq(), 2, vec![m]);
    assert_golden(&servers_render(&v), "servers.html", &[]);
}

#[test]
fn servers_empty() {
    let v = servers_view(Vec::new(), 0, Vec::new());
    assert_golden(&servers_render(&v), "servers-empty.html", &[SERVERS_NAV]);
}

#[test]
fn servers_error() {
    let m = qsweb::msg::Message::new(
        qsweb::msg::Kind::Error,
        "QMC2210",
        "Power off request for QSHQVS1 not performed. 2 partitions are running; shut them down first or choose Power off with partitions.",
    );
    let v = servers_view(servers_hq(), 2, vec![m]);
    assert_golden(&servers_render(&v), "servers-error.html", &[SERVERS_NAV]);
}

#[test]
fn servers_paged() {
    use webpages::systems::servers::Row;
    // The 25 branch hosts of the prototype's first page.
    let rows: Vec<Row> = (1..=25u32)
        .map(|n| {
            let i = n - 1;
            let status = match n % 7 {
                4 => "Standby",
                6 => "No connection",
                _ => "Operating",
            };
            let units = 100 + 50 * (i % 4);
            Row {
                name: format!("QSBR{n:02}VS1"),
                kind: "QS/VS host".into(),
                status: status.into(),
                processors: format!("{}.{:02}", units / 100, units % 100),
                memory: format!("{}.0", 16 + 8 * (i % 3)),
                model: "9Q10-V1".into(),
                serial: format!("10-9F{i:02X}{i:02X}"),
                code: if n % 7 == 6 { "B6000100" } else { "00000000" }.into(),
                partitions: (i % 4 + 1) as usize,
            }
        })
        .collect();
    let v = servers_view(rows, 27, Vec::new());
    // The prototype's paged list numbers its boxes from 0, its others
    // from 1: the console numbers each from 1.
    let renames: Vec<(String, String)> = (0..25)
        .rev()
        .map(|i| (format!("name=\"s{i}\""), format!("name=\"s{}\"", i + 1)))
        .collect();
    let mut fix: Vec<(&str, &str)> = renames
        .iter()
        .map(|(a, b)| (a.as_str(), b.as_str()))
        .collect();
    fix.push(SERVERS_NAV);
    // 27 entries at 25 a page are two pages.
    fix.push(("Page 1 of 3", "Page 1 of 2"));
    assert_golden(&servers_render(&v), "servers-paged.html", &fix);
}

fn system_tab(tab: webpages::systems::props::Tab, file: &str, fix: &[(&str, &str)]) {
    use webpages::systems::props::{Props, page};
    let v = Props {
        name: "QSHQVS1".into(),
        tab,
        messages: Vec::new(),
    };
    assert_golden(
        &render(
            "/qsmc/server/QSHQVS1/general",
            &page(&ctx("/qsmc/server/QSHQVS1/general"), &v),
        ),
        file,
        fix,
    );
}

#[test]
fn system_general() {
    use webpages::systems::props::{General, Tab};
    let g = General {
        name: "QSHQVS1".into(),
        kind: "QS/VS host".into(),
        model: "9Q10-V1".into(),
        serial: "10-9F11A2".into(),
        description: "Lab virtual server, rack R2".into(),
        state: "Operating".into(),
        code: "00000000".into(),
        service: "1.0.4".into(),
        version_label: "QS/VS version".into(),
        release: "V1R0M0".into(),
        build: "26280".into(),
        update: true,
        since: "09/18/26 08:02:11".into(),
        address: "10.0.10.30:30000".into(),
        defined: 3,
        running: 2,
    };
    system_tab(Tab::General(g), "system-general.html", &[]);
}

#[test]
fn system_processors() {
    use webpages::systems::props::{Processors, Tab};
    let row = |a: &str, b: &str, c: &str, d: &str| [a.into(), b.into(), c.into(), d.into()];
    let p = Processors {
        amounts: ["8".into(), "8".into(), "3.50".into(), "0".into()],
        pool: "6.00 (DefaultPool)".into(),
        dedicated: "2 (QSPROD01)".into(),
        rows: vec![
            row("QSPROD01", "Dedicated", "2.00", "2"),
            row("QSDEV01", "Shared, uncapped", "1.00", "2"),
            row("QSTEST01", "Shared, capped", "1.50", "2"),
        ],
    };
    system_tab(Tab::Processors(p), "system-processors.html", &[]);
}

#[test]
fn system_memory() {
    use webpages::systems::props::{Memory, Tab};
    let row = |a: &str, b: &str, c: &str, d: &str, e: &str| {
        [a.into(), b.into(), c.into(), d.into(), e.into()]
    };
    let m = Memory {
        amounts: [
            "131,072 MB".into(),
            "131,072 MB".into(),
            "40,960 MB".into(),
            "2,048 MB".into(),
            "256 MB".into(),
            "88,064 MB".into(),
        ],
        rows: vec![
            row("QSPROD01", "65,536", "32,768", "65,536", "98,304"),
            row("QSDEV01", "16,384", "8,192", "16,384", "32,768"),
            row("QSTEST01", "0 (not activated)", "4,096", "8,192", "16,384"),
        ],
    };
    system_tab(Tab::Memory(m), "system-memory.html", &[]);
}

#[test]
fn system_io() {
    use webpages::systems::props::Tab;
    let row = |slot: &str, d: &str, o: &str, b: &str| {
        [
            format!("U9Q10.V1.9F11A2-P1-{slot}"),
            slot.into(),
            d.into(),
            o.into(),
            b.into(),
        ]
    };
    let rows = vec![
        row(
            "C1",
            "10 GbE SFP+ 2-port adapter",
            "QS/VS host (shared through virtual Ethernet)",
            "PCIe3 x8",
        ),
        row(
            "C2",
            "SAS RAID controller (6 x 1.9 TB SSD)",
            "QS/VS host (storage pool POOL1)",
            "PCIe3 x8",
        ),
        row(
            "C3",
            "Fibre Channel 8 Gb 2-port (QLE2562)",
            "QSPROD01 (physical assignment)",
            "PCIe2 x8",
        ),
        row(
            "C4",
            "1 GbE 4-port adapter",
            "QS/VS host (management)",
            "PCIe2 x4",
        ),
        row("C5", "Empty", "", "PCIe3 x16"),
        row("T1", "USB 3.0 controller", "QS/VS host", "Planar"),
    ];
    system_tab(Tab::Io(rows), "system-io.html", &[]);
}

#[test]
fn system_power() {
    use webpages::systems::props::{Power, Tab};
    let p = Power {
        side: "A".into(),
        keylock: "Normal".into(),
    };
    system_tab(Tab::Power(p), "system-power.html", &[]);
}

#[test]
fn system_capabilities() {
    use webpages::systems::props::Tab;
    let rows = [
        ("Partitioning", "Yes (QS/VS)"),
        ("Maximum partitions", "64"),
        ("Shared processor pools", "Yes, 4 pools"),
        ("Dedicated processors", "Yes"),
        ("Virtual Ethernet", "Yes, 802.1Q VLAN aware"),
        ("Virtual storage (SCSI client)", "Yes"),
        ("Virtual optical", "Yes"),
        ("Virtual tape", "Yes, including FC pass-through"),
        ("Live partition migration", "No"),
        ("Service processor redundancy", "No"),
        ("Remote console (terminal window)", "Yes"),
        ("Control panel functions", "Yes, 01-70"),
    ]
    .iter()
    .map(|(a, b)| ((*a).to_string(), (*b).to_string()))
    .collect();
    system_tab(Tab::Capabilities(rows), "system-capabilities.html", &[]);
}

#[test]
fn system_refcode() {
    use webpages::systems::props::{Current, Tab};
    let c = Current {
        code: "00000000".into(),
        meaning: "Operating; no attention required.".into(),
        since: "09/18/26 08:04:50".into(),
        words: ["00000000"; 8].join(" "),
    };
    system_tab(Tab::Refcode(c), "system-refcode.html", &[]);
}

#[test]
fn confirm_poweroff() {
    use webpages::systems::power::{Ended, Off, off_page};
    let p = |name: &str, id: u32, state: &str, action: &str| Ended {
        name: name.into(),
        id,
        state: state.into(),
        code: "00000000".into(),
        action: action.into(),
    };
    let then = "Shut down (delayed, 300 s) then power off";
    let v = Off {
        name: "QSHQVS1".into(),
        restart: false,
        partitions: vec![
            p("QSPROD01", 3, "Running", then),
            p("QSDEV01", 1, "Running", then),
            p("QSTEST01", 2, "Not activated", "None"),
        ],
    };
    // The choice is in the form, so it is sent with OK.
    assert_golden(
        &render(
            "/qsmc/server/QSHQVS1/confirm",
            &off_page(&ctx("/qsmc/server/QSHQVS1/confirm"), &v),
        ),
        "confirm-poweroff.html",
        &[
            (
                "</table><p class=\"small grey\">Power off:",
                "</table><form method=\"post\"><input type=\"hidden\" name=\"ms\" value=\"QSHQVS1\"><p class=\"small grey\">Power off:",
            ),
            (
                "without shutdown</p><form method=\"post\"><div class=\"buttons\">",
                "without shutdown</p><div class=\"buttons\">",
            ),
        ],
    );
}

#[test]
fn confirm_poweron() {
    use webpages::systems::power::{On, on_page};
    let v = On {
        name: "QSHQVS1".into(),
        kind: "QS/VS host".into(),
        state: "Power off".into(),
        auto: "QSPROD01, QSDEV01".into(),
    };
    assert_golden(
        &render(
            "/qsmc/server/QSHQVS1/confirm",
            &on_page(&ctx("/qsmc/server/QSHQVS1/confirm"), &v),
        ),
        "confirm-poweron.html",
        &[
            (
                "</table><p class=\"small grey\">IPL side:",
                "</table><form method=\"post\"><input type=\"hidden\" name=\"ms\" value=\"QSHQVS1\"><p class=\"small grey\">IPL side:",
            ),
            (
                "partition standby</option></select></p><form method=\"post\"><div class=\"buttons\">",
                "partition standby</option></select></p><div class=\"buttons\">",
            ),
        ],
    );
}

#[test]
fn refcode_history() {
    use webpages::refcodes::history::{History, Row, page};
    let rows: Vec<Row> = [
        ("10/30/26 06:02:14", "00000000", "00000000", "", "QSDEV01"),
        ("10/30/26 06:01:40", "C6004031", "00000003", "", "QSDEV01"),
        ("10/30/26 06:01:12", "C6004025", "00000003", "", "QSDEV01"),
        ("10/30/26 06:00:58", "C6004010", "00000003", "", "QSDEV01"),
        ("10/30/26 06:00:31", "C2002100", "00000001", "", "QSDEV01"),
        ("10/30/26 06:00:02", "C1001008", "00000000", "", "QSDEV01"),
        ("10/29/26 23:30:44", "A6005008", "00000002", "", "QSTEST01"),
        ("10/29/26 23:30:10", "C6004031", "00000002", "", "QSTEST01"),
        (
            "10/29/26 22:14:05",
            "B6000100",
            "00000002",
            "4B4F4E53",
            "QSTEST01",
        ),
        ("10/29/26 22:13:52", "A6001000", "00000002", "", "QSTEST01"),
        ("10/29/26 14:20:00", "00000000", "00000000", "", "QSPROD01"),
        ("10/18/26 08:04:50", "00000000", "00000000", "", "(host)"),
        ("09/18/26 08:02:11", "C1001000", "00000000", "", "(host)"),
    ]
    .iter()
    .map(|(when, code, w2, w3, p)| {
        let mut words = vec![(*w2).to_string()];
        if !w3.is_empty() {
            words.push((*w3).to_string());
        }
        Row {
            when: (*when).into(),
            code: (*code).into(),
            words,
            partition: (*p).into(),
            link: format!("/refcode/{code}?ms=QSHQVS1"),
        }
    })
    .collect();
    let v = History {
        system: "QSHQVS1".into(),
        systems: vec!["QSHQVS1".into(), "QSHQ400".into()],
        partition: "All".into(),
        partitions: ["All", "(host)", "QSDEV01", "QSTEST01", "QSPROD01"]
            .iter()
            .map(|s| (*s).to_string())
            .collect(),
        from: "10/29/26".into(),
        to: "10/30/26".into(),
        rows,
        paging: qsweb::partials::Paging {
            page: 1,
            rows: 25,
            total: 88,
            keep: Vec::new(),
        },
        messages: Vec::new(),
    };
    assert_golden(
        &render("/qsmc/refcodes", &page(&ctx("/qsmc/refcodes"), &v)),
        "refcode-history.html",
        &[],
    );
}

#[test]
fn refcode() {
    use webpages::refcodes::page::{Explanation, Reporter, page};
    let v = Explanation {
        code: "A6005008".into(),
        family: "A6xx - partition attention, operating system waiting".into(),
        words: "00000002 00000000 00000000 00000000 00000000 00000000 00000000 00000000".into(),
        by: Some(Reporter {
            system: "QSHQVS1".into(),
            partition: Some(("QSTEST01".into(), 2)),
            when: "10/29/26 23:30:44".into(),
        }),
        severity: "Attention - IPL is waiting, no hardware fault".into(),
        meaning: "The partition reached the point in a manual (keylock Manual, attended) IPL where it presents the dedicated service tools or the sign-on display on its console, and no console device is connected. Word 2 is the partition ID.".into(),
        cause: "The console is a terminal window through the virtual serial adapter and no QS/MC session has opened one, or the previous session disconnected (QMC4001). This also appears when the console device in the partition settings does not match the adapter the operating system found.".into(),
        recovery: "Open a terminal window on the partition (Operations > Open terminal window). The IPL continues as soon as the console connects. If the code remains after the console is connected, select control panel function 21 to force DST to the console, then check the console device setting under Settings.".into(),
        messages: Vec::new(),
    };
    assert_golden(
        &render(
            "/qsmc/refcode/A6005008",
            &page(&ctx("/qsmc/refcode/A6005008"), &v),
        ),
        "refcode.html",
        &[],
    );
}

fn terminal_window(connected: bool) -> webpages::terminal::window::Window {
    use webpages::terminal::window::Window;
    Window {
        host: "QSHQVS1".into(),
        id: if connected { 1 } else { 2 },
        partition: if connected { "QSDEV01" } else { "QSTEST01" }.into(),
        connected,
        code: if connected { "00000000" } else { "A6005008" }.into(),
        meaning: if connected {
            "Operating"
        } else {
            "Waiting for console device; no console session is connected."
        }
        .into(),
        strip: true,
        scale: 1,
        size: (720, 400),
        cursor: (24, 6),
        may_type: true,
        keep: String::new(),
        script: true,
        bw: false,
        messages: if connected {
            Vec::new()
        } else {
            vec![qsweb::msg::Message::new(
                qsweb::msg::Kind::Error,
                "QMC4001",
                "Console connection to partition QSTEST01 closed by the managed system.",
            )]
        },
    }
}

fn terminal_render(v: &webpages::terminal::window::Window) -> String {
    use webpages::terminal::window::{page, without_nav};
    let here = "/qsmc/terminal/QSHQVS1/1";
    without_nav(&render(here, &page(&ctx(here), v)))
}

/// The picture is the script's to replace, so it has its name.
const TERM_ID: (&str, &str) = ("<img src=", "<img id=\"term\" src=");

#[test]
fn terminal() {
    assert_golden_script(
        &terminal_render(&terminal_window(true)),
        "terminal.html",
        &[TERM_ID],
        Some("/qsmc/terminal/script.js"),
    );
}

#[test]
fn terminal_disconnected() {
    // The window's status line names the partition the window is of.
    assert_golden(
        &terminal_render(&terminal_window(false)),
        "terminal-disconnected.html",
        &[
            TERM_ID,
            (
                "QSDEV01 on QSHQVS1: Not connected",
                "QSTEST01 on QSHQVS1: Not connected",
            ),
        ],
    );
}

#[test]
fn terminal_config() {
    use webpages::terminal::config::{Choice, Config, page};
    let choice = |label: &str| Choice {
        label: label.into(),
        host: "QSHQVS1".into(),
        id: 1,
    };
    let v = Config {
        choices: vec![
            choice("QSTEST01 (ID 2) on QSHQVS1 - A6005008 waiting for console"),
            choice("QSDEV01 (ID 1) on QSHQVS1"),
            choice("QSPROD01 (ID 3) on QSHQVS1"),
            choice("QSHQ400 (standalone)"),
        ],
        chosen: "QSTEST01 (ID 2) on QSHQVS1 - A6005008 waiting for console".into(),
        wide: true,
        bw: false,
        strip: true,
        take_over: false,
        messages: Vec::new(),
    };
    // One form, not one in another; the strip's box named apart from
    // the code page; the buttons named.
    assert_golden(
        &render("/qsmc/terminal", &page(&ctx("/qsmc/terminal"), &v)),
        "terminal-config.html",
        &[
            (
                "<form method=\"post\" action=\"terminal.html\"><form method=\"post\"><table",
                "<form method=\"post\"><table",
            ),
            (
                "</table></form><div class=\"buttons\"><input type=\"submit\" value=\"Open\"><input type=\"submit\" value=\"Cancel\"></div></form>",
                "</table>\n<div class=\"buttons\"><input type=\"submit\" name=\"open\" value=\"Open\"><input type=\"submit\" name=\"cancel\" value=\"Cancel\"></div></form>",
            ),
            (
                "<input type=\"checkbox\" name=\"cp\" checked> Show above",
                "<input type=\"checkbox\" name=\"strip\" checked> Show above",
            ),
        ],
    );
}

#[test]
fn control_panel() {
    use webpages::terminal::panel::{Panel, page};
    let v = Panel {
        host: "QSHQVS1".into(),
        id: 2,
        partition: "QSTEST01".into(),
        state: "Not activated".into(),
        keylock: "Manual".into(),
        code: "A6005008".into(),
        meaning: "Waiting for console device; no console session is connected.".into(),
        chosen: 21,
        result: vec![
            "Function 21 - Force DST".into(),
            "Reference code before: A6005008  Words 2-9: 00000002 00000000 00000000 00000000 00000000 00000000 00000000 00000000".into(),
            "Reference code after:  A6005008  (DST will be presented when a console connects)".into(),
        ],
        messages: vec![qmcmsg::message(
            qsweb::msg::Kind::Done,
            &qmcmsg::FUNCTION_ACCEPTED,
            &["21", "QSTEST01"],
        )],
    };
    let here = "/qsmc/partition/QSHQVS1/2/panel";
    assert_golden(
        &render(here, &page(&ctx(here), &v)),
        "control-panel.html",
        &[],
    );
}
