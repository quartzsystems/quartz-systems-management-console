// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! The pages through their handler: sign-on and its rules, sessions,
//! the CSRF check, roles. The conformance suites do the same from
//! outside a running console.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicI64, AtomicU64, Ordering};

use qsweb::http::{Peer, Request, Response};
use webd::serve::Pages as _;
use webpages::App;
use webpages::app::Console;
use webusers::Role;

/// The clock, the confirmations, and the state directory PID 1 writes.
struct Fixed(Arc<AtomicU64>, Arc<AtomicU64>, PathBuf);

/// Where the test clock starts.
const START: u64 = 1_790_000_000;

impl Console for Fixed {
    fn name(&self) -> String {
        "QSHQMC1".into()
    }
    fn uptime(&self) -> u64 {
        self.0.load(Ordering::Relaxed) - START + 3600
    }
    fn now(&self) -> u64 {
        self.0
            .load(Ordering::Relaxed)
            .saturating_add_signed(SHIFT.with(|s| s.load(Ordering::Relaxed)))
    }
    fn apply_network_confirmed(&self, _n: &str, _s: u32) -> Result<(), String> {
        Ok(())
    }
    fn confirm_network(&self) -> Result<(), String> {
        self.1.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }
    fn set_host(&self, name: &str, zone: &str, ntp: &[String]) -> Result<(), String> {
        use state::Document as _;
        let host = state::Host {
            name: Some(name.into()),
            time_zone: zone.into(),
            ntp_servers: ntp.to_vec(),
        };
        host.validate()?;
        state::Store::at(&self.2)
            .save(&host)
            .map_err(|e| e.to_string())
    }
    fn power_down(&self, restart: bool, by: &str) -> Result<(), String> {
        POWERED.with(|p| *p.borrow_mut() = Some((restart, by.to_string())));
        Ok(())
    }
    fn set_clock(&self, secs: i64) -> Result<(), String> {
        let ticks = i64::try_from(self.0.load(Ordering::Relaxed)).unwrap_or(0);
        SHIFT.with(|s| s.store(secs - ticks, Ordering::Relaxed));
        Ok(())
    }
    fn fact(&self, key: &str) -> String {
        match key {
            "management" => "192.0.2.10/24 on eth0, gateway 192.0.2.1".into(),
            _ => String::new(),
        }
    }
}

const PASSWORD: &str = "Install-pass-7";

thread_local! {
    /// The confirmations of this thread's test console.
    static CONFIRMS: Arc<AtomicU64> = Arc::new(AtomicU64::new(0));
    /// How far this thread's test clock was set by hand.
    static SHIFT: AtomicI64 = const { AtomicI64::new(0) };
    /// The power-down this thread's test console was asked for.
    static POWERED: std::cell::RefCell<Option<(bool, String)>> = const { std::cell::RefCell::new(None) };
}

