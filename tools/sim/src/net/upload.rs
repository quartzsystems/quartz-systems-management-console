// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! An image's upload on one connection: a stream from the console of
//! exactly the bytes its request said. Nothing of it is kept but its
//! count, a running SHA-256 and the few bytes of its primary volume
//! descriptor; credit is given back as the bytes are taken, at the pace
//! the system is told (`upload-rate`), so the sender is paced. Its end
//! is the `UploadEnded` event: the image in the library, or `QSI0701`
//! for more bytes than said, `QSI0702` for fewer or a stream cut off,
//! `QVS3206` for what is not ISO 9660.

use std::time::Instant;

use qsi::host::model::Note;
use qsi::host::vres::{self, Uploading};
use qsisess::{Session, StreamId};
use sha2::{Digest as _, Sha256};

use crate::system::vres::media::upload_ended;
use crate::system::{Shared, lock};

/// Where ISO 9660's primary volume descriptor is.
const PVD: u64 = 16 * 2048;
/// The bytes of it kept: its type, `CD001`, version, system and volume
/// identifiers.
const PVD_KEPT: u64 = 72;

/// One upload under way.
#[derive(Debug)]
struct Upload {
    stream: StreamId,
    name: String,
    bytes: u64,
    got: u64,
    /// Bytes taken in and not yet granted back.
    owed: u64,
    /// Bytes the pace allows granting now.
    allowance: f64,
    last: Instant,
    sha: Sha256,
    pvd: Vec<u8>,
}

impl Upload {
    /// Keep what of `bytes`, at the upload's place, is the descriptor.
    fn keep(&mut self, bytes: &[u8]) {
        let start = self.got;
        let end = start + bytes.len() as u64;
        let (lo, hi) = (PVD.max(start), (PVD + PVD_KEPT).min(end));
        if lo < hi {
            let a = usize::try_from(lo - start).unwrap_or(0);
            let b = usize::try_from(hi - start).unwrap_or(0);
            self.pvd.extend_from_slice(&bytes[a..b]);
        }
    }

    /// The volume identifier, when the descriptor is ISO 9660's.
    fn label(&self) -> Option<String> {
        let p = &self.pvd;
        (p.len() as u64 >= PVD_KEPT && p[0] == 1 && &p[1..6] == b"CD001")
            .then(|| String::from_utf8_lossy(&p[40..72]).trim().to_string())
    }
}

/// The uploads this connection takes.
#[derive(Debug, Default)]
pub struct Uploads {
    open: Vec<Upload>,
}

fn note(id: &str, text: String) -> Note {
    Note {
        msgid: id.into(),
        text,
        help: String::new(),
    }
}

impl Uploads {
    /// Open the stream of request `req`: the upload of image `name`,
    /// `bytes` long.
    pub fn open(
        &mut self,
        sys: &Shared,
        sess: &mut Session,
        req: u32,
        name: &str,
        bytes: u64,
    ) -> Result<(), qsisess::Error> {
        let body = ipc::encode(&Uploading {
            name: name.into(),
            bytes,
        });
        let stream = sess.reply_stream(req, body)?;
        let mut s = lock(sys);
        s.upload.busy.push(name.into());
        s.upload.name = name.into();
        s.upload.arrived = 0;
        s.upload.sha256.clear();
        s.upload.end.clear();
        s.note(format!("upload of {name} begun, {bytes} bytes"));
        self.open.push(Upload {
            stream,
            name: name.into(),
            bytes,
            got: 0,
            owed: 0,
            allowance: 0.0,
            last: Instant::now(),
            sha: Sha256::new(),
            pvd: Vec::new(),
        });
        Ok(())
    }

    /// Whether `stream` is an upload's.
    pub fn owns(&self, stream: StreamId) -> bool {
        self.open.iter().any(|u| u.stream == stream)
    }

    /// Data came on `stream`: counted, hashed, and owed back as credit.
    pub fn data(&mut self, sys: &Shared, sess: &mut Session, stream: StreamId, bytes: &[u8]) {
        let Some(i) = self.open.iter().position(|u| u.stream == stream) else {
            return;
        };
        let u = &mut self.open[i];
        u.keep(bytes);
        u.got += bytes.len() as u64;
        u.owed += bytes.len() as u64;
        u.sha.update(bytes);
        lock(sys).upload.arrived = u.got;
        if u.got > u.bytes {
            let u = self.open.remove(i);
            let text = format!(
                "The upload of {} carried more than the {} bytes its request said.",
                u.name, u.bytes
            );
            let _ = sess.reset_stream(stream, vres::msgid::UPLOAD_LONG, &text);
            finish(sys, u, Some(note(vres::msgid::UPLOAD_LONG, text)));
        }
    }

    /// `stream` ended from the console's side: `closed` cleanly, or
    /// reset.
    pub fn ended(&mut self, sys: &Shared, stream: StreamId, closed: bool) {
        let Some(i) = self.open.iter().position(|u| u.stream == stream) else {
            return;
        };
        let u = self.open.remove(i);
        let error = if closed && u.got == u.bytes {
            if u.label().is_none() {
                Some(note(
                    "QVS3206",
                    format!("{} is not an ISO 9660 image.", u.name),
                ))
            } else {
                None
            }
        } else {
            Some(note(
                vres::msgid::UPLOAD_CUT,
                format!(
                    "The upload of {} ended after {} of its {} bytes.",
                    u.name, u.got, u.bytes
                ),
            ))
        };
        finish(sys, u, error);
    }

