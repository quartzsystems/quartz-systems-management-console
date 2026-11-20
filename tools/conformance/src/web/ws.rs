// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! A terminal window's WebSocket as the page's script opens it: the
//! upgrade over TLS pinned to the console's certificate, then text
//! messages each way (`qsweb::ws`'s client frames and its reader of the
//! server's).

use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::{Duration, Instant};

use rustls::pki_types::ServerName;
use rustls::{ClientConnection, StreamOwned};

use super::{Browser, GUEST};

/// An open socket.
pub struct Ws {
    tls: StreamOwned<ClientConnection, TcpStream>,
    buf: Vec<u8>,
    /// Whether the server closed it.
    pub closed: bool,
}

impl Browser {
    /// Open the WebSocket at `path` in this browser's session.
    pub fn websocket(&self, path: &str) -> Result<Ws, String> {
        let sock = TcpStream::connect(("127.0.0.1", self.https)).map_err(|e| e.to_string())?;
        sock.set_read_timeout(Some(Duration::from_millis(200)))
            .map_err(|e| e.to_string())?;
        let name = ServerName::try_from(GUEST).map_err(|e| e.to_string())?;
        let conn = ClientConnection::new(self.tls.clone(), name).map_err(|e| e.to_string())?;
        let mut tls = StreamOwned::new(conn, sock);
        let cookie = self
            .cookie
            .as_ref()
            .map(|c| format!("Cookie: QSMCSID={c}\r\n"))
            .unwrap_or_default();
        let head = format!(
            "GET {path} HTTP/1.1\r\nHost: {}\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\nSec-WebSocket-Version: 13\r\n{cookie}\r\n",
            self.host
        );
        tls.write_all(head.as_bytes()).map_err(|e| e.to_string())?;
        let mut got = Vec::new();
        let until = Instant::now() + Duration::from_secs(30);
        while !got.windows(4).any(|w| w == b"\r\n\r\n") {
            let mut b = [0u8; 4096];
            match tls.read(&mut b) {
                Ok(0) => return Err("the server closed the upgrade".into()),
                Ok(n) => got.extend_from_slice(&b[..n]),
                Err(_) if Instant::now() < until => {}
                Err(e) => return Err(format!("the upgrade: {e}")),
            }
        }
        let end = got.windows(4).position(|w| w == b"\r\n\r\n").unwrap_or(0) + 4;
        let head = String::from_utf8_lossy(&got[..end]).to_string();
        if !head.starts_with("HTTP/1.1 101") {
            return Err(format!(
                "the upgrade was answered {}",
                head.lines().next().unwrap_or("")
            ));
        }
        Ok(Ws {
            tls,
            buf: got[end..].to_vec(),
            closed: false,
        })
    }
}

impl Ws {
    /// Send `text`.
    pub fn send(&mut self, text: &str) -> Result<(), String> {
        self.tls
            .write_all(&qsweb::ws::client_frame(text))
            .and_then(|()| self.tls.flush())
            .map_err(|e| e.to_string())
    }

    /// Every text message that came within `wait`.
    pub fn recv(&mut self, wait: Duration) -> Vec<String> {
        let until = Instant::now() + wait;
        while Instant::now() < until && !self.closed {
            let mut b = [0u8; 16 * 1024];
            match self.tls.read(&mut b) {
                Ok(0) => self.closed = true,
                Ok(n) => self.buf.extend_from_slice(&b[..n]),
                Err(e)
                    if matches!(
                        e.kind(),
                        std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                    ) => {}
                Err(_) => self.closed = true,
            }
        }
        let texts = qsweb::ws::server_texts(&self.buf);
        self.buf.clear();
        texts
    }
}