fn app(name: &str) -> (App, Arc<AtomicU64>, PathBuf) {
    let dir = std::env::temp_dir().join(format!("webpages-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    // `/data` as the console has it: the state folder in it.
    let state = dir.join("state");
    std::fs::create_dir_all(&state).unwrap();
    let db = state.join("users.db");
    let hash = users::hash(PASSWORD).unwrap();
    std::fs::write(&db, users::first("QSADMIN", &hash, 1).unwrap()).unwrap();
    let clock = Arc::new(AtomicU64::new(START));
    let app = App::open(
        &state,
        &db,
        Box::new(Fixed(
            clock.clone(),
            CONFIRMS.with(Clone::clone),
            state.clone(),
        )),
    )
    .unwrap();
    (app, clock, dir)
}

fn peer(ip: &str) -> Peer {
    Peer {
        addr: format!("{ip}:50000").parse().unwrap(),
        secure: true,
    }
}

struct Browser<'a> {
    app: &'a App,
    cookie: Option<String>,
    ip: &'static str,
    host: Option<&'static str>,
}

impl Browser<'_> {
    fn send(&mut self, method: &str, path: &str, form: &[(&str, &str)]) -> Response {
        let mut r = Request::new(method, path);
        if let Some(h) = self.host {
            r.headers.push(("host".into(), h.into()));
        }
        if let Some(c) = &self.cookie {
            r.headers.push(("cookie".into(), format!("QSMCSID={c}")));
        }
        if method == "POST" {
            r.headers.push((
                "content-type".into(),
                "application/x-www-form-urlencoded".into(),
            ));
            let body: Vec<String> = form
                .iter()
                .map(|(k, v)| {
                    format!(
                        "{}={}",
                        qsweb::http::form::encode(k),
                        qsweb::http::form::encode(v)
                    )
                })
                .collect();
            r.body = body.join("&").into_bytes();
        }
        let resp = self.app.page(&r, &peer(self.ip));
        if let Some(c) = resp.header("Set-Cookie") {
            let v = c
                .split(';')
                .next()
                .and_then(|kv| kv.split_once('='))
                .map(|(_, v)| v.to_string());
            self.cookie = v.filter(|v| !v.is_empty());
        }
        resp
    }

    /// A `multipart/form-data` POST: text fields, then the file `f`.
    fn upload(&mut self, path: &str, fields: &[(&str, &str)], file: &[u8]) -> Response {
        const B: &str = "qsmc-test-boundary";
        let mut body = Vec::new();
        for (k, v) in fields {
            body.extend_from_slice(
                format!("--{B}\r\nContent-Disposition: form-data; name=\"{k}\"\r\n\r\n{v}\r\n")
                    .as_bytes(),
            );
        }
        body.extend_from_slice(
            format!(
                "--{B}\r\nContent-Disposition: form-data; name=\"f\"; filename=\"c.pem\"\r\nContent-Type: application/octet-stream\r\n\r\n"
            )
            .as_bytes(),
        );
        body.extend_from_slice(file);
        body.extend_from_slice(format!("\r\n--{B}--\r\n").as_bytes());
        let mut r = Request::new("POST", path);
        if let Some(c) = &self.cookie {
            r.headers.push(("cookie".into(), format!("QSMCSID={c}")));
        }
        r.headers.push((
            "content-type".into(),
            format!("multipart/form-data; boundary={B}"),
        ));
        r.body = body;
        self.app.page(&r, &peer(self.ip))
    }

    fn sign_on(&mut self, id: &str, password: &str) -> Response {
        self.send(
            "POST",
            "/qsmc/signon",
            &[("user", id), ("password", password)],
        )
    }

    fn csrf(&mut self, path: &str) -> String {
        let page = self.send("GET", path, &[]).text();
        qsweb::golden::tokens(&page)
            .into_iter()
            .find_map(|t| match t {
                qsweb::golden::Tok::Open(n, a)
                    if n == "input" && a.get("name").map(String::as_str) == Some("csrf") =>
                {
                    a.get("value").cloned()
                }
                _ => None,
            })
            .unwrap_or_default()
    }
}

