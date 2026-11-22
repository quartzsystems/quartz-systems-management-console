// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! The scheduler: each second, every operation that is due is run in
//! its owner's name, the owner's role checked against the matrix then,
//! by asking `mgmtd` as a page does. A run whose time passed while the
//! console was down, or the clock jumped past it, is not run late: it
//! is recorded missed and logged. Every run is audited with its owner.

use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use mgmtproto::Asked;
use msg::Issued;
use qsi::any::AnyRequest;
use qsi::host::partition::{self, ShutdownKind};
use state::Store;
use webusers::{Task, WebUsers};

use crate::calls::Sched;
use crate::model::{Op, Outcome, Run, Schedule, Schedules};
use crate::when;

/// How late a run may start and still be run: past this it is missed.
pub const GRACE: i64 = 60;

/// A way to `mgmtd`, made for each run.
pub type MakeMgmt = Box<dyn Fn() -> Box<dyn mgmtproto::Calls + Send> + Send + Sync>;
/// A way to the download service, made for each run.
pub type MakeFetch = Box<dyn Fn() -> Box<dyn fetchd::proto::Calls + Send> + Send + Sync>;
/// A way to the console's update service, made for each run.
pub type MakeUpdd = Box<dyn Fn() -> Box<dyn update::proto::Calls + Send> + Send + Sync>;

/// What the service is given.
pub struct Config {
    /// The state directory: `schedules.toml`, `webusers.toml`,
    /// `host.toml`.
    pub store: Store,
    /// `mgmtd`.
    pub mgmt: MakeMgmt,
    /// The download service: what is downloaded, for an install.
    pub fetch: MakeFetch,
    /// The console's update service, for its own install.
    pub updd: MakeUpdd,
    /// The console's name, which an install of its own update is on.
    pub console: String,
    /// How long an install waits between looks at a step.
    pub look: std::time::Duration,
    /// The wall clock, Unix seconds.
    pub clock: Box<dyn Fn() -> i64 + Send + Sync>,
    /// Its log.
    pub log: Box<dyn Fn(&str) + Send + Sync>,
    /// The system plans' folder, `/data/plans`, the scheduler's alone.
    pub plans: std::path::PathBuf,
}

struct Inner {
    cfg: Config,
    doc: Mutex<Schedules>,
    plans: crate::plans::Plans,
    deployer: crate::plans::deploy::Deployer,
}

/// The scheduler.
#[derive(Clone)]
pub struct Service(Arc<Inner>);

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

/// The task an operation needs of its owner's role.
pub fn task(op: Op) -> Task {
    match op {
        Op::PowerOff | Op::PowerOn => Task::Power,
        Op::Backup => Task::Settings,
        Op::Update => Task::CodeUpdates,
        _ => Task::Activate,
    }
}

/// Why an operation is not scheduled at all: the console does not do it
/// from a schedule.
pub fn not_scheduled(op: Op) -> Option<(&'static str, &'static str)> {
    let d = match op {
        Op::PowerOn => qmcmsg::POWER_ON_NOT_HERE,
        Op::Backup => qmcmsg::NO_SCHEDULED_BACKUP,
        _ => return None,
    };
    Some((d.id, d.text))
}

impl Service {
    /// Start: the file read, what was missed while the service was not
    /// running recorded, the loop begun.
    pub fn start(mut cfg: Config) -> Self {
        let doc = cfg
            .store
            .load::<Schedules>()
            .ok()
            .flatten()
            .unwrap_or_default();
        // The deployments' threads share the ways to mgmtd and the clock.
        let mgmt: Arc<MakeMgmt> = Arc::new(std::mem::replace(
            &mut cfg.mgmt,
            Box::new(|| Box::new(crate::service::Down)),
        ));
        let m = mgmt.clone();
        cfg.mgmt = Box::new(move || m());
        let clock: Arc<dyn Fn() -> i64 + Send + Sync> =
            Arc::from(std::mem::replace(&mut cfg.clock, Box::new(|| 0)));
        let c = clock.clone();
        cfg.clock = Box::new(move || c());
        let plans = crate::plans::Plans::open(&cfg.plans);
        let deployer = crate::plans::deploy::Deployer::new(plans.clone(), mgmt, clock, cfg.look);
        deployer.recover();
        let svc = Self(Arc::new(Inner {
            cfg,
            doc: Mutex::new(doc),
            plans,
            deployer,
        }));
        svc.tick();
        let s = svc.clone();
        std::thread::spawn(move || {
            loop {
                std::thread::sleep(Duration::from_millis(500));
                s.tick();
            }
        });
        svc
    }

