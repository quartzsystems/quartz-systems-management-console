// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! The scheduler against a managed-systems service in memory: a run at
//! its time in its owner's name, an owner whose role was lowered
//! refused, a run missed while the service was down, Run now.

use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{Arc, Mutex};

use schedd::{Op, Outcome, Repeat, Sched as _, Schedule};

/// What the fake `mgmtd` was asked: (system, family, op, user).
type Asked = Arc<Mutex<Vec<(String, u16, u16, String)>>>;

struct Fake(Asked);

impl mgmtproto::Calls for Fake {
    fn call(&mut self, req: &mgmtproto::Request) -> Result<Vec<u8>, ipc::WireError> {
        Ok(match req {
            mgmtproto::Request::Ask { name, req, user } => {
                self.0
                    .lock()
                    .unwrap()
                    .push((name.clone(), req.family, req.op, user.clone()));
                ipc::encode(&Ok::<_, mgmtproto::Refusal>(mgmtproto::Answered::default()))
            }
            mgmtproto::Request::Power { name, user, .. } => {
                self.0
                    .lock()
                    .unwrap()
                    .push((name.clone(), 1, 0, user.clone()));
                ipc::encode(&Err::<(), _>(mgmtproto::Refusal::new(
                    "QVS1010",
                    "The host is busy.",
                )))
            }
            _ => Vec::new(),
        })
    }
}

fn dir(name: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("schedd-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn users(store: &state::Store, role: webusers::Role) {
    let mut u = webusers::WebUsers::default();
    // The file holds an enabled Administrator always.
    u.add(
        "qsadmin",
        "",
        webusers::Role::Administrator,
        "Admin-pass-word-1",
        1,
    )
    .unwrap();
    if let Err(e) = u.add("operator1", "", role, "Op-pass-word-1", 1) {
        panic!("{e}");
    }
    store.save(&u).unwrap();
}

fn service(d: &std::path::Path, clock: &Arc<AtomicI64>, asked: &Asked) -> schedd::Service {
    let c = clock.clone();
    let a = asked.clone();
    schedd::Service::start(schedd::Config {
        store: state::Store::at(d),
        mgmt: Box::new(move || Box::new(Fake(a.clone()))),
        clock: Box::new(move || c.load(Ordering::Relaxed)),
        log: Box::new(|l| eprintln!("schedd: {l}")),
    })
}

fn daily_activate(first: i64) -> Schedule {
    Schedule {
        op: Op::Activate,
        system: "QSHQVS1".into(),
        partition: 2,
        partition_name: "QSTEST01".into(),
        profile: "install".into(),
        first,
        repeat: Repeat {
            every: "daily".into(),
            ..Repeat::default()
        },
        ..Schedule::default()
    }
}

#[test]
fn a_run_at_its_time_in_its_owner_s_name_then_refused_when_the_role_is_lowered() {
    let d = dir("run");
    let store = state::Store::at(&d);
    users(&store, webusers::Role::Operator);
    let t0 = 1_793_000_000;
    let clock = Arc::new(AtomicI64::new(t0));
    let asked: Asked = Arc::default();
    let mut svc = service(&d, &clock, &asked);
    let id = svc
        .add(daily_activate(t0 + 60), "operator1".into())
        .unwrap();
    svc.tick();
    assert!(asked.lock().unwrap().is_empty(), "nothing is due yet");
    clock.store(t0 + 61, Ordering::Relaxed);
    svc.tick();
    {
        let a = asked.lock().unwrap();
        assert_eq!(a.len(), 1);
        assert_eq!(a[0], ("QSHQVS1".into(), 0x0012, 0x0004, "operator1".into()));
    }
    let l = svc.list();
    assert_eq!(l.runs.len(), 1);
    assert_eq!(l.runs[0].outcome, Outcome::Completed);
    assert_eq!(l.schedules[0].next, t0 + 60 + 86_400);

    // The role lowered: the next run is refused, nothing asked.
    users(&store, webusers::Role::Viewer);
    clock.store(t0 + 60 + 86_400 + 1, Ordering::Relaxed);
    svc.tick();
    assert_eq!(asked.lock().unwrap().len(), 1);
    let l = svc.list();
    assert_eq!(l.runs.last().unwrap().outcome, Outcome::Refused);
    assert_eq!(l.runs.last().unwrap().msgid, "QMC3406");

    // Run now: as its owner, checked again.
    users(&store, webusers::Role::Operator);
    let r = svc.run_now(id, "qsadmin".into()).unwrap();
    assert!(r.now && r.outcome == Outcome::Completed && r.owner == "operator1");
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn a_run_missed_while_the_service_was_down_is_not_run_late() {
    let d = dir("missed");
    let store = state::Store::at(&d);
    users(&store, webusers::Role::Operator);
    let t0 = 1_793_000_000;
    let clock = Arc::new(AtomicI64::new(t0));
    let asked: Asked = Arc::default();
    {
        let mut svc = service(&d, &clock, &asked);
        svc.add(daily_activate(t0 + 60), "operator1".into())
            .unwrap();
    }
    // Down across its time: started again an hour after it.
    clock.store(t0 + 3600, Ordering::Relaxed);
    let svc = service(&d, &clock, &asked);
    assert!(asked.lock().unwrap().is_empty(), "not run late");
    let l = schedd::Sched::list(&mut svc.clone());
    assert_eq!(l.runs.len(), 1);
    assert_eq!(l.runs[0].outcome, Outcome::Missed);
    assert_eq!(l.schedules[0].next, t0 + 60 + 86_400);
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn a_refusal_keeps_the_system_s_message_and_power_on_is_not_scheduled() {
    let d = dir("refused");
    let store = state::Store::at(&d);
    users(&store, webusers::Role::Operator);
    let t0 = 1_793_000_000;
    let clock = Arc::new(AtomicI64::new(t0));
    let asked: Asked = Arc::default();
    let mut svc = service(&d, &clock, &asked);
    let off = Schedule {
        op: Op::PowerOff,
        system: "QSHQ400".into(),
        first: t0 + 10,
        repeat: Repeat {
            every: "once".into(),
            ..Repeat::default()
        },
        ..Schedule::default()
    };
    svc.add(off, "operator1".into()).unwrap();
    clock.store(t0 + 11, Ordering::Relaxed);
    svc.tick();
    let l = svc.list();
    assert_eq!(l.runs[0].outcome, Outcome::Failed);
    assert_eq!(l.runs[0].msgid, "QVS1010");
    assert_eq!(l.schedules[0].next, 0, "once is done");
    let on = Schedule {
        op: Op::PowerOn,
        system: "QSHQ400".into(),
        first: t0 + 100,
        repeat: Repeat {
            every: "once".into(),
            ..Repeat::default()
        },
        ..Schedule::default()
    };
    assert!(
        svc.add(on, "operator1".into())
            .unwrap_err()
            .contains("QMC2201")
    );
    let _ = std::fs::remove_dir_all(d);
}
