// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! One managed system: a thread that keeps its session open (resumed
//! quietly after a drop, with back-off), what is kept about it, and the
//! commands the service's sockets send it.

mod apply;
pub mod console;
pub mod fuzzing;
mod resources;
mod upload;
mod worker;

use std::sync::atomic::AtomicBool;
use std::sync::mpsc::{Sender, channel};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::thread;

use mgmtproto::console::Watching;
use mgmtproto::{Kept, Refusal};
use qsi::any::AnyRequest;
use qsi::host::console::{In, Out};
use qsisess::Reply;

use crate::service::Inner;

/// What a window watching a console is sent.
#[derive(Debug)]
pub enum Seen {
    /// Whether it watches.
    Start(Result<Watching, Refusal>),
    /// The console's next piece.
    Out(Out),
}

/// What a system's thread is asked.
#[derive(Debug)]
#[allow(clippy::large_enum_variant, reason = "sent once and taken apart")]
pub enum Cmd {
    /// A request, for `user`; its reply or refusal.
    Ask {
        /// The request.
        req: AnyRequest,
        /// The signed-on user, for the managed system's log.
        user: String,
        /// Where the answer goes.
        tx: Sender<Result<Reply, Refusal>>,
    },
    /// Watch partition `id`'s console as window `wid`.
    Watch {
        /// The partition.
        id: u32,
        /// The window.
        wid: u64,
        /// Take it over from another console.
        take_over: bool,
        /// Whether its keys are sent.
        typing: bool,
        /// The signed-on user.
        user: String,
        /// Where the console goes.
        tx: Sender<Seen>,
    },
    /// A window's key or refresh.
    Input {
        /// The partition.
        id: u32,
        /// The window.
        wid: u64,
        /// What it sent.
        input: In,
    },
    /// Upload an image of `bytes` named `name` for `user`: its frames
    /// on `chunks`, the stream opened on `started`, how it ended on
    /// `ended`.
    Upload {
        /// The image's name.
        name: String,
        /// Its length.
        bytes: u64,
        /// The signed-on user.
        user: String,
        /// The image, a frame at a time.
        chunks: std::sync::mpsc::Receiver<Vec<u8>>,
        /// The host opened its stream, or refused.
        started: Sender<Result<(), Refusal>>,
        /// The host's word on it.
        ended: Sender<Result<(), Refusal>>,
    },
    /// A window closed.
    Unwatch {
        /// The partition.
        id: u32,
        /// The window.
        wid: u64,
    },
    /// Try to connect now.
    Wake,
}

/// A system's handle: what is kept and the way to its thread.
#[derive(Clone, Debug)]
pub struct Entry {
    /// Its name.
    pub name: String,
    /// What is kept about it.
    pub kept: Arc<Mutex<Kept>>,
    /// Its thread's commands.
    pub tx: Sender<Cmd>,
    /// Set to end its thread.
    pub stop: Arc<AtomicBool>,
}

impl Entry {
    /// What is kept, locked.
    pub fn kept(&self) -> MutexGuard<'_, Kept> {
        self.kept.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// Start the thread of the system `kept` describes.
pub fn start(inner: &Arc<Inner>, kept: Kept) -> Entry {
    let (tx, rx) = channel();
    let entry = Entry {
        name: kept.name.clone(),
        kept: Arc::new(Mutex::new(kept)),
        tx,
        stop: Arc::new(AtomicBool::new(false)),
    };
    let (inner, e) = (inner.clone(), entry.clone());
    let _ = thread::Builder::new()
        .name(format!("system {}", entry.name))
        .spawn(move || worker::Worker::new(inner, e, rx).run());
    entry
}
