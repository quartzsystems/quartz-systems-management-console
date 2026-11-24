// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! The web server reviewed from outside (`docs/SECURITY.md`), beside
//! what `web-hostile`, `sign-on` and `web-signon` already show: a body
//! said two ways; paths that climb out; the `Host` header and redirects;
//! a session fixed before sign-on, and after sign-off; every page's
//! headers and policy and every form refused without its token, by a
//! crawl, which changes nothing by GET; error pages; the terminal
//! socket's origin; the password hash's parameters and sign-on's time.

use std::collections::{BTreeSet, VecDeque};
use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::{Duration, Instant};

use crate::suite::base::{Run, channel};
use crate::suite::parts::pages::{key, links, window};
use crate::suite::parts::{Lab, lab};
use crate::suite::web::{ADMIN, PASSWORD, signed_on};
use crate::web::form::forms;
use crate::web::{Answer, Browser};

/// The answers in `raw`: each begins with a status line.
fn answers(raw: &[u8]) -> usize {
    String::from_utf8_lossy(raw).matches("HTTP/1.1 ").count()
}

/// A body said two ways is refused, and what follows it on its
/// connection is never acted on.
fn framing(b: &Browser) -> Result<String, String> {
    let next = "GET /qsmc/welcome HTTP/1.1\r\nHost: qsmc\r\n\r\n";
    let cases = [
        format!(
            "POST /qsmc/signon HTTP/1.1\r\nHost: qsmc\r\nContent-Length: 5\r\nTransfer-Encoding: chunked\r\n\r\n0\r\n\r\n{next}"
        ),
        format!(
            "POST /qsmc/signon HTTP/1.1\r\nHost: qsmc\r\nContent-Length: 0\r\nContent-Length: 44\r\n\r\n{next}"
        ),
        format!(
            "POST /qsmc/signon HTTP/1.1\r\nHost: qsmc\r\nTransfer-Encoding: xchunked\r\n\r\n0\r\n\r\n{next}"
        ),
    ];
    for c in &cases {
        let raw = b.raw(c.as_bytes())?;
        let status = Browser::status_of(&raw);
        // What followed on its connection is refused as well, never
        // answered as a page.
        let all = answers(&raw);
        let refusals = String::from_utf8_lossy(&raw)
            .matches("HTTP/1.1 400 ")
            .count();
        if !matches!(status, Some(400)) || all == 0 || refusals != all {
            return Err(format!(
                "a body said two ways answered {status:?} with {} answers: {:?}",
                answers(&raw),
                c.lines().take(4).collect::<Vec<_>>()
            ));
        }
    }
    Ok(format!(
        "{} requests whose body is said two ways each refused, and the request after each on its connection refused too",
        cases.len()
    ))
}

/// No path climbs out of what is served.
fn climbing(b: &mut Browser) -> Result<String, String> {
    let tries = [
        "/qsmc/../etc/passwd",
        "/qsmc/%2e%2e/%2e%2e/etc/passwd",
        "/qsmc/static/..%2f..%2f..%2fetc%2fpasswd",
        "/qsmc//etc/passwd",
        "/qsmc/dumps/..%2f..%2fstate%2fusers.db",
        "/qsmc/updates/qsmc/notes/..%2f..%2f..%2fstate%2fweb.toml",
        "/qsmc/qsmc.css/../../data/web/key.pem",
        "/qsmc/%00",
    ];
    for p in tries {
        let a = b.get(p)?;
        let t = a.text();
        if t.contains("root:") || t.contains("PRIVATE KEY") || t.contains("[[user]]") {
            return Err(format!(
                "{p} answered a file of the console's: {}",
                a.status
            ));
        }
        if a.status == 200 && !t.contains("<html") {
            return Err(format!("{p} answered 200 with something not a page"));
        }
    }
    Ok(format!("{} paths that climb answered no file", tries.len()))
}

/// Port 80 with a `Host` of its own choosing.
fn on_80(b: &Browser, host: &str) -> Result<String, String> {
    let mut s = TcpStream::connect(("127.0.0.1", b.http)).map_err(|e| e.to_string())?;
    s.set_read_timeout(Some(Duration::from_secs(10)))
        .map_err(|e| e.to_string())?;
    write!(
        s,
        "GET /qsmc/welcome HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\n\r\n"
    )
    .map_err(|e| e.to_string())?;
    let mut raw = Vec::new();
    let _ = s.read_to_end(&mut raw);
    let text = String::from_utf8_lossy(&raw).to_string();
    text.lines()
        .find_map(|l| {
            l.split_once(':')
                .filter(|(k, _)| k.eq_ignore_ascii_case("location"))
                .map(|(_, v)| v.trim().to_string())
        })
        .ok_or_else(|| format!("port 80 gave no Location for Host {host}"))
}

