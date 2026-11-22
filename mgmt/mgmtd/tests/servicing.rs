// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! Servicing against the simulator, over real TLS on loopback: the
//! fleet's problems under the console's numbers, the same across a
//! restart and a reconnection; close and note in the user's name; a
//! dump downloaded with its credit as the back-pressure and abandoned;
//! a host's update staged, installed, activated, confirmed and rolled
//! back.

use std::io::{Read as _, Write as _};
use std::path::PathBuf;
use std::time::{Duration, Instant};

use mgmtd::{Config, Service};
use mgmtproto::Mgmt as _;
use mgmtproto::calls::ProblemFilter;
use qsi::any::{AnyReply, AnyRequest};
use qsitrust::Identity;
use sha2::{Digest, Sha256};

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
    let d = std::env::temp_dir().join(format!("mgmtd-svc-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn service(d: &std::path::Path) -> Service {
    Service::start(Config {
        identity: Identity::from_seed(&[3; 32], "QSHQMC1", "", 1_790_000_000, [1; 16]),
        name: "QSHQMC1".into(),
        release: "V1R0M0".into(),
        state_dir: Some(d.join("state")),
        identity_dir: None,
        history_dir: Some(d.join("mgmt")),
        log: Box::new(|l| eprintln!("mgmtd: {l}")),
        post: Box::new(|_, _, _| {}),
        updates_dir: d.join("updates"),
    })
}

fn sim(sys: &sim::Shared, cmd: &str) -> String {
    sim::control::command(std::slice::from_ref(sys), cmd)
}

fn ask(svc: &mut Service, req: &AnyRequest) -> Result<AnyReply, mgmtproto::Refusal> {
    svc.ask(
        "QSHQVS1".into(),
        mgmtproto::Asked::of(req),
        "qsadmin".into(),
    )
    .map(|a| a.reply().unwrap())
}

#[test]
#[allow(clippy::too_many_lines, reason = "one scenario, step by step")]
fn problems_keep_the_console_s_numbers_and_act_at_the_host() {
    let host = sim::fixture::qshqvs1();
    let at = sim::net::listen(host.clone(), "127.0.0.1:0").unwrap();
    let d = dir("problems");
    let mut svc = service(&d);
    let pw = sim(&host, "pair QSHQVS1");
    svc.add(at.ip().to_string(), at.port(), pw, "qsadmin".into())
        .unwrap();
    until("the problems read", 10, || {
        svc.problems(ProblemFilter::default(), 0, 25).total == 3
    });
    let page = svc.problems(ProblemFilter::default(), 0, 25);
    let row = |n: u64| {
        page.rows
            .iter()
            .find(|r| r.summary.number == n)
            .unwrap()
            .clone()
    };
    let e1042 = row(1042);
    assert_eq!(e1042.summary.refcode.code, "B6000100");
    assert_eq!(e1042.severity, Some(qsi::service::Severity::Terminal));
    let mine = e1042.number;

    // Opened, seen again, cleared at the host: each kept under one number.
    let said = sim(
        &host,
        "problem QSHQVS1 open 1 A2009020 Partition QSDEV01 is not responding on its service channel.",
    );
    let theirs: u64 = said.split_whitespace().nth(1).unwrap().parse().unwrap();
    until("the new problem kept", 10, || {
        svc.problems(ProblemFilter::default(), 0, 25).total == 4
    });
    let new = svc
        .problems(ProblemFilter::default(), 0, 25)
        .rows
        .into_iter()
        .find(|r| r.summary.number == theirs)
        .unwrap();
    sim(&host, &format!("problem QSHQVS1 seen {theirs}"));
    until("seen again", 10, || {
        svc.problems(ProblemFilter::default(), 0, 25)
            .rows
            .iter()
            .any(|r| r.number == new.number && r.summary.count == 2)
    });
    sim(&host, &format!("problem QSHQVS1 clear {theirs}"));
    until("cleared", 10, || {
        svc.problems(ProblemFilter::default(), 0, 25)
            .rows
            .iter()
            .any(|r| r.number == new.number && !r.summary.holds)
    });

    // Read whole from the host now.
    let detail = svc.problem(mine, "qsadmin".into()).unwrap();
    assert_eq!(detail.resources[0].name, "virtual disk VD04 in pool POOL1");

    // A close of a condition that holds: refused unless anyway.
    let close = |anyway| {
        AnyRequest::Service(qsi::service::Request::Close(qsi::service::Close {
            number: 1042,
            anyway,
        }))
    };
    let no = ask(&mut svc, &close(false)).unwrap_err();
    assert_eq!(no.main.msgid, "QSI1001");
    let note = AnyRequest::Service(qsi::service::Request::Note(qsi::service::Note {
        number: 1042,
        text: "Pool checked".into(),
    }));
    ask(&mut svc, &note).unwrap();
    ask(&mut svc, &close(true)).unwrap();
    until("closed", 10, || {
        svc.problems(ProblemFilter::default(), 0, 25)
            .rows
            .iter()
            .any(|r| r.number == mine && r.summary.status == qsi::service::ProblemStatus::Closed)
    });
    let d1042 = svc.problem(mine, "qsadmin".into()).unwrap();
    assert!(
        d1042
            .log
            .iter()
            .any(|l| l.text == "Note by qsadmin: Pool checked")
    );
    assert_eq!(d1042.row.summary.closed_by, "qsadmin");

    // The console's numbers are the same across a reconnection and a
    // restart of the service.
    sim(&host, "drop QSHQVS1");
    until("reconnected", 15, || {
        svc.system("QSHQVS1".into())
            .is_some_and(|k| k.link == mgmtproto::kept::Link::Connected)
    });
    drop(svc);
    let mut svc = service(&d);
    until("read again", 15, || {
        svc.problems(ProblemFilter::default(), 0, 25).total == 4
    });
    let again = svc.problems(ProblemFilter::default(), 0, 25);
    assert!(
        again
            .rows
            .iter()
            .any(|r| r.number == mine && r.summary.number == 1042)
    );
    assert!(
        again
            .rows
            .iter()
            .any(|r| r.number == new.number && r.summary.number == theirs)
    );
    let open = ProblemFilter {
        status: Some(qsi::service::ProblemStatus::Open),
        ..ProblemFilter::default()
    };
    assert_eq!(svc.problems(open, 0, 25).total, 1);
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn a_dump_downloads_paced_by_its_reader_and_an_abandoned_one_ends_at_the_host() {
    let host = sim::fixture::qshqvs1();
    let at = sim::net::listen(host.clone(), "127.0.0.1:0").unwrap();
    let work = dir("dumps");
    let mut svc = service(&work);
    let pw = sim(&host, "pair QSHQVS1");
    svc.add(at.ip().to_string(), at.port(), pw, "qsadmin".into())
        .unwrap();
    // 64 MiB: larger than any buffer on the way.
    sim(&host, "dump QSHQVS1 1 BIG.core 64");
    until("the dumps read", 10, || {
        svc.system("QSHQVS1".into()).is_some_and(|k| {
            k.part_dumps
                .iter()
                .any(|p| p.dumps.iter().any(|x| x.name == "BIG.core"))
        })
    });
    let consoles = work.join("consoles.sock");
    let listener = std::os::unix::net::UnixListener::bind(&consoles).unwrap();
    let s2 = svc.clone();
    std::thread::spawn(move || mgmtd::service::consoles::serve(&listener, &s2));
    let open = |name: &str| {
        let mut stream = std::os::unix::net::UnixStream::connect(&consoles).unwrap();
        let o = mgmtproto::console::Open {
            watch: None,
            upload: None,
            fetch: Some(mgmtproto::console::Fetch {
                system: "QSHQVS1".into(),
                id: 1,
                name: name.into(),
                user: "qsadmin".into(),
            }),
        };
        ipc::frame::write_frame(&mut stream, &ipc::encode(&o)).unwrap();
        let a = ipc::frame::read_frame(&mut stream).unwrap();
        let bytes: Result<u64, mgmtproto::Refusal> = ipc::decode(&a).unwrap();
        (stream, bytes)
    };
    let (s, bytes) = open("BIG.core");
    assert_eq!(bytes.unwrap(), 64 << 20);
    let mut r = mgmtproto::console::Frames::new(s);
    let mut hash = Sha256::new();
    let mut buf = vec![0u8; 1 << 16];
    let mut got = 0usize;
    loop {
        let n = r.read(&mut buf).unwrap();
        if n == 0 {
            break;
        }
        hash.update(&buf[..n]);
        got += n;
    }
    assert_eq!(got, 64 << 20);
    let want = Sha256::digest(sim::system::servicing::content("BIG.core", 0, 64 << 20));
    assert_eq!(hash.finalize(), want);

    // Abandoned after a piece: the host is told, and its stream ends.
    let (s, _) = open("BIG.core");
    let mut r = mgmtproto::console::Frames::new(s);
    r.read_exact(&mut buf).unwrap();
    drop(r);
    until("the host saw the download end", 15, || {
        sim(&host, "lines QSHQVS1").contains("a servicing stream was reset by the console")
    });
    // No such dump: the host's refusal.
    let (_, no) = open("NONE.core");
    assert_eq!(no.unwrap_err().main.msgid, "QVS2312");
    let _ = std::fs::remove_dir_all(&work);
    let _ = std::io::stdout().flush();
}

#[test]
fn a_host_update_is_staged_installed_activated_and_confirmed_or_rolled_back() {
    let host = sim::fixture::qshqvs1();
    let at = sim::net::listen(host.clone(), "127.0.0.1:0").unwrap();
    let d = dir("update");
    let mut svc = service(&d);
    let pw = sim(&host, "pair QSHQVS1");
    svc.add(at.ip().to_string(), at.port(), pw, "qsadmin".into())
        .unwrap();
    until("the slots read", 10, || {
        svc.system("QSHQVS1".into())
            .is_some_and(|k| k.update.is_some())
    });
    // The stage reads only the download service's folder.
    let no = svc
        .stage(
            "QSHQVS1".into(),
            "/data/state/identity.key".into(),
            "qsadmin".into(),
        )
        .unwrap_err();
    assert_eq!(no.main.msgid, "QMC7210");
    // An image as the download service leaves one.
    std::fs::create_dir_all(d.join("updates/qsvs")).unwrap();
    let img = d.join("updates/qsvs/QSVS-V1R0M1-update-ab.upd");
    let mut bytes = b"QSVSUPDT".to_vec();
    bytes.extend(std::iter::repeat_n(7u8, 3 << 20));
    std::fs::write(&img, &bytes).unwrap();
    svc.stage(
        "QSHQVS1".into(),
        img.display().to_string(),
        "qsadmin".into(),
    )
    .unwrap();
    until("staged and verified", 15, || {
        svc.system("QSHQVS1".into()).is_some_and(|k| {
            k.update
                .as_ref()
                .and_then(|u| u.staged.as_ref())
                .is_some_and(|s| s.verification == qsi::update::Verification::Verified)
        })
    });
    let k = svc.system("QSHQVS1".into()).unwrap();
    assert_eq!(k.staging.as_ref().unwrap().sent, bytes.len() as u64);
    let install = AnyRequest::Update(qsi::update::Request::Install(qsi::Empty));
    ask(&mut svc, &install).unwrap();
    // Partitions run: refused unless they are shut down.
    let activate = |p| {
        AnyRequest::Update(qsi::update::Request::Activate(qsi::update::Activate {
            partitions: p,
            timeout: 30,
        }))
    };
    let no = ask(&mut svc, &activate(qsi::update::Partitions::Refuse)).unwrap_err();
    assert_eq!(no.main.msgid, "QSI1113");
    ask(&mut svc, &activate(qsi::update::Partitions::ShutDown)).unwrap();
    until("confirmed after the restart", 30, || {
        svc.system("QSHQVS1".into())
            .and_then(|k| k.update_end)
            .is_some_and(|e| e.confirmed && e.release == "V1R0M1")
    });

    // The next one rolls back, with the host's reason.
    sim(
        &host,
        "outcome QSHQVS1 rollback A2001035 the web of services did not start",
    );
    svc.stage(
        "QSHQVS1".into(),
        img.display().to_string(),
        "qsadmin".into(),
    )
    .unwrap();
    until("staged again", 15, || {
        svc.system("QSHQVS1".into())
            .is_some_and(|k| k.update.as_ref().and_then(|u| u.staged.as_ref()).is_some())
    });
    ask(
        &mut svc,
        &AnyRequest::Update(qsi::update::Request::Install(qsi::Empty)),
    )
    .unwrap();
    ask(&mut svc, &activate(qsi::update::Partitions::ShutDown)).unwrap();
    until("rolled back after the restart", 30, || {
        svc.system("QSHQVS1".into())
            .and_then(|k| k.update_end)
            .is_some_and(|e| {
                !e.confirmed && e.reason.contains("did not start") && e.code == "A2001035"
            })
    });
    let _ = std::fs::remove_dir_all(&d);
}
