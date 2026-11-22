// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! A scheduled install, for a host and for the console, against the
//! managed-systems service, the download service and the update service
//! in memory: the same checks as the page at its time, then the stage,
//! the install and the activation in the owner's name.

use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{Arc, Mutex};

use fetchd::Stream;
use fetchd::proto::{Entry, File, FileState, Known, Status as Fetched};
use mgmtproto::Kept;
use mgmtproto::kept::{Link, Staging};
use qsi::host::model::PartitionState;
use qsi::host::partitions::Status as Part;
use schedd::{Op, Outcome, Repeat, Sched as _, Schedule};

/// What each fake was asked, in order.
type Log = Arc<Mutex<Vec<String>>>;

struct Mgmt(Log, Arc<Mutex<Kept>>);

impl mgmtproto::Calls for Mgmt {
    fn call(&mut self, req: &mgmtproto::Request) -> Result<Vec<u8>, ipc::WireError> {
        use mgmtproto::Request as R;
        let mut k = self.1.lock().unwrap();
        Ok(match req {
            R::System { .. } => ipc::encode(&Some(k.clone())),
            R::Stage { name, path, user } => {
                self.0
                    .lock()
                    .unwrap()
                    .push(format!("stage {name} {path} {user}"));
                k.staging = Some(Staging {
                    name: "QSVS-V1R0M1-update-ab.upd".into(),
                    bytes: 4,
                    sent: 4,
                    done: true,
                    ..Staging::default()
                });
                k.update = Some(qsi::update::Status {
                    slots: vec![slot("A", true, qsi::update::Attempt::Confirmed)],
                    staged: Some(qsi::update::StagedImage {
                        name: "QSVS-V1R0M1-update-ab.upd".into(),
                        bytes: 4,
                        verification: qsi::update::Verification::Verified,
                        image: None,
                        error: None,
                        time: 1,
                    }),
                    installing: None,
                });
                ipc::encode(&Ok::<(), mgmtproto::Refusal>(()))
            }
            R::Ask { name, req, user } => {
                let what = match req.request() {
                    Some(qsi::any::AnyRequest::Update(qsi::update::Request::Install(_))) => {
                        if let Some(u) = k.update.as_mut() {
                            u.slots
                                .push(slot("B", false, qsi::update::Attempt::Pending));
                        }
                        "install".to_string()
                    }
                    Some(qsi::any::AnyRequest::Update(qsi::update::Request::Activate(a))) => {
                        format!("activate {:?} {}", a.partitions, a.timeout)
                    }
                    other => format!("{other:?}"),
                };
                self.0.lock().unwrap().push(format!("{what} {name} {user}"));
                ipc::encode(&Ok::<_, mgmtproto::Refusal>(mgmtproto::Answered::default()))
            }
            _ => Vec::new(),
        })
    }
}

fn slot(name: &str, running: bool, last: qsi::update::Attempt) -> qsi::update::Slot {
    qsi::update::Slot {
        name: name.into(),
        release: "V1R0M0".into(),
        build: "b".into(),
        fixes: Vec::new(),
        running,
        default: running,
        last,
        reason: String::new(),
        time: 1,
    }
}

struct Fetch(Fetched);

impl fetchd::proto::Calls for Fetch {
    fn call(&mut self, req: &fetchd::proto::Request) -> Result<Vec<u8>, ipc::WireError> {
        Ok(match req {
            fetchd::proto::Request::Status {} => ipc::encode(&self.0),
            _ => Vec::new(),
        })
    }
}

struct Updd(Log, Arc<Mutex<update::proto::Status>>);

impl update::proto::Calls for Updd {
    fn call(&mut self, req: &update::proto::Request) -> Result<Vec<u8>, ipc::WireError> {
        use update::proto::Request as R;
        let mut st = self.1.lock().unwrap();
        Ok(match req {
            R::Status {} => ipc::encode(&*st),
            R::StageFile { path, .. } => {
                self.0.lock().unwrap().push(format!("stage file {path}"));
                ipc::encode(&Ok::<_, Vec<msg::Issued>>(update::proto::Staged {
                    name: "QSMC-V1R0M1-update-cd.upd".into(),
                    bytes: 4,
                    verification: update::proto::Verification::Verified,
                    ..update::proto::Staged::default()
                }))
            }
            R::Install {} => {
                self.0.lock().unwrap().push("install".into());
                st.slots.push(update::proto::Slot {
                    name: "B".into(),
                    last: update::proto::Attempt::Pending,
                    ..update::proto::Slot::default()
                });
                ipc::encode(&Ok::<String, Vec<msg::Issued>>("B".into()))
            }
            R::Activate { by, .. } => {
                self.0.lock().unwrap().push(format!("activate {by}"));
                ipc::encode(&Ok::<(), Vec<msg::Issued>>(()))
            }
            _ => Vec::new(),
        })
    }
}

fn entry(release: &str, update: &str) -> Entry {
    Entry {
        release: release.into(),
        updates_from: "V1R0M0".into(),
        update: update.into(),
        update_bytes: 4,
        ..Entry::default()
    }
}

fn fetched(downloaded: bool) -> Fetched {
    let known = |stream, e| Known {
        stream,
        sequence: 3,
        entries: vec![e],
        ..Known::default()
    };
    let file = |stream, name: &str| File {
        stream,
        name: name.into(),
        bytes: 4,
        have: 4,
        state: FileState::Verified,
        ..File::default()
    };
    Fetched {
        streams: vec![
            known(Stream::Qsmc, entry("V1R0M1", "QSMC-V1R0M1-update-cd.upd")),
            known(Stream::Qsvs, entry("V1R0M1", "QSVS-V1R0M1-update-ab.upd")),
        ],
        files: if downloaded {
            vec![
                file(Stream::Qsmc, "QSMC-V1R0M1-update-cd.upd"),
                file(Stream::Qsvs, "QSVS-V1R0M1-update-ab.upd"),
            ]
        } else {
            Vec::new()
        },
        ..Fetched::default()
    }
}

