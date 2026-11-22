// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! An image's upload as a browser sends it, streamed: the multipart body
//! of Virtual Optical's form in its order (the form's token, the file,
//! then Upload's part), its `Content-Length` known before the first
//! byte, the image made a block at a time as it is sent. Nothing of it
//! is held whole, so an image of many gigabytes costs the harness a
//! block's memory.

use std::fmt::Write as _;
use std::io::{Read, Write};
use std::time::Duration;

use rustls::pki_types::ServerName;
use rustls::{ClientConnection, StreamOwned};

use super::{Answer, Browser, GUEST, parse};

/// The boundary the harness's bodies use.
const BOUNDARY: &str = "qsmc-stream-boundary-7c1f";
/// How much is made and written at a time.
const BLOCK: usize = 256 * 1024;
/// Where ISO 9660's primary volume descriptor is.
const PVD: u64 = 16 * 2048;

/// What to send.
#[derive(Clone, Debug)]
pub struct Upload {
    /// The form's token.
    pub csrf: String,
    /// The file's name.
    pub name: String,
    /// The image's length.
    pub bytes: u64,
    /// The image's length the body says, when it is not `bytes`: a body
    /// that announces more than it sends.
    pub announce: Option<u64>,
    /// Send this many of the image's bytes, then close: a browser that
    /// went away.
    pub cut: Option<u64>,
    /// Send only the parts before the file, then wait for the answer.
    pub head_only: bool,
    /// The volume identifier of the image's descriptor.
    pub label: String,
}

impl Upload {
    /// The parts before the file.
    fn head(&self) -> String {
        format!(
            "--{BOUNDARY}\r\nContent-Disposition: form-data; name=\"csrf\"\r\n\r\n{}\r\n--{BOUNDARY}\r\nContent-Disposition: form-data; name=\"iso\"; filename=\"{}\"\r\nContent-Type: application/octet-stream\r\n\r\n",
            self.csrf, self.name
        )
    }

    /// The part after the file: Upload's button.
    fn tail() -> String {
        format!(
            "\r\n--{BOUNDARY}\r\nContent-Disposition: form-data; name=\"upload\"\r\n\r\nUpload\r\n--{BOUNDARY}--\r\n"
        )
    }

    /// The image's descriptor block.
    fn descriptor(&self) -> Vec<u8> {
        let mut d = vec![0u8; 2048];
        d[0] = 1;
        d[1..6].copy_from_slice(b"CD001");
        d[6] = 1;
        let mut id = [b' '; 32];
        let l = self.label.as_bytes();
        id[..l.len().min(32)].copy_from_slice(&l[..l.len().min(32)]);
        d[40..72].copy_from_slice(&id);
        d
    }
}

/// Fill `buf`, the image's bytes from `at`: zeros, and the descriptor
/// where it falls.
fn fill(buf: &mut [u8], at: u64, descriptor: &[u8]) {
    buf.fill(0);
    let end = at + buf.len() as u64;
    let (lo, hi) = (PVD.max(at), (PVD + descriptor.len() as u64).min(end));
    if lo < hi {
        let (a, b) = (
            usize::try_from(lo - at).unwrap_or(0),
            usize::try_from(hi - at).unwrap_or(0),
        );
        let from = usize::try_from(lo - PVD).unwrap_or(0);
        buf[a..b].copy_from_slice(&descriptor[from..from + (b - a)]);
    }
}

impl Browser {
    /// POST `u` to `path` as a stream: the answer, or `None` when the
    /// body was cut and the connection closed before one came.
    pub fn stream_upload(&self, path: &str, u: &Upload) -> Result<Option<Answer>, String> {
        let announced = u.announce.unwrap_or(u.bytes);
        self.streamed(path, announced, u, |tls, req| body(tls, req, u))
    }

    /// POST the file at `file` to `path` as Virtual Optical's form
    /// sends it, a block at a time: the answer.
    pub fn stream_file(
        &self,
        path: &str,
        csrf: &str,
        file: &std::path::Path,
    ) -> Result<Option<Answer>, String> {
        let bytes = std::fs::metadata(file)
            .map_err(|e| format!("{}: {e}", file.display()))?
            .len();
        let u = Upload {
            csrf: csrf.into(),
            name: file
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default(),
            bytes,
            announce: None,
            cut: None,
            head_only: false,
            label: String::new(),
        };
        self.streamed(path, bytes, &u, |tls, req| {
            let mut w = |b: &[u8]| {
                tls.write_all(b)
                    .map_err(|e| format!("the upload's body: {e}"))
            };
            w(req.as_bytes())?;
            w(u.head().as_bytes())?;
            let mut f =
                std::fs::File::open(file).map_err(|e| format!("{}: {e}", file.display()))?;
            let mut buf = vec![0u8; BLOCK];
            loop {
                let n = f.read(&mut buf).map_err(|e| e.to_string())?;
                if n == 0 {
                    break;
                }
                w(&buf[..n])?;
            }
            w(Upload::tail().as_bytes())?;
            Ok(false)
        })
    }

