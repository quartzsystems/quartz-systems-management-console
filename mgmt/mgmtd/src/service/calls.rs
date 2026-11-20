// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! The web server's requests, answered.

use std::sync::atomic::Ordering;
use std::sync::mpsc::channel;
use std::time::{Duration, Instant};

use mgmtproto::calls::{File, Filter, Page, Panelled, Status, Tested};
use mgmtproto::kept::Link;
use mgmtproto::{Kept, Mgmt, Refusal};
use qsi::Empty;
use qsi::any::{AnyReply, AnyRequest};
use qsi::host::{console, panel, partitions};
use qsi::{PeerKind, base};
use qsipart::console::Frame;
use qsisess::Reply;

use super::{Inner, Service, kept_of, lock, now_s};
use crate::keep::files::Paired;
use crate::peer::link::{Link as Conn, resolve};
use crate::peer::pair::{self, ask, failed};
use crate::system::{self, Cmd};
use crate::words::qmc;

/// How long a request waits for its system's thread.
const WAIT: Duration = Duration::from_secs(70);
/// How long a system just paired is given to say what it is.
const FIRST_READ: Duration = Duration::from_secs(10);

impl Inner {
    /// Ask `name` `req` for `user`, on its session.
    pub fn ask(&self, name: &str, req: AnyRequest, user: &str) -> Result<Reply, Refusal> {
        let e = self
            .entry(name)
            .ok_or_else(|| qmc(&qmcmsg::NO_SUCH_SYSTEM, &[name]))?;
        {
            let k = e.kept();
            if k.link != Link::Connected {
                let why = if k.reason.is_empty() {
                    "the console has not reached it yet"
                } else {
                    &k.reason
                };
                return Err(qmc(&qmcmsg::NOT_CONNECTED, &[&k.name, why]));
            }
        }
        let (tx, rx) = channel();
        let _ = e.tx.send(Cmd::Ask {
            req,
            user: user.into(),
            tx,
        });
        match rx.recv_timeout(WAIT) {
            Ok(r) => r,
            Err(_) => Err(qmc(&qmcmsg::NO_REPLY, &[&e.name])),
        }
    }