    /// A plan of `system` saved before its update by `user`, named for
    /// the system and today's date in the console's zone: its name.
    pub fn save_plan_before(
        &self,
        mgmt: &mut dyn mgmtproto::Calls,
        system: &str,
        described: &str,
        user: &str,
    ) -> Result<String, Issued> {
        let now = self.now();
        let c = appliance::time::civil(now + i64::from(self.zone().minutes) * 60);
        let date = format!("{:04}{:02}{:02}", c.year, c.month, c.day);
        let r = self.0.plans.save_before(
            mgmt,
            system,
            described,
            user,
            u64::try_from(now).unwrap_or(0),
            &date,
        );
        let what = r.as_ref().map_or_else(
            |_| format!("save a system plan of {system} before its update"),
            |n| format!("save system plan {n} of {system} before its update"),
        );
        eprintln!(
            "audit: user {user} from the scheduler: {what}: {}",
            match &r {
                Ok(_) => "done".to_string(),
                Err(e) => format!("refused: {} {}", e.id, e.text),
            }
        );
        r
    }

    fn now(&self) -> i64 {
        (self.0.cfg.clock)()
    }

    fn log(&self, line: &str) {
        (self.0.cfg.log)(line);
    }

    fn zone(&self) -> appliance::time::Zone {
        self.0
            .cfg
            .store
            .load::<state::Host>()
            .ok()
            .flatten()
            .map_or(appliance::time::Zone::UTC, |h| when::zone(&h.time_zone))
    }

    fn save(&self, doc: &Schedules) {
        if let Err(e) = self.0.cfg.store.save(doc) {
            self.log(&format!("schedules.toml was not written: {e}"));
        }
    }

    /// Run or miss whatever is due now.
    pub fn tick(&self) {
        let now = self.now();
        let zone = self.zone();
        let due: Vec<(Schedule, i64)> = lock(&self.0.doc)
            .schedules
            .iter()
            .filter(|s| s.next != 0 && s.next <= now)
            .map(|s| (s.clone(), s.next))
            .collect();
        for (s, at) in due {
            let run = if now - at > GRACE {
                self.log(&format!(
                    "scheduled operation {} ({}) was due at {at} and missed: it is not run late",
                    s.id,
                    s.op.words()
                ));
                Self::audit(&s, "missed");
                Run {
                    id: s.id,
                    due: at,
                    at: now,
                    outcome: Outcome::Missed,
                    owner: s.owner.clone(),
                    ..Run::default()
                }
            } else {
                self.run(&s, at, false)
            };
            let mut doc = lock(&self.0.doc);
            doc.record(run);
            if let Some(x) = doc.schedules.iter_mut().find(|x| x.id == s.id) {
                x.next = when::next(x, zone, now.max(at)).unwrap_or(0);
            }
            let snapshot = doc.clone();
            drop(doc);
            self.save(&snapshot);
        }
    }

    fn audit(s: &Schedule, outcome: &str) {
        eprintln!(
            "audit: user {} from schedule {}: {} {}: {outcome}",
            s.owner,
            s.id,
            s.op.words(),
            target(s)
        );
    }

