// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! A system on the network: a TCP listener, each connection a thread
//! with TLS 1.3 against the system's pins and the managed side's QSI
//! session over it, as a QS/VS host's server runs one: its consoles'
//! streams and its uploads' beside its requests.

mod console;
mod power;
mod transfer;
mod upload;

use std::io::{self, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use qsi::any::{AnyReply, AnyRequest};
use qsi::{Empty, msgid, pair};
use qsisess::{Incoming, Limits, Note, Role, Session};
use qsitrust::pair::{Managed, Start};
use qsitrust::tls;
use rustls::{ServerConnection, StreamOwned};

use crate::system::answer::{self, Answer};
use crate::system::pairing::{SystemGuard, SystemPins};
use crate::system::{Shared, lock};
use console::Consoles;
use upload::Uploads;

/// How long a handshake may take.
const HANDSHAKE: Duration = Duration::from_secs(10);

/// Listen for `system` on `addr`: the address it listens on.
///
/// # Errors
/// The address cannot be bound.
pub fn listen(system: Shared, addr: &str) -> io::Result<SocketAddr> {
    let l = TcpListener::bind(addr)?;
    let local = l.local_addr()?;
    thread::spawn(move || {
        for sock in l.incoming().flatten() {
            let sys = system.clone();
            thread::spawn(move || {
                let _ = connection(&sys, sock);
            });
        }
    });
    Ok(local)
}

fn quiet(e: &io::Error) -> bool {
    matches!(
        e.kind(),
        io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut | io::ErrorKind::Interrupted
    )
}

/// One connection, to its end.
fn connection(sys: &Shared, sock: TcpStream) -> Result<(), String> {
    let (generation, conn, identity) = {
        let mut s = lock(sys);
        if !s.up || s.faults.refuse_connections {
            return Ok(());
        }
        let c = s.next_conn;
        s.next_conn += 1;
        (s.generation, c, s.identity.clone())
    };
    let peer = sock
        .peer_addr()
        .map_or_else(|_| "?".into(), |a| a.to_string());
    sock.set_read_timeout(Some(Duration::from_millis(10)))
        .map_err(|e| e.to_string())?;
    let _ = sock.set_nodelay(true);
    let pins = SystemPins(sys.clone());
    let config =
        tls::server_config(&identity, Arc::new(pins.clone())).map_err(|e| e.to_string())?;
    let mut io = StreamOwned::new(
        ServerConnection::new(config).map_err(|e| e.to_string())?,
        sock,
    );
    let until = Instant::now() + HANDSHAKE;
    while io.conn.is_handshaking() {
        match io.conn.complete_io(&mut io.sock) {
            Ok(_) => {}
            Err(e) if quiet(&e) && Instant::now() < until => {}
            Err(e) => {
                lock(sys).note(format!("handshake from {peer} failed: {e}"));
                return Err(e.to_string());
            }
        }
    }
    let pinned = tls::peer(&io.conn, &pins).is_some_and(|p| p.pinned);
    let binding = tls::binding(&io.conn, &identity).map_err(|e| e.to_string())?;
    let hello = lock(sys).hello.clone();
    let mut sess = Session::new(Role::Managed, hello, pinned, Limits::default(), 0)
        .map_err(|e| e.to_string())?;
    lock(sys).note(format!("session from {peer} opened, pinned {pinned}"));
    let mut link = Link {
        sys: sys.clone(),
        conn,
        t0: Instant::now(),
        pairing: None,
        binding,
        consoles: Consoles::default(),
        uploads: Uploads::default(),
        transfers: transfer::Transfers::default(),
        junk_sent: false,
    };
    let end = link.run(&mut sess, &mut io, generation);
    link.consoles.release(sys, conn);
    link.uploads.release(sys);
    lock(sys).note(format!("session from {peer} closed: {end}"));
    Ok(())
}

/// One connection's own state.
struct Link {
    sys: Shared,
    conn: u64,
    t0: Instant,
    pairing: Option<Managed>,
    binding: qsitrust::Binding,
    consoles: Consoles,
    uploads: Uploads,
    transfers: transfer::Transfers,
    junk_sent: bool,
}

impl Link {
    fn now(&self) -> u64 {
        u64::try_from(self.t0.elapsed().as_millis()).unwrap_or(u64::MAX)
    }

    /// Turn the session until it ends: why it ended.
    fn run(
        &mut self,
        sess: &mut Session,
        io: &mut StreamOwned<ServerConnection, TcpStream>,
        generation: u64,
    ) -> String {
        let mut buf = vec![0u8; 64 * 1024];
        loop {
            let faults = {
                let s = lock(&self.sys);
                if s.generation != generation || !s.up {
                    return "the system went away".into();
                }
                s.faults.clone()
            };
            if faults.stall {
                thread::sleep(Duration::from_millis(20));
                continue;
            }
            if sess.is_open() && !self.junk_sent && (faults.nonsense || faults.oversize) {
                self.junk_sent = true;
                let junk: Vec<u8> = if faults.oversize {
                    // A frame header that says more than any body may be.
                    let mut v = b"QI".to_vec();
                    v.extend_from_slice(&[0x10, 0x12, 0xFF, 0xFF, 0xFF, 0xFF]);
                    v.extend(std::iter::repeat_n(0xA5, 4096));
                    v
                } else {
                    b"this is not a frame of any kind at all".to_vec()
                };
                let _ = io.write_all(&junk);
                let _ = io.flush();
            }
            if faults.flood && sess.is_open() {
                let mut s = lock(&self.sys);
                for i in 0..200 {
                    crate::system::codes::post(&mut s, "C2001005", &format!("Host running {i}"));
                }
            }
            let out = sess.take_output();
            if !out.is_empty() && (io.write_all(&out).is_err() || io.flush().is_err()) {
                return "the console went away".into();
            }
            match io.read(&mut buf) {
                Ok(0) => return "the console closed the connection".into(),
                Ok(n) => sess.feed(&buf[..n], self.now()),
                Err(e) if quiet(&e) => {}
                Err(e) => return e.to_string(),
            }
            sess.tick(self.now());
            while let Some(i) = sess.poll() {
                match i {
                    Incoming::Request(r) => self.request(sess, &r),
                    Incoming::Closed { text, .. } => return text,
                    Incoming::Data { stream, bytes } if self.transfers.owns(stream) => {
                        self.transfers.data(sess, stream, &bytes);
                    }
                    Incoming::StreamClosed { stream } if self.transfers.owns(stream) => {
                        self.transfers.ended(&self.sys, stream, true);
                    }
                    Incoming::StreamReset { stream, .. } if self.transfers.owns(stream) => {
                        lock(&self.sys).note("a servicing stream was reset by the console".into());
                        self.transfers.ended(&self.sys, stream, false);
                    }
                    Incoming::Data { stream, bytes } if self.uploads.owns(stream) => {
                        self.uploads.data(&self.sys, sess, stream, &bytes);
                    }
                    Incoming::Data { stream, bytes } => {
                        self.consoles
                            .data(&self.sys, sess, stream, &bytes, faults.no_credit);
                    }
                    Incoming::StreamClosed { stream } if self.uploads.owns(stream) => {
                        self.uploads.ended(&self.sys, stream, true);
                    }
                    Incoming::StreamReset { stream, .. } if self.uploads.owns(stream) => {
                        self.uploads.ended(&self.sys, stream, false);
                    }
                    Incoming::StreamClosed { stream } | Incoming::StreamReset { stream, .. } => {
                        self.consoles.ended(&self.sys, stream);
                    }
                    _ => {}
                }
            }
            if sess.subscribed() {
                let s = lock(&self.sys);
                let _ = sess.publish(&s.log);
            }
            self.consoles.pump(&self.sys, sess, self.conn);
            self.uploads.pump(&self.sys, sess);
            self.transfers.pump(sess);
            if sess.is_closed() {
                return "closed".into();
            }
        }
    }

    fn request(&mut self, sess: &mut Session, r: &qsisess::Request) {
        let req = match r.decode() {
            Ok(Some(q)) => q,
            Ok(None) => {
                let _ = sess.refuse_unknown(r.id);
                return;
            }
            Err(e) => {
                let _ = sess.refuse_bad(r.id, &e);
                return;
            }
        };
        if let AnyRequest::Pair(p) = &req {
            lock(&self.sys).note(format!("pairing from user {}", r.user));
            self.pair(sess, r.id, p);
            return;
        }
        let a = answer::answer(&mut lock(&self.sys), &req, &r.user);
        let _ = match a {
            Answer::Reply(rep) => sess.reply_any(r.id, &rep),
            Answer::Refuse(notes) => {
                let mut n = notes.into_iter().map(|(id, text)| Note::new(&id, &text));
                let main = n.next().unwrap_or_else(|| Note::new(msgid::BAD, "refused"));
                let also: Vec<Note> = n.collect();
                sess.refuse_with(r.id, &main, &also)
            }
            Answer::Subscribe(since) => {
                let s = lock(&self.sys);
                sess.subscribe(r.id, &s.log, since)
            }
            Answer::Attach(id, take_over) => self
                .consoles
                .attach(&self.sys, sess, r.id, self.conn, id, take_over),
            Answer::Upload(name, bytes) => self.uploads.open(&self.sys, sess, r.id, &name, bytes),
            Answer::Stage(name, bytes) => self.transfers.stage(sess, r.id, &name, bytes),
            Answer::Download(id, name, bytes) => {
                lock(&self.sys).note(format!("download of {name} begun, {bytes} bytes"));
                self.transfers.download(sess, r.id, id, &name, bytes)
            }
            Answer::Activate(slot) => {
                let done =
                    sess.reply_any(r.id, &AnyReply::Update(qsi::update::Reply::Activate(Empty)));
                power::activate(&self.sys, &slot);
                done
            }
            Answer::Power(action, immediate) => {
                let done = sess.reply_any(r.id, &AnyReply::Base(qsi::base::Reply::Power(Empty)));
                power::begin(&self.sys, action, immediate);
                done
            }
            Answer::Unknown => sess.refuse_unknown(r.id),
        };
    }

    fn pair(&mut self, sess: &mut Session, id: u32, p: &pair::Request) {
        let mut s = lock(&self.sys);
        let _ = match p {
            pair::Request::Start(start) => {
                let name = s.hello.name.clone();
                let mut guard = SystemGuard(&mut s);
                match Managed::answer(
                    &mut guard,
                    self.binding,
                    &Start {
                        spake: start.spake.clone(),
                        name: start.name.clone(),
                    },
                    &name,
                ) {
                    Ok((managed, a)) => {
                        self.pairing = Some(managed);
                        sess.reply_any(
                            id,
                            &AnyReply::Pair(pair::Reply::Answer(pair::Answer {
                                spake: a.spake,
                                confirm: a.confirm,
                                name: a.name,
                            })),
                        )
                    }
                    Err(_) => sess.refuse(
                        id,
                        msgid::NO_PAIRING,
                        "No pairing password is out at this host, or it expired or its attempts are spent.",
                    ),
                }
            }
            pair::Request::Confirm(c) => match self.pairing.take() {
                None => sess.refuse(
                    id,
                    msgid::PAIRING_ORDER,
                    "A confirmation came without a start.",
                ),
                Some(m) => {
                    let mut guard = SystemGuard(&mut s);
                    match m.confirm(
                        &mut guard,
                        &qsitrust::pair::Confirm {
                            confirm: c.confirm.clone(),
                        },
                    ) {
                        Ok(_) => sess.reply_any(id, &AnyReply::Pair(pair::Reply::Done(Empty))),
                        Err(_) => sess.refuse(
                            id,
                            msgid::PAIRING_FAILED,
                            "The pairing did not confirm: the password is not the one this host showed.",
                        ),
                    }
                }
            },
        };
    }
}
