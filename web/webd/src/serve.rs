// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! What answers a request. On 443 ([`Site`]): the stylesheet with its
//! validators, the pages ([`Pages`]), every answer with the headers a
//! page without script can have. On 80 ([`Redirect`]): nothing but a
//! redirect to the same path on 443.

use std::fmt::Write as _;
use std::sync::Arc;

use qsweb::http::{Handler, Peer, Request, Response};

use crate::product::{BRAND, CSS_PATH, PREFIX};

/// The content security policy: the page itself and its stylesheet,
/// nothing else; forms post to the console only; no framing.
pub const CSP: &str = "default-src 'none'; style-src 'self'; img-src 'self'; form-action 'self'; frame-ancestors 'none'; base-uri 'none'";

/// The stylesheet's entity tag: its SHA-256, the first 16 hex digits.
pub fn css_etag() -> String {
    let sum = x509kit::crypto::sha256(qsweb::CSS);
    let mut tag = String::from("\"");
    for b in &sum[..8] {
        let _ = write!(tag, "{b:02x}");
    }
    tag.push('"');
    tag
}

/// `r` with the security headers every answer on 443 carries. A page
/// that set its own policy keeps it: the terminal window alone does,
/// for its script.
pub fn secured(r: Response) -> Response {
    let r = if r.header("Content-Security-Policy").is_some() {
        r
    } else {
        r.with("Content-Security-Policy", CSP)
    };
    let r = r
        .with("X-Frame-Options", "DENY")
        .with("X-Content-Type-Options", "nosniff")
        .with("Referrer-Policy", "no-referrer");
    if r.header("Cache-Control").is_some() {
        r
    } else {
        r.with("Cache-Control", "no-store")
    }
}

/// The connections whose request was refused for its framing: `qsweb`
/// would carry on, so each request after it is refused too, never acted
/// on, and the connection is read no further ([`crate::listen`]).
static ENDED: std::sync::Mutex<Vec<std::net::SocketAddr>> = std::sync::Mutex::new(Vec::new());

fn end(peer: &Peer) {
    if let Ok(mut v) = ENDED.lock() {
        v.push(peer.addr);
    }
}

/// Whether the connection from `addr` had a request refused for its
/// framing.
pub fn ended(addr: std::net::SocketAddr) -> bool {
    ENDED
        .lock()
        .is_ok_and(|v| !v.is_empty() && v.contains(&addr))
}

/// The connection from `addr` is closed.
pub fn forget(addr: std::net::SocketAddr) {
    if let Ok(mut v) = ENDED.lock() {
        v.retain(|a| *a != addr);
    }
}

/// [`framing_refused`], or a request after one on its connection.
fn refused(req: &Request, peer: &Peer) -> Option<Response> {
    if let Some(r) = framing_refused(req) {
        end(peer);
        return Some(r);
    }
    ended(peer.addr).then(|| {
        qsweb::http::plain(400, "The request's length is not said one way.")
            .with("Connection", "close")
    })
}

/// A request whose body could be read two ways is refused: a length and
/// chunks both, two lengths, or a transfer coding other than `chunked`
/// (`docs/SECURITY.md`). `qsweb` reads it one way; a request another
/// reader would see as two is never acted on.
pub fn framing_refused(req: &Request) -> Option<Response> {
    let count = |name: &str| req.headers.iter().filter(|(k, _)| k == name).count();
    let (lengths, codings) = (count("content-length"), count("transfer-encoding"));
    let coding_ok = req
        .header("transfer-encoding")
        .is_none_or(|t| t.trim().eq_ignore_ascii_case("chunked"));
    if lengths > 1 || codings > 1 || (lengths == 1 && codings == 1) || !coding_ok {
        Some(
            qsweb::http::plain(400, "The request's length is not said one way.")
                .with("Connection", "close"),
        )
    } else {
        None
    }
}

/// The stylesheet, or `304` when the client has it.
pub fn stylesheet(req: &Request) -> Response {
    let etag = css_etag();
    if req
        .header("if-none-match")
        .is_some_and(|tags| tags.split(',').any(|t| t.trim() == etag))
    {
        return Response::new(304, "text/css", Vec::new())
            .with("ETag", &etag)
            .with("Cache-Control", "max-age=3600");
    }
    Response::new(200, "text/css", qsweb::CSS.to_vec())
        .with("ETag", &etag)
        .with("Cache-Control", "max-age=3600")
}

/// The pages: everything under the prefix but the stylesheet.
pub trait Pages: Send + Sync {
    /// The answer to `req`.
    fn page(&self, req: &Request, peer: &Peer) -> Response;