    /// Run `s`, due at `due`, as its owner.
    pub fn run(&self, s: &Schedule, due: i64, now: bool) -> Run {
        let mut run = Run {
            id: s.id,
            due,
            at: self.now(),
            owner: s.owner.clone(),
            now,
            ..Run::default()
        };
        let fail = |run: &mut Run, outcome, msgid: &str, text: &str| {
            run.outcome = outcome;
            run.msgid = msgid.into();
            run.text = text.into();
        };
        // The owner's role, as it is now.
        let users = self
            .0
            .cfg
            .store
            .load::<WebUsers>()
            .ok()
            .flatten()
            .unwrap_or_default();
        let allowed = users
            .get(&s.owner)
            .is_some_and(|u| u.enabled && users.matrix.allows(u.role, task(s.op)));
        if !allowed {
            let text = qsweb::msg::substitute(
                qmcmsg::OWNER_MAY_NOT.text,
                &[&s.owner, &task(s.op).label().to_lowercase()],
            );
            fail(&mut run, Outcome::Refused, qmcmsg::OWNER_MAY_NOT.id, &text);
            Self::audit(s, "refused: the owner's role does not have the task");
            return run;
        }
        if let Some((id, text)) = not_scheduled(s.op) {
            fail(&mut run, Outcome::Failed, id, text);
            Self::audit(s, "refused");
            return run;
        }
        let mut mgmt = (self.0.cfg.mgmt)();
        if s.op == Op::Update {
            let mut fetch = (self.0.cfg.fetch)();
            let mut updd = (self.0.cfg.updd)();
            let mut saving = (self.0.cfg.mgmt)();
            let (system, owner) = (s.system.clone(), s.owner.clone());
            let mut plans =
                |described: &str| self.save_plan_before(&mut *saving, &system, described, &owner);
            let mut reach = crate::update::Reach {
                mgmt: &mut *mgmt,
                fetch: &mut *fetch,
                updd: &mut *updd,
                look: self.0.cfg.look,
                plans: &mut plans,
            };
            // The same checks as the page's, at the run's time.
            let done = if s.system.is_empty() {
                crate::update::install_console(&mut reach, &self.0.cfg.console, &s.owner)
            } else {
                crate::update::install_host(&mut reach, &s.system, &s.owner, s.delay)
            };
            match done {
                Ok(what) => {
                    run.text.clone_from(&what);
                    Self::audit(s, "done");
                    self.log(&format!("scheduled operation {} ran: {what}", s.id));
                }
                Err(no) => {
                    fail(&mut run, Outcome::Failed, &no.id, &no.text);
                    Self::audit(s, &format!("refused: {} {}", no.id, no.text));
                }
            }
            return run;
        }
        let answer = if s.op == Op::PowerOff {
            mgmt.power(s.system.clone(), false, false, s.delay, s.owner.clone())
        } else {
            let req = request(s);
            mgmt.ask(s.system.clone(), Asked::of(&req), s.owner.clone())
                .map(|r| r.map(|_| ()))
        };
        match answer {
            Ok(Ok(())) => {
                Self::audit(s, "done");
                self.log(&format!(
                    "scheduled operation {} ran: {}",
                    s.id,
                    s.op.words()
                ));
            }
            Ok(Err(no)) => {
                fail(&mut run, Outcome::Failed, &no.main.msgid, &no.main.text);
                Self::audit(s, &format!("refused: {} {}", no.main.msgid, no.main.text));
            }
            Err(e) => {
                let d = qmcmsg::MGMT_DOWN;
                let text = qsweb::msg::substitute(d.text, &[&e.to_string()]);
                fail(&mut run, Outcome::Failed, d.id, &text);
                Self::audit(s, "refused: the managed-systems service did not answer");
            }
        }
        run
    }
}

/// What an operation is done on: `QSTEST01 on QSHQVS1`, `QSHQ400`, and
/// `the console` for an install of its own update (no system named).
pub fn target(s: &Schedule) -> String {
    if s.op == Op::Update && s.system.is_empty() {
        "the console".into()
    } else if s.op.on_partition() {
        format!("{} on {}", s.partition_name, s.system)
    } else {
        s.system.clone()
    }
}

