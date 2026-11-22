// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! The streams of servicing on a connection: a staged update image
//! taken in (counted, its first bytes kept, credit given back as it
//! comes), and a dump sent out (its bytes made as the credit allows).

use qsi::any::AnyReply;
use qsisess::{Session, StreamId};

use crate::system::servicing::content;
use crate::system::{Shared, lock};

/// What is read of a dump at a time.
const PIECE: usize = 64 * 1024;

struct In {
    stream: StreamId,
    name: String,
    bytes: u64,
    got: u64,
    head: Vec<u8>,
}

struct Out {
    stream: StreamId,
    name: String,
    bytes: u64,
    sent: u64,
}

/// A connection's stages and downloads.
#[derive(Default)]
pub struct Transfers {
    stages: Vec<In>,
    downloads: Vec<Out>,
}

impl Transfers {
    /// A stage answered: its stream opened.
    pub fn stage(
        &mut self,
        sess: &mut Session,
        req: u32,
        name: &str,
        bytes: u64,
    ) -> Result<(), qsisess::Error> {
        let reply = AnyReply::Update(qsi::update::Reply::Stage(qsi::update::Staging {
            name: name.into(),
            bytes,
        }));
        let stream = sess.reply_stream(req, reply.encode())?;
        self.stages.push(In {
            stream,
            name: name.into(),
            bytes,
            got: 0,
            head: Vec::new(),
        });
        Ok(())
    }

    /// A download answered: its stream opened, `id` 0 for the system's
    /// own dump.
    pub fn download(
        &mut self,
        sess: &mut Session,
        req: u32,
        id: u32,
        name: &str,
        bytes: u64,
    ) -> Result<(), qsisess::Error> {
        let reply = if id == 0 {
            AnyReply::Base(qsi::base::Reply::DownloadDump(qsi::base::DumpDownloading {
                name: name.into(),
                bytes,
            }))
        } else {
            AnyReply::Dumps(qsi::host::dumps::Reply::Download(
                qsi::host::dumps::Downloading {
                    name: name.into(),
                    bytes,
                },
            ))
        };
        let stream = sess.reply_stream(req, reply.encode())?;
        self.downloads.push(Out {
            stream,
            name: name.into(),
            bytes,
            sent: 0,
        });
        Ok(())
    }

    /// Whether `stream` is one of these.
    pub fn owns(&self, stream: StreamId) -> bool {
        self.stages.iter().any(|s| s.stream == stream)
            || self.downloads.iter().any(|d| d.stream == stream)
    }

    /// Bytes of a stage came.
    pub fn data(&mut self, sess: &mut Session, stream: StreamId, bytes: &[u8]) {
        if let Some(s) = self.stages.iter_mut().find(|s| s.stream == stream) {
            s.got += bytes.len() as u64;
            let want = 8usize.saturating_sub(s.head.len());
            s.head.extend_from_slice(&bytes[..want.min(bytes.len())]);
            let _ = sess.grant(stream, u32::try_from(bytes.len()).unwrap_or(u32::MAX));
            if s.got > s.bytes {
                let _ = sess.reset_stream(stream, "QSI1109", "more bytes than the stage named");
            }
        }
    }

    /// A stream ended: a stage whole when closed with all its bytes.
    pub fn ended(&mut self, sys: &Shared, stream: StreamId, closed: bool) {
        if let Some(i) = self.stages.iter().position(|s| s.stream == stream) {
            let s = self.stages.remove(i);
            lock(sys).staged(&s.name, s.got, closed && s.got == s.bytes, &s.head);
        }
        // A console that went: the download ends.
        self.downloads.retain(|d| d.stream != stream);
    }

    /// Send each download what its credit allows.
    pub fn pump(&mut self, sess: &mut Session) {
        for d in &mut self.downloads {
            while d.sent < d.bytes {
                let n = usize::try_from((d.bytes - d.sent).min(PIECE as u64)).unwrap_or(PIECE);
                let piece = content(&d.name, d.sent, n);
                let took = sess.send(d.stream, &piece).unwrap_or(0);
                d.sent += took as u64;
                if took < n {
                    break;
                }
            }
            if d.sent >= d.bytes {
                let _ = sess.close_stream(d.stream);
            }
        }
        self.downloads.retain(|d| d.sent < d.bytes);
    }
}
