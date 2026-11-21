// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! One connection to a managed system: TCP, TLS 1.3 with the console's
//! identity against the system's pin (or, to pair, any system), and the
//! console's QSI session over it. QS/400's crates are the protocol;
//! this is the socket around them.

use std::io::{ErrorKind, Read, Write};
use std::net::{IpAddr, SocketAddr, TcpStream, ToSocketAddrs};
use std::sync::Arc;
use std::time::{Duration, Instant};

use qsi::{Hello, PeerKind};
use qsisess::{Incoming, Limits, Role, Session};
use qsitrust::{Fingerprint, Identity, Pins, tls};
use rustls::{ClientConnection, StreamOwned};

/// How long a TCP connection may take to open.
pub const CONNECT: Duration = Duration::from_secs(10);
/// How long TLS and the hellos may take.
pub const HANDSHAKE: Duration = Duration::from_secs(15);

/// What the console accepts of a system's certificate.
#[derive(Debug)]
struct Accept {
    pin: Option<Fingerprint>,
}

impl Pins for Accept {
    fn pinned(&self, peer: &Fingerprint) -> bool {
        self.pin.as_ref() == Some(peer)
    }
    fn pairing_open(&self) -> bool {
        self.pin.is_none()
    }
}

/// Why a connection could not be made, in words an operator can act on.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Failed {
    /// No TCP connection: refused, unreachable, timed out.
    Unreachable(String),
    /// The system closed the connection during TLS: with no pin, it
    /// let no unpaired console in.
    TurnedAway(String),
    /// The system's certificate is not the one pinned.
    Changed(String),
    /// TLS went, but the system said no QSI hello.
    NotQsi(String),
}

impl Failed {
    /// In words.
    pub fn words(&self) -> String {
        match self {
            Self::Unreachable(w) | Self::TurnedAway(w) | Self::Changed(w) | Self::NotQsi(w) => {
                w.clone()
            }
        }
    }
}

/// The steps of a connection, timed: what Test connection shows.
#[derive(Clone, Debug, Default)]
pub struct Timing {
    /// TCP's connect.
    pub tcp_ms: u64,
    /// TLS's handshake.
    pub tls_ms: u64,
    /// The hellos.
    pub hello_ms: u64,
}

/// A session with a managed system.
pub struct Link {
    /// The session.
    pub session: Session,
    io: StreamOwned<ClientConnection, TcpStream>,
    t0: Instant,
    buf: Vec<u8>,
    /// The system's hello.
    pub hello: Hello,
    /// Its certificate's SHA-256.
    pub peer: Fingerprint,
    /// The console's address the session leaves from.
    pub local: Option<IpAddr>,
    /// How long each step took.
    pub timing: Timing,
    /// What came with the hello that is not it.
    pub early: Vec<Incoming>,
}

fn quiet(e: &std::io::Error) -> bool {
    matches!(
        e.kind(),
        ErrorKind::WouldBlock | ErrorKind::TimedOut | ErrorKind::Interrupted
    )
}

fn ms(since: Instant) -> u64 {
    u64::try_from(since.elapsed().as_millis()).unwrap_or(u64::MAX)
}

/// The console's own hello: what it is and what it speaks.
pub fn hello(name: &str, release: &str) -> Hello {
    Hello {
        version: qsi::VERSION,
        caps: mgmtproto::ask::CONSOLE_CAPS,
        kind: PeerKind::Console,
        name: name.into(),
        release: release.into(),
        serial: String::new(),
    }
}

/// The first address of `address`:`port`.
///
/// # Errors
/// It names no address.
pub fn resolve(address: &str, port: u16) -> Result<SocketAddr, Failed> {
    (address, port)
        .to_socket_addrs()
        .map_err(|e| Failed::Unreachable(format!("{address}: {e}")))?
        .next()
        .ok_or_else(|| Failed::Unreachable(format!("{address}: no address")))
}

