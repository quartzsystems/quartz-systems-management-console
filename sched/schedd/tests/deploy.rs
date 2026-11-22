// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! A plan of the simulator's `QSHQVS1` deployed to its `QSHQVS2`
//! through the scheduler: the steps in order with their states, Errors
//! skipped, a Warning's change, activation, a step the host refuses,
//! Cancel, a restart, a deployment again, and nothing of the host's
//! harmed.

use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

use plan::job::{Job, JobState, StepState};
use schedd::Sched as _;
use sim::direct::Direct;

/// The direct systems, with a gate the first switch's creation waits at.
#[derive(Clone)]
struct Gated {
    d: Direct,
    gate: Arc<(Mutex<bool>, Condvar)>,
}

impl mgmtproto::Calls for Gated {
    fn call(&mut self, req: &mgmtproto::Request) -> Result<Vec<u8>, ipc::WireError> {
        if let mgmtproto::Request::Ask { req: a, .. } = req
            && a.family == qsi::family::VRES
            && a.op == qsi::host::vres::op::SWITCH_CREATE
        {
            let (m, cv) = &*self.gate;
            let mut open = m.lock().unwrap();
            while !*open {
                open = cv.wait(open).unwrap();
            }
        }
        self.d.call(req)
    }
}

struct Lab {
    dir: std::path::PathBuf,
    svc: schedd::Service,
    d: Direct,
    gate: Arc<(Mutex<bool>, Condvar)>,
}

fn open(gate: &Arc<(Mutex<bool>, Condvar)>, to: bool) {
    *gate.0.lock().unwrap() = to;
    gate.1.notify_all();
}

fn service(
    dir: &std::path::Path,
    d: &Direct,
    gate: &Arc<(Mutex<bool>, Condvar)>,
) -> schedd::Service {
    let g = Gated {
        d: d.clone(),
        gate: gate.clone(),
    };
    let clock = Arc::new(AtomicI64::new(1_793_340_000));
    schedd::Service::start(schedd::Config {
        store: state::Store::at(dir.join("state")),
        mgmt: Box::new(move || Box::new(g.clone())),
        fetch: Box::new(|| Box::new(fetchd::proto::Client::new("/nonexistent/fetchd.sock"))),
        updd: Box::new(|| Box::new(update::proto::Client::new("/nonexistent/updd.sock"))),
        console: "QSHQMC1".into(),
        look: schedd::update::NOW,
        clock: Box::new(move || clock.fetch_add(1, Ordering::Relaxed)),
        log: Box::new(|_| {}),
        plans: dir.join("plans"),
    })
}

