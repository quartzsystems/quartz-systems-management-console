// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! A stream server of the harness's own, as the console reads one
//! (`fetchd`): `GET /<stream>/v1/index` (or `catalog` for `qs400`) and
//! `GET /<stream>/v1/file/<name>`, plain HTTP/1.1, one request a
//! connection, a `Range: bytes=N-` answered 206. It can be made slow,
//! cut a file short, serve other bytes than it was given, or answer
//! nothing, and it says what it was asked and from where.

use std::collections::BTreeMap;
use std::io::{BufRead as _, BufReader, Write as _};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use ed25519_dalek::SigningKey;
use sha2::{Digest, Sha256};

/// How the server answers.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum Mode {
    /// As asked.
    #[default]
    Normal,
    /// Each piece of 16 KiB after this long.
    Slow(Duration),
    /// A file's body ends after this many bytes, the connection closed;
    /// once, then as asked again.
    CutOnce(u64),
    /// Every file's bytes changed: its size the same, its hash not.
    Wrong,
    /// Every connection closed at once, unanswered.
    Absent,
}

#[derive(Default)]
struct Served {
    files: BTreeMap<String, Vec<u8>>,
    mode: Mode,
    asked: Vec<(SocketAddr, String)>,
}

/// The server, until it is dropped.
pub struct Server {
    addr: SocketAddr,
    served: Arc<Mutex<Served>>,
    stop: Arc<AtomicBool>,
}

fn lock(m: &Mutex<Served>) -> MutexGuard<'_, Served> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

/// A release of a stream, as its index names it.
#[derive(Clone, Debug)]
pub struct Release {
    /// `V1R0M1`.
    pub release: String,
    /// Its build.
    pub build: String,
    /// The least release it updates from.
    pub updates_from: String,
    /// Its update image's file and bytes.
    pub update: (String, Vec<u8>),
    /// Its date, `YYYY-MM-DD`.
    pub released: String,
    /// Its notes' file and text.
    pub notes: (String, String),
}

impl Server {
    /// A server on `bind` (`0.0.0.0:0` for any port).
    ///
    /// # Panics
    /// The port could not be bound.
    pub fn start(bind: &str) -> Self {
        let l = TcpListener::bind(bind).expect("the stream server's port");
        let addr = l.local_addr().expect("its address");
        let served: Arc<Mutex<Served>> = Arc::default();
        let stop = Arc::new(AtomicBool::new(false));
        let (s, st) = (served.clone(), stop.clone());
        std::thread::spawn(move || {
            for c in l.incoming().flatten() {
                if st.load(Ordering::Relaxed) {
                    break;
                }
                let s = s.clone();
                std::thread::spawn(move || answer(&s, c));
            }
        });
        Self { addr, served, stop }
    }

    /// Its port.
    pub fn port(&self) -> u16 {
        self.addr.port()
    }

    /// Serve `bytes` at `path`.
    pub fn put(&self, path: &str, bytes: Vec<u8>) {
        lock(&self.served).files.insert(path.to_string(), bytes);
    }

    /// Serve no more at `path`.
    pub fn remove(&self, path: &str) {
        lock(&self.served).files.remove(path);
    }

    /// Answer as `mode` says from now on.
    pub fn set_mode(&self, mode: Mode) {
        lock(&self.served).mode = mode;
    }

    /// Every request so far: where it came from and its line.
    pub fn asked(&self) -> Vec<(SocketAddr, String)> {
        lock(&self.served).asked.clone()
    }

    /// Publish `releases` as stream `name`'s index at `sequence`, signed
    /// by `key`, each release's update image and notes beside it.
    pub fn publish(&self, name: &str, sequence: u64, releases: &[Release], key: &SigningKey) {
        let entries = releases
            .iter()
            .map(|r| stream::Entry {
                release: r.release.clone(),
                build: r.build.clone(),
                fixes: Vec::new(),
                updates_from: r.updates_from.clone(),
                published: 1_793_000_000,
                update: r.update.0.clone(),
                update_bytes: r.update.1.len() as u64,
                update_sha256: hex::encode(Sha256::digest(&r.update.1)),
                iso: format!("{}.iso", r.update.0),
                iso_bytes: 0,
                iso_sha256: "00".repeat(32),
                released: r.released.clone(),
                notes: r.notes.0.clone(),
            })
            .collect();
        let index = stream::Index {
            format: stream::FORMAT,
            stream: name.into(),
            sequence,
            generated: 1_793_000_000,
            releases: entries,
        };
        self.put(
            &format!("/{name}/v1/index"),
            stream::sign(&index, key).into_bytes(),
        );
        for r in releases {
            self.put(
                &format!("/{name}/v1/file/{}", r.update.0),
                r.update.1.clone(),
            );
            if !r.notes.0.is_empty() {
                self.put(
                    &format!("/{name}/v1/file/{}", r.notes.0),
                    r.notes.1.clone().into_bytes(),
                );
            }
        }
    }

    /// Publish QS/400's catalog at `sequence` with one cumulative package
    /// `name` of `release`, signed by `key`.
    pub fn publish_catalog(&self, sequence: u64, release: &str, name: &str, key: &SigningKey) {
        let c = updcat::Catalog {
            format: updcat::FORMAT,
            sequence,
            generated: 1_793_000_000,
            products: Vec::new(),
            ptfs: Vec::new(),
            groups: vec![updcat::CatalogGroup {
                name: name.into(),
                kind: updcat::GroupKind::Cumulative,
                level: 1,
                release: release.into(),
                text: "Cumulative PTF package".into(),
                ptfs: Vec::new(),
                related: Vec::new(),
            }],
        };
        self.put("/qs400/v1/catalog", c.sign(key).into_bytes());
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        // The accept is woken to see it.
        let _ = TcpStream::connect(self.addr);
    }
}