/// No redirect leaves the console.
fn redirects(b: &mut Browser) -> Result<String, String> {
    for evil in ["evil.example", "evil.example:80", "QSMC@evil.example"] {
        let to = on_80(b, evil)?;
        if to.contains("evil") {
            return Err(format!("port 80 sent Host {evil} to {to}"));
        }
    }
    // On 443 every redirect is a path of the console's.
    let a = b.send(
        "GET",
        "/qsmc/",
        &[("Host".into(), "evil.example".into())],
        &[],
    )?;
    let to = a.header("location").unwrap_or("");
    if !to.starts_with("/qsmc") {
        return Err(format!("443 redirected to {to}"));
    }
    Ok("port 80 and 443 redirect only to the console".into())
}

/// Whether `b`'s cookie is a session that reaches Welcome.
fn in_session(b: &mut Browser) -> Result<bool, String> {
    let a = b.get("/qsmc/welcome")?;
    Ok(a.status == 200 && a.text().contains("Welcome"))
}

/// A session chosen before sign-on is never the one signed on, and one
/// signed off is no session.
fn fixation(b: &Browser) -> Result<String, String> {
    let mut fixed = b.fresh();
    fixed.cookie = Some("0123456789abcdef0123456789abcdef".into());
    signed_on(&mut fixed, ADMIN, PASSWORD)?;
    if fixed.cookie.as_deref() == Some("0123456789abcdef0123456789abcdef") {
        return Err("sign-on kept the session the client chose".into());
    }
    let mut chosen = b.fresh();
    chosen.cookie = Some("0123456789abcdef0123456789abcdef".into());
    if in_session(&mut chosen)? {
        return Err("the session the client chose is a session".into());
    }
    let mut stale = fixed.clone_session();
    fixed.get("/qsmc/signoff")?;
    if in_session(&mut stale)? {
        return Err("a session signed off still reaches Welcome".into());
    }
    Ok("a session chosen before sign-on is not taken, and one signed off is no session".into())
}

/// What the crawl saw.
#[derive(Default)]
struct Walk {
    pages: usize,
    redirects: usize,
    windows: usize,
    forms: usize,
    actions: BTreeSet<String>,
}

/// `a`'s headers, as every page has them.
fn headers(path: &str, a: &Answer) -> Result<(), String> {
    let want = |k: &str, v: &str| {
        if a.header(k) == Some(v) {
            Ok(())
        } else {
            Err(format!("{path}: {k} is {:?}", a.header(k)))
        }
    };
    want("x-frame-options", "DENY")?;
    want("x-content-type-options", "nosniff")?;
    want("referrer-policy", "no-referrer")?;
    want("cache-control", "no-store")?;
    let csp = a.header("content-security-policy").unwrap_or("");
    if window(path) {
        // With its script, or without it where the window has no
        // console to show (`docs/TERMINAL.md`).
        if csp != webpages::terminal::CSP && csp != webpages::terminal::CSP_NO_SCRIPT {
            return Err(format!("{path}: the terminal window's policy is {csp:?}"));
        }
    } else if csp != webd::serve::CSP {
        return Err(format!("{path}: its policy is {csp:?}"));
    }
    if !window(path) && crate::web::scripted(&a.text()) {
        return Err(format!("{path} carries a script"));
    }
    Ok(())
}

/// What a crawl leaves alone: the pages of everything a GET could have
/// changed.
fn snapshot(b: &mut Browser) -> Result<Vec<String>, String> {
    [
        "/qsmc/partitions",
        "/qsmc/servers",
        "/qsmc/groups",
        "/qsmc/schedule",
        "/qsmc/plans",
        "/qsmc/console/users",
        "/qsmc/events",
    ]
    .iter()
    .map(|p| {
        let mut t = b.get(p)?.text();
        // The tree keeps the groups the crawl opened (the session's, as
        // QS/400's Navigator does): what the pages show is the rest.
        if let Some(a) = t.find("<td class=\"nav\"")
            && let Some(e) = t[a..].find("</td>")
        {
            t.replace_range(a..a + e, "");
        }
        // The frame's clock moves; the rest must not.
        Ok(t.lines()
            .filter(|l| !l.contains("class=\"clock\"") && !l.contains("Updated "))
            .collect::<Vec<_>>()
            .join("\n"))
    })
    .collect()
}

