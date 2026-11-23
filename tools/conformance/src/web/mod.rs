// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! The web interface from outside: a browser's requests to a guest's
//! 443 and 80, through QEMU's forwarded ports. TLS is pinned to the
//! certificate the harness was told to expect: a connection to any
//! other fails. A request to 443 is QS/400's `httpc`'s, pinned; what
//! `httpc` does not do is written here: a sign-on and a request to 80
//! that name the guest as a browser would, bytes of a hostile client,
//! the WebSocket.

pub mod form;
pub mod stream;
pub mod ws;

use std::fmt::Write as _;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::time::Duration;

use rustls::pki_types::ServerName;
use rustls::{ClientConfig, ClientConnection, StreamOwned};

/// The guest's address on QEMU's user network, as a browser names it.
pub const GUEST: &str = "10.0.2.15";

/// How long a request may take.
const WAIT: Duration = Duration::from_secs(30);

/// A free port on the host, for a forward.
pub fn free_port() -> Result<u16, String> {
    let l = TcpListener::bind("127.0.0.1:0").map_err(|e| e.to_string())?;
    l.local_addr().map(|a| a.port()).map_err(|e| e.to_string())
}

/// A client that trusts only `sha256` and checks it on every
/// connection: no session is resumed, as a resumption skips the check.
fn pinned(sha256: &str) -> Result<Arc<ClientConfig>, String> {
    let mut c = (*webd::selftest::pinned_client(sha256)?).clone();
    c.resumption = rustls::client::Resumption::disabled();
    Ok(Arc::new(c))
}

/// An answer.
#[derive(Clone, Debug, Default)]
pub struct Answer {
    /// Its status.
    pub status: u16,
    /// Its headers, names lower case.
    pub headers: Vec<(String, String)>,
    /// Its body.
    pub body: Vec<u8>,
}

impl Answer {
    /// A header's value.
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.as_str())
    }

    /// The body as text.
    pub fn text(&self) -> String {
        String::from_utf8_lossy(&self.body).into_owned()
    }

    /// The `csrf` field of the page's first form.
    pub fn csrf(&self) -> Option<String> {
        qsweb::golden::tokens(&self.text())
            .into_iter()
            .find_map(|t| match t {
                qsweb::golden::Tok::Open(n, a)
                    if n == "input" && a.get("name").map(String::as_str) == Some("csrf") =>
                {
                    a.get("value").cloned()
                }
                _ => None,
            })
    }
}

fn parse(raw: &[u8]) -> Result<Answer, String> {
    let end = raw
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .ok_or("the answer has no end to its head")?;
    let head = String::from_utf8_lossy(&raw[..end]);
    let mut lines = head.split("\r\n");
    let status = lines
        .next()
        .and_then(|l| l.split(' ').nth(1))
        .and_then(|s| s.parse().ok())
        .ok_or("the answer has no status")?;
    let headers: Vec<(String, String)> = lines
        .filter_map(|l| l.split_once(':'))
        .map(|(k, v)| (k.trim().to_ascii_lowercase(), v.trim().to_string()))
        .collect();
    let mut body = raw[end + 4..].to_vec();
    if let Some(n) = headers
        .iter()
        .find(|(k, _)| k == "content-length")
        .and_then(|(_, v)| v.parse::<usize>().ok())
    {
        body.truncate(n);
    }
    Ok(Answer {
        status,
        headers,
        body,
    })
}

/// A browser of one console: its forwarded ports, the certificate it
/// trusts, its session's cookie.
#[derive(Clone)]
pub struct Browser {
    /// The host port forwarded to the guest's 443.
    pub https: u16,
    /// The host port forwarded to the guest's 80.
    pub http: u16,
    /// The `Host` its requests name.
    pub host: String,
    tls: Arc<ClientConfig>,
    trust: httpc::Trust,
    /// The session's cookie value.
    pub cookie: Option<String>,
}

impl Browser {
    /// A browser that trusts only the certificate with `sha256`.
    pub fn new(https: u16, http: u16, sha256: &str) -> Result<Self, String> {
        Ok(Self {
            https,
            http,
            host: GUEST.into(),
            tls: pinned(sha256)?,
            trust: httpc::Trust::pin(sha256).map_err(|e| e.to_string())?,
            cookie: None,
        })
    }

    /// Another browser of the same console, trusting the same
    /// certificate, with no session.
    #[must_use]
    pub fn fresh(&self) -> Self {
        Self {
            cookie: None,
            ..self.clone()
        }
    }

    /// The same browser and session, as a second tab.
    #[must_use]
    pub fn clone_session(&self) -> Self {
        self.clone()
    }

