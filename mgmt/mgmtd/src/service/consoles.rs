// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! The console socket (`release::MGMTD_CONSOLES`): a terminal window's
//! connection, as `mgmtproto::console` says. Each is two threads: one
//! sends the console's pieces to the window, one takes the window's.

use std::io::{Read as _, Write as _};
use std::os::unix::net::{UnixListener, UnixStream};
use std::sync::atomic::Ordering;
use std::sync::mpsc::channel;
use std::thread;
use std::time::Duration;

use mgmtproto::Refusal;
use mgmtproto::console::{Watch, Watching};
use qsi::host::console::{In, Out, Pieces};

use super::Service;
use crate::system::{Cmd, Seen};
use crate::words::qmc;

/// How long a window may take to say what it watches.
const ASKING: Duration = Duration::from_secs(10);
/// How long the host may take to answer an attach.
const ATTACH: Duration = Duration::from_secs(30);

/// Serve terminal windows on `listener` for ever.
pub fn serve(listener: &UnixListener, svc: &Service) {
    for stream in listener.incoming().flatten() {
        let svc = svc.clone();
        thread::spawn(move || window(&svc, stream));
    }
}

fn answer(s: &mut UnixStream, a: &Result<Watching, Refusal>) -> bool {
    ipc::frame::write_frame(s, &ipc::encode(a)).is_ok()
}

fn window(svc: &Service, mut s: UnixStream) {
    let _ = s.set_read_timeout(Some(ASKING));
    let Ok(w) = ipc::frame::read_frame_max(&mut s, 4096).and_then(|b| ipc::decode::<Watch>(&b))
    else {
        return;
    };
    let inner = &svc.0;
    let Some(entry) = inner.entry(&w.system) else {
        answer(&mut s, &Err(qmc(&qmcmsg::NO_SUCH_SYSTEM, &[&w.system])));
        return;
    };
    let open = inner.windows.fetch_add(1, Ordering::Relaxed);
    if open >= inner.limits.max_consoles {
        inner.windows.fetch_sub(1, Ordering::Relaxed);
        let n = inner.limits.max_consoles.to_string();
        answer(&mut s, &Err(qmc(&qmcmsg::TOO_MANY_WINDOWS, &[&n])));
        return;
    }
    let wid = svc.window();
    let (tx, rx) = channel();
    let _ = entry.tx.send(Cmd::Watch {
        id: w.id,
        wid,
        take_over: w.take_over,
        typing: w.typing,
        user: w.user.clone(),
        tx,
    });
    let start = match rx.recv_timeout(ATTACH) {
        Ok(Seen::Start(a)) => a,
        _ => Err(qmc(&qmcmsg::NO_REPLY, &[&entry.name])),
    };
    let watching = start.is_ok();
    if answer(&mut s, &start) && watching {
        inner.log(&format!(
            "console of {} partition {} opened for user {}",
            entry.name, w.id, w.user
        ));
        let _ = s.set_read_timeout(None);
        if let Ok(mut from) = s.try_clone() {
            let to = entry.tx.clone();
            let id = w.id;
            thread::spawn(move || {
                let mut pieces = Pieces::default();
                let mut buf = [0u8; 4096];
                while let Ok(n) = from.read(&mut buf) {
                    if n == 0 {
                        break;
                    }
                    pieces.push(&buf[..n]);
                    while let Some(p) = pieces.next_piece() {
                        if let Ok(input) = In::decode(&p) {
                            let _ = to.send(Cmd::Input { id, wid, input });
                        }
                    }
                }
                // The window went: let it go now, not at the next screen.
                let _ = to.send(Cmd::Unwatch { id, wid });
                let _ = from.shutdown(std::net::Shutdown::Both);
            });
        }
        while let Ok(seen) = rx.recv() {
            let Seen::Out(out) = seen else {
                continue;
            };
            let ended = matches!(out, Out::Ended(_));
            let ok = out.piece().is_ok_and(|p| s.write_all(&p).is_ok());
            if ended || !ok {
                break;
            }
        }
    }
    let _ = entry.tx.send(Cmd::Unwatch { id: w.id, wid });
    let _ = s.shutdown(std::net::Shutdown::Both);
    inner.windows.fetch_sub(1, Ordering::Relaxed);
}
