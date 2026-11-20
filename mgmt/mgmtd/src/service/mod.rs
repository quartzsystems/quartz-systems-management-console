// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! The service: the console's QSI identity, every managed system's
//! thread, what is kept, and the answers to the web server's requests
//! (`mgmtproto::Mgmt`). A list is answered from what is kept and never
//! waits on a managed system; an action is a live request.

mod calls;
#[cfg(unix)]
pub mod consoles;

use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use mgmtproto::Kept;
use mgmtproto::kept::{Kind, Link};
use qsi::Hello;
use qsitrust::Identity;
use state::Store;

use crate::keep::files::{Limits, Paired, Systems};
use crate::keep::history::History;
use crate::peer::link::hello;
use crate::system::{self, Entry};

/// Now, Unix seconds.
pub fn now_s() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

/// How the service is started.
pub struct Config {
    /// The console's QSI identity.
    pub identity: Identity,
    /// The console's name, for its hello.
    pub name: String,
    /// Its release.
    pub release: String,
    /// Where `systems.toml` and `mgmt.toml` are; none to keep them in
    /// memory.
    pub state_dir: Option<PathBuf>,
    /// Where the reference code history is kept; none for memory.
    pub history_dir: Option<PathBuf>,
    /// A line for the log.
    pub log: Box<dyn Fn(&str) + Send + Sync>,
}

/// What every thread shares.
pub struct Inner {
    /// The console's QSI identity.
    pub identity: Identity,
    /// Its hello.
    pub me: Hello,
    /// Its limits.
    pub limits: Limits,
    store: Option<Store>,
    paired: Mutex<Systems>,
    history: Mutex<History>,
    entries: Mutex<Vec<Entry>>,
    setups: (Mutex<u32>, Condvar),
    log: Box<dyn Fn(&str) + Send + Sync>,
    windows: AtomicU32,
    next_window: AtomicU64,
}

/// A connection being set up: one of the limited number at once.
pub struct Setup<'a>(&'a Inner);

impl Drop for Setup<'_> {
    fn drop(&mut self) {
        let (m, c) = &self.0.setups;
        *lock(m) -= 1;
        c.notify_one();
    }
}

impl Inner {
    /// Wait for a place among the connections being set up.
    pub fn setup(&self) -> Setup<'_> {
        let (m, c) = &self.setups;
        let mut n = lock(m);
        while *n >= self.limits.max_setups {
            n = c
                .wait_timeout(n, Duration::from_secs(1))
                .unwrap_or_else(PoisonError::into_inner)
                .0;
        }
        *n += 1;
        Setup(self)
    }

    /// The history, locked.
    pub fn history(&self) -> MutexGuard<'_, History> {
        lock(&self.history)
    }

    /// A line for the log.
    pub fn log(&self, line: &str) {
        (self.log)(line);
    }

    /// Note the last event seen of `name`, so a new session resumes
    /// from it.
    pub fn save_seq(&self, name: &str, seq: u64) {
        let mut p = lock(&self.paired);
        let Some(s) = p.systems.iter_mut().find(|s| s.name == name) else {
            return;
        };
        if s.last_seq == seq {
            return;
        }
        s.last_seq = seq;
        let next = p.clone();
        drop(p);
        self.save(&next);
    }

    fn save(&self, s: &Systems) {
        if let Some(store) = &self.store
            && let Err(e) = store.save(s)
        {
            self.log(&format!("systems.toml not written: {e}"));
        }
    }

    fn entries(&self) -> MutexGuard<'_, Vec<Entry>> {
        lock(&self.entries)
    }

    fn entry(&self, name: &str) -> Option<Entry> {
        self.entries()
            .iter()
            .find(|e| e.name.eq_ignore_ascii_case(name))
            .cloned()
    }
}

/// What is kept of a system just paired or read back from its file.
fn kept_of(p: &Paired) -> Kept {
    Kept {
        name: p.name.clone(),
        address: p.address.clone(),
        port: p.port,
        kind: if p.kind == "standalone" {
            Kind::Standalone
        } else {
            Kind::Host
        },
        fingerprint: p.fingerprint.clone(),
        paired: p.paired,
        serial: p.serial.clone(),
        link: Link::Never,
        last_seq: p.last_seq,
        ..Kept::default()
    }
}

/// The service: cheap to clone, one for every connection of its sockets.
#[derive(Clone)]
pub struct Service(pub Arc<Inner>);

impl Service {
    /// Start it: read the files and start a thread for each system.
    pub fn start(c: Config) -> Self {
        let store = c.state_dir.as_ref().map(Store::at);
        let read =
            |what: &str, e: String| (c.log)(&format!("{what} not read, defaults in use: {e}"));
        let limits = match store.as_ref().map(Store::load::<Limits>) {
            Some(Ok(Some(l))) => l,
            Some(Err(e)) => {
                read("mgmt.toml", e.to_string());
                Limits::default()
            }
            _ => Limits::default(),
        };
        let systems = match store.as_ref().map(Store::load::<Systems>) {
            Some(Ok(Some(s))) => s,
            Some(Err(e)) => {
                read("systems.toml", e.to_string());
                Systems::default()
            }
            _ => Systems::default(),
        };
        let history = History::open(c.history_dir.as_deref(), limits.history as usize);
        let inner = Arc::new(Inner {
            me: hello(&c.name, &c.release),
            identity: c.identity,
            limits,
            store,
            paired: Mutex::new(systems.clone()),
            history: Mutex::new(history),
            entries: Mutex::new(Vec::new()),
            setups: (Mutex::new(0), Condvar::new()),
            log: c.log,
            windows: AtomicU32::new(0),
            next_window: AtomicU64::new(1),
        });
        let entries: Vec<Entry> = systems
            .systems
            .iter()
            .map(|p| system::start(&inner, kept_of(p)))
            .collect();
        *inner.entries() = entries;
        Self(inner)
    }

    /// The window number a new terminal window takes.
    pub fn window(&self) -> u64 {
        self.0.next_window.fetch_add(1, Ordering::Relaxed)
    }
}
