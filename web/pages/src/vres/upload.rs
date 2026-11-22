// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! An image uploaded from Virtual Optical, streamed: the browser's body
//! is read as it arrives and passed to `mgmtd` a frame at a time, which
//! passes it to the host as the host's stream takes it. Nothing of the
//! image is held whole, in memory or on the console's disk.
//!
//! QSI's upload says the image's length first. The form is the design's
//! with its Read-only box shown and not sent and its URL field and
//! Fetch a form of their own (`docs/WEB.md`), so after the file comes
//! only the Upload button's part: the length is the request's less what
//! came before the file and that tail. A body that does not end so is
//! cut, and the host keeps nothing of it.

use std::io::Read;
use std::time::Duration;

use qsweb::http::Response;
use qsweb::msg::{Kind, Message};

use mgmtproto::console::{CHUNK, Open, Upload};

use crate::app::routes::Rq;

/// The most an image may be.
pub const LIMIT: u64 = 64 << 30;
/// The most the parts before the file may take.
const HEAD_MAX: usize = 16 * 1024;

/// Whether `path` is a host's Virtual Optical: `<host>`.
pub fn host_of(path: &str) -> Option<&str> {
    let rest = path.strip_prefix(webd::product::PREFIX)?;
    let rest = rest.strip_prefix("/server/")?;
    let (host, tab) = rest.split_once('/')?;
    (tab == "virtual/optical" && !host.is_empty()).then_some(host)
}

/// Whether `req` is an upload: a multipart post to Virtual Optical.
pub fn is_upload(req: &qsweb::http::Request) -> bool {
    req.method == "POST"
        && host_of(&req.path).is_some()
        && req
            .header("content-type")
            .is_some_and(|c| c.starts_with("multipart/form-data"))
}

/// What came before the file: the form's token, the file's name, and
/// the image's first bytes read with them.
struct Head {
    csrf: String,
    filename: String,
    taken: usize,
    rest: Vec<u8>,
}

/// One part's head and value from `buf` at `at`: `(name, filename,
/// value end, data start)`; `None` until all of it is in `buf`.
fn field(buf: &[u8], at: usize, delim: &[u8]) -> Option<(String, Option<String>, usize, usize)> {
    let head_end = find(buf, b"\r\n\r\n", at)?;
    let head = std::str::from_utf8(&buf[at..head_end]).ok()?;
    let disp = head
        .lines()
        .find(|l| l.to_ascii_lowercase().starts_with("content-disposition:"))?;
    let attr = |key: &str| {
        let k = format!("{key}=\"");
        disp.find(&k).and_then(|i| {
            let v = &disp[i + k.len()..];
            v.find('"').map(|j| v[..j].to_string())
        })
    };
    let name = attr("name")?;
    let filename = attr("filename");
    let data = head_end + 4;
    if filename.is_some() {
        return Some((name, filename, data, data));
    }
    let end = find(buf, delim, data)?;
    Some((name, None, end, data))
}

fn find(hay: &[u8], needle: &[u8], from: usize) -> Option<usize> {
    if from > hay.len() || needle.is_empty() {
        return None;
    }
    hay[from..]
        .windows(needle.len())
        .position(|w| w == needle)
        .map(|p| p + from)
}

/// Read the parts before the file from `body`.
fn head(body: &mut dyn Read, boundary: &str) -> Result<Head, String> {
    let delim = format!("\r\n--{boundary}").into_bytes();
    let mut buf = Vec::new();
    let mut chunk = [0u8; 4096];
    let mut csrf = String::new();
    // The first delimiter has no line break before it.
    let first = format!("--{boundary}\r\n").into_bytes();
    loop {
        if buf.len() >= first.len() {
            if !buf.starts_with(&first) {
                return Err("the body is not the form's".into());
            }
            let mut at = first.len();
            while let Some((name, filename, end, data)) = field(&buf, at, &delim) {
                if let Some(f) = filename {
                    return Ok(Head {
                        csrf,
                        filename: f,
                        taken: data,
                        rest: buf[data..].to_vec(),
                    });
                }
                if name == qsweb::session::CSRF_FIELD {
                    csrf = String::from_utf8_lossy(&buf[data..end]).into_owned();
                }
                // Past the value, its delimiter and line break.
                at = end + delim.len() + 2;
            }
        }
        if buf.len() > HEAD_MAX {
            return Err("the form's fields before the file are too long".into());
        }
        let n = body.read(&mut chunk).map_err(|e| e.to_string())?;
        if n == 0 {
            return Err("the body ended before the file".into());
        }
        buf.extend_from_slice(&chunk[..n]);
    }
}