/// Every page from Welcome and the tree: its headers, its policy, and
/// each of its forms refused without the token.
fn walk(b: &mut Browser) -> Result<Walk, String> {
    let mut queue: VecDeque<String> = webd::product::NAV
        .iter()
        .flat_map(|g| g.items.iter().map(|(_, r)| format!("/qsmc{r}")))
        .chain(["/qsmc/welcome".to_string()])
        .collect();
    let mut seen = BTreeSet::new();
    let mut w = Walk::default();
    while let Some(path) = queue.pop_front() {
        let bare = path.split('?').next().unwrap_or("").to_string();
        if bare.ends_with("/signoff") || bare.ends_with("/ws") || !seen.insert(key(&path)) {
            continue;
        }
        let a = b.get(&path)?;
        if crate::web::opened(&path, a.status)? {
            continue;
        }
        // Every link of every page resolves (the design's acceptance).
        if a.status == 404 || a.status >= 500 {
            return Err(format!("{path}, a link of a page, answered {}", a.status));
        }
        if a.status == 303 {
            w.redirects += 1;
            continue;
        }
        headers(&path, &a)?;
        w.pages += 1;
        if window(&path) {
            w.windows += 1;
            continue;
        }
        let text = a.text();
        for f in forms(&text, &path) {
            if f.method != "post" {
                continue;
            }
            w.forms += 1;
            if !w.actions.insert(f.action.clone()) {
                continue;
            }
            let fields: Vec<(&str, &str)> = f
                .fields
                .iter()
                .filter(|(k, _)| k != qsweb::session::CSRF_FIELD)
                .map(|(k, v)| (k.as_str(), v.as_str()))
                .collect();
            let r = b.post(&f.action.replace("&amp;", "&"), &fields)?;
            if r.status != 403 || !r.text().contains("QMC") {
                return Err(format!(
                    "{path}: its form to {} without the token answered {}",
                    f.action, r.status
                ));
            }
        }
        queue.extend(links(&text));
        if w.pages > 3000 {
            return Err("more than 3000 pages: the crawl does not end".into());
        }
    }
    Ok(w)
}

/// An error page says what was asked and nothing of the inside.
fn errors(b: &mut Browser) -> Result<String, String> {
    let mut seen = 0;
    for (method, path) in [
        ("GET", "/qsmc/no-such-page"),
        ("GET", "/qsmc/partition/NOHOST/99999/general"),
        ("POST", "/qsmc/welcome"),
        ("GET", "/qsmc/msg/QMC99999"),
    ] {
        let a = b.send(method, path, &[], &[])?;
        let t = a.text();
        for inside in [
            "panicked",
            "src/",
            ".rs:",
            "/data/",
            "/run/",
            "RUST_BACKTRACE",
            "thread '",
        ] {
            if t.contains(inside) {
                return Err(format!("{method} {path} ({}) shows {inside:?}", a.status));
            }
        }
        seen += 1;
    }
    Ok(format!("{seen} error pages show nothing of the inside"))
}

/// The terminal's socket opens only from the console's own page.
fn socket(lab: &mut Lab<'_>) -> Result<String, String> {
    let b = &mut lab.c.browser;
    let path = "/qsmc/terminal/QSHQVS1/1/ws";
    for origin in [None, Some("https://evil.example"), Some("null")] {
        match b.websocket_from(path, origin) {
            Err(e) if e.contains("403") => {}
            Err(e) => return Err(format!("the socket from {origin:?}: {e}")),
            Ok(_) => return Err(format!("the socket opened from {origin:?}")),
        }
    }
    let mut stranger = b.fresh();
    if stranger.websocket(path).is_ok() {
        return Err("the socket opened with no session".into());
    }
    let _ = stranger.get("/qsmc/welcome")?;
    Ok("the terminal socket refused another origin, none, and no session".into())
}

/// The password hash's parameters, and how long a sign-on takes here.
fn hash(lab: &mut Lab<'_>) -> Result<String, String> {
    let text = channel(&mut lab.c.guest)?.text("/data/state/webusers.toml")?;
    let params = text
        .split("$argon2id$")
        .nth(1)
        .and_then(|r| r.split('$').nth(1))
        .ok_or("no Argon2id hash in webusers.toml")?
        .to_string();
    let mut times = Vec::new();
    for _ in 0..5 {
        let mut b = lab.c.browser.fresh();
        let at = Instant::now();
        signed_on(&mut b, ADMIN, PASSWORD)?;
        times.push(at.elapsed().as_millis());
    }
    times.sort_unstable();
    Ok(format!(
        "Argon2id {params}; sign-on {} ms (median of 5) on a guest of {} processors and {} MiB",
        times[2],
        crate::qemu::CPUS,
        crate::qemu::MEMORY_MB
    ))
}

/// The suite.
pub fn web(run: &Run<'_>) -> Result<String, String> {
    let mut lab = lab(run, "sec-web")?;
    let mut said = vec![framing(&lab.c.browser)?];
    said.push(climbing(&mut lab.c.browser)?);
    said.push(redirects(&mut lab.c.browser)?);
    said.push(fixation(&lab.c.browser)?);
    let before = snapshot(&mut lab.c.browser)?;
    let w = walk(&mut lab.c.browser)?;
    if snapshot(&mut lab.c.browser)? != before {
        return Err("the crawl's GETs changed what the pages show".into());
    }
    said.push(format!(
        "{} pages, every link resolving ({} redirects), each with the frame, sniffing, referrer and cache headers and the policy without \
         script ({} terminal windows with their own); {} forms, {} actions, each refused without \
         its token; nothing changed by the crawl's GETs",
        w.pages,
        w.redirects,
        w.windows,
        w.forms,
        w.actions.len()
    ));
    said.push(errors(&mut lab.c.browser)?);
    said.push(socket(&mut lab)?);
    said.push(hash(&mut lab)?);
    lab.c.guest.kill();
    Ok(said.join("; "))
}
