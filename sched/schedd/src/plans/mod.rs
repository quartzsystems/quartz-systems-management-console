// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! System plans' files (`docs/PLANS.md`): the scheduler alone writes
//! `/data/plans/`, each file whole, at the web server's request or its
//! own before a host's update. The index `plans.toml` is a row a plan;
//! each plan is `<name>.qmcplan`, its document as exported.
//!
//! - [`deploy`]: a deployment run as a job, its steps kept on disk.

pub mod deploy;

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use msg::Issued;
use plan::capture::{Meta, capture};
use plan::doc::{EXTENSION, Plan, check, text};
use plan::index::{Entry, Index};

fn issue(d: &qmcmsg::Def, values: &[&str]) -> Issued {
    qmcmsg::base_of(d).with(values)
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

/// The plans' folder and its index.
#[derive(Clone)]
pub struct Plans {
    dir: PathBuf,
    index: Arc<Mutex<Index>>,
}

impl Plans {
    /// The plans in `dir`: the index read, or made from the files there
    /// when it is not.
    pub fn open(dir: &Path) -> Self {
        let store = state::Store::at(dir);
        let index = match store.load::<Index>() {
            Ok(Some(i)) => i,
            _ => Self::rebuild(dir),
        };
        Self {
            dir: dir.to_path_buf(),
            index: Arc::new(Mutex::new(index)),
        }
    }

    /// An index of every plan file in `dir` that reads.
    fn rebuild(dir: &Path) -> Index {
        let mut i = Index::default();
        let Ok(rd) = std::fs::read_dir(dir) else {
            return i;
        };
        for e in rd.flatten() {
            let p = e.path();
            if p.extension().and_then(|x| x.to_str()) != Some(EXTENSION) {
                continue;
            }
            if let Ok(bytes) = std::fs::read(&p)
                && let Ok(plan) = text::read(&bytes)
            {
                i.plans.push(Entry::of(&plan, bytes.len()));
            }
        }
        i.plans.sort_by(|a, b| a.name.cmp(&b.name));
        i
    }

    /// The folder.
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    fn file(&self, name: &str) -> PathBuf {
        self.dir.join(format!("{name}.{EXTENSION}"))
    }

    fn save_index(&self, i: &Index) -> Result<(), Issued> {
        state::Store::at(&self.dir)
            .save(i)
            .map_err(|e| issue(&qmcmsg::PLAN_NOT_SAVED, &["plans.toml", &e.to_string()]))
    }

    /// Every plan's row, by name.
    pub fn list(&self) -> Index {
        lock(&self.index).clone()
    }

    /// A plan's text.
    pub fn text(&self, name: &str) -> Option<String> {
        if !check::plan_name_ok(name) || !lock(&self.index).plans.iter().any(|e| e.name == name) {
            return None;
        }
        std::fs::read_to_string(self.file(name)).ok()
    }

    /// A plan.
    pub fn get(&self, name: &str) -> Option<Plan> {
        text::read(self.text(name)?.as_bytes()).ok()
    }

    /// Write `plan` as a new plan: refused when one has its name.
    pub fn add(&self, plan: &Plan) -> Result<Entry, Issued> {
        let mut idx = lock(&self.index);
        if idx.plans.iter().any(|e| e.name == plan.name) {
            return Err(issue(&qmcmsg::PLAN_EXISTS, &[&plan.name]));
        }
        let body =
            text::write(plan).map_err(|e| issue(&qmcmsg::PLAN_NOT_SAVED, &[&plan.name, &e]))?;
        std::fs::create_dir_all(&self.dir)
            .and_then(|()| state::atomic_write(&self.file(&plan.name), body.as_bytes()))
            .map_err(|e| issue(&qmcmsg::PLAN_NOT_SAVED, &[&plan.name, &e.to_string()]))?;
        let entry = Entry::of(plan, body.len());
        let mut next = idx.clone();
        next.plans.push(entry.clone());
        next.plans.sort_by(|a, b| a.name.cmp(&b.name));
        self.save_index(&next)?;
        *idx = next;
        Ok(entry)
    }

    /// Remove plan `name`.
    pub fn delete(&self, name: &str) -> Result<(), Issued> {
        let mut idx = lock(&self.index);
        if !idx.plans.iter().any(|e| e.name == name) {
            return Err(issue(&qmcmsg::NO_SUCH_PLAN, &[name]));
        }
        let mut next = idx.clone();
        next.plans.retain(|e| e.name != name);
        self.save_index(&next)?;
        *idx = next;
        let _ = std::fs::remove_file(self.file(name));
        Ok(())
    }

    /// Rewrite plan `name` with `change` applied.
    fn change(&self, name: &str, change: impl FnOnce(&mut Plan)) -> Result<Entry, Issued> {
        let mut plan = self
            .get(name)
            .ok_or_else(|| issue(&qmcmsg::NO_SUCH_PLAN, &[name]))?;
        change(&mut plan);
        let body = text::write(&plan).map_err(|e| issue(&qmcmsg::PLAN_NOT_SAVED, &[name, &e]))?;
        let mut idx = lock(&self.index);
        state::atomic_write(&self.file(name), body.as_bytes())
            .map_err(|e| issue(&qmcmsg::PLAN_NOT_SAVED, &[name, &e.to_string()]))?;
        let entry = Entry::of(&plan, body.len());
        let mut next = idx.clone();
        if let Some(e) = next.plans.iter_mut().find(|e| e.name == name) {
            *e = entry.clone();
        }
        self.save_index(&next)?;
        *idx = next;
        Ok(entry)
    }

    /// Set plan `name`'s description.
    pub fn describe(&self, name: &str, description: &str) -> Result<(), Issued> {
        check::words("the description", description)
            .map_err(|e| issue(&qmcmsg::PLAN_NOT_SAVED, &[name, &e]))?;
        self.change(name, |p| p.description = description.to_string())
            .map(|_| ())
    }

    /// Plan `name` saved again as template `to`, made by `user` at `now`.
    pub fn template(&self, name: &str, to: &str, user: &str, now: u64) -> Result<Entry, Issued> {
        if !check::plan_name_ok(to) {
            return Err(issue(&qmcmsg::PLAN_NAME, &[to]));
        }
        let plan = self
            .get(name)
            .ok_or_else(|| issue(&qmcmsg::NO_SUCH_PLAN, &[name]))?;
        let mut t = plan.as_template(to);
        t.created = now;
        t.by = user.to_string();
        if t.description.is_empty() {
            t.description = format!("Template from {name}");
        }
        self.add(&t)
    }

    /// A file imported: hostile input, read strictly, kept as a plan of
    /// its own, nothing acted on.
    pub fn import(&self, bytes: &[u8]) -> Result<Entry, Issued> {
        let plan =
            text::read(bytes).map_err(|e| issue(&qmcmsg::PLAN_REFUSED, &[&e.to_string()]))?;
        self.add(&plan)
    }

    /// A plan of `system` captured now through `mgmt` as `user`.
    pub fn capture(
        &self,
        mgmt: &mut dyn mgmtproto::Calls,
        system: &str,
        meta: Meta,
    ) -> Result<Entry, Issued> {
        if !check::plan_name_ok(&meta.name) {
            return Err(issue(&qmcmsg::PLAN_NAME, &[&meta.name]));
        }
        if lock(&self.index).plans.iter().any(|e| e.name == meta.name) {
            return Err(issue(&qmcmsg::PLAN_EXISTS, &[&meta.name]));
        }
        let v = plan::view::read::host(mgmt, system, &meta.by).map_err(|e| match e {
            plan::view::read::ReadError::Refused(r) => Issued {
                id: r.main.msgid,
                text: r.main.text,
                help: r.main.help,
            },
            e => issue(&qmcmsg::CAPTURE_FAILED, &[system, &e.to_string()]),
        })?;
        if !v.is_host {
            return Err(issue(&qmcmsg::NO_PLAN_STANDALONE, &[system]));
        }
        let mut plan =
            capture(&v, meta).map_err(|e| issue(&qmcmsg::CAPTURE_FAILED, &[system, &e]))?;
        if plan.description.is_empty() {
            let n = plan.partitions.len();
            plan.description = format!(
                "{system} as built, {n} partition{}",
                if n == 1 { "" } else { "s" }
            );
        }
        self.add(&plan)
    }

    /// A free name for an automatic plan of `system` on `date`
    /// (`YYYYMMDD`): `qshqvs1-20261030`, then `-2` and on.
    pub fn automatic_name(&self, system: &str, date: &str) -> String {
        let base: String = system
            .to_ascii_lowercase()
            .chars()
            .filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_' || *c == '.')
            .take(22)
            .collect();
        let base = format!("{base}-{date}");
        let idx = lock(&self.index);
        let taken = |n: &str| idx.plans.iter().any(|e| e.name == n);
        if !taken(&base) {
            return base;
        }
        (2..10_000)
            .map(|n| format!("{base}-{n}"))
            .find(|n| !taken(n))
            .unwrap_or(base)
    }

    /// The plan saved of `system` before its update: captured, then the
    /// oldest automatic plans of the system past what is kept removed.
    /// Its name.
    pub fn save_before(
        &self,
        mgmt: &mut dyn mgmtproto::Calls,
        system: &str,
        description: &str,
        user: &str,
        now: u64,
        date: &str,
    ) -> Result<String, Issued> {
        let name = self.automatic_name(system, date);
        let e = self.capture(
            mgmt,
            system,
            Meta {
                name: name.clone(),
                description: description.chars().take(check::MAX_DESCRIPTION).collect(),
                created: now,
                by: user.to_string(),
                automatic: true,
            },
        )?;
        self.prune(system);
        Ok(e.name)
    }

    /// Keep `automatic_kept` automatic plans of `system`, removing the
    /// oldest; an operator's plans are never removed for them.
    pub fn prune(&self, system: &str) {
        let (keep, mut auto): (usize, Vec<Entry>) = {
            let idx = lock(&self.index);
            (
                idx.automatic_kept as usize,
                idx.plans
                    .iter()
                    .filter(|e| e.automatic && e.system == system)
                    .cloned()
                    .collect(),
            )
        };
        auto.sort_by(|a, b| a.created.cmp(&b.created).then_with(|| a.name.cmp(&b.name)));
        let extra = auto.len().saturating_sub(keep);
        for e in auto.iter().take(extra) {
            if self.delete(&e.name).is_ok() {
                eprintln!(
                    "audit: user {} from the scheduler: delete system plan {}: the oldest automatic plan of {system} past {keep}",
                    e.by, e.name
                );
            }
        }
    }
}