    /// Wait for a system just started to say what it is: what is kept.
    fn first_read(&self, name: &str) -> Kept {
        let until = Instant::now() + FIRST_READ;
        loop {
            let Some(e) = self.entry(name) else {
                return Kept::default();
            };
            let k = e.kept().clone();
            let read = k.identity.is_some() && k.state.is_some();
            if read || Instant::now() > until {
                return k;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    }

    fn forget(&self, name: &str) {
        let mut entries = self.entries();
        if let Some(i) = entries
            .iter()
            .position(|e| e.name.eq_ignore_ascii_case(name))
        {
            let e = entries.remove(i);
            e.stop.store(true, Ordering::Relaxed);
            let _ = e.tx.send(Cmd::Wake);
        }
    }

    fn pair_with(
        &self,
        address: &str,
        port: u16,
        password: &str,
        user: &str,
    ) -> Result<Paired, Refusal> {
        if password.trim().is_empty() {
            return Err(qmc(&qmcmsg::PASSWORD_NEEDED, &[]));
        }
        let done = {
            let _gate = self.setup();
            pair::pair(
                address,
                port,
                &self.identity,
                self.me.clone(),
                password,
                user,
            )?
        };
        let h = done.hello;
        self.log(&format!(
            "paired with {} at {address}:{port} for user {user}",
            h.name
        ));
        Ok(Paired {
            name: h.name,
            address: address.into(),
            port,
            kind: if h.kind == PeerKind::Host {
                "host".into()
            } else {
                "standalone".into()
            },
            fingerprint: done.paired.fingerprint.hex(),
            paired: now_s(),
            serial: h.serial,
            last_seq: 0,
            description: String::new(),
        })
    }
}

fn reply_of(r: &Reply) -> Option<AnyReply> {
    r.decode().ok().flatten()
}

impl Mgmt for Service {
    fn systems(&mut self) -> Vec<Kept> {
        let mut v: Vec<Kept> = self.0.entries().iter().map(|e| e.kept().clone()).collect();
        v.sort_by(|a, b| a.name.cmp(&b.name));
        v
    }

    fn system(&mut self, name: String) -> Option<Kept> {
        self.0.entry(&name).map(|e| e.kept().clone())
    }

    fn add(
        &mut self,
        address: String,
        port: u16,
        password: String,
        user: String,
    ) -> Result<Kept, Refusal> {
        let inner = self.0.clone();
        let address = address.trim().to_string();
        {
            let entries = inner.entries();
            if entries.len() >= inner.limits.max_systems as usize {
                return Err(qmc(
                    &qmcmsg::TOO_MANY_SYSTEMS,
                    &[&inner.limits.max_systems.to_string()],
                ));
            }
            if let Some(e) = entries.iter().find(|e| {
                let k = e.kept();
                k.address.eq_ignore_ascii_case(&address) && k.port == port
            }) {
                return Err(qmc(&qmcmsg::NOT_PAIRED_HERE, &[&e.name]));
            }
        }
        let p = inner.pair_with(&address, port, &password, &user)?;
        if inner.entry(&p.name).is_some() {
            return Err(qmc(&qmcmsg::NOT_PAIRED_HERE, &[&p.name]));
        }
        let next = {
            let mut paired = lock(&inner.paired);
            paired.systems.push(p.clone());
            paired.clone()
        };
        inner.save(&next);
        let entry = system::start(&inner, kept_of(&p));
        inner.entries().push(entry);
        inner.post(
            &refcode::SYSTEM_PAIRED,
            &format!("{} at {address}:{port}, by user {user}", p.name),
        );
        Ok(inner.first_read(&p.name))
    }

    fn test(&mut self, name: String, user: String) -> Result<Tested, Refusal> {
        let inner = self.0.clone();
        let k = self
            .system(name.clone())
            .ok_or_else(|| qmc(&qmcmsg::NO_SUCH_SYSTEM, &[&name]))?;
        let at = format!("{}:{}", k.address, k.port);
        let to = resolve(&k.address, k.port).map_err(|f| failed(&at, &f))?;
        let pin = qsitrust::Fingerprint::from_hex(&k.fingerprint);
        let mut conn = Conn::connect(to, &inner.identity, inner.me.clone(), pin)
            .map_err(|f| failed(&at, &f))?;
        let t = Instant::now();
        let mut besides = Vec::new();
        let ping = ask(
            &mut conn,
            &user,
            &AnyRequest::Base(base::Request::Time(Empty)),
            &mut besides,
        );
        let ping_ms = t.elapsed().as_millis();
        conn.close();
        let mut steps = vec![
            format!("TCP connect {} ms", conn.timing.tcp_ms),
            format!("TLS 1.3 handshake {} ms", conn.timing.tls_ms),
            format!(
                "QSI/{} hello {} ms",
                conn.hello.version, conn.timing.hello_ms
            ),
        ];
        let ok = ping.is_ok();
        steps.push(match ping {
            Ok(_) => format!("heartbeat {ping_ms} ms"),
            Err(e) => format!("heartbeat failed: {e}"),
        });
        if conn.hello.serial != k.serial && !k.serial.is_empty() {
            steps.push(format!(
                "serial number {} (paired as {})",
                conn.hello.serial, k.serial
            ));
        }
        Ok(Tested { steps, ok })
    }

    fn reset(&mut self, name: String, password: String, user: String) -> Result<Kept, Refusal> {
        let inner = self.0.clone();
        let k = self
            .system(name.clone())
            .ok_or_else(|| qmc(&qmcmsg::NO_SUCH_SYSTEM, &[&name]))?;
        let mut p = inner.pair_with(&k.address, k.port, &password, &user)?;
        p.name.clone_from(&k.name);
        p.description.clone_from(&k.description);
        inner.forget(&k.name);
        let next = {
            let mut paired = lock(&inner.paired);
            paired.systems.retain(|s| s.name != k.name);
            paired.systems.push(p.clone());
            paired.clone()
        };
        inner.save(&next);
        let entry = system::start(&inner, kept_of(&p));
        inner.entries().push(entry);
        inner.post(
            &refcode::SYSTEM_PAIRED_AGAIN,
            &format!("{}, by user {user}", p.name),
        );
        Ok(inner.first_read(&p.name))
    }

    fn remove(&mut self, name: String, user: String) -> Result<(), Refusal> {
        let inner = self.0.clone();
        let e = inner
            .entry(&name)
            .ok_or_else(|| qmc(&qmcmsg::NO_SUCH_SYSTEM, &[&name]))?;
        inner.forget(&e.name);
        let next = {
            let mut paired = lock(&inner.paired);
            paired.systems.retain(|s| s.name != e.name);
            paired.clone()
        };
        inner.save(&next);
        inner.history().remove(&e.name);
        inner.post(
            &refcode::SYSTEM_REMOVED,
            &format!("{}, by user {user}", e.name),
        );
        Ok(())
    }

    fn power(
        &mut self,
        name: String,
        restart: bool,
        immediate: bool,
        delay: u32,
        user: String,
    ) -> Result<(), Refusal> {
        let req = AnyRequest::Base(base::Request::Power(base::Power {
            action: if restart {
                base::PowerAction::Restart
            } else {
                base::PowerAction::Off
            },
            immediate,
            delay: delay.min(99_999),
        }));
        self.0.ask(&name, req, &user).map(|_| ())
    }

    fn panel(
        &mut self,
        name: String,
        id: u32,
        function: u8,
        user: String,
    ) -> Result<Panelled, Refusal> {
        let inner = self.0.clone();
        let code_of = |k: &Kept| {
            k.partitions
                .iter()
                .find(|p| p.id == id)
                .map(|p| p.code.clone())
                .unwrap_or_default()
        };
        let before = self
            .system(name.clone())
            .map(|k| code_of(&k))
            .unwrap_or_default();
        let req = AnyRequest::Panel(panel::Request::Function(panel::Function {
            id,
            function,
            args: panel::PanelArgs::default(),
        }));
        let r = inner.ask(&name, req, &user)?;
        let text = match reply_of(&r) {
            Some(AnyReply::Panel(panel::Reply::Function(s))) => s.text,
            _ => String::new(),
        };
        let get = AnyRequest::Partitions(partitions::Request::Get(partitions::Get { id }));
        let after = match inner
            .ask(&name, get, &user)
            .ok()
            .as_ref()
            .and_then(reply_of)
        {
            Some(AnyReply::Partitions(partitions::Reply::Get(p))) => p.status.code,
            _ => self.system(name).map(|k| code_of(&k)).unwrap_or_default(),
        };
        Ok(Panelled {
            text,
            before,
            after,
        })
    }

    fn history(&mut self, filter: Filter, skip: u32, rows: u32) -> Page {
        self.0.history().query(&filter, skip, rows.clamp(1, 1000))
    }

    fn screen(&mut self, name: String, id: u32, user: String) -> Result<Option<Frame>, Refusal> {
        let req = AnyRequest::Console(console::Request::Screen(console::Screen { id }));
        let r = self.0.ask(&name, req, &user)?;
        Ok(match reply_of(&r) {
            Some(AnyReply::Console(console::Reply::Screen(c))) => c.frame,
            _ => None,
        })
    }

    fn status(&mut self) -> Status {
        let inner = self.0.clone();
        let entries = inner.entries();
        Status {
            systems: u32::try_from(entries.len()).unwrap_or(u32::MAX),
            connected: u32::try_from(
                entries
                    .iter()
                    .filter(|e| e.kept().link == Link::Connected)
                    .count(),
            )
            .unwrap_or(u32::MAX),
            max_systems: inner.limits.max_systems,
            consoles: inner.windows.load(Ordering::Relaxed),
            max_consoles: inner.limits.max_consoles,
            fingerprint: inner.identity.fingerprint().hex(),
        }
    }

    fn describe(&mut self, name: String, description: String, user: String) -> Result<(), Refusal> {
        let inner = self.0.clone();
        let e = inner
            .entry(&name)
            .ok_or_else(|| qmc(&qmcmsg::NO_SUCH_SYSTEM, &[&name]))?;
        let description: String = description.chars().take(256).collect();
        let next = {
            let mut paired = lock(&inner.paired);
            if let Some(p) = paired.systems.iter_mut().find(|p| p.name == e.name) {
                p.description.clone_from(&description);
            }
            paired.clone()
        };
        inner.save(&next);
        e.kept().description = description;
        inner.log(&format!("{} described by user {user}", e.name));
        Ok(())
    }

    fn backup_files(&mut self) -> Vec<File> {
        let read = |dir: &std::path::Path, f: &str, top: &str, mode: u32| {
            std::fs::read(dir.join(f)).ok().map(|bytes| File {
                path: format!("{top}/{f}"),
                mode,
                bytes,
            })
        };
        let mut out = Vec::new();
        if let Some(d) = &self.0.identity_dir {
            out.extend(read(d, "identity.crt", "state", 0o644));
            out.extend(read(d, "identity.key", "state", 0o600));
        }
        if let Some(store) = &self.0.store {
            out.extend(read(store.dir(), "systems.toml", "mgmt", 0o600));
            out.extend(read(store.dir(), "mgmt.toml", "mgmt", 0o600));
        }
        out
    }
}
