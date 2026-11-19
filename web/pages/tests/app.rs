// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! The pages through their handler: sign-on and its rules, sessions,
//! the CSRF check, roles. The conformance suites do the same from
//! outside a running console.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use qsweb::http::{Peer, Request, Response};
use webd::serve::Pages as _;
use webpages::App;
use webpages::app::Console;
use webusers::Role;

struct Fixed(Arc<AtomicU64>);

impl Console for Fixed {
    fn name(&self) -> String {
        "QSHQMC1".into()
    }
    fn uptime(&self) -> u64 {
        3600
    }
    fn now(&self) -> u64 {
        self.0.load(Ordering::Relaxed)
    }
    fn fact(&self, key: &str) -> String {
        match key {
            "management" => "192.0.2.10/24 on eth0, gateway 192.0.2.1".into(),
            _ => String::new(),
        }
    }
}

const PASSWORD: &str = "Install-pass-7";

fn app(name: &str) -> (App, Arc<AtomicU64>, PathBuf) {
    let dir = std::env::temp_dir().join(format!("webpages-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let db = dir.join("users.db");
    let hash = users::hash(PASSWORD).unwrap();
    std::fs::write(&db, users::first("QSADMIN", &hash, 1).unwrap()).unwrap();
    let clock = Arc::new(AtomicU64::new(1_790_000_000));
    let app = App::open(&dir, &db, Box::new(Fixed(clock.clone()))).unwrap();
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
}

impl Browser<'_> {
    fn send(&mut self, method: &str, path: &str, form: &[(&str, &str)]) -> Response {
        let mut r = Request::new(method, path);
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
