// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! The listeners, as PID 1 hands them over (`qsmcinit`'s `host::web`):
//! a set of `port address` lines and an empty line, then the sockets on
//! one byte. Each new set replaces the last. A connection is admitted
//! within the limits ([`Gate`]), given its timeouts, and served on a
//! thread of its own: TLS on 443, the redirect on 80.

use std::collections::HashMap;
use std::io::{self, Read};
use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::os::fd::OwnedFd;
use std::os::unix::net::UnixStream;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::thread;
use std::time::Duration;

use qsweb::http::{Handler, Peer};
use rustls::{ServerConfig, ServerConnection, StreamOwned};

/// Connections at once.
pub const MAX_CONNECTIONS: usize = 64;
/// Connections at once from one address.
pub const MAX_PER_ADDRESS: usize = 16;
/// How long a connection may be silent: a request's head and body
/// arrive within it, and an idle keep-alive ends after it.
pub const IDLE: Duration = Duration::from_secs(30);
/// How long the listener sleeps between looks when nothing arrives.
const LOOK: Duration = Duration::from_millis(200);

/// One listening socket of a set.
#[derive(Debug, PartialEq, Eq)]
pub struct Line {
    /// Its port.
    pub port: u16,
    /// Its address.
    pub address: Ipv4Addr,
}

/// The lines of a set, as PID 1 wrote them.
pub fn parse(text: &str) -> Result<Vec<Line>, String> {
    text.lines()
        .filter(|l| !l.is_empty())
        .map(|l| {
            let (port, address) = l.split_once(' ').ok_or_else(|| format!("`{l}`"))?;
            Ok(Line {
                port: port.parse().map_err(|_| format!("`{l}`"))?,
                address: address.parse().map_err(|_| format!("`{l}`"))?,
            })
        })
        .collect()
}

/// Read one set: the text to its empty line, a byte at a time so the
/// byte that carries the sockets is left for `fdpass`, then the sockets.
pub fn receive(stream: &UnixStream) -> io::Result<Vec<(Line, OwnedFd)>> {
    let mut text = Vec::new();
    let mut byte = [0u8];
    loop {
        let mut s = stream;
        if s.read(&mut byte)? == 0 {
            return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "PID 1 closed"));
        }
        text.push(byte[0]);
        if text == b"\n" || text.ends_with(b"\n\n") {
            break;
        }
    }
    let lines = parse(&String::from_utf8_lossy(&text)).map_err(io::Error::other)?;
    let fds = fdpass::recv_all(stream, lines.len())?;
    if fds.len() != lines.len() {
        return Err(io::Error::other(format!(
            "{} sockets for {} lines",
            fds.len(),
            lines.len()
        )));
    }
    Ok(lines.into_iter().zip(fds).collect())
}

/// The counts a connection is admitted by.
#[derive(Debug, Default)]
pub struct Gate {
    all: usize,
    by: HashMap<IpAddr, usize>,
}

impl Gate {
    /// Admit a connection from `ip`.
    pub fn admit(&mut self, ip: IpAddr) -> bool {
        let n = self.by.get(&ip).copied().unwrap_or(0);
        if self.all >= MAX_CONNECTIONS || n >= MAX_PER_ADDRESS {
            return false;
        }
        self.all += 1;
        self.by.insert(ip, n + 1);
        true
    }

    /// A connection from `ip` ended.
    pub fn closed(&mut self, ip: IpAddr) {
        self.all = self.all.saturating_sub(1);
        if let Some(n) = self.by.get_mut(&ip) {
            *n -= 1;
            if *n == 0 {
                self.by.remove(&ip);
            }
        }
    }
}

/// What every listener serves with.
pub struct Serving {
    /// TLS for 443.
    pub tls: Arc<ServerConfig>,
    /// The console.
    pub site: Arc<dyn Handler>,
    /// The console's host name, which a redirect on 80 may name.
    pub host: String,
    /// The limits.
    pub gate: Mutex<Gate>,
    /// The pages, which say which addresses are answered.
    pub pages: Arc<dyn crate::serve::Pages>,
}

/// A connection read no further once a request on it was refused for
/// its framing ([`crate::serve::ended`]).
struct Ending<'a, C> {
    inner: &'a mut C,
    from: SocketAddr,
}

impl<C: Read> Read for Ending<'_, C> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if crate::serve::ended(self.from) {
            return Ok(0);
        }
        self.inner.read(buf)
    }
}

impl<C: io::Write> io::Write for Ending<'_, C> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.inner.write(buf)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