    /// The server has started with `certs`, which the pages may show
    /// and replace.
    fn started(&self, _certs: Arc<crate::tls::Certs>) {}

    /// Whether a connection from `ip` is answered at all (Allow web UI
    /// from). One that is not is closed at once.
    fn admits(&self, _ip: std::net::IpAddr) -> bool {
        true
    }

    /// Whether `req`'s body is read by the page as it arrives
    /// ([`Pages::page_stream`]) rather than whole first: an upload.
    fn streams(&self, _req: &Request, _peer: &Peer) -> bool {
        false
    }

    /// The answer to `req`, whose body is read from `body`.
    fn page_stream(&self, req: &Request, _body: &mut dyn std::io::Read, peer: &Peer) -> Response {
        self.page(req, peer)
    }
}

/// No pages: the stylesheet only, every other path not found.
pub struct NoPages;

impl Pages for NoPages {
    fn page(&self, _req: &Request, _peer: &Peer) -> Response {
        qsweb::http::plain(404, "There is no such page.")
    }
}

/// The console on 443.
pub struct Site {
    /// The pages.
    pub pages: Arc<dyn Pages>,
}

impl Handler for Site {
    fn server(&self) -> String {
        BRAND.server_header()
    }

    fn handle(&self, req: &Request, peer: &Peer) -> Response {
        if let Some(r) = refused(req, peer) {
            return secured(r);
        }
        let r = if req.path == CSS_PATH {
            if req.method == "GET" || req.method == "HEAD" {
                stylesheet(req)
            } else {
                qsweb::http::plain(405, "The stylesheet is only read.")
            }
        } else if req.path == "/" || req.path == PREFIX {
            Response::redirect(&format!("{PREFIX}/"))
        } else {
            self.pages.page(req, peer)
        };
        secured(r)
    }

    fn streams(&self, req: &Request, peer: &Peer) -> bool {
        // A request refused for its framing is answered before its body
        // is read, and its connection is closed after.
        framing_refused(req).is_some()
            || ended(peer.addr)
            || (req.path != CSS_PATH && self.pages.streams(req, peer))
    }

    fn handle_stream(&self, req: &Request, body: &mut dyn std::io::Read, peer: &Peer) -> Response {
        if let Some(r) = refused(req, peer) {
            return secured(r);
        }
        secured(self.pages.page_stream(req, body, peer))
    }
}

/// Port 80: every request is sent to 443 of the console itself.
pub struct Redirect {
    /// The console's host name.
    pub host: String,
    /// The address the request came to, as a URL names it.
    pub local: String,
}

/// The address `ip` as a URL's host names it.
pub fn url_host(ip: std::net::IpAddr) -> String {
    match ip {
        std::net::IpAddr::V4(v4) => v4.to_string(),
        std::net::IpAddr::V6(v6) => format!("[{v6}]"),
    }
}

/// Where a request on 80 is sent: the host it asked for, without its
/// port, when that names the console (`local`, the address it came to,
/// or `name`, its host name, alone or with a domain), else `local`; its
/// path and query. A redirect never leaves the console.
pub fn https_location(req: &Request, name: &str, local: &str) -> String {
    let asked = req
        .header("host")
        .map(str::trim)
        .filter(|h| !h.is_empty() && h.bytes().all(|b| b.is_ascii_graphic()))
        .unwrap_or("");
    // `[v6]:port` keeps its brackets; `name:port` loses the port.
    let asked = match asked.rsplit_once(':') {
        Some((h, p)) if p.bytes().all(|b| b.is_ascii_digit()) && !h.ends_with(':') => h,
        _ => asked,
    };
    let first = asked.split('.').next().unwrap_or("");
    let named = !name.is_empty()
        && first.eq_ignore_ascii_case(name)
        && asked
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'-');
    let host = if named || asked.eq_ignore_ascii_case(local) {
        asked
    } else {
        local
    };
    // A target that is not a path would join the host: `/` instead.
    if !req.path.starts_with('/') {
        return format!("https://{host}/");
    }
    let mut to = format!("https://{host}{}", req.path);
    if !req.query.is_empty() {
        to.push('?');
        to.push_str(&req.query);
    }
    to
}

impl Handler for Redirect {
    fn server(&self) -> String {
        BRAND.server_header()
    }