fn host() -> Kept {
    Kept {
        name: "QSHQVS1".into(),
        link: Link::Connected,
        caps: qsi::caps::HOST_V1 | qsi::caps::SERVICING,
        state: Some(qsi::base::State {
            power: qsi::base::PowerState::Running,
            refcode: None,
            release: "V1R0M0".into(),
            slot: "A".into(),
            keylock: None,
        }),
        partitions: vec![Part {
            id: 1,
            name: "QSDEV01".into(),
            state: PartitionState::Running,
            code: String::new(),
            text: String::new(),
            current: None,
            profile: String::new(),
            last_end: String::new(),
            message: None,
            ethernet: Vec::new(),
            channel: qsi::host::model::Channel::default(),
        }],
        ..Kept::default()
    }
}

struct World {
    dir: std::path::PathBuf,
    log: Log,
    svc: schedd::Service,
}

fn world(name: &str, downloaded: bool) -> World {
    let dir = std::env::temp_dir().join(format!("schedd-install-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let store = state::Store::at(&dir);
    let mut u = webusers::WebUsers::default();
    u.add(
        "qsadmin",
        "",
        webusers::Role::Administrator,
        "Admin-pass-word-1",
        1,
    )
    .unwrap();
    store.save(&u).unwrap();
    let log: Log = Arc::default();
    let kept = Arc::new(Mutex::new(host()));
    let st = Arc::new(Mutex::new(update::proto::Status::default()));
    let clock = Arc::new(AtomicI64::new(1_793_000_000));
    let (l1, l2) = (log.clone(), log.clone());
    let c = clock.clone();
    let svc = schedd::Service::start(schedd::Config {
        store,
        mgmt: Box::new(move || Box::new(Mgmt(l1.clone(), kept.clone()))),
        fetch: Box::new(move || Box::new(Fetch(fetched(downloaded)))),
        updd: Box::new(move || Box::new(Updd(l2.clone(), st.clone()))),
        console: "QSHQMC1".into(),
        look: schedd::update::NOW,
        clock: Box::new(move || c.load(Ordering::Relaxed)),
        log: Box::new(|_| {}),
    });
    World { dir, log, svc }
}

fn install(system: &str, delay: u32) -> Schedule {
    Schedule {
        op: Op::Update,
        system: system.into(),
        delay,
        first: 1_793_000_000 + 3600,
        repeat: Repeat {
            every: "once".into(),
            ..Repeat::default()
        },
        ..Schedule::default()
    }
}

#[test]
fn a_host_install_runs_with_the_page_s_checks_in_the_owner_s_name() {
    let mut w = world("host", true);
    // Partitions run and the schedule did not say to shut them down.
    let id = w.svc.add(install("QSHQVS1", 0), "qsadmin".into()).unwrap();
    let r = w.svc.run_now(id, "qsadmin".into()).unwrap();
    assert_eq!((r.outcome, r.msgid.as_str()), (Outcome::Failed, "QMC7004"));
    assert!(r.text.contains("QSDEV01"), "{}", r.text);
    assert!(w.log.lock().unwrap().is_empty(), "nothing was asked");
    // Said to shut them down, each given 300 s.
    let id = w
        .svc
        .add(install("QSHQVS1", 300), "qsadmin".into())
        .unwrap();
    let r = w.svc.run_now(id, "qsadmin".into()).unwrap();
    assert_eq!(r.outcome, Outcome::Completed, "{} {}", r.msgid, r.text);
    let log = w.log.lock().unwrap().clone();
    assert_eq!(
        log,
        [
            "stage QSHQVS1 /data/updates/qsvs/QSVS-V1R0M1-update-ab.upd qsadmin",
            "install QSHQVS1 qsadmin",
            "activate ShutDown 300 QSHQVS1 qsadmin",
        ]
    );
    let _ = std::fs::remove_dir_all(&w.dir);
}

#[test]
fn the_console_s_install_runs_and_one_not_downloaded_is_refused() {
    let mut w = world("console", true);
    let id = w.svc.add(install("", 0), "qsadmin".into()).unwrap();
    let r = w.svc.run_now(id, "qsadmin".into()).unwrap();
    assert_eq!(r.outcome, Outcome::Completed, "{} {}", r.msgid, r.text);
    assert_eq!(
        w.log.lock().unwrap().clone(),
        [
            "stage file /data/updates/qsmc/QSMC-V1R0M1-update-cd.upd",
            "install",
            "activate qsadmin",
        ]
    );
    assert_eq!(schedd::service::target(&install("", 0)), "the console");
    let _ = std::fs::remove_dir_all(&w.dir);

    let mut w = world("missing", false);
    for system in ["", "QSHQVS1"] {
        let id = w.svc.add(install(system, 300), "qsadmin".into()).unwrap();
        let r = w.svc.run_now(id, "qsadmin".into()).unwrap();
        assert_eq!((r.outcome, r.msgid.as_str()), (Outcome::Failed, "QMC7003"));
    }
    assert!(w.log.lock().unwrap().is_empty());
    let _ = std::fs::remove_dir_all(&w.dir);
}
