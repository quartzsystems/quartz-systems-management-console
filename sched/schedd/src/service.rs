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

/// What the service is given.
pub struct Config {
    /// The state directory: `schedules.toml`, `webusers.toml`,
    /// `host.toml`.
    pub store: Store,
    /// `mgmtd`.
    pub mgmt: MakeMgmt,
    /// The wall clock, Unix seconds.
    pub clock: Box<dyn Fn() -> i64 + Send + Sync>,
    /// Its log.
    pub log: Box<dyn Fn(&str) + Send + Sync>,
}

struct Inner {
    cfg: Config,
    doc: Mutex<Schedules>,
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
        Op::Update => qmcmsg::NO_SCHEDULED_UPDATE,
        _ => return None,
    };
    Some((d.id, d.text))
}

impl Service {
    /// Start: the file read, what was missed while the service was not
    /// running recorded, the loop begun.
    pub fn start(cfg: Config) -> Self {
        let doc = cfg
            .store
            .load::<Schedules>()
            .ok()
            .flatten()
            .unwrap_or_default();
        let svc = Self(Arc::new(Inner {
            cfg,
            doc: Mutex::new(doc),
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

/// What an operation is done on: `QSTEST01 on QSHQVS1`, `QSHQ400`.
pub fn target(s: &Schedule) -> String {
    if s.op.on_partition() {
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
}
