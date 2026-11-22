// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! The two streams of servicing on a system's session, each paced by the
//! stream's credit and nothing of either held whole:
//!
//! - a stage: an update image read from its downloaded file a piece at
//!   a time and sent to the host as the stream's credit allows, closed
//!   after the last byte (the host then says `Staged` and `Verified`);
//! - a download: a dump's bytes from the system, handed to the web
//!   server's connection a piece at a time, with credit given back only
//!   as that connection takes them; a connection that goes ends the
//!   system's stream.

use std::fs::File;
use std::io::Read as _;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::Sender;

use mgmtproto::Refusal;
use qsisess::{Session, StreamId};

use crate::words::qmc;

/// What is read of a file at a time.
const PIECE: usize = 64 * 1024;

/// One image being staged.
struct Up {
    name: String,
    file: File,
    total: u64,
    sent: u64,
    stream: Option<StreamId>,
    buf: Vec<u8>,
    /// Told when the host opened the stream, or refused.
    started: Option<Sender<Result<(), Refusal>>>,
    finished: bool,
}

/// The stages of one session.
#[derive(Default)]
pub struct Stages(Vec<Up>);

impl Stages {
    /// A stage asked of the host: waiting for its stream.
    pub fn add(
        &mut self,
        name: String,
        file: File,
        total: u64,
        started: Sender<Result<(), Refusal>>,
    ) {
        self.0.push(Up {
            name,
            file,
            total,
            sent: 0,
            stream: None,
            buf: Vec::new(),
            started: Some(started),
            finished: false,
        });
    }

    /// The host answered stage `name`: its stream, or its refusal.
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

    /// Send what the credit allows: each stage's bytes sent so far, and
    /// why one stopped.
    pub fn pump(&mut self, sess: &mut Session) -> Vec<(String, u64, Option<String>)> {
        let mut said = Vec::new();
        for u in &mut self.0 {
            let Some(s) = u.stream.filter(|_| !u.finished) else {
                continue;
            };
            let before = u.sent;
            let mut stopped = None;
            loop {
                if u.buf.is_empty() {
                    if u.sent >= u.total {
                        u.finished = true;
                        let _ = sess.close_stream(s);
                        break;
                    }
                    let mut piece = vec![0u8; PIECE];
                    match u.file.read(&mut piece) {
                        Ok(0) | Err(_) => {
                            u.finished = true;
                            let why = "the downloaded image could not be read whole";
                            let _ = sess.reset_stream(s, qsi::update::msgid::UPLOAD_CUT, why);
                            stopped = Some(why.to_string());
                            break;
                        }
                        Ok(n) => {
                            piece.truncate(n);
                            u.buf = piece;
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
            if u.sent != before || stopped.is_some() {
                said.push((u.name.clone(), u.sent, stopped));
            }
        }
        self.0.retain(|u| !u.finished);
        said
    }

    /// The session ended: a stage waiting for its stream is refused.
    pub fn lost(&mut self, system: &str, why: &str) {
        for u in self.0.drain(..) {
            if let Some(tx) = u.started {
                let _ = tx.send(Err(qmc(&qmcmsg::NOT_CONNECTED, &[system, why])));
            }
        }
    }
}

/// What a download's connection is given: a piece, or the end.
#[derive(Debug)]
pub enum Piece {
    /// The stream opened, of so many bytes; or the system refused.
    Start(Result<u64, Refusal>),
    /// The next bytes.
    Bytes(Vec<u8>),
    /// All of it came.
    End,
    /// It stopped before the end, and why.
    Cut(String),
}

/// One dump being downloaded.
struct Down {
    key: u64,
    stream: Option<StreamId>,
    to: Sender<Piece>,
    /// What the connection has taken, which the credit follows.
    taken: Arc<AtomicU64>,
    /// What the system was given credit for past its first window.
    granted: u64,
    got: u64,
}

/// The downloads of one session.
#[derive(Default)]
pub struct Downloads(Vec<Down>);

impl Downloads {
    /// A download asked of the system, numbered `key`.
    pub fn add(&mut self, key: u64, to: Sender<Piece>, taken: Arc<AtomicU64>) {
        self.0.push(Down {
            key,
            stream: None,
            to,
            taken,
            granted: 0,
            got: 0,
        });
    }

    /// The system answered download `key`: its stream and size, or its
    /// refusal.
    pub fn opened(&mut self, key: u64, answer: Result<(StreamId, u64), Refusal>) {
        let Some(i) = self.0.iter().position(|d| d.key == key) else {
            return;
        };
        match answer {
            Ok((s, bytes)) => {
                let d = &mut self.0[i];
                d.stream = Some(s);
                if d.to.send(Piece::Start(Ok(bytes))).is_err() {
                    d.stream = None;
                }
            }
            Err(no) => {
                let d = self.0.remove(i);
                let _ = d.to.send(Piece::Start(Err(no)));
            }
        }
    }

    /// Whether `stream` is a download's.
    pub fn has(&self, stream: StreamId) -> bool {
        self.0.iter().any(|d| d.stream == Some(stream))
    }

    /// Bytes of a download came: handed on, or the stream reset when its
    /// connection went.
    pub fn data(&mut self, sess: &mut Session, stream: StreamId, bytes: &[u8]) {
        let Some(i) = self.0.iter().position(|d| d.stream == Some(stream)) else {
            return;
        };
        let d = &mut self.0[i];
        d.got += bytes.len() as u64;
        if d.to.send(Piece::Bytes(bytes.to_vec())).is_err() {
            let _ = sess.reset_stream(stream, "QSI0106", "the console's download was abandoned");
            self.0.remove(i);
        }
    }

    /// Give credit back as each connection took its bytes.
    pub fn pump(&mut self, sess: &mut Session) {
        for d in &mut self.0 {
            let Some(s) = d.stream else { continue };
            let taken = d.taken.load(Ordering::Relaxed);
            if taken > d.granted {
                let n = u32::try_from(taken - d.granted).unwrap_or(u32::MAX);
                if sess.grant(s, n).is_ok() {
                    d.granted += u64::from(n);
                }
            }
        }
    }

    /// The system closed download `stream`: all of it came.
    pub fn closed(&mut self, stream: StreamId) {
        if let Some(i) = self.0.iter().position(|d| d.stream == Some(stream)) {
            let d = self.0.remove(i);
            let _ = d.to.send(Piece::End);
        }
    }

    /// The system reset download `stream`, with why.
    pub fn reset(&mut self, stream: StreamId, why: &str) {
        if let Some(i) = self.0.iter().position(|d| d.stream == Some(stream)) {
            let d = self.0.remove(i);
            let _ = d.to.send(Piece::Cut(why.into()));
        }
    }

    /// The session ended.
    pub fn lost(&mut self, system: &str, why: &str) {
        for d in self.0.drain(..) {
            let _ = d.to.send(if d.stream.is_some() {
                Piece::Cut(format!("the connection to {system} was lost: {why}"))
            } else {
                Piece::Start(Err(qmc(&qmcmsg::NOT_CONNECTED, &[system, why])))
            });
        }
    }
}
