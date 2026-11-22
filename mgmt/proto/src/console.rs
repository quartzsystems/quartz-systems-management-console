// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! A terminal window's connection, or an image's upload, on
//! `release::MGMTD_CONSOLES`. The web server opens it with one [`Open`]
//! as an `ipc` frame.
//!
//! A window's ([`Watch`]) is answered `Result<Watching, Refusal>` the
//! same way; after that the connection
//! carries QSI's own console pieces (`qsi::host::console`): `Out` from
//! `mgmtd` (the latest whole screen first, then its updates, and
//! `Ended` with why), `In` from the web server (keys, a refresh). The
//! host has one viewer of a partition's console, `mgmtd`, which shares
//! it among every window that has it open.

use ipc::wire_struct;

/// What a window asks to watch.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Watch {
    /// The managed system.
    pub system: String,
    /// The partition.
    pub id: u32,
    /// The signed-on user, for the managed system's log.
    pub user: String,
    /// Take the console from another console that views it.
    pub take_over: bool,
    /// Whether the user may type; a key from a window that may not is
    /// dropped.
    pub typing: bool,
}
wire_struct!(Watch {
    system,
    id,
    user,
    take_over,
    typing
});

/// What a window watches.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Watching {
    /// The name the screen's status row shows: the partition's system,
    /// as its channel says it, or the partition's own name.
    pub system: String,
    /// Whether the console is a serial port, with no frames.
    pub serial: bool,
}
wire_struct!(Watching { system, serial });

/// An image's upload to a host's media library.
///
/// Answered `Result<(), Refusal>`: the host took the request and its
/// stream is open. Then the web server sends the image as `ipc` frames
/// of at most [`CHUNK`] bytes as it reads them from the browser, and an
/// empty frame when it has sent [`Upload::bytes`]; `mgmtd` reads the
/// next frame only when the host's stream had credit for the last, so
/// the browser is paced by the host. Last, `mgmtd` answers
/// `Result<(), Refusal>` with how the host ended it. A connection that
/// closes before the empty frame cuts the upload, and the host keeps
/// nothing of it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Upload {
    /// The host.
    pub system: String,
    /// The image's name.
    pub name: String,
    /// Its length.
    pub bytes: u64,
    /// The signed-on user.
    pub user: String,
}
wire_struct!(Upload {
    system,
    name,
    bytes,
    user
});

/// The most an upload's frame carries.
pub const CHUNK: usize = 256 * 1024;

/// A dump's download from the system that holds it.
///
/// Answered `Result<u64, Refusal>`: the system opened its stream of that
/// many bytes. Then `mgmtd` sends them as `ipc` frames of at most
/// [`CHUNK`] bytes as the system sends them, and an empty frame after
/// the last. The system's stream is given credit only as the web server
/// takes the frames, so nothing is held whole; a connection the web
/// server closes ends the system's stream.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Fetch {
    /// The managed system.
    pub system: String,
    /// The partition whose dump it is; 0 for the system's own.
    pub id: u32,
    /// The dump's name.
    pub name: String,
    /// The signed-on user.
    pub user: String,
}
wire_struct!(Fetch {
    system,
    id,
    name,
    user
});

/// What a connection is for: one of the three.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Open {
    /// A terminal window.
    pub watch: Option<Watch>,
    /// An upload.
    pub upload: Option<Upload>,
    /// A dump's download.
    pub fetch: Option<Fetch>,
}
wire_struct!(Open {
    watch,
    upload,
    fetch
});

/// A dump's bytes as the web server reads them from its connection: the
/// frames that follow the answer, until the empty one.
#[derive(Debug)]
pub struct Frames<S: std::io::Read> {
    stream: S,
    buf: Vec<u8>,
    at: usize,
    done: bool,
}

impl<S: std::io::Read> Frames<S> {
    /// The frames that follow the answer on `stream`.
    pub fn new(stream: S) -> Self {
        Self {
            stream,
            buf: Vec::new(),
            at: 0,
            done: false,
        }
    }
}

impl<S: std::io::Read> std::io::Read for Frames<S> {
    fn read(&mut self, out: &mut [u8]) -> std::io::Result<usize> {
        while self.at == self.buf.len() {
            if self.done {
                return Ok(0);
            }
            self.buf = ipc::frame::read_frame_max(&mut self.stream, CHUNK)
                .map_err(|e| std::io::Error::other(e.to_string()))?;
            self.at = 0;
            if self.buf.is_empty() {
                self.done = true;
            }
        }
        let n = out.len().min(self.buf.len() - self.at);
        out[..n].copy_from_slice(&self.buf[self.at..self.at + n]);
        self.at += n;
        Ok(n)
    }
}