    fn handle(&self, req: &Request, _: &Peer) -> Response {
        Response::new(
            301,
            "text/plain; charset=utf-8",
            b"Moved to HTTPS.\n".to_vec(),
        )
        .with("Location", &https_location(req, &self.host, &self.local))
        .with("Cache-Control", "no-store")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(target: &str, host: Option<&str>) -> Request {
        let mut r = Request::new("GET", target);
        if let Some(h) = host {
            r.headers.push(("host".into(), h.into()));
        }
        r
    }

    #[test]
    fn port_80_sends_the_same_path_to_443() {
        let to = |target, host| https_location(&request(target, host), "QSMC123456", "10.0.0.1");
        assert_eq!(
            to("/qsmc/welcome?x=1", Some("qsmc123456.example:80")),
            "https://qsmc123456.example/qsmc/welcome?x=1"
        );
        assert_eq!(to("/", Some("QSMC123456")), "https://QSMC123456/");
        assert_eq!(to("/", Some("10.0.0.1:80")), "https://10.0.0.1/");
        assert_eq!(to("/", None), "https://10.0.0.1/");
        assert_eq!(to("/", Some("bad host\r\n")), "https://10.0.0.1/");
        assert_eq!(url_host("fe80::1".parse().unwrap()), "[fe80::1]");
    }

    #[test]
    fn a_body_said_two_ways_is_refused() {
        let with = |headers: &[(&str, &str)]| {
            let mut r = request("/qsmc/signon", None);
            for (k, v) in headers {
                r.headers.push(((*k).into(), (*v).into()));
            }
            framing_refused(&r).map(|r| r.status)
        };
        assert_eq!(with(&[("content-length", "4")]), None);
        assert_eq!(with(&[("transfer-encoding", "chunked")]), None);
        for two in [
            &[("content-length", "4"), ("transfer-encoding", "chunked")][..],
            &[("content-length", "4"), ("content-length", "5")],
            &[
                ("transfer-encoding", "chunked"),
                ("transfer-encoding", "chunked"),
            ],
            &[("transfer-encoding", "gzip, chunked")],
            &[("transfer-encoding", "xchunked")],
        ] {
            assert_eq!(with(two), Some(400), "{two:?}");
        }
    }

    #[test]
    fn what_follows_a_refused_body_on_its_connection_is_refused() {
        let peer = Peer {
            addr: "192.0.2.7:40000".parse().unwrap(),
            secure: true,
        };
        let other = Peer {
            addr: "192.0.2.7:40001".parse().unwrap(),
            secure: true,
        };
        let mut two = request("/qsmc/signon", None);
        two.headers.push(("content-length".into(), "0".into()));
        two.headers
            .push(("transfer-encoding".into(), "chunked".into()));
        assert_eq!(refused(&two, &peer).map(|r| r.status), Some(400));
        let next = request("/qsmc/welcome", None);
        assert_eq!(refused(&next, &peer).map(|r| r.status), Some(400));
        assert!(refused(&next, &other).is_none());
        forget(peer.addr);
        assert!(refused(&next, &peer).is_none());
    }

    #[test]
    fn a_redirect_never_leaves_the_console() {
        let to = |host| https_location(&request("/x", Some(host)), "QSMC123456", "10.0.0.1");
        for elsewhere in [
            "evil.example",
            "evil.example:80",
            "qsmc123456@evil.example",
            "10.0.0.2",
            "qsmc1234567.example",
            "[::1]:80",
        ] {
            assert_eq!(to(elsewhere), "https://10.0.0.1/x", "{elsewhere}");
        }
        // The fuzzer's: a target that is not a path joins no host.
        for target in ["..//qsmc/signon", ".evil.example/x", "@evil.example", ""] {
            let at = https_location(&request(target, None), "QSMC123456", "10.0.0.1");
            assert_eq!(at, "https://10.0.0.1/", "{target}");
        }
    }

    #[test]
    fn the_stylesheet_is_qsweb_s_with_validators() {
        let r = stylesheet(&request(CSS_PATH, None));
        assert_eq!(r.status, 200);
        assert_eq!(r.bytes(), qsweb::CSS);
        let mut again = request(CSS_PATH, None);
        again.headers.push(("if-none-match".into(), css_etag()));
        assert_eq!(stylesheet(&again).status, 304);
    }

    #[test]
    fn every_answer_carries_the_headers() {
        let site = Site {
            pages: Arc::new(NoPages),
        };
        let peer = Peer {
            addr: "192.0.2.9:50000".parse().unwrap(),
            secure: true,
        };
        let r = site.handle(&request("/qsmc/nothing", None), &peer);
        assert_eq!(r.status, 404);
        assert_eq!(r.header("Content-Security-Policy"), Some(CSP));
        assert_eq!(r.header("X-Frame-Options"), Some("DENY"));
        assert_eq!(r.header("Cache-Control"), Some("no-store"));
    }
}
