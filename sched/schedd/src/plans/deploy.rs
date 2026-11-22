// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! A deployment: validated again when it is started, its steps run one
//! by one on a thread of its own through `mgmtd` in the deploying
//! user's name, one job at a time a host. The job is written to
//! `deploy/<HOST>.toml` before and after each step; Cancel and a step
//! the host refuses stop it before its next. When the scheduler starts,
//! a job left running is stopped and its step marked interrupted.

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use mgmtproto::{Asked, Calls};
use msg::Issued;
use plan::deploy::{self, Action};
use plan::job::{Job, JobState, Record, Step, StepState};
use plan::validate::{Item, validate};
use qsi::any::{AnyReply, AnyRequest};
use qsi::host::partition;

use super::{Plans, issue, lock};
use crate::service::MakeMgmt;

/// How many looks at a thick disk's writing at most: two hours at the
/// usual look.
const LOOKS: u32 = 3600;

/// The deployments.
#[derive(Clone)]
pub struct Deployer {
    plans: Plans,
    dir: PathBuf,
    mgmt: Arc<MakeMgmt>,
    clock: Arc<dyn Fn() -> i64 + Send + Sync>,
    look: Duration,
    running: Arc<Mutex<BTreeSet<String>>>,
    files: Arc<Mutex<()>>,
}