fn status(c: &mut TcpStream, code: u16, words: &str) {
    let _ = write!(
        c,
        "HTTP/1.1 {code} {words}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
    );
}

fn answer(served: &Mutex<Served>, mut c: TcpStream) {
    let _ = c.set_read_timeout(Some(Duration::from_secs(10)));
    let Ok(peer) = c.peer_addr() else { return };
    let mut r = BufReader::new(match c.try_clone() {
        Ok(c) => c,
        Err(_) => return,
    });
    let mut line = String::new();
    if r.read_line(&mut line).is_err() {
        return;
    }
    let mut from = 0u64;
    loop {
        let mut h = String::new();
        if r.read_line(&mut h).is_err() || h.trim().is_empty() {
            break;
        }
        if let Some(v) = h.to_ascii_lowercase().strip_prefix("range:") {
            from = v
                .trim()
                .strip_prefix("bytes=")
                .and_then(|v| v.trim_end_matches('-').split('-').next()?.parse().ok())
                .unwrap_or(0);
        }
    }
    let (body, mode) = {
        let mut s = lock(served);
        s.asked.push((peer, line.trim().to_string()));
        let path = line.split_whitespace().nth(1).unwrap_or("").to_string();
        let body = s.files.get(&path).cloned();
        let mode = s.mode.clone();
        if matches!(mode, Mode::CutOnce(_)) && path.contains("/v1/file/") {
            s.mode = Mode::Normal;
        }
        (body, mode)
    };
    if mode == Mode::Absent {
        return;
    }
    let Some(mut body) = body else {
        return status(&mut c, 404, "Not Found");
    };
    if mode == Mode::Wrong && line.contains("/v1/file/") {
        for b in &mut body {
            *b ^= 0x5a;
        }
    }
    let total = body.len() as u64;
    let start = from.min(total);
    let head = if from > 0 {
        format!(
            "HTTP/1.1 206 Partial Content\r\nContent-Range: bytes {start}-{}/{total}\r\n",
            total.saturating_sub(1)
        )
    } else {
        "HTTP/1.1 200 OK\r\n".to_string()
    };
    let len = total - start;
    let _ = write!(
        c,
        "{head}Content-Type: application/octet-stream\r\nContent-Length: {len}\r\nConnection: close\r\n\r\n"
    );
    let rest = &body[usize::try_from(start).unwrap_or(0)..];
    let end = match mode {
        Mode::CutOnce(n) if line.contains("/v1/file/") => {
            usize::try_from(n).unwrap_or(0).min(rest.len())
        }
        _ => rest.len(),
    };
    for piece in rest[..end].chunks(16 * 1024) {
        if let Mode::Slow(d) = mode {
            std::thread::sleep(d);
        }
        if c.write_all(piece).is_err() {
            return;
        }
    }
    let _ = c.flush();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn get(port: u16, path: &str, from: u64) -> (u16, Vec<u8>) {
        let mut out = Vec::new();
        let req = httpc::Request {
            method: "GET".into(),
            url: format!("http://127.0.0.1:{port}{path}"),
            timeout: 5,
            ..httpc::Request::default()
        };
        match httpc::download(&req, from, 1 << 20, &mut out) {
            Ok(f) => (f.status, out),
            Err(httpc::Error::Status(s)) => (s, out),
            Err(_) => (0, out),
        }
    }

    #[test]
    fn a_stream_is_served_resumed_cut_and_absent() {
        let s = Server::start("127.0.0.1:0");
        let key = super::super::keys::test_key("qsvs");
        s.publish(
            "qsvs",
            2,
            &[Release {
                release: "V1R0M1".into(),
                build: "t".into(),
                updates_from: "V1R0M0".into(),
                update: ("QSVS-V1R0M1-update-ab.upd".into(), b"0123456789".to_vec()),
                released: "2027-01-02".into(),
                notes: ("NOTES.md".into(), "# Notes".into()),
            }],
            &key,
        );
        let (st, index) = get(s.port(), "/qsvs/v1/index", 0);
        assert_eq!(st, 200);
        let i = stream::verify(&index, &[key.verifying_key().to_bytes()], 0).unwrap();
        assert_eq!(i.releases[0].update_bytes, 10);
        assert_eq!(
            get(s.port(), "/qsvs/v1/file/QSVS-V1R0M1-update-ab.upd", 4),
            (206, b"456789".to_vec())
        );
        s.set_mode(Mode::CutOnce(3));
        assert_eq!(
            get(s.port(), "/qsvs/v1/file/QSVS-V1R0M1-update-ab.upd", 0).1,
            b"012"
        );
        assert_eq!(
            get(s.port(), "/qsvs/v1/file/QSVS-V1R0M1-update-ab.upd", 0).1,
            b"0123456789"
        );
        s.set_mode(Mode::Wrong);
        assert_ne!(
            get(s.port(), "/qsvs/v1/file/QSVS-V1R0M1-update-ab.upd", 0).1,
            b"0123456789"
        );
        s.set_mode(Mode::Absent);
        assert_eq!(get(s.port(), "/qsvs/v1/index", 0).0, 0);
        assert_eq!(get(s.port(), "/nothing", 0).0, 0);
        s.set_mode(Mode::Normal);
        assert_eq!(get(s.port(), "/nothing", 0).0, 404);
        assert!(s.asked().len() >= 7);
    }
}