    /// Trust another certificate from now on.
    pub fn pin(&mut self, sha256: &str) -> Result<(), String> {
        self.tls = pinned(sha256)?;
        self.trust = httpc::Trust::pin(sha256).map_err(|e| e.to_string())?;
        Ok(())
    }

    fn head(&self, method: &str, path: &str, extra: &[(String, String)], len: usize) -> String {
        let mut h = format!(
            "{method} {path} HTTP/1.1\r\nHost: {}\r\nConnection: close\r\n",
            self.host
        );
        if let Some(c) = &self.cookie {
            let _ = write!(h, "Cookie: QSMCSID={c}\r\n");
        }
        for (k, v) in extra {
            let _ = write!(h, "{k}: {v}\r\n");
        }
        if method == "POST" {
            let _ = write!(h, "Content-Length: {len}\r\n");
        }
        h.push_str("\r\n");
        h
    }

    fn connect(port: u16) -> Result<TcpStream, String> {
        let s = TcpStream::connect(("127.0.0.1", port)).map_err(|e| format!("connect: {e}"))?;
        s.set_read_timeout(Some(WAIT)).map_err(|e| e.to_string())?;
        s.set_write_timeout(Some(WAIT)).map_err(|e| e.to_string())?;
        Ok(s)
    }

    /// One exchange over TLS; the cookie it sets is kept.
    pub fn send(
        &mut self,
        method: &str,
        path: &str,
        extra: &[(String, String)],
        body: &[u8],
    ) -> Result<Answer, String> {
        let mut headers = extra.to_vec();
        if let Some(c) = &self.cookie {
            headers.push(("Cookie".into(), format!("QSMCSID={c}")));
        }
        let req = httpc::Request {
            method: method.into(),
            url: format!("https://127.0.0.1:{}{path}", self.https),
            headers,
            body: body.to_vec(),
            timeout: u32::try_from(WAIT.as_secs()).unwrap_or(30),
            redirects: 0,
            trust: self.trust.clone(),
            agent: String::new(),
        };
        let r = httpc::send(&req).map_err(|e| format!("{method} {path}: {e}"))?;
        let a = Answer {
            status: r.status,
            headers: r
                .headers
                .into_iter()
                .map(|(k, v)| (k.to_ascii_lowercase(), v))
                .collect(),
            body: r.body,
        };
        self.keep(&a);
        Ok(a)
    }

    /// The cookie an answer sets, kept.
    fn keep(&mut self, a: &Answer) {
        if let Some(c) = a.header("set-cookie") {
            let v = c
                .split(';')
                .next()
                .and_then(|kv| kv.split_once('='))
                .map(|(_, v)| v.to_string());
            self.cookie = v.filter(|v| !v.is_empty());
        }
    }

    fn encoded(fields: &[(&str, &str)]) -> String {
        let body: Vec<String> = fields
            .iter()
            .map(|(k, v)| {
                format!(
                    "{}={}",
                    qsweb::http::form::encode(k),
                    qsweb::http::form::encode(v)
                )
            })
            .collect();
        body.join("&")
    }

    /// A GET.
    pub fn get(&mut self, path: &str) -> Result<Answer, String> {
        self.send("GET", path, &[], &[])
    }

    /// A form's POST.
    pub fn post(&mut self, path: &str, fields: &[(&str, &str)]) -> Result<Answer, String> {
        self.send(
            "POST",
            path,
            &[(
                "Content-Type".into(),
                "application/x-www-form-urlencoded".into(),
            )],
            Self::encoded(fields).as_bytes(),
        )
    }

    /// A file's upload: `fields`, then the file as `f`.
    pub fn upload(
        &mut self,
        path: &str,
        fields: &[(&str, &str)],
        file: &[u8],
    ) -> Result<Answer, String> {
        const B: &str = "qsmc-harness-boundary";
        let mut body = Vec::new();
        for (k, v) in fields {
            body.extend_from_slice(
                format!("--{B}\r\nContent-Disposition: form-data; name=\"{k}\"\r\n\r\n{v}\r\n")
                    .as_bytes(),
            );
        }
        body.extend_from_slice(
            format!(
                "--{B}\r\nContent-Disposition: form-data; name=\"f\"; filename=\"f\"\r\nContent-Type: application/octet-stream\r\n\r\n"
            )
            .as_bytes(),
        );
        body.extend_from_slice(file);
        body.extend_from_slice(format!("\r\n--{B}--\r\n").as_bytes());
        self.send(
            "POST",
            path,
            &[(
                "Content-Type".into(),
                format!("multipart/form-data; boundary={B}"),
            )],
            &body,
        )
    }