    /// Send a body of `announced` file bytes written by `write`, which
    /// says whether it cut the body short.
    fn streamed(
        &self,
        path: &str,
        announced: u64,
        u: &Upload,
        write: impl FnOnce(
            &mut StreamOwned<ClientConnection, std::net::TcpStream>,
            &str,
        ) -> Result<bool, String>,
    ) -> Result<Option<Answer>, String> {
        let length = u.head().len() as u64 + announced + Upload::tail().len() as u64;
        let mut req = format!(
            "POST {path} HTTP/1.1\r\nHost: {}\r\nConnection: close\r\nContent-Type: multipart/form-data; boundary={BOUNDARY}\r\nContent-Length: {length}\r\n",
            self.host
        );
        if let Some(c) = &self.cookie {
            let _ = write!(req, "Cookie: QSMCSID={c}\r\n");
        }
        req.push_str("\r\n");
        let sock = Self::connect(self.https)?;
        // A paced upload waits on the host: give each write its time. A
        // body cut short after its first parts waits for a refusal, which
        // comes when the server's read times out (30 s, item 40).
        let long = Some(Duration::from_secs(300));
        let wait = if u.head_only {
            Some(Duration::from_secs(75))
        } else {
            long
        };
        sock.set_write_timeout(long).map_err(|e| e.to_string())?;
        sock.set_read_timeout(wait).map_err(|e| e.to_string())?;
        let name = ServerName::try_from(GUEST).map_err(|e| e.to_string())?;
        let conn = ClientConnection::new(self.tls.clone(), name).map_err(|e| e.to_string())?;
        let mut tls = StreamOwned::new(conn, sock);
        let sent = write(&mut tls, &req);
        if let Ok(true) = sent {
            let _ = tls.flush();
            tls.conn.send_close_notify();
            let _ = tls.flush();
            let _ = tls.sock.shutdown(std::net::Shutdown::Both);
            return Ok(None);
        }
        // A console that refuses part way answers before it closes.
        let _ = tls.flush();
        let mut back = Vec::new();
        let _ = tls.read_to_end(&mut back);
        if back.is_empty() {
            return Err(sent
                .err()
                .unwrap_or_else(|| "the console closed the upload without an answer".into()));
        }
        parse(&back).map(Some)
    }
}

/// Write the request `req` and `u`'s body: whether it was cut.
fn body(
    tls: &mut StreamOwned<ClientConnection, std::net::TcpStream>,
    req: &str,
    u: &Upload,
) -> Result<bool, String> {
    let mut w = |b: &[u8]| {
        tls.write_all(b)
            .map_err(|e| format!("the upload's body: {e}"))
    };
    w(req.as_bytes())?;
    w(u.head().as_bytes())?;
    if u.head_only {
        return Ok(false);
    }
    let descriptor = u.descriptor();
    let stop = u.cut.unwrap_or(u.bytes).min(u.bytes);
    let mut buf = vec![0u8; BLOCK];
    let mut at = 0u64;
    while at < stop {
        let n = usize::try_from((stop - at).min(BLOCK as u64)).unwrap_or(BLOCK);
        fill(&mut buf[..n], at, &descriptor);
        w(&buf[..n])?;
        at += n as u64;
    }
    if u.cut.is_some() {
        return Ok(true);
    }
    w(Upload::tail().as_bytes())?;
    Ok(false)
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_descriptor_lands_at_its_place_across_blocks() {
        let u = Upload {
            csrf: "t".into(),
            name: "A.iso".into(),
            bytes: 1 << 20,
            announce: None,
            cut: None,
            head_only: false,
            label: "QSMC_TEST".into(),
        };
        let d = u.descriptor();
        let mut a = vec![0u8; 1000];
        fill(&mut a, PVD - 10, &d);
        assert_eq!(&a[10..16], &[1, b'C', b'D', b'0', b'0', b'1']);
        let mut b = vec![0u8; 100];
        fill(&mut b, PVD + 40, &d);
        assert_eq!(&b[..9], b"QSMC_TEST");
        assert!(u.head().ends_with("\r\n\r\n") && Upload::tail().contains("name=\"upload\""));
    }
}
