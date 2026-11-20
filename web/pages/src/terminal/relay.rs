// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! A terminal window's connection to the managed-systems service's
//! console socket (`mgmtproto::console`), and the windows the web server
//! holds for browsers without script: the latest whole screen kept, keys
//! sent as they are posted. A window ends with its web session unless
//! the session option keeps terminal windows open
//! (`web.toml`'s `keep_terminals`).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Sender, channel};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use mgmtproto::Refusal;
use mgmtproto::console::{Watch, Watching};
use qsi::host::console::{In, Out, Pieces};
use qsipart::console::{FromSystem, Grid};

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

/// What a window shows: the grid as last sent, the name its status row
/// shows, and why it ended once it has.
#[derive(Clone, Debug, Default)]
pub struct Shown {
    /// The grid.
    pub grid: Grid,
    /// The name on the status row.
    pub system: String,
    /// Why the console went.
    pub ended: Option<String>,
    /// Bumped at each change.
    pub version: u64,
}

impl Shown {
    /// Apply what the console sent.
    pub fn apply(&mut self, out: &Out) {
        match out {
            Out::Frame(f) => {
                let _ = self.grid.apply(&FromSystem::Frame(f.clone()));
            }
            Out::Update(u) => {
                let _ = self.grid.apply(&FromSystem::Update(u.clone()));
            }
            Out::Ended(why) => self.ended = Some(why.clone()),
            _ => {}
        }
        self.version += 1;
    }
}

/// A connection to the console socket, watching `w`: the stream and
/// what is watched, or the refusal.
#[cfg(unix)]
pub fn connect(
    socket: &std::path::Path,
    w: &Watch,
) -> Result<(std::os::unix::net::UnixStream, Watching), Refusal> {
    use std::time::Duration;
    let mut s = std::os::unix::net::UnixStream::connect(socket)
        .map_err(|e| crate::systems::own(&qmcmsg::MGMT_DOWN, &[&e.to_string()]))?;
    let _ = s.set_read_timeout(Some(Duration::from_secs(40)));
    let sent = ipc::frame::write_frame(&mut s, &ipc::encode(w));
    let answer: Result<Result<Watching, Refusal>, ipc::WireError> = sent
        .and_then(|()| ipc::frame::read_frame_max(&mut s, 64 * 1024))
        .and_then(|b| ipc::decode(&b));
    let _ = s.set_read_timeout(None);
    match answer {
        Ok(Ok(watching)) => Ok((s, watching)),
        Ok(Err(r)) => Err(r),
        Err(e) => Err(crate::systems::own(&qmcmsg::MGMT_DOWN, &[&e.to_string()])),
    }
}

/// A window the web server holds for a session.
#[derive(Debug)]
pub struct Held {
    /// The session.
    pub session: String,
    /// The managed system.
    pub host: String,
    /// The partition.
    pub id: u32,
    /// What it shows.
    pub shown: Arc<Mutex<Shown>>,
    keys: Mutex<Option<Sender<In>>>,
    closed: AtomicBool,
}

impl Held {
    /// What it shows now.
    pub fn now(&self) -> Shown {
        lock(&self.shown).clone()
    }

    /// Send `input`.
    pub fn send(&self, input: In) {
        if let Some(tx) = lock(&self.keys).as_ref() {
            let _ = tx.send(input);
        }
    }

    /// Whether it is still attached.
    pub fn open(&self) -> bool {
        !self.closed.load(Ordering::Relaxed) && lock(&self.shown).ended.is_none()
    }

    fn close(&self) {
        self.closed.store(true, Ordering::Relaxed);
        *lock(&self.keys) = None;
    }
}

/// The windows held, and the sockets' flags.
#[derive(Debug, Default)]
pub struct Windows {
    held: Mutex<Vec<Arc<Held>>>,
    sockets: Mutex<Vec<(String, Arc<AtomicBool>)>>,
}

impl Windows {
    /// The window `session` holds on `host`'s partition `id`, when open.
    pub fn find(&self, session: &str, host: &str, id: u32) -> Option<Arc<Held>> {
        lock(&self.held)
            .iter()
            .find(|h| h.session == session && h.host == host && h.id == id && h.open())
            .cloned()
    }

