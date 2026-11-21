// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! The service against the simulator, over real TLS on loopback:
//! pairing, what is kept, a refusal as the host gave it, a session
//! resumed with its events, a changed certificate, a shared console.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use mgmtd::{Config, Service};
use mgmtproto::Mgmt as _;
use mgmtproto::calls::Filter;
use mgmtproto::kept::Link;
use qsitrust::Identity;

fn until(what: &str, secs: u64, mut ok: impl FnMut() -> bool) {
    let end = Instant::now() + Duration::from_secs(secs);
    while Instant::now() < end {
        if ok() {
            return;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    panic!("{what} did not happen in {secs} s");
}

fn dir(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("mgmtd-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn service(name: &str) -> (Service, PathBuf) {
    let d = dir(name);
    let svc = Service::start(Config {
        identity: Identity::make("QSHQMC1", "", 1_790_000_000).unwrap(),
        name: "QSHQMC1".into(),
        release: "V1R0M0".into(),
        state_dir: Some(d.join("state")),
        identity_dir: None,
        history_dir: Some(d.join("mgmt")),
        log: Box::new(|l| eprintln!("mgmtd: {l}")),
        post: Box::new(|_, _| {}),
    });
    (svc, d)
}

fn sim(sys: &sim::Shared, cmd: &str) -> String {
    sim::control::command(std::slice::from_ref(sys), cmd)
}

#[test]
#[allow(clippy::too_many_lines, reason = "one scenario, step by step")]
fn pairing_keeping_and_refusals() {
    let host = sim::fixture::qshqvs1();
    let at = sim::net::listen(host.clone(), "127.0.0.1:0").unwrap();
    let (addr, port) = (at.ip().to_string(), at.port());
    let (mut svc, d) = service("pair");

    // No password out: the host turns the console away at TLS.
    let r = svc.add(
        addr.clone(),
        port,
        "AAAA-BBBB-CCCC-DDDD".into(),
        "qsadmin".into(),
    );
    assert_eq!(r.unwrap_err().main.msgid, "QMC1102");

    // A wrong password: the host's confirmation fails here, and the
    // host's own refusal follows.
    sim(&host, "pair QSHQVS1");
    let r = svc
        .add(
            addr.clone(),
            port,
            "AAAA-BBBB-CCCC-DDDD".into(),
            "qsadmin".into(),
        )
        .unwrap_err();
    assert_eq!(r.main.msgid, "QMC1111");
    assert!(r.also.iter().any(|n| n.msgid == "QSI0202"), "{r:?}");

    // Expired: turned away again.
    sim(&host, "expire QSHQVS1");
    let r = svc.add(
        addr.clone(),
        port,
        "AAAA-BBBB-CCCC-DDDD".into(),
        "qsadmin".into(),
    );
    assert_eq!(r.unwrap_err().main.msgid, "QMC1102");

    // A good password pairs, and the host's facts are kept.
    let pw = sim(&host, "pair QSHQVS1");
    let k = svc
        .add(addr.clone(), port, pw.clone(), "qsadmin".into())
        .unwrap();
    assert_eq!(k.name, "QSHQVS1");
    assert_eq!(k.serial, "10-9F11A2");
    until("the partitions read", 10, || {
        svc.system("QSHQVS1".into())
            .is_some_and(|k| k.partitions.len() == 3 && k.host.is_some())
    });
    let k = svc.system("QSHQVS1".into()).unwrap();
    assert_eq!(k.status(), "Operating");
    assert_eq!(k.code(), "00000000");
    assert_eq!(k.model(), "9Q10-V1");
    // The password is not kept: the console's state holds no trace of it.
    let state = std::fs::read_to_string(d.join("state/systems.toml")).unwrap();
    assert!(!state.contains(&pw) && state.contains("QSHQVS1"));
    // The same password again: used. The console is pinned there now,
    // so the host answers, with its own message.
    let r = svc.reset("QSHQVS1".into(), pw.clone(), "qsadmin".into());
    assert_eq!(r.unwrap_err().main.msgid, "QSI0201");

    // The history: QSTEST01's code, read by its cursor.
    until("the history read", 10, || {
        svc.history(Filter::default(), 0, 100)
            .rows
            .iter()
            .any(|r| r.posted.refcode.code == "A6005008" && r.partition_name == "QSTEST01")
    });

    // A refusal shows the host's own message.
    sim(
        &host,
        "refuse-power QSHQVS1 QVS1011 Two partitions are running.",
    );
    let r = svc
        .power("QSHQVS1".into(), false, false, 0, "operator1".into())
        .unwrap_err();
    assert_eq!(
        (r.main.msgid.as_str(), r.main.text.as_str()),
        ("QVS1011", "Two partitions are running.")
    );
    let r = svc
        .panel("QSHQVS1".into(), 1, 34, "operator1".into())
        .unwrap_err();
    assert_eq!(r.main.msgid, "QVS2307");
    sim(&host, "keylock QSHQVS1 1 manual");
    let r = svc
        .panel("QSHQVS1".into(), 1, 34, "operator1".into())
        .unwrap_err();
    assert_eq!(r.main.msgid, "QVS2309");
    let p = svc
        .panel("QSHQVS1".into(), 1, 11, "operator1".into())
        .unwrap();
    assert_eq!(p.text, "11  00000000");
    // Each request named the user, pairing too.
    let lines = sim(&host, "lines QSHQVS1");
    assert!(lines.contains("from user operator1") && lines.contains("pairing from user qsadmin"));

    // Dropped: resumed quietly, and a code posted meanwhile is seen once.
    sim(&host, "drop QSHQVS1");
    sim(&host, "pcode QSHQVS1 1 C6004031 Partition IPL in progress");
    until("the session back", 15, || {
        svc.system("QSHQVS1".into())
            .is_some_and(|k| k.link == Link::Connected)
    });
    until("the code replayed", 10, || {
        svc.history(
            Filter {
                code: "C6004031".into(),
                ..Filter::default()
            },
            0,
            10,
        )
        .total
            == 1
    });
    std::thread::sleep(Duration::from_millis(500));
    assert_eq!(
        svc.history(
            Filter {
                code: "C6004031".into(),
                ..Filter::default()
            },
            0,
            10
        )
        .total,
        1
    );

    // A new certificate under the same name: not trusted.
    sim(&host, "renew QSHQVS1");
    until("the change seen", 15, || {
        svc.system("QSHQVS1".into())
            .is_some_and(|k| k.link == Link::Changed)
    });
    let k = svc.system("QSHQVS1".into()).unwrap();
    assert_eq!(k.status(), "No connection");
    assert!(k.reason.contains("certificate"));

    // Removed: forgotten.
    svc.remove("QSHQVS1".into(), "qsadmin".into()).unwrap();
    assert!(svc.system("QSHQVS1".into()).is_none());
}

#[cfg(unix)]
#[test]
fn a_console_shared_by_two_windows() {
    use std::io::{Read as _, Write as _};
    use std::os::unix::net::{UnixListener, UnixStream};

    use mgmtproto::console::{Watch, Watching};
    use qsi::host::console::{In, Out, Pieces};
    use qsipart::console::Key;

    let host = sim::fixture::qshqvs1();
    let at = sim::net::listen(host.clone(), "127.0.0.1:0").unwrap();
    let (mut svc, d) = service("console");
    let pw = sim(&host, "pair QSHQVS1");
    svc.add(at.ip().to_string(), at.port(), pw, "qsadmin".into())
        .unwrap();
    until("the partitions read", 10, || {
        svc.system("QSHQVS1".into())
            .is_some_and(|k| k.partitions.len() == 3)
    });
    let path = d.join("consoles.sock");
    let listener = UnixListener::bind(&path).unwrap();
    let s2 = svc.clone();
    std::thread::spawn(move || mgmtd::service::consoles::serve(&listener, &s2));

    let open = |typing: bool| {
        let mut s = UnixStream::connect(&path).unwrap();
        s.set_read_timeout(Some(Duration::from_secs(20))).unwrap();
        let w = Watch {
            system: "QSHQVS1".into(),
            id: 1,
            user: "qsadmin".into(),
            take_over: false,
            typing,
        };
        let first = mgmtproto::console::Open {
            watch: Some(w),
            upload: None,
        };
        ipc::frame::write_frame(&mut s, &ipc::encode(&first)).unwrap();
        let a: Result<Watching, mgmtproto::Refusal> =
            ipc::decode(&ipc::frame::read_frame(&mut s).unwrap()).unwrap();
        assert_eq!(a.unwrap().system, "QSHQ400");
        s
    };
    let next = |s: &mut UnixStream, p: &mut Pieces| -> Out {
        let mut buf = [0u8; 8192];
        loop {
            if let Some(piece) = p.next_piece() {
                return Out::decode(&piece).unwrap();
            }
            let n = s.read(&mut buf).unwrap();
            assert!(n > 0, "the console closed");
            p.push(&buf[..n]);
        }
    };
    let (mut a, mut pa) = (open(true), Pieces::default());
    let Out::Frame(first) = next(&mut a, &mut pa) else {
        panic!("no whole screen first");
    };
    assert_eq!((first.rows, first.cols), (24, 80));
    // A second window, a viewer: the same screen at once.
    let (mut b, mut pb) = (open(false), Pieces::default());
    let Out::Frame(second) = next(&mut b, &mut pb) else {
        panic!("no whole screen first");
    };
    assert_eq!(second.cells, first.cells);
    // The viewer's key is dropped; the operator's reaches the system and
    // both see it.
    b.write_all(&In::Key(Key::Char('Z')).piece().unwrap())
        .unwrap();
    a.write_all(&In::Key(Key::Char('W')).piece().unwrap())
        .unwrap();
    let Out::Update(u) = next(&mut b, &mut pb) else {
        panic!("no update");
    };
    assert_eq!(u.runs[0].cells[0].ch, 'W');
    assert!(matches!(next(&mut a, &mut pa), Out::Update(_)));
    assert_eq!(sim(&host, "keys QSHQVS1 1"), "[Char('W')]");
}

#[test]
fn codes_posted_in_a_burst_are_kept() {
    let host = sim::fixture::qshqvs1();
    let at = sim::net::listen(host.clone(), "127.0.0.1:0").unwrap();
    let (mut svc, _d) = service("burst");
    let pw = sim(&host, "pair QSHQVS1");
    svc.add(at.ip().to_string(), at.port(), pw, "qsadmin".into())
        .unwrap();
    for i in 0..30 {
        sim(
            &host,
            &format!("pcode QSHQVS1 1 C6004031 Partition IPL step {i}"),
        );
    }
    sim(
        &host,
        "code QSHQVS1 C2005090 Partition started: QSDEV01, side A",
    );
    sim(&host, "code QSHQVS1 A2999999 A code no table has");
    until("the codes kept", 20, || {
        let p = svc.history(
            Filter {
                system: "QSHQVS1".into(),
                ..Filter::default()
            },
            0,
            100,
        );
        p.rows.iter().any(|r| r.posted.refcode.code == "A2999999")
    });
    let p = svc.history(
        Filter {
            system: "QSHQVS1".into(),
            ..Filter::default()
        },
        0,
        100,
    );
    assert_eq!(
        p.total,
        33,
        "{:?}",
        p.rows
            .iter()
            .map(|r| &r.posted.refcode.code)
            .collect::<Vec<_>>()
    );
}

/// Pair `sys` (listening at `at`) with `svc`, and wait for what it says
/// of itself.
fn paired(svc: &mut Service, sys: &sim::Shared, at: std::net::SocketAddr, name: &str) {
    let pw = sim(sys, &format!("pair {name}"));
    svc.add(at.ip().to_string(), at.port(), pw, "qsadmin".into())
        .unwrap();
}

#[test]
fn a_standalone_system_answers_its_own_panel_and_console_at_id_0() {
    let sys = sim::fixture::qshq400();
    let at = sim::net::listen(sys.clone(), "127.0.0.1:0").unwrap();
    let (mut svc, _d) = service("standalone");
    paired(&mut svc, &sys, at, "QSHQ400");
    until("its functions read", 10, || {
        svc.system("QSHQ400".into())
            .is_some_and(|k| !k.functions(0).is_empty())
    });
    let k = svc.system("QSHQ400".into()).unwrap();
    assert_eq!(k.kind, mgmtproto::kept::Kind::Standalone);
    assert!(k.offers(qsi::caps::PANEL_FUNCTIONS | qsi::caps::CONSOLE));
    assert!(!k.offers(qsi::caps::PARTITIONS));
    let own = k.part(0).unwrap();
    assert_eq!(
        (own.name.as_str(), own.code.as_str()),
        ("QSHQ400", "00000000")
    );
    assert!(k.part(1).is_none());
    let f = k.functions(0);
    assert!(f.iter().any(|f| f.function == 21 && f.manual));
    assert!(f.iter().any(|f| f.function == 8 && f.confirm));
    // Function 01 at any time; 21 needs the keylock in manual.
    let shown = svc.panel("QSHQ400".into(), 0, 1, "qsadmin".into()).unwrap();
    assert!(shown.text.starts_with("01"), "{shown:?}");
    assert_eq!(shown.after, "00000000");
    let r = svc
        .panel("QSHQ400".into(), 0, 21, "qsadmin".into())
        .unwrap_err();
    assert_eq!(r.main.msgid, "QSI0452");
    // Its screen at ID 0; partition 1 is no partition of it.
    assert!(
        svc.screen("QSHQ400".into(), 0, "qsadmin".into())
            .unwrap()
            .is_some()
    );
    let r = svc
        .screen("QSHQ400".into(), 1, "qsadmin".into())
        .unwrap_err();
    assert_eq!(r.main.msgid, "QSI0331");
    // A host's family it does not offer is named.
    let pools = mgmtproto::Asked::of(&qsi::any::AnyRequest::Vres(
        qsi::host::vres::Request::Pools(qsi::Empty),
    ));
    let r = svc
        .ask("QSHQ400".into(), pools, "qsadmin".into())
        .unwrap_err();
    assert_eq!(r.main.msgid, "QMC1114");
    assert!(r.main.text.contains("virtual resources"), "{r:?}");
}

#[test]
fn a_host_s_function_list_and_power_on_settings_are_its_own() {
    use qsi::any::{AnyReply, AnyRequest};
    use qsi::host::host;
    let sys = sim::fixture::qshqvs1();
    let at = sim::net::listen(sys.clone(), "127.0.0.1:0").unwrap();
    let (mut svc, _d) = service("poweron");
    paired(&mut svc, &sys, at, "QSHQVS1");
    until("the functions and the settings read", 10, || {
        svc.system("QSHQVS1".into())
            .is_some_and(|k| k.power_on.is_some() && (1..=3).all(|id| k.functions(id).len() == 30))
    });
    let k = svc.system("QSHQVS1".into()).unwrap();
    let on = k.power_on.unwrap();
    assert!(on.start_partitions && on.delay == 300);
    // Set through `ask`; the host's event keeps the new settings.
    let set = AnyRequest::Host(host::Request::SetPowerOn(host::PowerOn {
        start_partitions: false,
        power_off: host::PowerOff::Immediate,
        delay: 300,
    }));
    let a = svc
        .ask(
            "QSHQVS1".into(),
            mgmtproto::Asked::of(&set),
            "qsadmin".into(),
        )
        .unwrap();
    assert!(matches!(
        a.reply(),
        Some(AnyReply::Host(host::Reply::SetPowerOn(_)))
    ));
    until("the event kept", 10, || {
        svc.system("QSHQVS1".into())
            .and_then(|k| k.power_on)
            .is_some_and(|p| !p.start_partitions)
    });
    assert!(sim(&sys, "lines QSHQVS1").contains("0010/0021 from user qsadmin"));
    // A request the pages do not send is not carried.
    let time = mgmtproto::Asked::of(&AnyRequest::Base(qsi::base::Request::Time(qsi::Empty)));
    let r = svc
        .ask("QSHQVS1".into(), time, "qsadmin".into())
        .unwrap_err();
    assert_eq!(r.main.msgid, "QMC1115");

    // A host without the new bits: the settings are refused by name.
    sim(&sys, "withhold QSHQVS1 new");
    until("the session back without them", 15, || {
        svc.system("QSHQVS1".into()).is_some_and(|k| {
            k.link == Link::Connected && k.caps != 0 && !k.offers(qsi::caps::POWER_ON)
        })
    });
    let r = svc
        .ask(
            "QSHQVS1".into(),
            mgmtproto::Asked::of(&set),
            "qsadmin".into(),
        )
        .unwrap_err();
    assert_eq!(r.main.msgid, "QMC1114");
    assert!(r.main.text.contains("power-on settings"), "{r:?}");
}