fn lab(name: &str) -> Lab {
    let dir = std::env::temp_dir().join(format!("schedd-deploy-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let d = Direct::new(vec![sim::fixture::qshqvs1(), sim::fixture::qshqvs2()]);
    let gate = Arc::new((Mutex::new(true), Condvar::new()));
    let mut svc = service(&dir, &d, &gate);
    svc.capture(
        "QSHQVS1".into(),
        "qshqvs1-baseline".into(),
        "QSHQVS1 as built, 3 partitions".into(),
        "qsadmin".into(),
    )
    .unwrap();
    Lab { dir, svc, d, gate }
}

fn ended(svc: &mut schedd::Service, target: &str) -> Job {
    let until = Instant::now() + Duration::from_secs(30);
    loop {
        let j = svc.job(target.into()).and_then(|t| plan::job::read(&t));
        if let Some(j) = j.filter(|j| j.state.ended()) {
            return j;
        }
        assert!(Instant::now() < until, "the deployment did not end");
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn states(j: &Job) -> Vec<(String, StepState)> {
    j.steps.iter().map(|s| (s.what.clone(), s.state)).collect()
}

/// What of `QSHQVS2` a deployment may not touch: its pool, its disks,
/// its library.
fn untouchable(d: &Direct) -> String {
    let h = sim::lock(&d.systems[1]);
    let p = &h.vres.pools[0];
    format!(
        "{} {} {} {:?} {:?} {:?}",
        p.name,
        p.protection,
        p.bytes,
        p.units.iter().map(|u| &u.disk).collect::<Vec<_>>(),
        h.vres
            .disks
            .iter()
            .map(|x| (&x.name, x.foreign, &x.pool))
            .collect::<Vec<_>>(),
        h.vres.images.iter().map(|i| &i.name).collect::<Vec<_>>()
    )
}

#[test]
#[allow(clippy::too_many_lines, reason = "one deployment and what it left")]
fn a_deployment_runs_its_steps_skips_errors_and_activates() {
    let mut l = lab("whole");
    let before = untouchable(&l.d);
    let items = l
        .svc
        .validate(
            "qshqvs1-baseline".into(),
            "QSHQVS2".into(),
            Vec::new(),
            "qsadmin".into(),
        )
        .unwrap();
    assert!(
        items
            .iter()
            .any(|i| i.label == "Storage pool POOL2" && i.status == plan::Status::Error)
    );
    l.svc
        .deploy(
            "qshqvs1-baseline".into(),
            "QSHQVS2".into(),
            vec![1, 2, 3],
            true,
            "qsadmin".into(),
        )
        .unwrap();
    let j = ended(&mut l.svc, "QSHQVS2");
    assert_eq!(j.state, JobState::Done, "{:#?}", states(&j));
    let skipped: Vec<&str> = j
        .steps
        .iter()
        .filter(|s| s.state == StepState::Skipped)
        .map(|s| s.what.as_str())
        .collect();
    assert_eq!(
        skipped,
        [
            "Create storage pool POOL2",
            "Create virtual disk VD05 (2,000 GB, POOL2)"
        ]
    );
    assert!(
        j.steps
            .iter()
            .all(|s| matches!(s.state, StepState::Complete | StepState::Skipped))
    );
    {
        let h = sim::lock(&l.d.systems[1]);
        let names: Vec<&str> = h.vres.switches.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(names, ["ETHSW0", "ETHSW1"]);
        assert!(
            h.vres.switches[0].uplink.is_some(),
            "ETHSW0 with its uplink"
        );
        assert_eq!(h.parts.len(), 3);
        // The Warning's change: QSPROD01 without VD05 and the C3 adapter.
        let prod = h
            .parts
            .iter()
            .find(|p| p.definition.name == "QSPROD01")
            .unwrap();
        let pr = &prod.profiles[0].profile;
        assert!(pr.physical_io.is_empty());
        assert!(
            pr.scsi
                .iter()
                .all(|a| a.luns.iter().all(|x| x.backing != "POOL2/VD05"))
        );
        // The host made its own MACs and serial numbers.
        assert!(prod.definition.serial != "K2M4103");
        assert!(
            pr.ethernet
                .iter()
                .all(|e| e.mac.as_deref().is_some_and(|m| m.starts_with("02:51:54:")))
        );
        // Activated.
        assert!(
            h.parts.iter().all(sim::system::Part::active),
            "every partition activated"
        );
        // The default profiles are the plan's.
        let dev = h
            .parts
            .iter()
            .find(|p| p.definition.name == "QSDEV01")
            .unwrap();
        assert_eq!(dev.definition.default_profile, "default");
        assert_eq!(dev.profiles.len(), 3);
    }
    assert_eq!(
        untouchable(&l.d),
        before,
        "nothing of the host's was changed"
    );
    // Deployed again: what exists is skipped; the activations alone, refused
    // as the partitions run, are not asked: there is nothing to make.
    l.svc
        .deploy(
            "qshqvs1-baseline".into(),
            "QSHQVS2".into(),
            vec![1, 2, 3],
            false,
            "qsadmin".into(),
        )
        .unwrap();
    let again = ended(&mut l.svc, "QSHQVS2");
    assert_eq!(again.state, JobState::Done);
    assert!(
        again.steps.iter().all(|s| s.state == StepState::Skipped),
        "{:#?}",
        states(&again)
    );
    let _ = std::fs::remove_dir_all(&l.dir);
}

#[test]
fn a_refused_step_stops_the_job_and_deploying_again_finishes() {
    let mut l = lab("refused");
    sim::lock(&l.d.systems[1])
        .faults
        .refuse_family
        .push(sim::system::FamilyRefusal {
            family: qsi::family::VRES,
            op: Some(qsi::host::vres::op::VDISK_CREATE),
            id: "QVS3107".into(),
            text: "There is no room in POOL1.".into(),
        });
    l.svc
        .deploy(
            "qshqvs1-baseline".into(),
            "QSHQVS2".into(),
            vec![1, 2],
            false,
            "qsadmin".into(),
        )
        .unwrap();
    let j = ended(&mut l.svc, "QSHQVS2");
    assert_eq!(j.state, JobState::Failed);
    let failed = j
        .steps
        .iter()
        .find(|s| s.state == StepState::Failed)
        .unwrap();
    assert!(
        failed.what.starts_with("Create virtual disk VD03"),
        "{}",
        failed.what
    );
    assert_eq!(failed.message, "QVS3107 There is no room in POOL1.");
    let made = j.made();
    assert!(made.contains(&"Create virtual switch ETHSW0"), "{made:?}");
    assert!(
        j.steps
            .iter()
            .skip_while(|s| s.state != StepState::Failed)
            .skip(1)
            .all(|s| s.state == StepState::Pending)
    );
    sim::lock(&l.d.systems[1]).faults.refuse_family.clear();
    l.svc
        .deploy(
            "qshqvs1-baseline".into(),
            "QSHQVS2".into(),
            vec![1, 2],
            false,
            "qsadmin".into(),
        )
        .unwrap();
    let j = ended(&mut l.svc, "QSHQVS2");
    assert_eq!(j.state, JobState::Done, "{:#?}", states(&j));
    assert!(
        !j.steps
            .iter()
            .any(|s| s.what.starts_with("Create virtual switch")),
        "the switches exist"
    );
    assert_eq!(sim::lock(&l.d.systems[1]).parts.len(), 2);
    let _ = std::fs::remove_dir_all(&l.dir);
}

#[test]
fn cancel_stops_before_the_next_step_and_a_restart_interrupts() {
    let mut l = lab("cancel");
    open(&l.gate, false);
    l.svc
        .deploy(
            "qshqvs1-baseline".into(),
            "QSHQVS2".into(),
            vec![1],
            false,
            "qsadmin".into(),
        )
        .unwrap();
    // A second deployment to the host while one runs is refused.
    let busy = l
        .svc
        .deploy(
            "qshqvs1-baseline".into(),
            "QSHQVS2".into(),
            vec![1],
            false,
            "qsadmin".into(),
        )
        .unwrap_err();
    assert_eq!(busy.id, "QMC6006");
    l.svc.cancel("QSHQVS2".into(), "qsadmin".into()).unwrap();
    open(&l.gate, true);
    let j = ended(&mut l.svc, "QSHQVS2");
    assert_eq!(j.state, JobState::Cancelled);
    assert_eq!(j.made(), ["Create virtual switch ETHSW0"]);
    assert!(
        j.steps[1..]
            .iter()
            .all(|s| matches!(s.state, StepState::Pending | StepState::Skipped))
    );

    // A restart while a step runs: the step interrupted, the job stopped.
    open(&l.gate, false);
    sim::lock(&l.d.systems[1]).vres.switches.clear();
    l.svc
        .deploy(
            "qshqvs1-baseline".into(),
            "QSHQVS2".into(),
            vec![1],
            false,
            "qsadmin".into(),
        )
        .unwrap();
    let path = l.dir.join("plans/deploy/QSHQVS2.toml");
    let until = Instant::now() + Duration::from_secs(10);
    while !std::fs::read_to_string(&path)
        .unwrap()
        .contains("state = \"going\"")
    {
        assert!(Instant::now() < until);
        std::thread::sleep(Duration::from_millis(10));
    }
    let left = std::fs::read_to_string(&path).unwrap();
    let mut again = service(&l.dir.join("copy"), &l.d, &l.gate);
    std::fs::create_dir_all(l.dir.join("copy/plans/deploy")).unwrap();
    std::fs::write(l.dir.join("copy/plans/deploy/QSHQVS2.toml"), &left).unwrap();
    std::fs::copy(
        l.dir.join("plans/qshqvs1-baseline.qmcplan"),
        l.dir.join("copy/plans/qshqvs1-baseline.qmcplan"),
    )
    .unwrap();
    drop(again);
    again = service(&l.dir.join("copy"), &l.d, &l.gate);
    let j = plan::job::read(&again.job("QSHQVS2".into()).unwrap()).unwrap();
    assert_eq!(j.state, JobState::Stopped);
    assert_eq!(j.steps[0].state, StepState::Interrupted);
    open(&l.gate, true);
    let _ = ended(&mut l.svc, "QSHQVS2");
    // Deployed again from the restarted service: it finishes.
    again
        .deploy(
            "qshqvs1-baseline".into(),
            "QSHQVS2".into(),
            vec![1],
            false,
            "qsadmin".into(),
        )
        .unwrap();
    let j = ended(&mut again, "QSHQVS2");
    assert_eq!(j.state, JobState::Done, "{:#?}", states(&j));
    let _ = std::fs::remove_dir_all(&l.dir);
}
