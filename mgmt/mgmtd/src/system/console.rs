// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! A system's partition consoles, shared. A host allows one viewer of a
//! partition's console; `mgmtd` is that viewer, keeps the latest whole
//! screen (so a window that opens gets it at once), and sends every
//! frame and update to each window that has it open. Keys are taken from
//! any window whose user may type. Take over is between consoles, not
//! between windows.

use std::sync::mpsc::Sender;

use mgmtproto::console::Watching;
use mgmtproto::{Kept, Refusal};
use qsi::host::console::{In, Out, Pieces};
use qsipart::console::{Frame, FromSystem, Grid};
use qsisess::{Session, StreamId};

use super::Seen;
use crate::words::qmc;

/// A window with the console open.
#[derive(Debug)]
struct Window {
    wid: u64,
    typing: bool,
    tx: Sender<Seen>,
}

/// One partition's console.
#[derive(Debug)]
struct Console {
    id: u32,
    /// The stream, once attached.
    stream: Option<StreamId>,
    /// The attach request outstanding.
    asked: Option<u32>,
    grid: Grid,
    pieces: Pieces,
    windows: Vec<Window>,
    /// Pieces for the system, waiting for credit.
    out: Vec<u8>,
    watching: Watching,
}

/// Every console of one system.
#[derive(Debug, Default)]
pub struct Consoles {
    open: Vec<Console>,
}

/// The grid as a whole frame.
pub fn frame_of(g: &Grid) -> Frame {
    Frame {
        seq: g.seq,
        rows: g.rows,
        cols: g.cols,
        cells: g.cells.clone(),
        status: g.status.clone(),
    }
}

fn send(w: &Window, s: Seen) -> bool {
    w.tx.send(s).is_ok()
}

impl Consoles {
    /// Windows open, across this system's consoles.
    pub fn windows(&self) -> usize {
        self.open.iter().map(|c| c.windows.len()).sum()
    }

    /// The latest whole screen of partition `id`, when a console has one.
    pub fn screen(&self, id: u32) -> Option<Frame> {
        self.open
            .iter()
            .find(|c| c.id == id && c.grid.ready())
            .map(|c| frame_of(&c.grid))
    }

    /// Window `wid` opens partition `id`'s console: at once when the
    /// console is attached, else when the host answers the attach this
    /// asks. True when an attach is to be asked.
    pub fn watch(
        &mut self,
        kept: &Kept,
        id: u32,
        wid: u64,
        typing: bool,
        tx: Sender<Seen>,
    ) -> bool {
        let w = Window { wid, typing, tx };
        if let Some(c) = self.open.iter_mut().find(|c| c.id == id) {
            if c.stream.is_some() {
                send(&w, Seen::Start(Ok(c.watching.clone())));
                if c.grid.ready() {
                    send(&w, Seen::Out(Out::Frame(frame_of(&c.grid))));
                }
            }
            c.windows.push(w);
            return false;
        }
        let p = kept.part(id);
        let system = p
            .map(|p| {
                if p.channel.system.is_empty() {
                    p.name.clone()
                } else {
                    p.channel.system.clone()
                }
            })
            .unwrap_or_default();
        self.open.push(Console {
            id,
            stream: None,
            asked: None,
            grid: Grid::default(),
            pieces: Pieces::default(),
            windows: vec![w],
            out: Vec::new(),
            watching: Watching {
                system,
                serial: false,
            },
        });
        true
    }

    /// The attach of partition `id` was asked as request `req`.
    pub fn asked(&mut self, id: u32, req: u32) {
        if let Some(c) = self.open.iter_mut().find(|c| c.id == id) {
            c.asked = Some(req);
        }
    }

    /// The host answered the attach of partition `id`: the stream, or
    /// its refusal.
    pub fn attached(&mut self, id: u32, answer: Result<(StreamId, bool), Refusal>) {
        let Some(i) = self.open.iter().position(|c| c.id == id) else {
            return;
        };
        match answer {
            Ok((stream, serial)) => {
                let c = &mut self.open[i];
                c.stream = Some(stream);
                c.asked = None;
                c.watching.serial = serial;
                for w in &c.windows {
                    send(w, Seen::Start(Ok(c.watching.clone())));
                }
            }
            Err(r) => {
                let c = self.open.remove(i);
                for w in &c.windows {
                    send(w, Seen::Start(Err(r.clone())));
                }
            }
        }
    }

