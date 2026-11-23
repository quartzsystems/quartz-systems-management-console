// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! The service: the certificate, the listeners PID 1 hands over, the
//! first answer over TLS that makes it ready. It ends when PID 1 closes
//! the listeners' connection; supervision starts it again.

use std::os::unix::net::UnixStream;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use supervise::control::{Calls, Client};

use crate::listen::{self, Gate, Running, Serving};
use crate::serve::{Pages, Site};
use crate::tls::{self, Certs};

/// How long PID 1's socket is tried for at the start.
const PID1_WAIT: Duration = Duration::from_secs(30);

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// The host name and the `open` address, as PID 1 has them.
pub fn host_and_addresses() -> (String, Vec<String>) {
    let status = Client::new(release::CONTROL_SOCKET).status().ok();
    let fact = |k: &str| {
        status
            .as_ref()
            .and_then(|s| s.fact(k))
            .unwrap_or("")
            .to_string()
    };
    let host = fact(release::fact::HOST);
    let address = fact(release::fact::MANAGEMENT)
        .split_whitespace()
        .next()
        .and_then(|a| a.split('/').next())
        .filter(|a| a.parse::<std::net::Ipv4Addr>().is_ok())
        .map(str::to_string);
    let host = if host.is_empty() {
        "qsmc".to_string()
    } else {
        host
    };
    (host, address.into_iter().collect())
}

fn say(line: &str) {
    // Standard error reaches the log through PID 1's pipe.
    eprintln!("{line}");
}

fn connect() -> Result<UnixStream, String> {
    let until = Instant::now() + PID1_WAIT;
    loop {
        match UnixStream::connect(release::WEB_SOCKET) {
            Ok(s) => return Ok(s),
            Err(e) if Instant::now() >= until => {
                return Err(format!("{}: {e}", release::WEB_SOCKET));
            }
            Err(_) => std::thread::sleep(Duration::from_millis(500)),
        }
    }
}

/// The first answer over TLS, asked over a socket pair: the same TLS
/// configuration, certificate and pages as a browser's, with no need
/// of an address (the console keeps no loopback interface).
fn first_answer(sha256: &str, serving: &Arc<Serving>) -> Result<(), String> {
    let client = crate::selftest::pinned_client(sha256)?;
    let (a, b) = UnixStream::pair().map_err(|e| e.to_string())?;
    for s in [&a, &b] {
        s.set_read_timeout(Some(Duration::from_secs(10)))
            .map_err(|e| e.to_string())?;
    }
    let s = serving.clone();
    let served = std::thread::spawn(move || {
        if let Ok(conn) = rustls::ServerConnection::new(s.tls.clone()) {
            let mut tls = rustls::StreamOwned::new(conn, b);
            let peer = qsweb::http::Peer {
                addr: "127.0.0.1:1".parse().expect("an address"),
                secure: true,
            };
            qsweb::http::serve(&mut tls, &peer, s.site.as_ref());
            tls.conn.send_close_notify();
            let _ = tls.conn.complete_io(&mut tls.sock);
        }
    });
    let asked = crate::selftest::ask(client, a);
    let _ = served.join();
    asked
}

/// Run the web server with `pages` until PID 1 closes its connection.
pub fn run(pages: Arc<dyn Pages>) -> Result<(), String> {
    let lacking = processor::lacking();
    if !lacking.is_empty() {
        return Err(format!(
            "the processor lacks {}: the web server cannot run",
            lacking.join(", ")
        ));
    }
    let (host, addresses) = host_and_addresses();
    let (certs, made) = Certs::load_or_make(Path::new(release::WEB_DIR), &host, &addresses, now())?;
    let certs = Arc::new(certs);
    let shown = certs.shown();
    if made {
        say(&format!(
            "a self-signed certificate was made for {}: SHA-256 {}",
            shown.info.san.join(", "),
            shown.sha256
        ));
    }
    pages.started(certs.clone());
    let serving = Arc::new(Serving {
        tls: tls::server_config(certs.clone())?,
        site: Arc::new(Site {
            pages: pages.clone(),
        }),
        host: host.clone(),
        gate: Mutex::new(Gate::default()),
        pages,
    });
    let pid1 = connect()?;
    let mut running = Running::default();
    let mut ready = false;
    loop {
        // PID 1 sends a set as soon as it is connected, then one at
        // each change of the `open` address.
        let set = listen::receive(&pid1).map_err(|e| format!("PID 1's listeners: {e}"))?;
        // A certificate the console made names the addresses it is
        // reached at, which the first start may not have had yet: made
        // again before the new set listens.
        let mut at: Vec<String> = set.iter().map(|(l, _)| l.address.to_string()).collect();
        at.dedup();
        let (host, _) = host_and_addresses();
        match certs.follow(&host, &at, now()) {
            Ok(Some(s)) => say(&format!(
                "the self-signed certificate was made again for {}: SHA-256 {}",
                s.info.san.join(", "),
                s.sha256
            )),
            Ok(None) => {}
            Err(e) => say(&format!("the certificate was not made again: {e}")),
        }
        running.replace(&serving, set);
        say(&format!(
            "listening: {}",
            if running.on.is_empty() {
                "nowhere".to_string()
            } else {
                running.on.join(", ")
            }
        ));
        if !ready {
            first_answer(&certs.shown().sha256, &serving)?;
            say("answered a request over TLS: ready");
            supervise::ready::signal().map_err(|e| format!("ready: {e}"))?;
            ready = true;
        }
    }
}