/// The page's answer: back to Virtual Optical with `ms`.
fn back(rq: &Rq<'_>, host: &str, ms: Vec<Message>) -> Response {
    rq.done_all(&format!("/server/{host}/virtual/optical"), ms)
}

fn error(def: &qmcmsg::Def, data: &[&str]) -> Vec<Message> {
    vec![qmcmsg::message(Kind::Error, def, data)]
}

/// `POST /server/<host>/virtual/optical`, multipart: the upload.
pub fn post(rq: &Rq<'_>, body: &mut dyn Read) -> Response {
    let host = host_of(&rq.req.path).unwrap_or_default().to_string();
    let ct = rq.req.header("content-type").unwrap_or("");
    let Some(boundary) = qsweb::http::multipart::boundary(ct) else {
        return back(
            rq,
            &host,
            error(&qmcmsg::UPLOAD_REFUSED, &["the form is not the console's"]),
        );
    };
    let total: u64 = rq
        .req
        .header("content-length")
        .and_then(|l| l.trim().parse().ok())
        .unwrap_or(0);
    let h = match head(body, &boundary) {
        Ok(h) => h,
        Err(why) => return back(rq, &host, error(&qmcmsg::UPLOAD_REFUSED, &[&why])),
    };
    if !qsweb::session::csrf_ok(&rq.session, &h.csrf) {
        rq.audit(&format!("POST {}", rq.req.path), "refused: no form token");
        return crate::app::routes::error(rq, 403, &qmcmsg::FORM_EXPIRED, &[]);
    }
    if let Some(no) = crate::app::routes::lacks(rq, webusers::Task::VirtualResources) {
        return no;
    }
    let name = h
        .filename
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or("")
        .to_string();
    if name.is_empty() {
        return back(
            rq,
            &host,
            error(&qmcmsg::NOT_VALID, &["Choose an image to upload."]),
        );
    }
    let tail = format!(
        "\r\n--{boundary}\r\nContent-Disposition: form-data; name=\"upload\"\r\n\r\nUpload\r\n--{boundary}--\r\n"
    );
    let Some(bytes) = total
        .checked_sub(h.taken as u64)
        .and_then(|n| n.checked_sub(tail.len() as u64))
    else {
        return back(
            rq,
            &host,
            error(
                &qmcmsg::UPLOAD_REFUSED,
                &["the form's length is not its own"],
            ),
        );
    };
    let what = format!("upload image {name} ({bytes} bytes) to {host}");
    // Refused before any of the image is read.
    if bytes > LIMIT {
        rq.audit(&what, "refused: past the limit");
        let limit = format!("{}", LIMIT >> 30);
        return back(
            rq,
            &host,
            error(&qmcmsg::UPLOAD_TOO_LARGE, &[&name, &limit]),
        );
    }
    let Some(socket) = rq.app.console.consoles() else {
        return back(rq, &host, error(&qmcmsg::MGMT_DOWN, &["no socket"]));
    };
    let r = send(rq, &socket, &host, &name, bytes, &boundary, h.rest, body);
    rq.audit(&what, if r.is_ok() { "uploaded" } else { "refused" });
    match r {
        Ok(()) => back(
            rq,
            &host,
            vec![qmcmsg::message(
                Kind::Done,
                &qmcmsg::IMAGE_UPLOADED,
                &[&name, &host],
            )],
        ),
        Err(mut ms) => {
            ms.insert(
                0,
                qmcmsg::message(Kind::Error, &qmcmsg::UPLOAD_NOT_TAKEN, &[&name, &host]),
            );
            back(rq, &host, ms)
        }
    }
}

