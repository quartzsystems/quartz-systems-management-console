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

/// `r` with the security headers every answer on 443 carries.
pub fn secured(r: Response) -> Response {
    let r = r
        .with("Content-Security-Policy", CSP)
        .with("X-Frame-Options", "DENY")
        .with("X-Content-Type-Options", "nosniff")
        .with("Referrer-Policy", "no-referrer");
    if r.header("Cache-Control").is_some() {
        r
    } else {
        r.with("Cache-Control", "no-store")
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
}

/// Port 80: every request is sent to 443.
pub struct Redirect;

/// Where a request on 80 is sent: its host without a port, its path
/// and query.
pub fn https_location(req: &Request, local: &str) -> String {
    let host = req
        .header("host")
        .map(str::trim)
        .filter(|h| !h.is_empty() && h.bytes().all(|b| b.is_ascii_graphic()))
        .unwrap_or(local);
    // `[v6]:port` keeps its brackets; `name:port` loses the port.
    let host = match host.rsplit_once(':') {
        Some((h, p)) if p.bytes().all(|b| b.is_ascii_digit()) && !h.ends_with(':') => h,
        _ => host,
    };
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

    fn handle(&self, req: &Request, peer: &Peer) -> Response {
        let local = peer.addr.ip().to_string();
        Response::new(
            301,
            "text/plain; charset=utf-8",
            b"Moved to HTTPS.\n".to_vec(),
        )
        .with("Location", &https_location(req, &local))
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
        assert_eq!(
            https_location(
                &request("/qsmc/welcome?x=1", Some("qsmc.example:80")),
                "10.0.0.1"
            ),
            "https://qsmc.example/qsmc/welcome?x=1"
        );
        assert_eq!(
            https_location(&request("/", None), "10.0.0.1"),
            "https://10.0.0.1/"
        );
        assert_eq!(
            https_location(&request("/", Some("bad host\r\n")), "10.0.0.1"),
            "https://10.0.0.1/"
        );
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
