// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! A partition's console on one connection: one viewer at a time, as a
//! QS/VS host allows (`QVS2314` to a second without take over); the
//! recorded screen first, then a key typed is echoed at the cursor and
//! kept for the tests.

use qsi::host::console::{self, In, Out, Pieces};
use qsipart::console::{Cell, Frame, Key, Run, Update, attr};
use qsisess::{Session, StreamId};

use crate::system::{Shared, lock};

/// One console attached on this connection.
#[derive(Debug)]
struct Attached {
    stream: StreamId,
    id: u32,
    pieces: Pieces,
    out: Vec<u8>,
    seq: u32,
    ended: bool,
}

/// The consoles this connection views.
#[derive(Debug, Default)]
pub struct Consoles {
    open: Vec<Attached>,
}

fn queue(a: &mut Attached, o: &Out) {
    if let Ok(p) = o.piece() {
        a.out.extend_from_slice(&p);
    }
}

impl Consoles {
    /// Attach to partition `id` for request `req` of connection `conn`.
    pub fn attach(
        &mut self,
        sys: &Shared,
        sess: &mut Session,
        req: u32,
        conn: u64,
        id: u32,
        take_over: bool,
    ) -> Result<(), qsisess::Error> {
        let mut s = lock(sys);
        let frame = s.screen_of(id);
        let Some(p) = s.part(id) else {
            return sess.refuse(req, "QVS2001", "The partition does not exist.");
        };
        if let Some(other) = p.viewer
            && other != conn
            && !take_over
        {
            let text = format!(
                "The console of partition {} is in use by another console.",
                p.status.name
            );
            return sess.refuse(req, "QVS2314", &text);
        }
        if p.viewer.is_some_and(|v| v != conn) {
            p.taken_by = Some(conn);
        }
        p.viewer = Some(conn);
        let body = ipc::encode(&console::Attached { serial: false });
        let stream = sess.reply_stream(req, body)?;
        let mut a = Attached {
            stream,
            id,
            pieces: Pieces::default(),
            out: Vec::new(),
            seq: 1,
            ended: false,
        };
        if let Some(f) = frame {
            a.seq = f.seq;
            queue(&mut a, &Out::Frame(f));
        }
        self.open.push(a);
        Ok(())
    }

    /// Data came on `stream`: the console's pieces.
    pub fn data(
        &mut self,
        sys: &Shared,
        sess: &mut Session,
        stream: StreamId,
        bytes: &[u8],
        no_credit: bool,
    ) {
        let Some(a) = self.open.iter_mut().find(|a| a.stream == stream) else {
            return;
        };
        a.pieces.push(bytes);
        if !no_credit {
            let _ = sess.grant(stream, u32::try_from(bytes.len()).unwrap_or(u32::MAX));
        }
        while let Some(piece) = a.pieces.next_piece() {
            match In::decode(&piece) {
                Ok(In::Key(k)) => {
                    let update = type_key(sys, a.id, k, a.seq + 1);
                    if let Some(u) = update {
                        a.seq = u.seq;
                        queue(a, &Out::Update(u));
                    }
                }
                Ok(In::Refresh) => {
                    if let Some(f) = lock(sys).screen_of(a.id) {
                        a.seq = f.seq;
                        queue(a, &Out::Frame(f));
                    }
                }
                _ => {}
            }
        }
    }

    /// `stream` ended from the console's side.
    pub fn ended(&mut self, sys: &Shared, stream: StreamId) {
        if let Some(i) = self.open.iter().position(|a| a.stream == stream) {
            let a = self.open.remove(i);
            let mut s = lock(sys);
            if let Some(p) = s.part(a.id)
                && p.viewer.is_some()
                && p.taken_by.is_none()
            {
                p.viewer = None;
            }
        }
    }

    /// Send what waits, as the credit allows; end a console another
    /// connection took over.
    pub fn pump(&mut self, sys: &Shared, sess: &mut Session, conn: u64) {
        for a in &mut self.open {
            if !a.ended {
                let mut s = lock(sys);
                if let Some(p) = s.part(a.id)
                    && p.viewer != Some(conn)
                {
                    p.taken_by = None;
                    a.ended = true;
                    drop(s);
                    queue(a, &Out::Ended("taken over by another console".into()));
                }
            }
            if !a.out.is_empty() {
                let n = sess.send(a.stream, &a.out).unwrap_or(0);
                a.out.drain(..n);
            }
            if a.ended && a.out.is_empty() {
                let _ = sess.close_stream(a.stream);
            }
        }
        self.open.retain(|a| !(a.ended && a.out.is_empty()));
    }

    /// The connection ended: let go of what it viewed.
    pub fn release(&mut self, sys: &Shared, conn: u64) {
        let mut s = lock(sys);
        for p in &mut s.parts {
            if p.viewer == Some(conn) {
                p.viewer = None;
            }
        }
        self.open.clear();
    }
}

/// `key` at partition `id`'s console: kept, and a character put at the
/// cursor; the update that says so.
fn type_key(sys: &Shared, id: u32, key: Key, seq: u32) -> Option<Update> {
    let mut sys = lock(sys);
    let part = sys.part(id)?;
    part.keys.push(key);
    let frame: &mut Frame = &mut part.screen.as_mut()?.frame;
    let (row, col) = (frame.status.row, frame.status.col);
    let cols = usize::from(frame.cols);
    frame.seq = seq;
    let mut runs = Vec::new();
    if let Key::Char(ch) = key {
        let cell = Cell {
            ch,
            attr: attr::GREEN,
        };
        if let Some(slot) = frame
            .cells
            .get_mut(usize::from(row) * cols + usize::from(col))
        {
            *slot = cell;
        }
        if usize::from(col) + 1 < cols {
            frame.status.col = col + 1;
        }
        runs.push(Run {
            row,
            col,
            cells: vec![cell],
        });
    }
    Some(Update {
        seq,
        runs,
        status: frame.status.clone(),
    })
}
