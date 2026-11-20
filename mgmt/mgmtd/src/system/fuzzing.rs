// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! What `mgmtd` does with a managed system's bytes after the handshake,
//! for the fuzzer (`tools/fuzz`, target `mgmt-peer`): a console's
//! session that has the host's hello, fed the bytes, and everything it
//! hands up applied as a system's thread applies it: replies to its
//! reads, events, a console's stream.

use std::sync::mpsc::channel;
use std::sync::{Arc, OnceLock};

use qsi::{Hello, PeerKind, caps};
use qsisess::{Limits, Role, Session};

use super::Entry;
use super::worker::Worker;
use crate::service::Inner;

/// A host's hello, as the bytes its session sends first.
fn host_hello() -> &'static [u8] {
    static BYTES: OnceLock<Vec<u8>> = OnceLock::new();
    BYTES.get_or_init(|| {
        let hello = Hello {
            version: qsi::VERSION,
            caps: caps::HOST_V1 | caps::SERVICING | caps::PHYSICAL_IO | caps::PARTITION_V2,
            kind: PeerKind::Host,
            name: "FUZZVS".into(),
            release: "V1R0M0".into(),
            serial: "F0000001".into(),
        };
        Session::new(Role::Managed, hello, true, Limits::default(), 0)
            .map(|mut s| s.take_output())
            .unwrap_or_default()
    })
}

fn inner() -> &'static Arc<Inner> {
    static INNER: OnceLock<Arc<Inner>> = OnceLock::new();
    INNER.get_or_init(Inner::bare)
}

/// Feed `data`, after the host's hello, to a system's thread's handling.
pub fn peer(data: &[u8]) {
    let (tx, rx) = channel();
    let entry = Entry {
        name: "FUZZVS".into(),
        kept: Arc::new(std::sync::Mutex::new(mgmtproto::Kept::default())),
        tx,
        stop: Arc::new(std::sync::atomic::AtomicBool::new(false)),
    };
    let mut w = Worker::new(Arc::clone(inner()), entry, rx);
    let me = crate::peer::link::hello("FUZZMC", "V1R0M0");
    let Ok(mut s) = Session::new(Role::Console, me, true, Limits::default(), 0) else {
        return;
    };
    s.feed(host_hello(), 1);
    // The thread's first request: the subscription its reply answers.
    w.mine(
        &mut s,
        super::worker::Mine::Subscribe,
        &qsi::any::AnyRequest::Base(qsi::base::Request::Subscribe(qsi::base::Subscribe {
            since: None,
        })),
    );
    let _ = s.take_output();
    for chunk in data.chunks(4096) {
        s.feed(chunk, 2);
        while let Some(i) = s.poll() {
            if w.incoming(&mut s, i).is_some() {
                return;
            }
        }
        let _ = s.take_output();
    }
}

#[cfg(test)]
mod tests {
    use qsi::any::AnyRequest;
    use qsisess::{EventLog, Incoming, Limits, Role, Session};

    #[test]
    fn junk_and_nothing_are_taken() {
        super::peer(b"");
        super::peer(b"QI\x10\x12\xff\xff\xff\xff junk");
        super::peer(&[0u8; 9000]);
    }

    /// What a host sends after its hello: the subscription's reply, then
    /// every sample event. Written as the target's seeds when
    /// `QSMC_SEEDS` names a folder; fed to the target always.
    #[test]
    fn a_host_s_events_are_taken() {
        let mut log = EventLog::new(&Limits::default());
        for e in qsi::samples::events() {
            let _ = log.push_any(&e);
        }
        let host = qsi::Hello {
            version: qsi::VERSION,
            caps: qsi::caps::HOST_V1
                | qsi::caps::SERVICING
                | qsi::caps::PHYSICAL_IO
                | qsi::caps::PARTITION_V2,
            kind: qsi::PeerKind::Host,
            name: "FUZZVS".into(),
            release: "V1R0M0".into(),
            serial: "F0000001".into(),
        };
        let mut m = Session::new(Role::Managed, host, true, Limits::default(), 0).unwrap();
        let mut c = Session::new(
            Role::Console,
            crate::peer::link::hello("FUZZMC", "V1R0M0"),
            true,
            Limits::default(),
            0,
        )
        .unwrap();
        let theirs = m.take_output();
        c.feed(&theirs, 1);
        let _ = c.poll();
        let hello = c.take_output();
        m.feed(&hello, 1);
        let _ = m.poll();
        let req = AnyRequest::Base(qsi::base::Request::Subscribe(qsi::base::Subscribe {
            since: None,
        }));
        let id = c.request_any("QSMC", &req).unwrap();
        m.feed(&c.take_output(), 2);
        while let Some(i) = m.poll() {
            if let Incoming::Request(r) = i {
                assert_eq!(r.id, id);
                m.subscribe(r.id, &log, Some(0)).unwrap();
            }
        }
        let seed = m.take_output();
        assert!(!seed.is_empty());
        super::peer(&seed);
        if let Ok(dir) = std::env::var("QSMC_SEEDS") {
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(std::path::Path::new(&dir).join("host-events"), &seed).unwrap();
        }
    }
}