    /// Bytes came on `stream`: the console's pieces, applied to the
    /// screen and sent to every window. Credit is granted as they are
    /// taken.
    pub fn data(&mut self, sess: &mut Session, stream: StreamId, bytes: &[u8]) {
        let Some(i) = self.open.iter().position(|c| c.stream == Some(stream)) else {
            return;
        };
        let _ = sess.grant(stream, u32::try_from(bytes.len()).unwrap_or(u32::MAX));
        let c = &mut self.open[i];
        c.pieces.push(bytes);
        let mut ended = false;
        while let Some(piece) = c.pieces.next_piece() {
            let Ok(out) = Out::decode(&piece) else {
                continue;
            };
            if out.check().is_err() {
                continue;
            }
            let applied = match &out {
                Out::Frame(f) => c.grid.apply(&FromSystem::Frame(f.clone())).is_ok(),
                Out::Update(u) => c.grid.apply(&FromSystem::Update(u.clone())).is_ok(),
                Out::Ended(_) => {
                    ended = true;
                    true
                }
                _ => true,
            };
            if !applied {
                // An update that does not fit: ask for the whole screen.
                if let Ok(p) = In::Refresh.piece() {
                    c.out.extend_from_slice(&p);
                }
                continue;
            }
            c.windows.retain(|w| send(w, Seen::Out(out.clone())));
        }
        if ended {
            self.open.remove(i);
        }
    }

    /// `stream` ended from the host's side.
    pub fn ended(&mut self, stream: StreamId, why: &str) {
        if let Some(i) = self.open.iter().position(|c| c.stream == Some(stream)) {
            let c = self.open.remove(i);
            for w in &c.windows {
                send(w, Seen::Out(Out::Ended(why.into())));
            }
        }
    }

    /// Window `wid`'s input for partition `id`: a key is sent only when
    /// its user may type.
    pub fn input(&mut self, id: u32, wid: u64, input: &In) {
        let Some(c) = self.open.iter_mut().find(|c| c.id == id) else {
            return;
        };
        let may = c.windows.iter().any(|w| w.wid == wid && w.typing);
        let send_it = matches!(input, In::Refresh) || may;
        if send_it && let Ok(p) = input.piece() {
            c.out.extend_from_slice(&p);
        }
    }

    /// Window `wid` closed partition `id`'s console: with none left, the
    /// console is let go (the stream closed, which detaches).
    pub fn unwatch(&mut self, sess: &mut Session, id: u32, wid: u64) {
        let Some(i) = self.open.iter().position(|c| c.id == id) else {
            return;
        };
        self.open[i].windows.retain(|w| w.wid != wid);
        if self.open[i].windows.is_empty() {
            let c = self.open.remove(i);
            if let Some(s) = c.stream {
                let _ = sess.close_stream(s);
            }
        }
    }

    /// Send what waits for the system as its credit allows; drop the
    /// windows that went.
    pub fn pump(&mut self, sess: &mut Session) {
        for c in &mut self.open {
            if let Some(s) = c.stream
                && !c.out.is_empty()
            {
                let n = sess.send(s, &c.out).unwrap_or(0);
                c.out.drain(..n);
            }
        }
    }

    /// The session with `name` ended: every window is told why.
    pub fn lost(&mut self, name: &str, why: &str) {
        for c in self.open.drain(..) {
            for w in &c.windows {
                if c.stream.is_some() {
                    send(w, Seen::Out(Out::Ended(why.into())));
                } else {
                    send(
                        w,
                        Seen::Start(Err(qmc(&qmcmsg::NOT_CONNECTED, &[name, why]))),
                    );
                }
            }
        }
    }

    /// The attach request `req` belongs to: its partition.
    pub fn of_request(&self, req: u32) -> Option<u32> {
        self.open
            .iter()
            .find(|c| c.asked == Some(req))
            .map(|c| c.id)
    }
}