    /// Grant back what was taken, as the pace allows.
    pub fn pump(&mut self, sys: &Shared, sess: &mut Session) {
        if self.open.is_empty() {
            return;
        }
        let (rate, stalled) = {
            let s = lock(sys);
            (s.upload.rate, s.upload.stalled)
        };
        for u in &mut self.open {
            let now = Instant::now();
            let dt = now.duration_since(u.last).as_secs_f64();
            u.last = now;
            if stalled || u.owed == 0 {
                u.allowance = 0.0;
                continue;
            }
            let n = if rate == 0 {
                u.owed
            } else {
                #[allow(clippy::cast_precision_loss, reason = "a pace, not a count")]
                let more = dt * rate as f64;
                u.allowance = (u.allowance + more).min(64.0 * 1024.0);
                #[allow(
                    clippy::cast_possible_truncation,
                    clippy::cast_sign_loss,
                    reason = "a whole, positive allowance"
                )]
                let whole = u.allowance as u64;
                whole.min(u.owed)
            };
            if n == 0 {
                continue;
            }
            #[allow(clippy::cast_precision_loss, reason = "a pace, not a count")]
            {
                u.allowance = (u.allowance - n as f64).max(0.0);
            }
            let n32 = u32::try_from(n).unwrap_or(u32::MAX);
            if sess.grant(u.stream, n32).is_ok() {
                u.owed -= u64::from(n32);
            }
        }
    }

    /// The connection ended: every upload under way is cut off.
    pub fn release(&mut self, sys: &Shared) {
        for u in std::mem::take(&mut self.open) {
            let text = format!(
                "The upload of {} ended after {} of its {} bytes: the connection closed.",
                u.name, u.got, u.bytes
            );
            finish(sys, u, Some(note(vres::msgid::UPLOAD_CUT, text)));
        }
    }
}

/// An upload's end, on the system: in the library or not, and said.
fn finish(sys: &Shared, u: Upload, error: Option<Note>) {
    let label = u.label().unwrap_or_default();
    let sha = crate::system::hex(&u.sha.finalize());
    let mut s = lock(sys);
    s.upload.busy.retain(|n| n != &u.name);
    s.upload.arrived = u.got;
    s.upload.sha256 = sha;
    s.upload.end = error
        .as_ref()
        .map_or_else(|| "ok".into(), |e| format!("{} {}", e.msgid, e.text));
    let end = s.upload.end.clone();
    s.note(format!("upload of {} ended: {end}", u.name));
    upload_ended(&mut s, &u.name, u.bytes, &label, error);
}

#[cfg(test)]
mod tests {
    use qsi::any::{AnyEvent, AnyRequest};
    use qsi::host::vres::{self, ImageUpload};
    use qsisess::loopback::Loopback;
    use qsisess::{Incoming, Limits};

    use super::*;

    /// An ISO 9660 image of `blocks` 2048-byte blocks, volume `label`.
    fn iso(blocks: usize, label: &str) -> Vec<u8> {
        let mut v = vec![0x5a; blocks * 2048];
        let pvd = 16 * 2048;
        v[pvd] = 1;
        v[pvd + 1..pvd + 6].copy_from_slice(b"CD001");
        v[pvd + 6] = 1;
        let mut id = [b' '; 32];
        id[..label.len()].copy_from_slice(label.as_bytes());
        v[pvd + 40..pvd + 72].copy_from_slice(&id);
        v
    }

    /// What reached the managed side, handed to the sink.
    fn feed(l: &mut Loopback, sys: &Shared, ups: &mut Uploads) {
        for i in std::mem::take(&mut l.at_managed) {
            match i {
                Incoming::Data { stream, bytes } => ups.data(sys, &mut l.managed, stream, &bytes),
                Incoming::StreamClosed { stream } => ups.ended(sys, stream, true),
                Incoming::StreamReset { stream, .. } => ups.ended(sys, stream, false),
                _ => {}
            }
        }
    }

    /// Begin an upload of `name` said to be `bytes` long: its stream.
    fn begin(l: &mut Loopback, sys: &Shared, ups: &mut Uploads, name: &str, bytes: u64) -> u32 {
        let req = AnyRequest::Vres(vres::Request::ImageUpload(ImageUpload {
            name: name.into(),
            bytes,
        }));
        let id = l.console.request_any("qsadmin", &req).unwrap();
        l.pump();
        l.take_request(id).unwrap();
        ups.open(sys, &mut l.managed, id, name, bytes).unwrap();
        l.pump();
        l.take_reply(id).unwrap().stream.unwrap()
    }