/// Pass `bytes` of the image, `first` already read and the rest from
/// `body`, to `mgmtd` on `socket`; then check the body ends with the
/// form's tail before the upload is called whole.
#[allow(clippy::too_many_arguments, reason = "one upload's parts")]
fn send(
    rq: &Rq<'_>,
    socket: &std::path::Path,
    host: &str,
    name: &str,
    bytes: u64,
    boundary: &str,
    first: Vec<u8>,
    body: &mut dyn Read,
) -> Result<(), Vec<Message>> {
    use std::os::unix::net::UnixStream;
    let down = |e: &dyn std::fmt::Display| error(&qmcmsg::MGMT_DOWN, &[&e.to_string()]);
    let mut s = UnixStream::connect(socket).map_err(|e| down(&e))?;
    let _ = s.set_read_timeout(Some(Duration::from_mins(11)));
    let open = Open {
        watch: None,
        upload: Some(Upload {
            system: host.into(),
            name: name.into(),
            bytes,
            user: rq.user().into(),
        }),
        fetch: None,
    };
    ipc::frame::write_frame(&mut s, &ipc::encode(&open)).map_err(|e| down(&e))?;
    let answer = |s: &mut UnixStream| -> Result<(), Vec<Message>> {
        let b = ipc::frame::read_frame_max(s, 64 * 1024).map_err(|e| down(&e))?;
        let r: Result<(), mgmtproto::Refusal> = ipc::decode(&b).map_err(|e| down(&e))?;
        r.map_err(|no| crate::systems::refused(&no))
    };
    answer(&mut s)?;
    let delim = format!("\r\n--{boundary}").into_bytes();
    let mut pending = first;
    let mut sent = 0u64;
    let mut chunk = vec![0u8; CHUNK];
    let cut = |why: &str| error(&qmcmsg::UPLOAD_REFUSED, &[why]);
    while sent < bytes {
        let left = usize::try_from(bytes - sent).unwrap_or(usize::MAX);
        // A delimiter in the image's bytes: the form was not the one
        // whose length was taken.
        if find(&pending[..pending.len().min(left)], &delim, 0).is_some() {
            return Err(cut("the form had more fields than the console's"));
        }
        // Keep what could be a delimiter's start until more is read,
        // unless the image ends there.
        let safe = if pending.len() >= left {
            left
        } else {
            pending.len().saturating_sub(delim.len() - 1)
        };
        if safe > 0 {
            let n = safe.min(CHUNK);
            ipc::frame::write_frame(&mut s, &pending[..n]).map_err(|e| down(&e))?;
            pending.drain(..n);
            sent += n as u64;
            continue;
        }
        let n = body.read(&mut chunk).map_err(|e| cut(&e.to_string()))?;
        if n == 0 {
            return Err(cut("the browser stopped before the image's end"));
        }
        pending.extend_from_slice(&chunk[..n]);
    }
    // The tail: the next bytes must be the form's own.
    while pending.len() < delim.len() {
        let n = body.read(&mut chunk).map_err(|e| cut(&e.to_string()))?;
        if n == 0 {
            break;
        }
        pending.extend_from_slice(&chunk[..n]);
    }
    if !pending.starts_with(&delim) {
        return Err(cut("the image was longer than the form said"));
    }
    std::io::copy(body, &mut std::io::sink()).map_err(|e| cut(&e.to_string()))?;
    ipc::frame::write_frame(&mut s, &[]).map_err(|e| down(&e))?;
    answer(&mut s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_parts_before_the_file_are_read_and_counted() {
        let body = b"--B\r\nContent-Disposition: form-data; name=\"csrf\"\r\n\r\ntok\r\n--B\r\nContent-Disposition: form-data; name=\"iso\"; filename=\"C:\\\\x\\\\A.ISO\"\r\nContent-Type: application/octet-stream\r\n\r\nDATA";
        let h = head(&mut &body[..], "B").unwrap();
        assert_eq!(h.csrf, "tok");
        assert_eq!(h.rest, b"DATA");
        assert_eq!(h.taken, body.len() - 4);
        assert!(h.filename.ends_with("A.ISO"));
        assert_eq!(
            host_of("/qsmc/server/QSHQVS1/virtual/optical"),
            Some("QSHQVS1")
        );
        assert_eq!(host_of("/qsmc/server/QSHQVS1/virtual/storage"), None);
    }
}