fn one(serving: &Serving, stream: TcpStream, from: SocketAddr, port: u16) {
    if stream.set_read_timeout(Some(IDLE)).is_err()
        || stream.set_write_timeout(Some(IDLE)).is_err()
        || stream.set_nonblocking(false).is_err()
    {
        return;
    }
    if port == release::HTTPS_PORT {
        let Ok(conn) = ServerConnection::new(serving.tls.clone()) else {
            return;
        };
        // The terminal window's socket waits for the browser a short
        // while at a time: its read timeout is set through a clone of
        // the stream, and goes back to the idle limit after.
        let Ok(timed) = stream.try_clone() else {
            return;
        };
        let timeout = move |d: Option<std::time::Duration>| {
            let _ = timed.set_read_timeout(Some(d.unwrap_or(IDLE)));
        };
        let mut tls = StreamOwned::new(conn, stream);
        let peer = Peer {
            addr: from,
            secure: true,
        };
        let mut ending = Ending {
            inner: &mut tls,
            from,
        };
        qsweb::http::serve_guarded_timed(&mut ending, &peer, &serving.site, &timeout);
        crate::serve::forget(from);
        // The close is said and the connection ends: nothing more is
        // waited for from a client that may have gone silent.
        tls.conn.send_close_notify();
        while tls.conn.wants_write() {
            if tls.conn.write_tls(&mut tls.sock).is_err() {
                break;
            }
        }
    } else {
        let mut plain = stream;
        let peer = Peer {
            addr: from,
            secure: false,
        };
        // Sent on to 443 of the address it came to, or a name of it.
        let local = plain
            .local_addr()
            .map_or_else(|_| String::new(), |a| crate::serve::url_host(a.ip()));
        let redirect = crate::serve::Redirect {
            host: serving.host.clone(),
            local,
        };
        qsweb::http::serve(&mut plain, &peer, &redirect);
    }
}

fn accept(serving: &Arc<Serving>, listener: &TcpListener, port: u16, stop: &AtomicBool) {
    while !stop.load(Ordering::Relaxed) {
        match listener.accept() {
            Ok((stream, from)) => {
                let ip = from.ip();
                if !serving.pages.admits(ip) {
                    continue;
                }
                if !serving
                    .gate
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .admit(ip)
                {
                    // Turned away: closed at once.
                    continue;
                }
                let s = serving.clone();
                let spawned = thread::Builder::new().name("conn".into()).spawn(move || {
                    one(&s, stream, from, port);
                    s.gate
                        .lock()
                        .unwrap_or_else(PoisonError::into_inner)
                        .closed(ip);
                });
                if spawned.is_err() {
                    serving
                        .gate
                        .lock()
                        .unwrap_or_else(PoisonError::into_inner)
                        .closed(ip);
                }
            }
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => thread::sleep(LOOK),
            Err(_) => thread::sleep(LOOK),
        }
    }
}

/// The listeners of the set in use.
#[derive(Default)]
pub struct Running {
    stops: Vec<Arc<AtomicBool>>,
    /// The set's lines, as `port address`.
    pub on: Vec<String>,
}

impl Running {
    /// Stop the listeners of the last set and start `set`'s.
    pub fn replace(&mut self, serving: &Arc<Serving>, set: Vec<(Line, OwnedFd)>) {
        for stop in self.stops.drain(..) {
            stop.store(true, Ordering::Relaxed);
        }
        self.on.clear();
        for (line, fd) in set {
            let listener = TcpListener::from(fd);
            if listener.set_nonblocking(true).is_err() {
                continue;
            }
            let stop = Arc::new(AtomicBool::new(false));
            let (s, st, port) = (serving.clone(), stop.clone(), line.port);
            let started = thread::Builder::new()
                .name(format!("listen-{port}"))
                .spawn(move || accept(&s, &listener, port, &st));
            if started.is_ok() {
                self.stops.push(stop);
                self.on.push(format!("{} {}", line.port, line.address));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_set_s_lines_are_read() {
        assert_eq!(
            parse("443 10.0.2.15\n80 10.0.2.15\n\n").unwrap(),
            [
                Line {
                    port: 443,
                    address: Ipv4Addr::new(10, 0, 2, 15)
                },
                Line {
                    port: 80,
                    address: Ipv4Addr::new(10, 0, 2, 15)
                }
            ]
        );
        assert!(parse("\n").unwrap().is_empty());
        assert!(parse("443\n").is_err());
    }

    #[test]
    fn the_gate_counts_by_address() {
        let mut g = Gate::default();
        let a: IpAddr = "192.0.2.1".parse().unwrap();
        let b: IpAddr = "192.0.2.2".parse().unwrap();
        for _ in 0..MAX_PER_ADDRESS {
            assert!(g.admit(a));
        }
        assert!(!g.admit(a));
        assert!(g.admit(b));
        g.closed(a);
        assert!(g.admit(a));
    }

    #[test]
    fn a_set_arrives_with_its_sockets() {
        use std::io::Write as _;
        use std::os::fd::AsFd;
        let (pid1, webd) = UnixStream::pair().unwrap();
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = l.local_addr().unwrap().port();
        (&pid1)
            .write_all(format!("{port} 127.0.0.1\n\n").as_bytes())
            .unwrap();
        fdpass::send(&pid1, &[l.as_fd()]).unwrap();
        let set = receive(&webd).unwrap();
        assert_eq!(set.len(), 1);
        let got = TcpListener::from(set.into_iter().next().unwrap().1);
        assert_eq!(got.local_addr().unwrap().port(), port);
    }
}
