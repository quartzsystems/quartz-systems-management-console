// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! The web server's listeners. `webd` binds nothing: it connects to
//! [`release::WEB_SOCKET`] and PID 1 sends it TCP 443 and 80, bound on
//! the `open` interface's address, with `base/fdpass`. When the network
//! service notes another address, PID 1 binds again and sends the new
//! set on the same connection; `webd` closes the old ones.
//!
//! A set is text, one `port address` line each and an empty line, then
//! the byte that carries the sockets in the same order.

use std::fmt::Write as _;
use std::io::{self, Write};
use std::net::{Ipv4Addr, TcpListener};
use std::os::fd::{AsFd, BorrowedFd};
use std::os::unix::net::{UnixListener, UnixStream};

/// What PID 1 keeps of the web server's listeners.
#[derive(Default)]
pub struct Web {
    listener: Option<UnixListener>,
    client: Option<UnixStream>,
    /// The addresses last sent.
    sent: Option<Vec<Ipv4Addr>>,
}

/// The address in a status line (`10.0.2.15/24 on eth0, gateway ...`).
pub fn address(line: &str) -> Option<Ipv4Addr> {
    line.split_whitespace()
        .next()?
        .split('/')
        .next()?
        .parse()
        .ok()
}

/// The text of a set of listeners.
pub fn set_text(addresses: &[Ipv4Addr]) -> String {
    let mut text = String::new();
    for a in addresses {
        for port in [release::HTTPS_PORT, release::HTTP_PORT] {
            let _ = writeln!(text, "{port} {a}");
        }
    }
    text.push('\n');
    text
}

impl Web {
    /// Take the socket. A console without it has no web server.
    pub fn open() -> io::Result<Self> {
        let _ = std::fs::remove_file(release::WEB_SOCKET);
        let listener = UnixListener::bind(release::WEB_SOCKET)?;
        listener.set_nonblocking(true)?;
        Ok(Self {
            listener: Some(listener),
            ..Self::default()
        })
    }

    /// The socket to poll, when there is one.
    pub fn fd(&self) -> Option<BorrowedFd<'_>> {
        self.listener.as_ref().map(AsFd::as_fd)
    }

    /// `webd` connected: it is the one sent to from now on, and is sent
    /// the listeners for `addresses` at once.
    pub fn accept(&mut self, addresses: &[Ipv4Addr]) -> Result<(), String> {
        let Some(listener) = &self.listener else {
            return Ok(());
        };
        let Ok((stream, _)) = listener.accept() else {
            return Ok(());
        };
        stream
            .set_nonblocking(false)
            .map_err(|e| format!("the web server's connection: {e}"))?;
        self.client = Some(stream);
        self.sent = None;
        self.follow(addresses)
    }

    /// The addresses may have changed: send a new set when they did.
    pub fn follow(&mut self, addresses: &[Ipv4Addr]) -> Result<(), String> {
        if self.client.is_none() || self.sent.as_deref() == Some(addresses) {
            return Ok(());
        }
        let mut bound = Vec::new();
        let mut on = Vec::new();
        let mut failed = Vec::new();
        for a in addresses {
            let pair: io::Result<Vec<TcpListener>> = [release::HTTPS_PORT, release::HTTP_PORT]
                .into_iter()
                .map(|port| TcpListener::bind((*a, port)))
                .collect();
            match pair {
                Ok(pair) => {
                    bound.extend(pair);
                    on.push(*a);
                }
                Err(e) => failed.push(format!("{a}: {e}")),
            }
        }
        let fds: Vec<BorrowedFd<'_>> = bound.iter().map(AsFd::as_fd).collect();
        let sent = self.client.as_mut().map(|c| {
            c.write_all(set_text(&on).as_bytes())
                .and_then(|()| fdpass::send(c, &fds))
        });
        if let Some(Err(e)) = sent {
            self.client = None;
            return Err(format!("the web server's listeners were not sent: {e}"));
        }
        // PID 1's copies close here; the web server's stay open.
        drop(bound);
        // Kept as sent even when one failed: the web server holds the
        // others, which could not be bound again. The next address
        // change, or the web server's next start, tries again.
        self.sent = Some(addresses.to_vec());
        if failed.is_empty() {
            Ok(())
        } else {
            Err(format!("not bound: {}", failed.join("; ")))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_address_is_the_status_line_s_first_word() {
        assert_eq!(
            address("10.0.2.15/24 on eth0, gateway 10.0.2.2"),
            Some(Ipv4Addr::new(10, 0, 2, 15))
        );
        assert_eq!(address("none: the network service has not started"), None);
    }

    #[test]
    fn a_set_names_443_then_80_for_each_address() {
        let a = Ipv4Addr::new(192, 0, 2, 7);
        assert_eq!(set_text(&[a]), "443 192.0.2.7\n80 192.0.2.7\n\n");
        assert_eq!(set_text(&[]), "\n");
    }
}
