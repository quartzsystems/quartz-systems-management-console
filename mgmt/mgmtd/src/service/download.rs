// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! A dump's download on the console socket (`mgmtproto::console`): the
//! system asked by its thread, its pieces written to the web server as
//! they come, and the credit given back only as the web server takes
//! them, so the console holds no more than the stream's window. A web
//! server that goes ends the system's stream.

use std::os::unix::net::UnixStream;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use mgmtproto::Refusal;
use mgmtproto::console::{CHUNK, Fetch};
use qsi::caps;

use super::Service;
use crate::system::{Cmd, Piece};
use crate::words::qmc;

/// How long the system may take to open the stream.
const START: Duration = Duration::from_secs(60);
/// How long the system may send nothing.
const BETWEEN: Duration = Duration::from_secs(120);

fn answer(s: &mut UnixStream, a: &Result<u64, Refusal>) -> bool {
    ipc::frame::write_frame(s, &ipc::encode(a)).is_ok()
}

/// Carry dump `f` from its system to the web server on `s`.
pub fn download(svc: &Service, mut s: UnixStream, f: &Fetch) {
    let inner = &svc.0;
    let Some(entry) = inner.entry(&f.system) else {
        answer(&mut s, &Err(qmc(&qmcmsg::NO_SUCH_SYSTEM, &[&f.system])));
        return;
    };
    let need = if f.id == 0 {
        caps::SYSTEM_DUMPS
    } else {
        caps::DUMPS
    };
    if let Some(what) = mgmtproto::ask::lacks(entry.kept().caps, need) {
        answer(&mut s, &Err(qmc(&qmcmsg::LACKS, &[&entry.name, what])));
        return;
    }
    let (to, from) = std::sync::mpsc::channel();
    let taken = Arc::new(AtomicU64::new(0));
    let _ = entry.tx.send(Cmd::Download {
        id: f.id,
        name: f.name.clone(),
        user: f.user.clone(),
        to,
        taken: taken.clone(),
    });
    let start = match from.recv_timeout(START) {
        Ok(Piece::Start(r)) => r,
        _ => Err(qmc(&qmcmsg::NO_REPLY, &[&entry.name])),
    };
    if !answer(&mut s, &start) {
        return;
    }
    let Ok(total) = start else {
        return;
    };
    inner.log(&format!(
        "download of {} from {} for user {} begun: {total} bytes",
        f.name, entry.name, f.user
    ));
    let mut sent = 0u64;
    let how = loop {
        match from.recv_timeout(BETWEEN) {
            Ok(Piece::Bytes(b)) => {
                // The web server gone: the system's stream ends when its
                // next piece finds no one to take it.
                if b.chunks(CHUNK)
                    .any(|piece| ipc::frame::write_frame_max(&mut s, piece, CHUNK).is_err())
                {
                    break "the web server went".to_string();
                }
                sent += b.len() as u64;
                taken.fetch_add(b.len() as u64, Ordering::Relaxed);
            }
            Ok(Piece::End) => {
                let _ = ipc::frame::write_frame(&mut s, &[]);
                break "whole".to_string();
            }
            Ok(Piece::Cut(why)) => break format!("cut: {why}"),
            Ok(Piece::Start(_)) => {}
            Err(_) => break "the system sent nothing for too long".to_string(),
        }
    };
    drop(from);
    inner.log(&format!(
        "download of {} from {}: {sent} of {total} bytes, {how}",
        f.name, entry.name
    ));
}
