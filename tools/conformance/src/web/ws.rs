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
    /// Open the WebSocket at `path` in this browser's session, from the
    /// console's own page, as a browser says (`Origin`).
    pub fn websocket(&self, path: &str) -> Result<Ws, String> {
        self.websocket_from(path, Some(&format!("https://{}", self.host)))
    }

    /// The same from `origin`; `None`: no `Origin` said.
    pub fn websocket_from(&self, path: &str, origin: Option<&str>) -> Result<Ws, String> {
        let sock = TcpStream::connect(("127.0.0.1", self.https)).map_err(|e| e.to_string())?;
        // The handshake and the upgrade wait as a browser would; the
        // frames after are read a short while at a time.
        sock.set_read_timeout(Some(Duration::from_secs(30)))
            .map_err(|e| e.to_string())?;
        let short = sock.try_clone().map_err(|e| e.to_string())?;
        let name = ServerName::try_from(GUEST).map_err(|e| e.to_string())?;
        let conn = ClientConnection::new(self.tls.clone(), name).map_err(|e| e.to_string())?;
        let mut tls = StreamOwned::new(conn, sock);
        let cookie = self
            .cookie
            .as_ref()
            .map(|c| format!("Cookie: QSMCSID={c}\r\n"))
            .unwrap_or_default();
        let origin = origin
            .map(|o| format!("Origin: {o}\r\n"))
            .unwrap_or_default();
        let head = format!(
            "GET {path} HTTP/1.1\r\nHost: {}\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\nSec-WebSocket-Version: 13\r\n{origin}{cookie}\r\n",
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
        short
            .set_read_timeout(Some(Duration::from_millis(200)))
            .map_err(|e| e.to_string())?;
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
        let mut texts = Vec::new();
        while let Some((op, body, used)) = frame(&self.buf) {
            self.buf.drain(..used);
            match op {
                1 => texts.push(String::from_utf8_lossy(&body).into_owned()),
                8 => self.closed = true,
                _ => {}
            }
        }
        texts
    }
}

/// The first whole frame of `buf` from the server (unmasked): its
/// opcode, its payload and the bytes it took. A frame not all here
/// yet waits for the next read.
fn frame(buf: &[u8]) -> Option<(u8, Vec<u8>, usize)> {
    let op = buf.first()? & 0x0f;
    let short = buf.get(1)? & 0x7f;
    let (len, at) = match short {
        126 => (
            usize::from(u16::from_be_bytes([*buf.get(2)?, *buf.get(3)?])),
            4,
        ),
        127 => {
            let b: [u8; 8] = buf.get(2..10)?.try_into().ok()?;
            (usize::try_from(u64::from_be_bytes(b)).ok()?, 10)
        }
        n => (usize::from(n), 2),
    };
    let body = buf.get(at..at + len)?;
    Some((op, body.to_vec(), at + len))
}

#[cfg(test)]
mod tests {
    use super::frame;

    #[test]
    fn a_frame_cut_short_waits() {
        let mut f = vec![0x81, 126, 0x01, 0x00];
        f.extend(std::iter::repeat_n(b'x', 256));
        assert!(frame(&f[..100]).is_none());
        let (op, body, used) = frame(&f).unwrap();
        assert_eq!((op, body.len(), used), (1, 256, 260));
    }
}
