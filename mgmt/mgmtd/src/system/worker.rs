// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! A system's thread: connect, subscribe from the last event seen, read
//! what the system says when it says to (or when nothing is kept yet),
//! then turn the session, carrying the service's requests and consoles,
//! until it ends; then try again after a back-off that doubles to a
//! minute.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender};
use std::time::{Duration, Instant};

use mgmtproto::Refusal;
use mgmtproto::kept::Link as State;
use qsi::any::AnyRequest;
use qsi::base;
use qsisess::{Incoming, Reply, Session};
use qsitrust::Fingerprint;

use super::console::Consoles;
use super::{Cmd, Entry};
use crate::peer::link::{Failed, Link, resolve};
use crate::peer::pair::refusal;
use crate::service::{Inner, now_s};
use crate::words::qmc;

/// The first wait before trying again.
const FIRST: Duration = Duration::from_secs(1);
/// The longest.
const LONGEST: Duration = Duration::from_mins(1);
/// How often the last event seen is written down.
const SAVE: Duration = Duration::from_secs(2);
/// How long a request of the service waits for its reply.
const ASK: Duration = Duration::from_secs(60);

/// A request this thread made, by what it was for.
#[derive(Debug)]
pub(super) enum Mine {
    Subscribe,
    Identify,
    State,
    Host,
    Hardware,
    List,
    Profile(u32),
    BaseCodes,
    PartCodes(u32),
    Attach(u32),
}

/// A request outstanding.
#[derive(Debug)]
pub(super) enum Pending {
    Mine(Mine),
    Theirs(Sender<Result<Reply, Refusal>>, Instant),
}

/// One system's thread.
pub(super) struct Worker {
    pub(super) inner: Arc<Inner>,
    pub(super) entry: Entry,
    rx: Receiver<Cmd>,
    pub(super) pending: HashMap<u32, Pending>,
    pub(super) consoles: Consoles,
    /// Partitions whose history gained codes while a read of it was
    /// out: read again when it ends.
    pub(super) stale: HashSet<u32>,
}

impl Worker {
    pub(super) fn new(inner: Arc<Inner>, entry: Entry, rx: Receiver<Cmd>) -> Self {
        Self {
            inner,
            entry,
            rx,
            pending: HashMap::new(),
            consoles: Consoles::default(),
            stale: HashSet::new(),
        }
    }

    fn stopped(&self) -> bool {
        self.entry.stop.load(Ordering::Relaxed)
    }

    fn name(&self) -> String {
        self.entry.name.clone()
    }

    /// Answer a command while there is no session.
    fn refuse(&self, cmd: Cmd, why: &str) {
        let r = qmc(&qmcmsg::NOT_CONNECTED, &[&self.name(), why]);
        match cmd {
            Cmd::Ask { tx, .. } => {
                let _ = tx.send(Err(r));
            }
            Cmd::Watch { tx, .. } => {
                let _ = tx.send(super::Seen::Start(Err(r)));
            }
            _ => {}
        }
    }

    /// Wait `d`, answering commands; a wake ends the wait.
    fn wait(&self, d: Duration, why: &str) {
        let until = Instant::now() + d;
        while !self.stopped() {
            let left = until.saturating_duration_since(Instant::now());
            if left.is_zero() {
                return;
            }
            match self.rx.recv_timeout(left.min(Duration::from_millis(250))) {
                Ok(Cmd::Wake) | Err(RecvTimeoutError::Disconnected) => return,
                Ok(cmd) => self.refuse(cmd, why),
                Err(RecvTimeoutError::Timeout) => {}
            }
        }
    }