/// Whether `t` may name a host's file: a managed system's name.
fn target_ok(t: &str) -> bool {
    !t.is_empty()
        && t.len() <= 64
        && t.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

fn refused(r: &mgmtproto::Refusal) -> String {
    r.notes()
        .iter()
        .map(|n| format!("{} {}", n.msgid, n.text).trim().to_string())
        .collect::<Vec<_>>()
        .join("; ")
}

impl Deployer {
    /// Deployments of `plans`, through `mgmt`, timed by `clock`, `look`
    /// between looks at a disk being written.
    pub fn new(
        plans: Plans,
        mgmt: Arc<MakeMgmt>,
        clock: Arc<dyn Fn() -> i64 + Send + Sync>,
        look: Duration,
    ) -> Self {
        let dir = plans.dir().join("deploy");
        Self {
            plans,
            dir,
            mgmt,
            clock,
            look,
            running: Arc::default(),
            files: Arc::default(),
        }
    }

    fn now(&self) -> u64 {
        u64::try_from((self.clock)()).unwrap_or(0)
    }

    fn path(&self, target: &str) -> PathBuf {
        self.dir.join(format!("{target}.toml"))
    }

    /// What deployments to `target` keep.
    pub fn record(&self, target: &str) -> Record {
        if !target_ok(target) {
            return Record::default();
        }
        std::fs::read_to_string(self.path(target))
            .ok()
            .and_then(|t| state::store::parse::<Record>(&t).ok())
            .unwrap_or_default()
    }

    fn save(&self, target: &str, r: &Record) {
        let written = state::store::render(r)
            .map_err(|e| e.to_string())
            .and_then(|t| {
                std::fs::create_dir_all(&self.dir)
                    .and_then(|()| state::atomic_write(&self.path(target), t.as_bytes()))
                    .map_err(|e| e.to_string())
            });
        if let Err(e) = written {
            eprintln!("the deployment to {target} was not written: {e}");
        }
    }

    /// Change `target`'s record and write it.
    fn update(&self, target: &str, f: impl FnOnce(&mut Record)) -> Record {
        let _g = lock(&self.files);
        let mut r = self.record(target);
        f(&mut r);
        self.save(target, &r);
        r
    }

    /// The latest job of `target`.
    pub fn job(&self, target: &str) -> Option<Job> {
        self.record(target).job
    }

    /// Jobs a restart left running: stopped, the step under way marked
    /// interrupted.
    pub fn recover(&self) {
        let Ok(rd) = std::fs::read_dir(&self.dir) else {
            return;
        };
        let now = self.now();
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            let Some(target) = name.strip_suffix(".toml").filter(|t| target_ok(t)) else {
                continue;
            };
            let r = self.record(target);
            if !r.job.as_ref().is_some_and(|j| j.state == JobState::Running) {
                continue;
            }
            self.update(target, |r| {
                if let Some(j) = &mut r.job {
                    for s in &mut j.steps {
                        if s.state == StepState::Going {
                            s.state = StepState::Interrupted;
                            s.ended = now;
                            "The console restarted during this step.".clone_into(&mut s.message);
                        }
                    }
                    j.state = JobState::Stopped;
                    j.ended = now;
                }
            });
            eprintln!(
                "audit: user {} from the scheduler: deploy system plan {} to {target}: stopped, the console restarted",
                r.job.as_ref().map_or("", |j| j.user.as_str()),
                r.job.as_ref().map_or("", |j| j.plan.as_str())
            );
        }
    }

    /// Plan `name` validated against `target` for `user`, the chosen
    /// partitions or every one.
    pub fn validate(
        &self,
        name: &str,
        target: &str,
        chosen: Option<&[u32]>,
        user: &str,
    ) -> Result<(plan::Plan, plan::validate::Validation), Issued> {
        if !target_ok(target) {
            return Err(issue(&qmcmsg::NO_SUCH_SYSTEM, &[target]));
        }
        let plan = self
            .plans
            .get(name)
            .ok_or_else(|| issue(&qmcmsg::NO_SUCH_PLAN, &[name]))?;
        let mut m = (self.mgmt)();
        let v = plan::view::read::host(&mut *m, target, user).map_err(|e| match e {
            plan::view::read::ReadError::NoSystem => issue(&qmcmsg::NO_SUCH_SYSTEM, &[target]),
            plan::view::read::ReadError::Refused(r) => Issued {
                id: r.main.msgid,
                text: r.main.text,
                help: r.main.help,
            },
            e @ plan::view::read::ReadError::Down(_) => {
                issue(&qmcmsg::MGMT_DOWN, &[&e.to_string()])
            }
        })?;
        let made = self.record(target).made;
        Ok((plan.clone(), validate(&plan, &v, &made, chosen)))
    }

    /// The rows of [`Deployer::validate`].
    pub fn items(
        &self,
        name: &str,
        target: &str,
        chosen: Option<&[u32]>,
        user: &str,
    ) -> Result<Vec<Item>, Issued> {
        self.validate(name, target, chosen, user)
            .map(|(_, v)| v.items)
    }

    /// Deploy plan `name` to `target` for `user`: the partitions
    /// `chosen`, activated after with `activate`. Begun on a thread.
    pub fn start(
        &self,
        name: &str,
        target: &str,
        chosen: &[u32],
        activate: bool,
        user: &str,
    ) -> Result<(), Issued> {
        if !lock(&self.running).insert(target.to_string()) {
            return Err(issue(&qmcmsg::DEPLOY_RUNNING, &[target]));
        }
        let begun = self.begin(name, target, chosen, activate, user);
        if begun.is_err() {
            lock(&self.running).remove(target);
            return begun;
        }
        eprintln!(
            "audit: user {user} from the web interface: deploy system plan {name} to {target}: begun"
        );
        let me = self.clone();
        let target = target.to_string();
        std::thread::spawn(move || {
            me.run(&target);
            lock(&me.running).remove(&target);
        });
        Ok(())
    }

    fn begin(
        &self,
        name: &str,
        target: &str,
        chosen: &[u32],
        activate: bool,
        user: &str,
    ) -> Result<(), Issued> {
        let (plan, v) = self.validate(name, target, Some(chosen), user)?;
        if v.refused() {
            let why = v.items.first().map_or("", |i| i.message.as_str());
            return Err(issue(&qmcmsg::DEPLOY_REFUSED, &[name, target, why]));
        }
        let steps = deploy::steps(&plan, &v, chosen, activate)
            .into_iter()
            .map(|p| Step {
                state: if p.action.is_some() {
                    StepState::Pending
                } else {
                    StepState::Skipped
                },
                what: p.what,
                action: p.action,
                message: p.skipped,
                ..Step::default()
            })
            .collect();
        let job = Job {
            plan: name.to_string(),
            target: target.to_string(),
            user: user.to_string(),
            began: self.now(),
            partitions: chosen.to_vec(),
            activate,
            steps,
            ..Job::default()
        };
        self.update(target, |r| r.job = Some(job));
        Ok(())
    }

    /// Ask Cancel of `target`'s job: it stops before its next step.
    pub fn cancel(&self, target: &str, user: &str) -> Result<(), Issued> {
        let r = self.record(target);
        let Some(j) = r.job.filter(|j| j.state == JobState::Running) else {
            return Err(issue(&qmcmsg::NO_DEPLOYMENT, &[target]));
        };
        self.update(target, |r| {
            if let Some(j) = &mut r.job {
                j.cancel = true;
            }
        });
        eprintln!(
            "audit: user {user} from the web interface: cancel the deployment of system plan {} to {target}: asked",
            j.plan
        );
        Ok(())
    }

    fn run(&self, target: &str) {
        loop {
            let r = self.record(target);
            let Some(job) = r.job else {
                return;
            };
            let next = job.steps.iter().position(|s| s.state == StepState::Pending);
            let (Some(i), false) = (next, job.cancel) else {
                let now = self.now();
                let state = if job.cancel {
                    JobState::Cancelled
                } else {
                    JobState::Done
                };
                self.update(target, |r| {
                    if let Some(j) = &mut r.job {
                        j.state = state;
                        j.ended = now;
                    }
                });
                eprintln!(
                    "audit: user {} from the web interface: deploy system plan {} to {target}: {}",
                    job.user,
                    job.plan,
                    if job.cancel { "cancelled" } else { "done" }
                );
                return;
            };
            let now = self.now();
            self.update(target, |r| {
                if let Some(j) = &mut r.job {
                    j.steps[i].state = StepState::Going;
                    j.steps[i].started = now;
                }
            });
            let step = &job.steps[i];
            let done = step
                .action
                .as_ref()
                .map_or(Ok(String::new()), |a| self.act(target, &job.user, i, a));
            let ended = self.now();
            let failed = done.is_err();
            eprintln!(
                "audit: user {} from deployment of system plan {}: {} on {target}: {}",
                job.user,
                job.plan,
                step.what,
                match &done {
                    Ok(_) => "complete".to_string(),
                    Err(e) => format!("refused: {e}"),
                }
            );
            self.update(target, |r| {
                if let Some(j) = &mut r.job {
                    let s = &mut j.steps[i];
                    s.ended = ended;
                    match done {
                        Ok(m) => {
                            s.state = StepState::Complete;
                            s.message = m;
                        }
                        Err(e) => {
                            s.state = StepState::Failed;
                            s.message = e;
                            j.state = JobState::Failed;
                            j.ended = ended;
                        }
                    }
                }
            });
            if failed {
                return;
            }
        }
    }

    fn ask(
        m: &mut dyn Calls,
        target: &str,
        user: &str,
        req: &AnyRequest,
    ) -> Result<AnyReply, String> {
        let a = m
            .ask(target.to_string(), Asked::of(req), user.to_string())
            .map_err(|e| format!("the managed-systems service did not answer: {e}"))?
            .map_err(|r| refused(&r))?;
        a.reply()
            .ok_or_else(|| "the host's reply did not read".to_string())
    }

    /// Carry out `step`'s action: what it says when done, or the
    /// host's messages.
    fn act(
        &self,
        target: &str,
        user: &str,
        step: usize,
        action: &Action,
    ) -> Result<String, String> {
        let mut made_calls = (self.mgmt)();
        let calls = &mut *made_calls;
        match action {
            Action::Partition {
                create,
                partition: p,
                put,
            } => {
                let mut made = None;
                if *create {
                    match Self::ask(calls, target, user, &deploy::create(p))? {
                        AnyReply::Partition(partition::Reply::CreateWith(d)) => made = Some(d),
                        _ => return Err("the host answered with another reply".into()),
                    }
                    let key = format!("{} {}", p.id, p.name);
                    self.update(target, |r| {
                        if !r.made.contains(&key) {
                            r.made.push(key);
                        }
                    });
                }
                for name in put {
                    let req = deploy::put(p, name).ok_or("a profile of the plan does not read")?;
                    Self::ask(calls, target, user, &req)?;
                }
                let d = match made {
                    Some(d) => d,
                    None => {
                        calls
                            .partition(target.to_string(), p.id, user.to_string())
                            .map_err(|e| {
                                format!("the managed-systems service did not answer: {e}")
                            })?
                            .map_err(|r| refused(&r))?
                            .definition
                    }
                };
                Self::ask(calls, target, user, &deploy::define(&d, p))?;
                Ok(format!(
                    "{} profile{} put",
                    put.len(),
                    if put.len() == 1 { "" } else { "s" }
                ))
            }
            Action::Activate { id, profile, .. } => {
                Self::ask(calls, target, user, &deploy::activation(*id, profile))?;
                Ok(String::new())
            }
            Action::Switch { .. } => {
                let req = deploy::request(action).ok_or("not a request")?;
                Self::ask(calls, target, user, &req)?;
                if let Some(up) = deploy::uplink(action) {
                    Self::ask(calls, target, user, &up)?;
                }
                Ok(String::new())
            }
            Action::Vdisk {
                pool, name, thick, ..
            } => {
                let req = deploy::request(action).ok_or("not a request")?;
                Self::ask(calls, target, user, &req)?;
                if *thick {
                    self.written(calls, target, step, pool, name);
                }
                Ok(String::new())
            }
            _ => {
                let req = deploy::request(action).ok_or("not a request")?;
                Self::ask(calls, target, user, &req)?;
                Ok(String::new())
            }
        }
    }

    /// Show a thick disk's writing as the host reports it, until it is
    /// written whole or the host says no more.
    fn written(&self, m: &mut dyn Calls, target: &str, i: usize, pool: &str, name: &str) {
        for _ in 0..LOOKS {
            let Ok(Some(k)) = m.system(target.to_string()) else {
                return;
            };
            let Some(d) = k
                .resources
                .vdisks
                .iter()
                .find(|d| d.pool == pool && d.name == name)
            else {
                return;
            };
            if d.bytes == 0 || d.allocated >= d.bytes {
                return;
            }
            let pct = d.allocated * 100 / d.bytes;
            self.update(target, |r| {
                if let Some(j) = &mut r.job {
                    j.steps[i].message = format!("{pct}% written");
                }
            });
            if self.look.is_zero() {
                return;
            }
            std::thread::sleep(self.look);
        }
    }
}