    /// Sign on: the answer, a redirect to Welcome when it worked.
    /// It names [`Self::host`] as a browser of that address would: the
    /// console keeps an `open` change on trial when a sign-on names the
    /// new address, and `httpc` names the forward's 127.0.0.1
    /// (`docs/OPEN_QUESTIONS.md`, item 39).
    pub fn sign_on(&mut self, user: &str, password: &str) -> Result<Answer, String> {
        let body = Self::encoded(&[("user", user), ("password", password)]);
        let ct = [(
            "Content-Type".to_string(),
            "application/x-www-form-urlencoded".to_string(),
        )];
        let mut bytes = self
            .head("POST", "/qsmc/signon", &ct, body.len())
            .into_bytes();
        bytes.extend_from_slice(body.as_bytes());
        let a = parse(&self.raw(&bytes)?)?;
        self.keep(&a);
        Ok(a)
    }

    /// The form token of `path`'s page.
    pub fn csrf(&mut self, path: &str) -> Result<String, String> {
        self.get(path)?
            .csrf()
            .ok_or_else(|| format!("{path} has no form token"))
    }

    /// A connection to 443 that is not yet TLS: what a hostile client
    /// does with it is its own.
    pub fn tcp(&self) -> Result<TcpStream, String> {
        Self::connect(self.https)
    }

    /// `bytes` as they are over TLS, and what came back before the
    /// close: empty when the server closed at once.
    pub fn raw(&self, bytes: &[u8]) -> Result<Vec<u8>, String> {
        let sock = Self::connect(self.https)?;
        let name = ServerName::try_from(GUEST).map_err(|e| e.to_string())?;
        let conn = ClientConnection::new(self.tls.clone(), name).map_err(|e| e.to_string())?;
        let mut tls = StreamOwned::new(conn, sock);
        // A server that closes in the middle is an answer too.
        let _ = tls.write_all(bytes).and_then(|()| tls.flush());
        let mut back = Vec::new();
        let _ = tls.read_to_end(&mut back);
        Ok(back)
    }

    /// The status of a raw answer, when there was one.
    pub fn status_of(raw: &[u8]) -> Option<u16> {
        parse(raw).ok().map(|a| a.status)
    }

    /// A GET on port 80, in the clear.
    pub fn plain(&self, path: &str) -> Result<Answer, String> {
        let mut s = Self::connect(self.http)?;
        s.write_all(self.head("GET", path, &[], 0).as_bytes())
            .map_err(|e| e.to_string())?;
        let mut raw = Vec::new();
        let _ = s.read_to_end(&mut raw);
        parse(&raw)
    }
}

/// The SHA-256 of a PEM certificate file's first certificate, as the
/// pages show it.
pub fn sha256_of_pem(pem: &[u8]) -> Result<String, String> {
    let text = std::str::from_utf8(pem).map_err(|e| e.to_string())?;
    let b = x509kit::bundle::read_pem(text, None).map_err(|e| e.to_string())?;
    let leaf = b.chain.first().ok_or("no certificate in the file")?;
    Ok(webd::tls::colons(&x509kit::crypto::sha256(&leaf.der)))
}

/// Whether `path` is the tree's `[+]` or `[-]` (`/qsmc/nav`), which
/// opens or closes a group for the session: not a page.
pub fn toggle(path: &str) -> bool {
    path.split('?').next() == Some("/qsmc/nav")
}

/// A link as a crawl follows it: the tree's `[+]` once a group (its
/// return dropped, so every page's is the same), never its `[-]`, so
/// each group once opened stays open and its pages are reached;
/// anything else as it is.
pub fn crawled(link: String) -> Option<String> {
    if !toggle(&link) {
        return Some(link);
    }
    let q = link.split_once('?').map_or("", |(_, q)| q);
    q.split('&')
        .find(|kv| kv.starts_with("open="))
        .map(|kv| format!("/qsmc/nav?{kv}"))
}

/// Whether the crawl's `path`, answered `status`, was a `[+]` of the
/// tree, which answers 303 and is not checked as a page.
pub fn opened(path: &str, status: u16) -> Result<bool, String> {
    match (toggle(path), status) {
        (false, _) => Ok(false),
        (true, 303) => Ok(true),
        (true, s) => Err(format!("{path} answered {s}")),
    }
}

/// Whether `page` carries script of its own: a `script` element other
/// than the navigation script every framed page loads, a `javascript:`
/// link, or an inline handler other than the help window's Close
/// window (`webd::serve::CLOSE_HANDLER`, the one the policy admits).
pub fn scripted(page: &str) -> bool {
    let nav = format!(
        "<script type=\"text/javascript\" src=\"{}\" defer=\"defer\"></script>",
        webd::product::NAV_PATH
    );
    let close = format!("onclick=\"{}\"", webd::serve::CLOSE_HANDLER);
    let rest = page
        .replace(&nav, "")
        .replace(&close, "")
        .to_ascii_lowercase();
    rest.contains("<script") || rest.contains("javascript:") || rest.contains(" onclick=")
}
