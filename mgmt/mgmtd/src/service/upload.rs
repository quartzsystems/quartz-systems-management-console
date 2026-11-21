// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! An image's upload on the console socket (`mgmtproto::console`): the
//! host asked by the system's thread, the web server's frames passed to
//! it one at a time, so the web server reads the browser only as fast
//! as the host's stream takes the bytes, and how the host ended it.
//! Nothing of the image is kept here beyond the frame in hand.

use std::os::unix::net::UnixStream;
use std::sync::mpsc::{channel, sync_channel};
use std::time::Duration;

use mgmtproto::Refusal;
use mgmtproto::console::{CHUNK, Upload};

use super::Service;
use crate::system::Cmd;
use crate::words::qmc;

/// How long the host may take to open the upload's stream.
const START: Duration = Duration::from_secs(60);
/// How long the web server may send nothing.
const BETWEEN: Duration = Duration::from_secs(120);
/// How long the host may take to say how it ended, after the last byte.
const END: Duration = Duration::from_secs(600);

fn answer(s: &mut UnixStream, a: &Result<(), Refusal>) -> bool {
    ipc::frame::write_frame(s, &ipc::encode(a)).is_ok()
}

/// Carry upload `u` from the web server on `s` to its host.
pub fn upload(svc: &Service, mut s: UnixStream, u: &Upload) {
    let inner = &svc.0;
    let Some(entry) = inner.entry(&u.system) else {
        answer(&mut s, &Err(qmc(&qmcmsg::NO_SUCH_SYSTEM, &[&u.system])));
        return;
    };
    let need = qsi::caps::VRES;
    if let Some(what) = mgmtproto::ask::lacks(entry.kept().caps, need) {
        answer(&mut s, &Err(qmc(&qmcmsg::LACKS, &[&entry.name, what])));
        return;
    }
    // One frame waits at most: the next is read when the host took it.
    let (chunks, from) = sync_channel::<Vec<u8>>(1);
    let (started_tx, started) = channel();
    let (ended_tx, ended) = channel();
    let _ = entry.tx.send(Cmd::Upload {
        name: u.name.clone(),
        bytes: u.bytes,
        user: u.user.clone(),
        chunks: from,
        started: started_tx,
        ended: ended_tx,
    });
    let start = started
        .recv_timeout(START)
        .unwrap_or_else(|_| Err(qmc(&qmcmsg::NO_REPLY, &[&entry.name])));
    if !answer(&mut s, &start) || start.is_err() {
        return;
    }
    inner.log(&format!(
        "upload of {} to {} for user {} begun: {} bytes",
        u.name, entry.name, u.user, u.bytes
    ));
    let _ = s.set_read_timeout(Some(BETWEEN));
    let mut got = 0u64;
    let whole = loop {
        match ipc::frame::read_frame_max(&mut s, CHUNK) {
            Ok(b) if b.is_empty() => break true,
            Ok(b) => {
                got += b.len() as u64;
                if got > u.bytes || chunks.send(b).is_err() {
                    break false;
                }
            }
            Err(_) => break false,
        }
    };
    // The thread sees the channel end: closed when all of it went, cut
    // otherwise.
    drop(chunks);
    if !whole {
        inner.log(&format!(
            "upload of {} to {} cut after {got} of {} bytes",
            u.name, entry.name, u.bytes
        ));
        return;
    }
    let end = ended
        .recv_timeout(END)
        .unwrap_or_else(|_| Err(qmc(&qmcmsg::NO_REPLY, &[&entry.name])));
    inner.log(&format!(
        "upload of {} to {}: {}",
        u.name,
        entry.name,
        match &end {
            Ok(()) => "taken".to_string(),
            Err(r) => format!("refused, {} {}", r.main.msgid, r.main.text),
        }
    ));
    answer(&mut s, &end);
}