/// The partition family's request of an operation on a partition.
pub fn request(s: &Schedule) -> AnyRequest {
    let id = s.partition;
    AnyRequest::Partition(match s.op {
        Op::Activate => partition::Request::Activate(partition::Activate {
            id,
            profile: s.profile.clone(),
            source: None,
            keylock: None,
        }),
        Op::Restart => partition::Request::Restart(partition::Restart { id }),
        Op::ShutDownImmediate => partition::Request::ShutDown(partition::ShutDown {
            id,
            kind: ShutdownKind::Immediate,
            delay: 0,
            restart: false,
        }),
        _ => partition::Request::ShutDown(partition::ShutDown {
            id,
            kind: ShutdownKind::Delayed,
            delay: s.delay.min(99_999),
            restart: false,
        }),
    })
}

impl Sched for Service {
    fn list(&mut self) -> Schedules {
        lock(&self.0.doc).clone()
    }

    fn add(&mut self, mut s: Schedule, user: String) -> Result<u32, String> {
        if let Some((id, text)) = not_scheduled(s.op) {
            return Err(format!("{id} {text}"));
        }
        let now = self.now();
        s.owner = user;
        s.next =
            when::next(&s, self.zone(), now - 1).ok_or("That time is past: it is never due.")?;
        let mut doc = lock(&self.0.doc);
        doc.next_id = doc
            .next_id
            .max(doc.schedules.iter().map(|x| x.id).max().unwrap_or(0))
            + 1;
        s.id = doc.next_id;
        let id = s.id;
        doc.schedules.push(s.clone());
        let snapshot = doc.clone();
        drop(doc);
        self.save(&snapshot);
        Self::audit(&s, "scheduled");
        Ok(id)
    }

    fn change(&mut self, mut s: Schedule, user: String) -> Result<(), String> {
        if let Some((id, text)) = not_scheduled(s.op) {
            return Err(format!("{id} {text}"));
        }
        let now = self.now();
        s.owner = user;
        s.next =
            when::next(&s, self.zone(), now - 1).ok_or("That time is past: it is never due.")?;
        let mut doc = lock(&self.0.doc);
        let Some(x) = doc.schedules.iter_mut().find(|x| x.id == s.id) else {
            return Err(format!("There is no scheduled operation {}.", s.id));
        };
        *x = s.clone();
        let snapshot = doc.clone();
        drop(doc);
        self.save(&snapshot);
        Self::audit(&s, "changed");
        Ok(())
    }

    fn remove(&mut self, id: u32, user: String) -> Result<(), String> {
        let mut doc = lock(&self.0.doc);
        let Some(i) = doc.schedules.iter().position(|x| x.id == id) else {
            return Err(format!("There is no scheduled operation {id}."));
        };
        let s = doc.schedules.remove(i);
        let snapshot = doc.clone();
        drop(doc);
        self.save(&snapshot);
        eprintln!(
            "audit: user {user} from the web interface: remove scheduled operation {id} ({}): removed",
            s.op.words()
        );
        Ok(())
    }

    fn run_now(&mut self, id: u32, user: String) -> Result<Run, String> {
        let Some(s) = lock(&self.0.doc)
            .schedules
            .iter()
            .find(|x| x.id == id)
            .cloned()
        else {
            return Err(format!("There is no scheduled operation {id}."));
        };
        eprintln!(
            "audit: user {user} from the web interface: run scheduled operation {id} now as {}",
            s.owner
        );
        let run = self.run(&s, self.now(), true);
        let mut doc = lock(&self.0.doc);
        doc.record(run.clone());
        let snapshot = doc.clone();
        drop(doc);
        self.save(&snapshot);
        Ok(run)
    }

    fn plans(&mut self) -> (Vec<plan::index::Entry>, u32) {
        let i = self.0.plans.list();
        (i.plans, i.automatic_kept)
    }

    fn plan(&mut self, name: String) -> Option<String> {
        self.0.plans.text(&name)
    }

