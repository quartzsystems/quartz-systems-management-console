// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! The terminal window's WebSocket: QS/400's exchange (`termlook`'s
//! `screen`, `delta`, `closed`, `message` out; keys by name in) between
//! the page's script and the console socket of the managed-systems
//! service. Keys go only from a user whose role may type. It ends with
//! the web session unless terminal windows are kept open, or after the
//! session's idle time without a key.

use std::io::{Read as _, Write as _};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::channel;
use std::time::{Duration, Instant};

use mgmtproto::console::Watch;
use qsi::host::console::{In, Out, Pieces};
use qsipart::console::Key;
use qsweb::ws::{Message, Socket};
use termlook::exchange::{self, Input};
use termlook::look::render::Scheme;
use termlook::screen::grid::View;

use super::relay::{Shown, connect};

/// What one socket relays.
pub struct Relay {
    /// The console socket.
    pub path: std::path::PathBuf,
    /// What it watches.
    pub watch: Watch,
    /// Black on white.
    pub bw: bool,
    /// Set when the web session ended and windows are not kept.
    pub ended: Arc<AtomicBool>,
    /// How long without a key before it ends; none when windows are kept.
    pub idle: Option<Duration>,
}

fn view(s: &Shown, bw: bool) -> View {
    View {
        grid: s.grid.clone(),
        system: s.system.clone(),
        connected: s.ended.is_none(),
        tls: true,
        scheme: if bw {
            Scheme::BlackOnWhite
        } else {
            Scheme::Console
        },
    }
}

/// The console keys `input` is. The console has no key that places the
/// cursor: a click on the screen (`Cursor`, 1-based) is the arrow keys
/// from where the latest frame has it.
fn keys_of(input: &Input, shown: &Shown) -> Vec<Key> {
    let Input::Cursor(r, c) = *input else {
        return input.console_keys();
    };
    let s = &shown.grid.status;
    moves(
        (usize::from(s.row), usize::from(s.col)),
        (r.saturating_sub(1), c.saturating_sub(1)),
    )
}

/// The arrow keys from `from` to `to` (row, column, 0-based).
fn moves(from: (usize, usize), to: (usize, usize)) -> Vec<Key> {
    let mut out = Vec::new();
    let (v, n) = if to.0 < from.0 {
        (Key::Up, from.0 - to.0)
    } else {
        (Key::Down, to.0 - from.0)
    };
    out.extend(std::iter::repeat_n(v, n));
    let (h, n) = if to.1 < from.1 {
        (Key::Left, from.1 - to.1)
    } else {
        (Key::Right, to.1 - from.1)
    };
    out.extend(std::iter::repeat_n(h, n));
    out
}

/// Relay until either end goes.
pub fn run(r: &Relay, ws: &mut Socket<'_>) {
    let (stream, watching) = match connect(&r.path, &r.watch) {
        Ok(c) => c,
        Err(no) => {
            let _ = ws.send(&exchange::message(&no.main.msgid, &no.main.text));
            let _ = ws.send(&exchange::closed(&no.main.text));
            ws.close();
            return;
        }
    };
    let Ok(mut reader) = stream.try_clone() else {
        return;
    };
    let mut writer = stream;
    let (tx, rx) = channel::<Out>();
    std::thread::spawn(move || {
        let mut pieces = Pieces::default();
        let mut buf = [0u8; 16 * 1024];
        while let Ok(n) = reader.read(&mut buf) {
            if n == 0 {
                break;
            }
            pieces.push(&buf[..n]);
            while let Some(p) = pieces.next_piece() {
                if let Ok(out) = Out::decode(&p)
                    && tx.send(out).is_err()
                {
                    return;
                }
            }
        }
    });
    let mut shown = Shown {
        system: watching.system,
        ..Shown::default()
    };
    let mut prev = None;
    let mut last_key = Instant::now();
    loop {
        if r.ended.load(Ordering::Relaxed) {
            let _ = ws.send(&exchange::closed("the web session ended"));
            break;
        }
        if r.idle.is_some_and(|d| last_key.elapsed() > d) {
            let _ = ws.send(&exchange::closed("no key for the session's idle time"));
            break;
        }
        let mut changed = false;
        loop {
            match rx.try_recv() {
                Ok(out) => {
                    shown.apply(&out);
                    changed = true;
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => break,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    if shown.ended.is_none() {
                        shown.ended = Some("the managed-systems service closed the console".into());
                    }
                    break;
                }
            }
        }
        if changed && shown.grid.ready() {
            let now = view(&shown, r.bw).shown();
            let state = if shown.ended.is_some() { "off" } else { "on" };
            if ws
                .send(&exchange::screen(&now, prev.as_ref(), shown.version, state))
                .is_err()
            {
                break;
            }
            prev = Some(now);
        }
        if let Some(why) = &shown.ended {
            let _ = ws.send(&exchange::closed(why));
            break;
        }
        match ws.recv(Duration::from_millis(30)) {
            Ok(Some(Message::Text(t))) => {
                if let Some(input) = Input::parse(&t) {
                    last_key = Instant::now();
                    if r.watch.typing {
                        for k in keys_of(&input, &shown) {
                            if In::Key(k)
                                .piece()
                                .is_ok_and(|p| writer.write_all(&p).is_err())
                            {
                                break;
                            }
                        }
                    }
                }
            }
            Ok(_) => {}
            Err(_) => break,
        }
    }
    let _ = writer.shutdown(std::net::Shutdown::Both);
    ws.close();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_click_is_the_arrow_keys_to_its_cell() {
        assert_eq!(
            moves((5, 10), (3, 12)),
            [Key::Up, Key::Up, Key::Right, Key::Right]
        );
        assert_eq!(moves((0, 4), (1, 2)), [Key::Down, Key::Left, Key::Left]);
        assert!(moves((7, 7), (7, 7)).is_empty());
        let mut shown = Shown::default();
        shown.grid.status.row = 5;
        shown.grid.status.col = 52;
        assert_eq!(
            keys_of(&Input::Cursor(6, 55), &shown),
            [Key::Right, Key::Right]
        );
        assert_eq!(keys_of(&Input::Key("Enter".into()), &shown), [Key::Enter]);
    }
}