    /// Open (or find) the window `session` holds on `w`.
    #[cfg(unix)]
    pub fn open(
        &self,
        socket: &std::path::Path,
        session: &str,
        w: &Watch,
    ) -> Result<Arc<Held>, Refusal> {
        use std::io::{Read as _, Write as _};
        if let Some(h) = self.find(session, &w.system, w.id) {
            return Ok(h);
        }
        let (stream, watching) = connect(socket, w)?;
        let shown = Arc::new(Mutex::new(Shown {
            system: watching.system,
            ..Shown::default()
        }));
        let (tx, rx) = channel::<In>();
        let held = Arc::new(Held {
            session: session.into(),
            host: w.system.clone(),
            id: w.id,
            shown: Arc::clone(&shown),
            keys: Mutex::new(Some(tx)),
            closed: AtomicBool::new(false),
        });
        let mut reader = stream
            .try_clone()
            .map_err(|e| crate::systems::own(&qmcmsg::MGMT_DOWN, &[&e.to_string()]))?;
        let mut writer = stream;
        let s2 = Arc::clone(&shown);
        std::thread::spawn(move || {
            let mut pieces = Pieces::default();
            let mut buf = [0u8; 16 * 1024];
            loop {
                match reader.read(&mut buf) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => pieces.push(&buf[..n]),
                }
                while let Some(p) = pieces.next_piece() {
                    if let Ok(out) = Out::decode(&p) {
                        lock(&s2).apply(&out);
                    }
                }
            }
            let mut s = lock(&s2);
            if s.ended.is_none() {
                s.ended = Some("the managed-systems service closed the console".into());
            }
        });
        std::thread::spawn(move || {
            for input in rx {
                if input.piece().is_ok_and(|p| writer.write_all(&p).is_err()) {
                    break;
                }
            }
            let _ = writer.shutdown(std::net::Shutdown::Both);
        });
        let mut all = lock(&self.held);
        all.retain(|h| h.open() || h.session == session);
        all.push(Arc::clone(&held));
        Ok(held)
    }

    /// Why the window `session` held on `host`'s partition `id` ended,
    /// once: the window is forgotten.
    pub fn ended(&self, session: &str, host: &str, id: u32) -> Option<String> {
        let mut all = lock(&self.held);
        let i = all
            .iter()
            .position(|h| h.session == session && h.host == host && h.id == id && !h.open())?;
        let h = all.remove(i);
        let why = lock(&h.shown).ended.clone();
        Some(why.unwrap_or_else(|| "the window was disconnected".into()))
    }

    /// Close the window `session` holds on `host`'s partition `id`.
    pub fn close(&self, session: &str, host: &str, id: u32) {
        let mut all = lock(&self.held);
        for h in all
            .iter()
            .filter(|h| h.session == session && h.host == host && h.id == id)
        {
            h.close();
        }
        all.retain(|h| h.open());
    }

    /// A socket of `session` opened: the flag that ends it.
    pub fn socket(&self, session: &str) -> Arc<AtomicBool> {
        let flag = Arc::new(AtomicBool::new(false));
        let mut all = lock(&self.sockets);
        all.retain(|(_, f)| !f.load(Ordering::Relaxed) && Arc::strong_count(f) > 1);
        all.push((session.into(), Arc::clone(&flag)));
        flag
    }

    /// `session` ended: its windows and sockets end with it, unless
    /// windows are kept open.
    pub fn session_ended(&self, session: &str, keep: bool) {
        if keep {
            return;
        }
        let mut held = lock(&self.held);
        for h in held.iter().filter(|h| h.session == session) {
            h.close();
        }
        held.retain(|h| h.open());
        for (s, f) in lock(&self.sockets).iter() {
            if s == session {
                f.store(true, Ordering::Relaxed);
            }
        }
    }

    /// The sessions with a window or socket open.
    pub fn sessions(&self) -> Vec<String> {
        let mut v: Vec<String> = lock(&self.held)
            .iter()
            .filter(|h| h.open())
            .map(|h| h.session.clone())
            .collect();
        v.extend(
            lock(&self.sockets)
                .iter()
                .filter(|(_, f)| !f.load(Ordering::Relaxed))
                .map(|(s, _)| s.clone()),
        );
        v.sort();
        v.dedup();
        v
    }
}