    /// The thread, until the system is removed.
    pub(super) fn run(mut self) {
        let mut backoff = FIRST;
        while !self.stopped() {
            {
                let mut k = self.entry.kept();
                k.link = match k.link {
                    State::Connected | State::Lost => State::Lost,
                    State::Changed => State::Changed,
                    _ => State::Connecting,
                };
            }
            let why = match self.connect() {
                Ok(link) => {
                    backoff = FIRST;
                    self.session(link)
                }
                Err(f) => {
                    let mut k = self.entry.kept();
                    if matches!(f, Failed::Changed(_)) {
                        if k.link != State::Changed {
                            let at = format!("{} at {}:{}", k.name, k.address, k.port);
                            self.inner.post(&refcode::SYSTEM_CERTIFICATE_CHANGED, &at);
                        }
                        k.link = State::Changed;
                        k.reason = "it answered with a certificate other than the one pinned when it was paired; pair it again when that is right".into();
                    } else {
                        k.reason = f.words();
                    }
                    k.reason.clone()
                }
            };
            if self.stopped() {
                break;
            }
            self.inner.log(&format!("{}: {why}", self.name()));
            self.wait(backoff, &why);
            backoff = (backoff * 2).min(LONGEST);
        }
        self.consoles.lost(&self.name(), "the system was removed");
    }

    fn connect(&self) -> Result<Link, Failed> {
        let (address, port, pin) = {
            let k = self.entry.kept();
            (
                k.address.clone(),
                k.port,
                Fingerprint::from_hex(&k.fingerprint),
            )
        };
        let _gate = self.inner.setup();
        let to = resolve(&address, port)?;
        Link::connect(to, &self.inner.identity, self.inner.me.clone(), pin)
    }

    /// The session, until it ends: why.
    fn session(&mut self, mut link: Link) -> String {
        let name = self.name();
        {
            let mut k = self.entry.kept();
            k.link = State::Connected;
            k.reason.clear();
            k.last_contact = now_s();
            k.via = link.local.map(|a| a.to_string()).unwrap_or_default();
            k.caps = link.hello.caps;
            k.said_serial = if link.hello.serial == k.serial || k.serial.is_empty() {
                String::new()
            } else {
                link.hello.serial.clone()
            };
        }
        self.inner.log(&format!("{name}: connected"));
        let since = Some(self.entry.kept().last_seq).filter(|s| *s > 0);
        self.mine(
            &mut link.session,
            Mine::Subscribe,
            &AnyRequest::Base(base::Request::Subscribe(base::Subscribe { since })),
        );
        let early = std::mem::take(&mut link.early);
        for i in early {
            self.incoming(&mut link.session, i);
        }
        let mut saved = Instant::now();
        let mut seen = self.entry.kept().last_seq;
        let why = loop {
            if self.stopped() {
                link.close();
                break "the system was removed".to_string();
            }
            while let Ok(cmd) = self.rx.try_recv() {
                self.command(&mut link, cmd);
            }
            match link.step() {
                Err(e) => break e,
                Ok(got) => {
                    if !got.is_empty() {
                        self.entry.kept().last_contact = now_s();
                    }
                    for i in got {
                        if let Some(why) = self.incoming(&mut link.session, i) {
                            link.close();
                            return self.end(&why);
                        }
                    }
                }
            }
            self.consoles.pump(&mut link.session);
            self.expire();
            if let Some(n) = link.session.last_event() {
                self.entry.kept().last_seq = n;
            }
            let last = self.entry.kept().last_seq;
            if last != seen && saved.elapsed() > SAVE {
                self.inner.save_seq(&name, last);
                seen = last;
                saved = Instant::now();
            }
            if link.session.is_closed() {
                break "the session closed".into();
            }
        };
        self.end(&why)
    }

    /// The session ended: everything waiting is answered.
    fn end(&mut self, why: &str) -> String {
        let name = self.name();
        let last = self.entry.kept().last_seq;
        self.inner.save_seq(&name, last);
        for (_, p) in self.pending.drain() {
            if let Pending::Theirs(tx, _) = p {
                let _ = tx.send(Err(qmc(&qmcmsg::NOT_CONNECTED, &[&name, why])));
            }
        }
        self.consoles.lost(&name, why);
        let mut k = self.entry.kept();
        k.link = State::Lost;
        k.reason = format!("the connection was lost: {why}");
        k.reason.clone()
    }