#[test]
fn the_first_administrator_signs_on_with_the_installation_s_password() {
    let (app, _, dir) = app("first");
    assert_eq!(
        app.users().get("qsadmin").map(|u| u.role),
        Some(Role::Administrator)
    );
    let mut b = Browser {
        app: &app,
        cookie: None,
        ip: "192.0.2.10",
        host: None,
    };
    assert_eq!(b.send("GET", "/qsmc/welcome", &[]).status, 303);
    let r = b.sign_on("qsadmin", PASSWORD);
    assert_eq!(
        (r.status, r.header("Location")),
        (303, Some("/qsmc/welcome"))
    );
    let cookie = r.header("Set-Cookie").unwrap().to_string();
    for want in ["HttpOnly", "Secure", "SameSite=Strict", "Path=/qsmc"] {
        assert!(cookie.contains(want), "{cookie}");
    }
    let w = b.send("GET", "/qsmc/welcome", &[]);
    assert_eq!(w.status, 200);
    assert!(
        w.text()
            .contains("Signed on as qsadmin (Administrator) from 192.0.2.10.")
    );
    assert!(w.text().contains("QMC9031"));
    assert!(
        app.users()
            .get("qsadmin")
            .is_some_and(|u| u.last_from == "192.0.2.10")
    );
    // Signed off, the session is gone.
    let old = b.cookie.clone();
    let off = b.send("GET", "/qsmc/signoff", &[]);
    assert!(off.text().contains("QMC9005"));
    b.cookie = old;
    let after = b.send("GET", "/qsmc/welcome", &[]);
    assert!(after.text().contains("QMC9006"), "{}", after.text());
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_form_without_its_token_is_refused() {
    let (app, _, dir) = app("csrf");
    let mut b = Browser {
        app: &app,
        cookie: None,
        ip: "192.0.2.11",
        host: None,
    };
    b.sign_on("qsadmin", PASSWORD);
    let r = b.send("POST", "/qsmc/welcome", &[("csrf", "forged")]);
    assert_eq!(r.status, 403);
    assert!(r.text().contains("QMC9011"));
    let token = b.csrf("/qsmc/signoff");
    assert!(token.is_empty(), "the sign-on page has no session token");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_role_without_the_task_is_not_authorized_and_a_disabled_user_s_session_ends() {
    let (app, _, dir) = app("roles");
    app.change_users(|u| u.add("viewer", "", Role::Viewer, "Look-only-42", 2))
        .unwrap();
    let mut v = Browser {
        app: &app,
        cookie: None,
        ip: "192.0.2.12",
        host: None,
    };
    assert_eq!(v.sign_on("viewer", "Look-only-42").status, 303);
    assert_eq!(v.send("GET", "/qsmc/welcome", &[]).status, 200);
    let r = v.send("GET", "/qsmc/console", &[]);
    assert_eq!(r.status, 403);
    assert!(r.text().contains("QMC9010"));
    // A later function says so.
    let later = v.send("GET", "/qsmc/servers", &[]);
    assert!(later.text().contains("QMC9030"));
    app.change_users(|u| u.set_enabled("viewer", false))
        .unwrap();
    assert!(
        v.send("GET", "/qsmc/welcome", &[])
            .text()
            .contains("QMC9006")
    );
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn five_wrong_passwords_lock_the_id_and_a_session_has_a_length() {
    let (app, clock, dir) = app("lock");
    let mut b = Browser {
        app: &app,
        cookie: None,
        ip: "192.0.2.13",
        host: None,
    };
    for _ in 0..4 {
        assert!(b.sign_on("qsadmin", "wrong").text().contains("QMC9001"));
    }
    assert!(b.sign_on("qsadmin", "wrong").text().contains("QMC9002"));
    assert!(b.sign_on("qsadmin", PASSWORD).text().contains("QMC9002"));
    clock.fetch_add(301, Ordering::Relaxed);
    assert_eq!(b.sign_on("qsadmin", PASSWORD).status, 303);
    assert_eq!(b.send("GET", "/qsmc/welcome", &[]).status, 200);
    clock.fetch_add(12 * 3600, Ordering::Relaxed);
    assert!(
        b.send("GET", "/qsmc/welcome", &[])
            .text()
            .contains("QMC9006")
    );
    let missing = {
        b.sign_on("qsadmin", PASSWORD);
        b.send("GET", "/qsmc/nothing", &[])
    };
    assert_eq!(missing.status, 404);
    assert!(missing.text().contains("QMC9020"));
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn console_settings_shows_the_console_s_own_values() {
    let (app, _, dir) = app("settings");
    let mut b = Browser {
        app: &app,
        cookie: None,
        ip: "192.0.2.14",
        host: None,
    };
    b.sign_on("qsadmin", PASSWORD);
    let t = b.send("GET", "/qsmc/console", &[]).text();
    for want in [
        "1 users, 4 roles",
        "Public 192.0.2.10/24, private none",
        "No backup made yet",
        "30 minutes idle",
    ] {
        assert!(t.contains(want), "{want} is not on the page");
    }
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn users_are_made_changed_and_deleted_and_the_last_administrator_stays() {
    let (app, _, dir) = app("users");
    let mut b = Browser {
        app: &app,
        cookie: None,
        ip: "192.0.2.15",
        host: None,
    };
    b.sign_on("qsadmin", PASSWORD);
    let token = b.csrf("/qsmc/console/users?create");
    let r = b.send(
        "POST",
        "/qsmc/console/users",
        &[
            ("csrf", &token),
            ("create", "OK"),
            ("id", "Operator1"),
            ("description", "Operations desk"),
            ("role", "Operator"),
            ("password", "Desk-shift-42"),
            ("again", "Desk-shift-42"),
        ],
    );
    assert_eq!(r.status, 303);
    let list = b.send("GET", "/qsmc/console/users", &[]).text();
    assert!(
        list.contains("QMC8001") && list.contains("operator1"),
        "{list}"
    );
    // The last enabled Administrator is kept.
    let r = b.send(
        "POST",
        "/qsmc/console/users",
        &[
            ("csrf", &token),
            ("properties", "OK"),
            ("id", "qsadmin"),
            ("role", "Viewer"),
        ],
    );
    assert!(r.text().contains("QMC8010"));
    let r = b.send(
        "POST",
        "/qsmc/confirm/user-delete",
        &[("csrf", &token), ("ok", "OK"), ("u", "qsadmin")],
    );
    assert_eq!(r.status, 303);
    assert!(
        b.send("GET", "/qsmc/console/users", &[])
            .text()
            .contains("QMC8010")
    );
    // The new user signs on; a viewer's password change is its own.
    let mut o = Browser {
        app: &app,
        cookie: None,
        ip: "192.0.2.16",
        host: None,
    };
    assert_eq!(o.sign_on("operator1", "Desk-shift-42").status, 303);
    assert_eq!(o.send("GET", "/qsmc/console/users", &[]).status, 403);
    // Deleted through the confirmation page, its session ends.
    let confirm = b
        .send("GET", "/qsmc/confirm/user-delete?u=operator1", &[])
        .text();
    assert!(confirm.contains("Confirm Delete User"));
    b.send(
        "POST",
        "/qsmc/confirm/user-delete",
        &[("csrf", &token), ("ok", "OK"), ("u", "operator1")],
    );
    assert!(app.users().get("operator1").is_none());
    assert!(
        o.send("GET", "/qsmc/welcome", &[])
            .text()
            .contains("QMC9006")
    );
    // The roles: a viewer given Console settings.
    let r = b.send(
        "POST",
        "/qsmc/console/roles",
        &[
            ("csrf", &token),
            ("apply", "Apply"),
            ("r9_2", "on"),
            ("r0_2", "on"),
        ],
    );
    assert_eq!(r.status, 303);
    assert!(
        app.users()
            .matrix
            .allows(Role::Viewer, webusers::Task::Settings)
    );
    assert!(
        !app.users()
            .matrix
            .allows(Role::Operator, webusers::Task::View)
    );
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn network_settings_refuse_what_waits_and_keep_the_allow_list() {
    use webd::serve::Pages as _;
    let (app, _, dir) = app("network");
    let mut b = Browser {
        app: &app,
        cookie: None,
        ip: "192.0.2.17",
        host: None,
    };
    b.sign_on("qsadmin", PASSWORD);
    let page = b.send("GET", "/qsmc/console/network", &[]);
    assert_eq!(page.status, 200);
    let token = b.csrf("/qsmc/console/network");
    let base = [
        ("csrf", token.as_str()),
        ("apply", "Apply"),
        ("h", "QSHQMC1"),
    ];
    let with = |extra: &[(&'static str, &'static str)]| {
        let mut f: Vec<(&str, &str)> = base.to_vec();
        f.extend_from_slice(extra);
        f
    };
    let r = b.send(
        "POST",
        "/qsmc/console/network",
        &with(&[("ssh", "on"), ("dhcp", "on")]),
    );
    let t = r.text();
    assert!(
        t.contains("SSH to the console is not in this release."),
        "{t}"
    );
    assert!(t.contains("A DHCP server on the private network is not in this release."));
    // A change of the open address is said first, then put on trial.
    let own = [("ip", "192.0.2.99"), ("mask", "255.255.255.0")];
    let r = b.send("POST", "/qsmc/console/network", &with(&own));
    assert!(r.text().contains("Confirm Network Change"), "{}", r.text());
    let r = b.send(
        "POST",
        "/qsmc/console/network",
        &with(&[own[0], own[1], ("confirmed", "yes")]),
    );
    let t = r.text();
    assert!(t.contains("QMC8100") && t.contains("120 seconds"), "{t}");
    // Reached at another address nothing is kept; at the new one it is.
    let mut there = Browser {
        app: &app,
        cookie: None,
        ip: "192.0.2.18",
        host: Some("192.0.2.98"),
    };
    there.sign_on("qsadmin", PASSWORD);
    assert_eq!(CONFIRMS.with(|c| c.load(Ordering::Relaxed)), 0);
    there.host = Some("192.0.2.99");
    there.sign_on("qsadmin", PASSWORD);
    assert_eq!(CONFIRMS.with(|c| c.load(Ordering::Relaxed)), 1);
    assert!(
        there
            .send("GET", "/qsmc/welcome", &[])
            .text()
            .contains("QMC8101")
    );
    let r = b.send(
        "POST",
        "/qsmc/console/network",
        &with(&[("allow", "192.0.2.0/24, 10.0.0.0/8")]),
    );
    assert!(r.text().contains("QMC8103"), "{}", r.text());
    assert_eq!(app.settings().allow, ["192.0.2.0/24", "10.0.0.0/8"]);
    assert!(app.admits("192.0.2.200".parse().unwrap()));
    assert!(!app.admits("198.51.100.1".parse().unwrap()));
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn date_and_time_sets_the_servers_the_offset_and_the_clock() {
    let (app, clock, dir) = app("datetime");
    let mut b = Browser {
        app: &app,
        cookie: None,
        ip: "192.0.2.19",
        host: None,
    };
    b.sign_on("qsadmin", PASSWORD);
    let page = b.send("GET", "/qsmc/console/datetime", &[]).text();
    assert!(page.contains("Not used"), "{page}");
    let token = b.csrf("/qsmc/console/datetime");
    let post = |b: &mut Browser<'_>, f: &[(&str, &str)]| {
        let mut all = vec![("csrf", token.as_str()), ("apply", "Apply")];
        all.extend_from_slice(f);
        b.send("POST", "/qsmc/console/datetime", &all).text()
    };
    // Serving time is not in V1; no server and no clock is refused.
    let t = post(
        &mut b,
        &[("serve", "on"), ("s", "ntp"), ("ntp", "10.0.10.1")],
    );
    assert!(t.contains("QMC8700"), "{t}");
    let t = post(&mut b, &[("s", "ntp"), ("ntp", " "), ("tz", "UTC")]);
    assert!(t.contains("QMC8011"), "{t}");
    // Servers and an offset go to host.toml.
    let t = post(
        &mut b,
        &[
            ("s", "ntp"),
            ("ntp", "10.0.10.1, time.quartz.example"),
            ("tz", "UTC-04:00"),
        ],
    );
    assert!(t.contains("QMC8200"), "{t}");
    let host = app.host();
    assert_eq!(host.time_zone, "UTC-04:00");
    assert_eq!(host.ntp_servers, ["10.0.10.1", "time.quartz.example"]);
    assert!(t.contains("Not synchronized"), "{t}");
    // By hand: no servers, and the clock at the offset's time.
    let t = post(
        &mut b,
        &[
            ("s", "manual"),
            ("md", "10/30/26"),
            ("mt", "07:14"),
            ("tz", "UTC-04:00"),
        ],
    );
    assert!(
        t.contains("QMC8201") && t.contains("10/30/26 07:14 UTC-04:00"),
        "{t}"
    );
    assert!(app.host().ntp_servers.is_empty());
    let at = appliance::time::from_civil(2026, 10, 30, 11, 14, 0);
    let shift = SHIFT.with(|s| s.load(Ordering::Relaxed));
    assert_eq!(
        clock.load(Ordering::Relaxed).saturating_add_signed(shift),
        u64::try_from(at).unwrap()
    );
    // The session, timed by the uptime, goes on.
    assert_eq!(b.send("GET", "/qsmc/welcome", &[]).status, 200);
    let t = post(
        &mut b,
        &[("s", "manual"), ("md", "02/30/26"), ("mt", "07:14")],
    );
    assert!(t.contains("is not a date"), "{t}");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
#[allow(clippy::too_many_lines, reason = "one page's whole story")]
fn certificates_are_made_imported_and_trusted() {
    let (app, clock, dir) = app("certs");
    let now = clock.load(Ordering::Relaxed);
    let names = ["192.0.2.10".to_string()];
    let (certs, made) =
        webd::tls::Certs::load_or_make(&dir.join("web"), "QSHQMC1", &names, now).unwrap();
    assert!(made);
    let certs = Arc::new(certs);
    app.started(certs.clone());
    let mut b = Browser {
        app: &app,
        cookie: None,
        ip: "192.0.2.20",
        host: None,
    };
    b.sign_on("qsadmin", PASSWORD);
    let t = b.send("GET", "/qsmc/console/certs", &[]).text();
    assert!(
        t.contains("expires in 364 days") && t.contains("self-signed"),
        "{t}"
    );
    assert!(t.contains("No certificate authority is trusted."));
    let token = b.csrf("/qsmc/console/certs");
    // View: every field, the names among them.
    let t = b
        .send(
            "POST",
            "/qsmc/console/certs",
            &[("csrf", &token), ("view", "View")],
        )
        .text();
    assert!(
        t.contains("Fingerprint (SHA-1)") && t.contains("QSHQMC1, 192.0.2.10"),
        "{t}"
    );
    // A new self-signed certificate, after the confirmation.
    let first = certs.shown().sha256;
    let r = b.send(
        "POST",
        "/qsmc/console/certs",
        &[("csrf", &token), ("generate_self-signed...", "x")],
    );
    assert_eq!(r.header("Location"), Some("/qsmc/confirm/cert-new"));
    assert!(
        b.send("GET", "/qsmc/confirm/cert-new", &[])
            .text()
            .contains("ECDSA P-256")
    );
    b.send(
        "POST",
        "/qsmc/confirm/cert-new",
        &[("csrf", &token), ("ok", "OK")],
    );
    assert_ne!(certs.shown().sha256, first);
    let t = b.send("GET", "/qsmc/console/certs", &[]).text();
    assert!(t.contains("QMC8300"), "{t}");
    // A PEM chain and its key, imported and in use.
    let other = x509kit::server::self_signed("other.example", &["other.example"], now, 90).unwrap();
    let key = x509kit::pem::encode("PRIVATE KEY", &other.key.to_pkcs8_der().unwrap());
    let pem = format!("{}{key}", x509kit::pem::encode("CERTIFICATE", &other.cert));
    b.upload(
        "/qsmc/console/certs",
        &[("csrf", &token), ("pw", "")],
        pem.as_bytes(),
    );
    assert_eq!(
        certs.shown().info.common_name.as_deref(),
        Some("other.example")
    );
    assert!(
        b.send("GET", "/qsmc/console/certs", &[])
            .text()
            .contains("QMC8301")
    );
    // A key that is not the certificate's is refused; nothing changes.
    let third = x509kit::server::self_signed("third.example", &["third.example"], now, 90).unwrap();
    let wrong = format!("{}{key}", x509kit::pem::encode("CERTIFICATE", &third.cert));
    let t = b
        .upload(
            "/qsmc/console/certs",
            &[("csrf", &token), ("pw", "")],
            wrong.as_bytes(),
        )
        .text();
    assert!(t.contains("QMC8302"), "{t}");
    assert_eq!(
        certs.shown().info.common_name.as_deref(),
        Some("other.example")
    );
    // No token, no import.
    let r = b.upload("/qsmc/console/certs", &[("pw", "")], pem.as_bytes());
    assert_eq!(r.status, 403);
    // An authority's certificate alone is trusted, viewed and removed.
    let ca = include_bytes!("data/ca.pem");
    b.upload("/qsmc/console/certs", &[("csrf", &token), ("pw", "")], ca);
    let t = b.send("GET", "/qsmc/console/certs", &[]).text();
    assert!(
        t.contains("QMC8303") && t.contains("CN=Test Authority"),
        "{t}"
    );
    let t = b
        .send("GET", "/qsmc/console/certs?ca=Test%20Authority", &[])
        .text();
    assert!(t.contains("Certificate authority Test Authority"), "{t}");
    b.send(
        "POST",
        "/qsmc/confirm/ca-remove",
        &[("csrf", &token), ("l", "Test Authority"), ("ok", "OK")],
    );
    let t = b.send("GET", "/qsmc/console/certs", &[]).text();
    assert!(
        t.contains("QMC8304") && t.contains("No certificate authority is trusted."),
        "{t}"
    );
    // The download is the certificate in use.
    let d = b.send(
        "POST",
        "/qsmc/console/certs",
        &[("csrf", &token), ("download_ca_certificate", "x")],
    );
    assert_eq!(d.text(), x509kit::pem::encode("CERTIFICATE", &other.cert));
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
#[allow(clippy::too_many_lines, reason = "one page's whole story")]
fn a_backup_is_made_opened_and_left_for_the_restart() {
    const PASS: &str = "a long backup passphrase";
    let (app, _, dir) = app("backup");
    let mut b = Browser {
        app: &app,
        cookie: None,
        ip: "192.0.2.21",
        host: None,
    };
    b.sign_on("qsadmin", PASSWORD);
    let t = b.send("GET", "/qsmc/console/backup", &[]).text();
    assert!(t.contains("No backup has been made."), "{t}");
    let token = b.csrf("/qsmc/console/backup");
    let post = |b: &mut Browser<'_>, f: &[(&str, &str)]| {
        let mut all = vec![("csrf", token.as_str())];
        all.extend_from_slice(f);
        b.send("POST", "/qsmc/console/backup", &all)
    };
    // What V1 does not do, and a short passphrase, make nothing.
    let t = post(
        &mut b,
        &[
            ("d", "on"),
            ("h", "on"),
            ("w", "remote"),
            ("pp", "short"),
            ("back_up_now", "x"),
        ],
    )
    .text();
    for id in ["QMC8700", "QMC8405", "QMC8406"] {
        assert!(t.contains(id), "{id}: {t}");
    }
    assert!(
        post(&mut b, &[("schedule", "x")])
            .text()
            .contains("QMC8700")
    );
    // A backup: downloaded, recorded, and opened by its passphrase only.
    let r = post(
        &mut b,
        &[
            ("d", "on"),
            ("w", "file"),
            ("pp", PASS),
            ("back_up_now", "x"),
        ],
    );
    let disposition = r.header("Content-Disposition").unwrap_or("").to_string();
    assert!(
        disposition.contains("qshqmc1-20260921.qmcb"),
        "{disposition}"
    );
    let file = r.bytes().to_vec();
    let opened = qmcbackup::seal::open(&file, PASS).unwrap();
    assert_eq!(opened.manifest.console, "QSHQMC1");
    assert!(
        opened
            .entries
            .iter()
            .any(|e| e.path == "state/webusers.toml")
    );
    assert!(opened.entries.iter().any(|e| e.path == "state/users.db"));
    assert!(!file.windows(7).any(|w| w == b"qsadmin"));
    let t = b
        .send("GET", "/qsmc/console/backup?tab=history", &[])
        .text();
    assert!(
        t.contains("Download qshqmc1-20260921.qmcb") && t.contains("Complete"),
        "{t}"
    );
    // Opened for a restore: a wrong passphrase, a damaged file and a newer
    // release are refused.
    let restore = "/qsmc/console/backup?tab=restore";
    let open = |b: &mut Browser<'_>, file: &[u8], pp: &str| {
        b.upload(
            restore,
            &[("csrf", &token), ("pp", pp), ("open", "Open")],
            file,
        )
        .text()
    };
    assert!(open(&mut b, &file, "not the passphrase").contains("QMC8402"));
    let mut damaged = file.clone();
    damaged[60] ^= 1;
    assert!(open(&mut b, &damaged, PASS).contains("QMC8402"));
    assert!(open(&mut b, b"not a backup", PASS).contains("QMC8403"));
    let mut newer = opened.clone();
    newer.manifest.release = "V9R0M0".into();
    let newer = qmcbackup::seal::seal(&newer, PASS).unwrap();
    let t = open(&mut b, &newer, PASS);
    assert!(t.contains("QMC8404") && t.contains("V9R0M0"), "{t}");
    // Opened, it shows what it holds; confirmed, it waits for the IPL.
    let t = open(&mut b, &file, PASS);
    assert!(
        t.contains("The backup holds") && t.contains("QSHQMC1"),
        "{t}"
    );
    let staged = dir.join("restore");
    assert!(staged.join("uploaded").exists());
    let t = b.send("GET", "/qsmc/confirm/restore", &[]).text();
    assert!(t.contains("Confirm Restore"), "{t}");
    b.send(
        "POST",
        "/qsmc/confirm/restore",
        &[("csrf", &token), ("ok", "OK")],
    );
    assert!(staged.join("pending").exists() && !staged.join("uploaded").exists());
    let pending = std::fs::read(staged.join("pending")).unwrap();
    assert_eq!(qmcbackup::Backup::decode(&pending).unwrap(), opened);
    // And the console restarts for it.
    let t = b.send("GET", restore, &[]).text();
    assert!(t.contains("QMC8501"), "{t}");
    let powered = POWERED.with(|p| p.borrow_mut().take());
    assert_eq!(
        powered,
        Some((true, "qsadmin from the web interface: a restore".into()))
    );
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn shut_down_or_restart_asks_pid_1_and_tells_every_session() {
    let (app, _, dir) = app("shutdown");
    let mut b = Browser {
        app: &app,
        cookie: None,
        ip: "192.0.2.22",
        host: None,
    };
    b.sign_on("qsadmin", PASSWORD);
    let t = b.send("GET", "/qsmc/console/shutdown", &[]).text();
    assert!(t.contains("1:00") && t.contains("QSHQMC1"), "{t}");
    let token = b.csrf("/qsmc/console/shutdown");
    let r = b.send(
        "POST",
        "/qsmc/console/shutdown",
        &[("csrf", &token), ("cancel", "Cancel")],
    );
    assert_eq!(r.header("Location"), Some("/qsmc/console"));
    assert!(POWERED.with(|p| p.borrow().is_none()));
    let t = b
        .send(
            "POST",
            "/qsmc/console/shutdown",
            &[
                ("csrf", &token),
                ("how", "off"),
                ("m", "Back at noon"),
                ("ok", "OK"),
            ],
        )
        .text();
    assert!(t.contains("QMC8500"), "{t}");
    let powered = POWERED.with(|p| p.borrow_mut().take());
    assert_eq!(
        powered,
        Some((false, "qsadmin from the web interface: Back at noon".into()))
    );
    // Another session sees the message on its next page.
    let mut other = Browser {
        app: &app,
        cookie: None,
        ip: "192.0.2.23",
        host: None,
    };
    other.sign_on("qsadmin", PASSWORD);
    let t = other.send("GET", "/qsmc/welcome", &[]).text();
    assert!(t.contains("Message from qsadmin: Back at noon"), "{t}");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn language_and_locale_keep_the_formats_and_the_timeouts() {
    let (app, _, dir) = app("locale");
    let mut b = Browser {
        app: &app,
        cookie: None,
        ip: "192.0.2.24",
        host: None,
    };
    b.sign_on("qsadmin", PASSWORD);
    let token = b.csrf("/qsmc/console/locale");
    let post = |b: &mut Browser<'_>, f: &[(&str, &str)]| {
        // The first of a name counts: the test's own fields first.
        let mut all = vec![("csrf", token.as_str()), ("apply", "Apply")];
        all.extend_from_slice(f);
        all.extend_from_slice(&[
            ("df", "YYYY-MM-DD"),
            ("tf", "12-hour AM/PM"),
            ("nf", "1.234,56"),
            ("to", "15"),
            ("max", "8"),
        ]);
        b.send("POST", "/qsmc/console/locale", &all).text()
    };
    let t = post(&mut b, &[("l", "German"), ("tw", "on")]);
    assert!(
        t.contains("The language German is not in this release."),
        "{t}"
    );
    assert!(t.contains("Keeping terminal window sessions open is not in this release."));
    assert_eq!(app.settings().idle_minutes, 30);
    let t = post(&mut b, &[("l", "English (US)"), ("to", "0")]);
    assert!(t.contains("1 to 480 minutes"), "{t}");
    let t = post(&mut b, &[("l", "English (US)")]);
    assert!(t.contains("QMC8600"), "{t}");
    let s = app.settings();
    assert_eq!(
        (
            s.date_format.as_str(),
            s.time_format.as_str(),
            s.idle_minutes,
            s.max_hours
        ),
        ("YYYY-MM-DD", "12-hour AM/PM", 15, 8)
    );
    // The pages show dates in the new format.
    let t = b.send("GET", "/qsmc/console", &[]).text();
    assert!(t.contains("2026-09-21"), "{t}");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn console_version_shows_the_release_and_the_machine() {
    use state::Document as _;
    let (app, _, dir) = app("version");
    let mut inv = state::Inventory::default();
    inv.system.manufacturer = "QEMU".into();
    inv.system.product = "Standard PC (Q35 + ICH9, 2009)".into();
    inv.processors.threads = 2;
    inv.memory.total_bytes = 4096 * 1024 * 1024;
    inv.validate().unwrap();
    app.store().save(&inv).unwrap();
    let mut b = Browser {
        app: &app,
        cookie: None,
        ip: "192.0.2.25",
        host: None,
    };
    b.sign_on("qsadmin", PASSWORD);
    let t = b.send("GET", "/qsmc/console/version", &[]).text();
    for want in [
        release::RELEASE.to_string().as_str(),
        release::BUILD_ID,
        "Virtual machine (QEMU Standard PC (Q35 + ICH9, 2009))",
        "2 vCPU",
        "4,096 MB",
        "1:00 (since",
    ] {
        assert!(t.contains(want), "{want}: {t}");
    }
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn the_pattern_reference_is_a_test_image_s() {
    let (app, _, dir) = app("components");
    let mut b = Browser {
        app: &app,
        cookie: None,
        ip: "192.0.2.26",
        host: None,
    };
    b.sign_on("qsadmin", PASSWORD);
    let r = b.send("GET", "/qsmc/dev/components", &[]);
    if cfg!(test_hooks) {
        assert!(r.status == 200 && r.text().contains("Pattern reference"));
        assert_eq!(b.send("GET", "/qsmc/dev/error", &[]).status, 404);
    } else {
        assert_eq!(r.status, 404);
    }
    let _ = std::fs::remove_dir_all(dir);
}
