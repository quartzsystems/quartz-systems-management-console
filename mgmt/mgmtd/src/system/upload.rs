// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! Uploads on a system's session: each the host's stream opened by its
//! `ImageUpload`, fed from the web server's frames as the stream's
//! credit allows, closed when all of it went and reset when it was cut
//! (the host keeps nothing then), answered when the host's `UploadEnded`
//! says how it ended.

use std::sync::mpsc::{Receiver, Sender, TryRecvError};

use mgmtproto::Refusal;
use qsisess::{Session, StreamId};

use crate::words::qmc;

/// One upload.
struct Up {
    name: String,
    total: u64,
    sent: u64,
    stream: Option<StreamId>,
    chunks: Receiver<Vec<u8>>,
    buf: Vec<u8>,
    started: Option<Sender<Result<(), Refusal>>>,
    ended: Option<Sender<Result<(), Refusal>>>,
    /// The stream was closed or reset: only the host's word waits.
    finished: bool,
}

/// The most uploads to one host at once.
pub const MAX_UPLOADS: usize = 4;

/// The uploads of one session.
#[derive(Default)]
pub struct Uploads(Vec<Up>);

impl Uploads {
    /// The uploads under way.
    #[allow(clippy::len_without_is_empty, reason = "only the count is asked")]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// An upload asked of the host: waiting for its stream.
    pub fn add(
        &mut self,
        name: String,
        total: u64,
        chunks: Receiver<Vec<u8>>,
        started: Sender<Result<(), Refusal>>,
        ended: Sender<Result<(), Refusal>>,
    ) {
        self.0.push(Up {
            name,
            total,
            sent: 0,
            stream: None,
            chunks,
            buf: Vec::new(),
            started: Some(started),
            ended: Some(ended),
            finished: false,
        });
    }

    /// The host answered upload `name`: its stream, or its refusal.
    pub fn opened(&mut self, name: &str, answer: Result<StreamId, Refusal>) {
        let Some(i) = self
            .0
            .iter()
            .position(|u| u.name == name && u.stream.is_none())
        else {
            return;
        };
        match answer {
            Ok(s) => {
                let u = &mut self.0[i];
                u.stream = Some(s);
                if let Some(tx) = u.started.take() {
                    let _ = tx.send(Ok(()));
                }
            }
            Err(no) => {
                let u = self.0.remove(i);
                if let Some(tx) = u.started {
                    let _ = tx.send(Err(no));
                }
            }
        }
    }

    /// Send what waits as the credit allows; close or reset each whose
    /// web server is done.
    pub fn pump(&mut self, sess: &mut Session) {
        for u in &mut self.0 {
            let Some(s) = u.stream.filter(|_| !u.finished) else {
                continue;
            };
            loop {
                if u.buf.is_empty() {
                    match u.chunks.try_recv() {
                        Ok(c) => u.buf = c,
                        Err(TryRecvError::Empty) => break,
                        Err(TryRecvError::Disconnected) => {
                            u.finished = true;
                            if u.sent == u.total {
                                let _ = sess.close_stream(s);
                            } else {
                                let _ = sess.reset_stream(
                                    s,
                                    qsi::host::vres::msgid::UPLOAD_CUT,
                                    "the upload was cut at the console",
                                );
                            }
                            break;
                        }
                    }
                }
                let n = sess.send(s, &u.buf).unwrap_or(0);
                u.sent += n as u64;
                u.buf.drain(..n);
                if !u.buf.is_empty() {
                    break;
                }
            }
        }
    }

    /// The host said how upload `name` ended.
    pub fn ended(&mut self, name: &str, error: Option<Refusal>) {
        let Some(i) = self
            .0
            .iter()
            .position(|u| u.name == name && u.stream.is_some())
        else {
            return;
        };
        let u = self.0.remove(i);
        if let Some(tx) = u.ended {
            let _ = tx.send(error.map_or(Ok(()), Err));
        }
    }

    /// The session ended: every upload is refused with why.
    pub fn lost(&mut self, system: &str, why: &str) {
        for u in self.0.drain(..) {
            let no = || Err(qmc(&qmcmsg::NOT_CONNECTED, &[system, why]));
            if let Some(tx) = u.started {
                let _ = tx.send(no());
            }
            if let Some(tx) = u.ended {
                let _ = tx.send(no());
            }
        }
    }
}