    fn capture(
        &mut self,
        system: String,
        name: String,
        description: String,
        user: String,
    ) -> Result<plan::index::Entry, Issued> {
        let mut m = (self.0.cfg.mgmt)();
        let meta = plan::capture::Meta {
            name: name.clone(),
            description,
            created: u64::try_from(self.now()).unwrap_or(0),
            by: user.clone(),
            automatic: false,
        };
        let r = self.0.plans.capture(&mut *m, &system, meta);
        plan_audit(
            &user,
            &format!("create system plan {name} from {system}"),
            &r,
        );
        r
    }

    fn import(&mut self, bytes: Vec<u8>, user: String) -> Result<plan::index::Entry, Issued> {
        let r = self.0.plans.import(&bytes);
        let what = r.as_ref().map_or_else(
            |_| format!("import a system plan of {} bytes", bytes.len()),
            |e| format!("import system plan {}", e.name),
        );
        plan_audit(&user, &what, &r);
        r
    }

    fn delete_plan(&mut self, name: String, user: String) -> Result<(), Issued> {
        let r = self.0.plans.delete(&name);
        plan_audit(&user, &format!("delete system plan {name}"), &r);
        r
    }

    fn describe_plan(
        &mut self,
        name: String,
        description: String,
        user: String,
    ) -> Result<(), Issued> {
        let r = self.0.plans.describe(&name, &description);
        plan_audit(&user, &format!("describe system plan {name}"), &r);
        r
    }

    fn template(
        &mut self,
        name: String,
        to: String,
        user: String,
    ) -> Result<plan::index::Entry, Issued> {
        let now = u64::try_from(self.now()).unwrap_or(0);
        let r = self.0.plans.template(&name, &to, &user, now);
        plan_audit(
            &user,
            &format!("save system plan {name} as template {to}"),
            &r,
        );
        r
    }

    fn validate(
        &mut self,
        name: String,
        target: String,
        chosen: Vec<u32>,
        user: String,
    ) -> Result<Vec<plan::validate::Item>, Issued> {
        let chosen = (!chosen.is_empty()).then_some(chosen.as_slice());
        self.0.deployer.items(&name, &target, chosen, &user)
    }

    fn deploy(
        &mut self,
        name: String,
        target: String,
        chosen: Vec<u32>,
        activate: bool,
        user: String,
    ) -> Result<(), Issued> {
        let r = self
            .0
            .deployer
            .start(&name, &target, &chosen, activate, &user);
        if let Err(e) = &r {
            eprintln!(
                "audit: user {user} from the web interface: deploy system plan {name} to {target}: refused: {} {}",
                e.id, e.text
            );
        }
        r
    }

    fn job(&mut self, target: String) -> Option<String> {
        self.0.deployer.job(&target).map(|j| plan::job::write(&j))
    }

    fn cancel(&mut self, target: String, user: String) -> Result<(), Issued> {
        self.0.deployer.cancel(&target, &user)
    }

    fn save_before(
        &mut self,
        system: String,
        description: String,
        user: String,
    ) -> Result<String, Issued> {
        let mut m = (self.0.cfg.mgmt)();
        self.save_plan_before(&mut *m, &system, &description, &user)
    }
}

/// An audit line of a plan's request for `user`.
fn plan_audit<T>(user: &str, what: &str, r: &Result<T, Issued>) {
    let outcome = match r {
        Ok(_) => "done".to_string(),
        Err(e) => format!("refused: {} {}", e.id, e.text),
    };
    eprintln!("audit: user {user} from the web interface: {what}: {outcome}");
}

/// No way to `mgmtd`: every call fails.
pub struct Down;

impl mgmtproto::Calls for Down {
    fn call(&mut self, _req: &mgmtproto::Request) -> Result<Vec<u8>, ipc::WireError> {
        Err(std::io::Error::new(
            std::io::ErrorKind::NotConnected,
            "the managed-systems service is not reached",
        )
        .into())
    }
}