    /// Refuse requests of the service that waited too long.
    fn expire(&mut self) {
        let name = self.name();
        let late: Vec<u32> = self
            .pending
            .iter()
            .filter(|(_, p)| matches!(p, Pending::Theirs(_, t) if t.elapsed() > ASK))
            .map(|(id, _)| *id)
            .collect();
        for id in late {
            if let Some(Pending::Theirs(tx, _)) = self.pending.remove(&id) {
                let _ = tx.send(Err(qmc(&qmcmsg::NO_REPLY, &[&name])));
            }
        }
    }

    /// Ask for this thread's own purposes.
    pub(super) fn mine(&mut self, s: &mut Session, what: Mine, req: &AnyRequest) {
        if let Ok(id) = s.request_any("QSMC", req) {
            self.pending.insert(id, Pending::Mine(what));
        }
    }

    /// Whether a request of this thread for `what` is outstanding.
    pub(super) fn asking(&self, what: &Mine) -> bool {
        self.pending.values().any(|p| match p {
            Pending::Mine(m) => std::mem::discriminant(m) == std::mem::discriminant(what),
            Pending::Theirs(..) => false,
        })
    }

    fn command(&mut self, link: &mut Link, cmd: Cmd) {
        match cmd {
            Cmd::Ask { req, user, tx } => match link.session.request_any(&user, &req) {
                Ok(id) => {
                    self.pending.insert(id, Pending::Theirs(tx, Instant::now()));
                }
                Err(e) => {
                    let _ = tx.send(Err(Refusal::new(e.msgid(), &e.to_string())));
                }
            },
            Cmd::Watch {
                id,
                wid,
                take_over,
                typing,
                user,
                tx,
            } => {
                let kept = self.entry.kept().clone();
                if self.consoles.watch(&kept, id, wid, typing, tx) {
                    let req = AnyRequest::Console(qsi::host::console::Request::Attach(
                        qsi::host::console::Attach { id, take_over },
                    ));
                    match link.session.request_any(&user, &req) {
                        Ok(r) => {
                            self.pending.insert(r, Pending::Mine(Mine::Attach(id)));
                            self.consoles.asked(id, r);
                        }
                        Err(e) => self
                            .consoles
                            .attached(id, Err(Refusal::new(e.msgid(), &e.to_string()))),
                    }
                }
            }
            Cmd::Input { id, wid, input } => self.consoles.input(id, wid, &input),
            Cmd::Unwatch { id, wid } => self.consoles.unwatch(&mut link.session, id, wid),
            Cmd::Wake => {}
        }
    }

    /// What came: `Some(why)` when the session must end.
    pub(crate) fn incoming(&mut self, s: &mut Session, i: Incoming) -> Option<String> {
        match i {
            Incoming::Reply(r) => match self.pending.remove(&r.id) {
                Some(Pending::Theirs(tx, _)) => {
                    let _ = tx.send(refusal(&r).map_or(Ok(r), Err));
                }
                Some(Pending::Mine(m)) => self.reply(s, m, &r),
                None => {}
            },
            Incoming::Event(e) => self.event(s, &e),
            Incoming::Data { stream, bytes } => {
                self.consoles.data(s, stream, &bytes);
            }
            Incoming::StreamClosed { stream } => {
                self.consoles
                    .ended(stream, "the managed system closed the console");
            }
            Incoming::StreamReset { stream, text, .. } => self.consoles.ended(stream, &text),
            Incoming::Closed {
                msgid,
                text,
                by_peer,
            } => {
                let who = if by_peer {
                    "the managed system"
                } else {
                    "the console"
                };
                return Some(match msgid {
                    Some(id) => format!("{who} closed the session ({id} {text})"),
                    None => format!("{who} closed the session"),
                });
            }
            _ => {}
        }
        None
    }
}