    /// Send `data` on `stream` as the credit allows, `rounds` times at
    /// most: how much went.
    fn send(
        l: &mut Loopback,
        sys: &Shared,
        ups: &mut Uploads,
        stream: u32,
        data: &[u8],
        rounds: usize,
    ) -> usize {
        let mut sent = 0;
        for _ in 0..rounds {
            if sent == data.len() {
                break;
            }
            sent += l.console.send(stream, &data[sent..]).unwrap_or(0);
            l.pump();
            feed(l, sys, ups);
            ups.pump(sys, &mut l.managed);
            l.pump();
        }
        sent
    }

    fn ended(sys: &Shared) -> Vec<vres::UploadEnded> {
        let s = lock(sys);
        s.log
            .from(0)
            .filter_map(|e| match e.decode().ok().flatten() {
                Some(AnyEvent::Vres(vres::Event::Upload(u))) => Some(u),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn an_upload_of_the_bytes_said_is_an_image() {
        let sys = crate::fixture::qshqvs1();
        let (mut l, mut ups) = (Loopback::new(true), Uploads::default());
        l.piece = 64 * 1024;
        let data = iso(200, "QS400_TEST");
        let stream = begin(&mut l, &sys, &mut ups, "TEST.iso", data.len() as u64);
        assert_eq!(send(&mut l, &sys, &mut ups, stream, &data, 100), data.len());
        l.console.close_stream(stream).unwrap();
        l.pump();
        feed(&mut l, &sys, &mut ups);
        let e = ended(&sys);
        let image = e[0].image.clone().unwrap();
        assert_eq!(
            (image.bytes, image.label.as_str(), e[0].error.clone()),
            (409_600, "QS400_TEST", None)
        );
        let s = lock(&sys);
        assert!(s.vres.images.iter().any(|i| i.name == "TEST.iso"));
        assert_eq!(s.upload.arrived, 409_600);
        assert_eq!(s.upload.sha256, crate::system::hex(&Sha256::digest(&data)));
        assert!(s.upload.busy.is_empty() && s.upload.end == "ok");
    }

    #[test]
    fn more_bytes_than_said_or_fewer_leave_no_image() {
        let sys = crate::fixture::qshqvs1();
        let (mut l, mut ups) = (Loopback::new(true), Uploads::default());
        l.piece = 64 * 1024;
        let data = iso(20, "LONG");
        let stream = begin(&mut l, &sys, &mut ups, "LONG.iso", 8192);
        send(&mut l, &sys, &mut ups, stream, &data, 3);
        let stream = begin(&mut l, &sys, &mut ups, "CUT.iso", data.len() as u64);
        send(&mut l, &sys, &mut ups, stream, &data[..10_000], 3);
        l.console.close_stream(stream).unwrap();
        l.pump();
        feed(&mut l, &sys, &mut ups);
        let stream = begin(&mut l, &sys, &mut ups, "GONE.iso", data.len() as u64);
        send(&mut l, &sys, &mut ups, stream, &data[..4096], 3);
        ups.release(&sys);
        let stream = begin(&mut l, &sys, &mut ups, "FLAT.iso", 4096);
        send(&mut l, &sys, &mut ups, stream, &[0; 4096], 3);
        l.console.close_stream(stream).unwrap();
        l.pump();
        feed(&mut l, &sys, &mut ups);
        let ids: Vec<_> = ended(&sys)
            .iter()
            .map(|u| {
                (
                    u.name.clone(),
                    u.image.is_some(),
                    u.error.clone().map(|e| e.msgid),
                )
            })
            .collect();
        assert_eq!(
            ids,
            [
                ("LONG.iso".into(), false, Some("QSI0701".into())),
                ("CUT.iso".into(), false, Some("QSI0702".into())),
                ("GONE.iso".into(), false, Some("QSI0702".into())),
                ("FLAT.iso".into(), false, Some("QVS3206".into())),
            ]
        );
        let s = lock(&sys);
        for gone in ["LONG.iso", "CUT.iso", "GONE.iso", "FLAT.iso"] {
            assert!(!s.vres.images.iter().any(|i| i.name == gone), "{gone}");
        }
        assert!(s.upload.busy.is_empty());
    }

    #[test]
    fn a_stalled_sink_holds_the_sender_to_its_window() {
        let sys = crate::fixture::qshqvs1();
        lock(&sys).upload.stalled = true;
        let (mut l, mut ups) = (Loopback::new(true), Uploads::default());
        l.piece = 64 * 1024;
        let data = iso(400, "SLOW");
        let stream = begin(&mut l, &sys, &mut ups, "SLOW.iso", data.len() as u64);
        let window = Limits::default().window as usize;
        assert_eq!(send(&mut l, &sys, &mut ups, stream, &data, 50), window);
        assert_eq!(lock(&sys).upload.arrived, window as u64);
        assert!(ended(&sys).is_empty());
        // Paced at a rate: credit comes back a little at a time.
        lock(&sys).upload.stalled = false;
        lock(&sys).upload.rate = 1 << 30;
        let rest = send(&mut l, &sys, &mut ups, stream, &data[window..], 200);
        assert_eq!(window + rest, data.len());
        l.console.close_stream(stream).unwrap();
        l.pump();
        feed(&mut l, &sys, &mut ups);
        assert!(ended(&sys)[0].image.is_some());
    }
}