impl Link {
    /// Connect to `to` as `identity` (whose hello is `me`), accepting
    /// the system pinned as `pin`, or, with none, any system (to pair).
    ///
    /// # Errors
    /// [`Failed`], by the step that failed.
    pub fn connect(
        to: SocketAddr,
        identity: &Identity,
        me: Hello,
        pin: Option<Fingerprint>,
    ) -> Result<Self, Failed> {
        let t = Instant::now();
        let sock = TcpStream::connect_timeout(&to, CONNECT)
            .map_err(|e| Failed::Unreachable(format!("{to}: {e}")))?;
        let tcp_ms = ms(t);
        let local = sock.local_addr().ok().map(|a| a.ip());
        sock.set_read_timeout(Some(Duration::from_millis(10)))
            .map_err(|e| Failed::Unreachable(e.to_string()))?;
        let _ = sock.set_nodelay(true);
        let pinned = pin.is_some();
        let config = tls::client_config(identity, Arc::new(Accept { pin }))
            .map_err(|e| Failed::NotQsi(e.to_string()))?;
        let conn = ClientConnection::new(config, tls::server_name())
            .map_err(|e| Failed::NotQsi(e.to_string()))?;
        let mut io = StreamOwned::new(conn, sock);
        let t = Instant::now();
        let until = t + HANDSHAKE;
        while io.conn.is_handshaking() {
            match io.conn.complete_io(&mut io.sock) {
                Ok(_) => {}
                Err(e) if quiet(&e) && Instant::now() < until => {}
                Err(e) => {
                    let text = e.to_string();
                    // A pinned system's other certificate is refused by
                    // our side; an unpaired console is turned away by
                    // theirs, which closes the connection.
                    return Err(if pinned && text.contains("certificate") {
                        Failed::Changed(text)
                    } else if pinned {
                        Failed::Unreachable(format!("TLS with {to}: {text}"))
                    } else {
                        Failed::TurnedAway(text)
                    });
                }
            }
        }
        let tls_ms = ms(t);
        let peer = io
            .conn
            .peer_certificates()
            .and_then(|c| c.first())
            .map(|c| Fingerprint::of(c))
            .ok_or_else(|| Failed::NotQsi("no certificate".into()))?;
        let session = Session::new(Role::Console, me, true, Limits::default(), 0)
            .map_err(|e| Failed::NotQsi(e.to_string()))?;
        let mut link = Self {
            session,
            io,
            t0: Instant::now(),
            buf: vec![0; 64 * 1024],
            hello: Hello {
                version: 0,
                caps: 0,
                kind: PeerKind::Host,
                name: String::new(),
                release: String::new(),
                serial: String::new(),
            },
            peer,
            local,
            timing: Timing {
                tcp_ms,
                tls_ms,
                hello_ms: 0,
            },
            early: Vec::new(),
        };
        let t = Instant::now();
        let until = t + HANDSHAKE;
        loop {
            // In TLS 1.3 the client's handshake ends before the system
            // has judged its certificate: a system that does not take
            // this console closes the connection here.
            let got = link.step().map_err(|e| {
                Failed::TurnedAway(format!("{to} closed the connection after TLS: {e}"))
            })?;
            for i in got {
                match i {
                    Incoming::Ready { peer, .. } => link.hello = peer,
                    Incoming::Closed { msgid, text, .. } => {
                        return Err(Failed::NotQsi(format!(
                            "{} {text}",
                            msgid.unwrap_or_default()
                        )));
                    }
                    other => link.early.push(other),
                }
            }
            if link.session.is_open() {
                break;
            }
            if Instant::now() > until {
                return Err(Failed::NotQsi(format!("{to} said no QSI hello")));
            }
        }
        link.timing.hello_ms = ms(t);
        Ok(link)
    }

    /// What ties a pairing to this connection.
    ///
    /// # Errors
    /// The connection has none.
    pub fn binding(&self, identity: &Identity) -> Result<qsitrust::Binding, String> {
        tls::binding(&self.io.conn, identity).map_err(|e| e.to_string())
    }

    /// One turn: send what is queued, read what came (waiting at most
    /// 10 ms), hand up what happened.
    ///
    /// # Errors
    /// The connection ended.
    pub fn step(&mut self) -> Result<Vec<Incoming>, String> {
        let now = ms(self.t0);
        let out = self.session.take_output();
        if !out.is_empty() {
            self.io.write_all(&out).map_err(|e| e.to_string())?;
            self.io.flush().map_err(|e| e.to_string())?;
        }
        match self.io.read(&mut self.buf) {
            Ok(0) => return Err("the managed system closed the connection".into()),
            Ok(n) => self.session.feed(&self.buf[..n], now),
            Err(e) if quiet(&e) => {}
            Err(e) => return Err(e.to_string()),
        }
        self.session.tick(now);
        Ok(std::iter::from_fn(|| self.session.poll()).collect())
    }

    /// Close cleanly.
    pub fn close(&mut self) {
        self.session.close("done");
        let _ = self.step();
        self.io.conn.send_close_notify();
        let _ = self.io.flush();
    }
}
