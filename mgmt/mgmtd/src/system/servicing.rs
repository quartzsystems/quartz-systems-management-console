// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! What a system says of its serviceability, kept: its problems (the
//! service family, with the console's numbers), its own dumps and each
//! partition's with the space they take, a host's code slots and what
//! its update did, its fix level. Read when the session begins and kept
//! by events; where an event does not say all of it, it is read again.

use mgmtproto::kept::{PartDumps, UpdateEnd, UpdateLine};
use qsi::any::{AnyEvent, AnyReply, AnyRequest};
use qsi::host::dumps;
use qsi::{Empty, base, caps, service, update};
use qsisess::Session;

use super::worker::{Mine, Worker};

/// Problems read a page at a time.
const PAGE: u32 = 100;
/// Lines of what an update did kept of a host.
const LINES: usize = 50;

impl Worker {
    fn can(s: &Session, need: u64) -> bool {
        s.peer().is_some_and(|h| h.caps & need == need)
    }

    /// Read what the service pages show, as far as the system offers it.
    pub(super) fn read_servicing(&mut self, s: &mut Session) {
        if Self::can(s, caps::SERVICE) {
            self.read_problems(s, None);
        }
        if Self::can(s, caps::SYSTEM_DUMPS) {
            self.read_system_dumps(s);
        }
        if Self::can(s, caps::UPDATE) {
            self.read_update(s);
        }
        if Self::can(s, caps::FIX_LEVEL) {
            self.mine(
                s,
                Mine::FixLevel,
                &AnyRequest::Base(base::Request::FixLevel(Empty)),
            );
        }
    }

    fn read_problems(&mut self, s: &mut Session, before: Option<u64>) {
        let list = service::List {
            filter: service::Filter::default(),
            before,
            limit: PAGE,
        };
        let req = if Self::can(s, caps::PROBLEM_NOTES) {
            service::Request::ListRated(list)
        } else {
            service::Request::List(list)
        };
        self.mine(s, Mine::Problems, &AnyRequest::Service(req));
    }

    fn read_system_dumps(&mut self, s: &mut Session) {
        if !self.asking(&Mine::SystemDumps) {
            self.mine(
                s,
                Mine::SystemDumps,
                &AnyRequest::Base(base::Request::Dumps(Empty)),
            );
        }
        if Self::can(s, caps::SYSTEM_DUMPS | caps::DUMP_SPACE) && !self.asking(&Mine::SystemSpace) {
            self.mine(
                s,
                Mine::SystemSpace,
                &AnyRequest::Base(base::Request::DumpSpace(Empty)),
            );
        }
    }

    /// Read partition `id`'s dumps, and the space every partition's take.
    pub(super) fn read_part_dumps(&mut self, s: &mut Session, id: u32) {
        if !Self::can(s, caps::DUMPS) {
            return;
        }
        if !self.asking_this(&Mine::PartDumps(id)) {
            self.mine(
                s,
                Mine::PartDumps(id),
                &AnyRequest::Dumps(dumps::Request::List(dumps::List { id })),
            );
        }
        if Self::can(s, caps::DUMPS | caps::DUMP_SPACE) && !self.asking(&Mine::PartSpace) {
            self.mine(
                s,
                Mine::PartSpace,
                &AnyRequest::Dumps(dumps::Request::Space(dumps::GetSpace { id: 0 })),
            );
        }
    }

    fn read_update(&mut self, s: &mut Session) {
        if !self.asking(&Mine::UpdateStatus) {
            self.mine(
                s,
                Mine::UpdateStatus,
                &AnyRequest::Update(update::Request::Status(Empty)),
            );
        }
    }

    fn keep_problem(&self, severity: Option<service::Severity>, summary: service::Summary) {
        self.inner
            .problems()
            .keep(&self.entry.name, severity, summary);
    }

    /// A reply to one of these reads: whether it was one.
    pub(super) fn servicing_reply(&mut self, s: &mut Session, what: &Mine, any: AnyReply) -> bool {
        match (what, any) {
            (Mine::Problems, AnyReply::Service(service::Reply::ListRated(page))) => {
                for r in page.problems {
                    self.keep_problem(Some(r.severity), r.summary);
                }
                if let Some(before) = page.more {
                    self.read_problems(s, Some(before));
                }
            }
            (Mine::Problems, AnyReply::Service(service::Reply::List(page))) => {
                for p in page.problems {
                    self.keep_problem(None, p);
                }
                if let Some(before) = page.more {
                    self.read_problems(s, Some(before));
                }
            }
            (Mine::SystemDumps, AnyReply::Base(base::Reply::Dumps(d))) => {
                self.entry.kept().dumps = d;
            }
            (Mine::SystemSpace, AnyReply::Base(base::Reply::DumpSpace(sp))) => {
                self.entry.kept().dump_space = Some(sp);
            }
            (Mine::PartDumps(id), AnyReply::Dumps(dumps::Reply::List(d))) => {
                let mut k = self.entry.kept();
                k.part_dumps.retain(|p| p.id != *id);
                k.part_dumps.push(PartDumps { id: *id, dumps: d });
                k.part_dumps.sort_by_key(|p| p.id);
            }
            (Mine::PartSpace, AnyReply::Dumps(dumps::Reply::Space(sp))) => {
                self.entry.kept().part_space = sp;
            }
            (Mine::UpdateStatus, AnyReply::Update(update::Reply::Status(st))) => {
                self.entry.kept().update = Some(st);
            }
            (Mine::FixLevel, AnyReply::Base(base::Reply::FixLevel(l))) => {
                self.entry.kept().fix_level = Some(l);
            }
            _ => return false,
        }
        true
    }

    /// A line of what the host's update did.
    fn update_line(&self, time: u64, step: &str, text: String, failed: bool) {
        let mut k = self.entry.kept();
        k.update_log.push(UpdateLine {
            time,
            step: step.into(),
            text,
            failed,
        });
        let over = k.update_log.len().saturating_sub(LINES);
        k.update_log.drain(..over);
    }

    /// An event of these families: whether it was one.
    #[allow(clippy::too_many_lines, reason = "one arm an event")]
    pub(super) fn servicing_event(&mut self, s: &mut Session, any: &AnyEvent) -> bool {
        match any {
            AnyEvent::Service(
                service::Event::Opened(n)
                | service::Event::Seen(n)
                | service::Event::Cleared(n)
                | service::Event::Closed(n),
            ) => self.keep_problem(None, n.problem.clone()),
            // A note changes the analysis log alone, which is read when
            // the problem is shown.
            AnyEvent::Service(_) => {}
            AnyEvent::Base(base::Event::DumpTaken(_) | base::Event::DumpRemoved(_)) => {
                self.read_system_dumps(s);
            }
            AnyEvent::Dumps(dumps::Event::Taken(t)) => self.read_part_dumps(s, t.id),
            AnyEvent::Dumps(dumps::Event::Removed(r)) => self.read_part_dumps(s, r.id),
            AnyEvent::Update(e) => {
                match e {
                    update::Event::Staged(st) => {
                        let (text, failed) = match &st.error {
                            None => (format!("{} received, {} bytes", st.name, st.bytes), false),
                            Some(n) => (format!("{} {}", n.msgid, n.text), true),
                        };
                        self.update_line(st.time, "stage", text, failed);
                        let mut k = self.entry.kept();
                        if let Some(g) = k.staging.as_mut().filter(|g| g.name == st.name) {
                            g.done = true;
                            g.error = st.error.as_ref().map(note);
                        }
                    }
                    update::Event::Verified(v) => {
                        let (text, failed) = match (&v.error, &v.image) {
                            (Some(n), _) => (format!("{} {}", n.msgid, n.text), true),
                            (None, Some(i)) => (
                                format!("{} {} build {} verified", i.product, i.release, i.build),
                                false,
                            ),
                            (None, None) => (format!("{} verified", v.name), false),
                        };
                        self.update_line(v.time, "verify", text, failed);
                    }
                    update::Event::Installing(p) => {
                        if let Some(u) = self.entry.kept().update.as_mut() {
                            u.installing = Some(p.clone());
                        }
                        return true;
                    }
                    update::Event::Installed(i) => {
                        let (text, failed) = match &i.error {
                            None => (format!("slot {} written and pending", i.slot), false),
                            Some(n) => (format!("{} {}", n.msgid, n.text), true),
                        };
                        self.update_line(i.time, "install", text, failed);
                    }
                    update::Event::Activating(a) => {
                        self.update_line(
                            a.time,
                            "activate",
                            format!("restart into slot {} begun", a.slot),
                            false,
                        );
                    }
                    update::Event::Confirmed(c) => {
                        self.update_line(
                            c.time,
                            "confirm",
                            format!(
                                "{} build {} in slot {} confirmed",
                                c.release, c.build, c.slot
                            ),
                            false,
                        );
                        self.entry.kept().update_end = Some(UpdateEnd {
                            slot: c.slot.clone(),
                            confirmed: true,
                            release: c.release.clone(),
                            build: c.build.clone(),
                            reason: String::new(),
                            code: String::new(),
                            time: c.time,
                        });
                    }
                    update::Event::RolledBack(r) => {
                        let code = if r.code.is_empty() {
                            String::new()
                        } else {
                            format!(" ({})", r.code)
                        };
                        self.update_line(
                            r.time,
                            "confirm",
                            format!("slot {} rolled back: {}{code}", r.slot, r.reason),
                            true,
                        );
                        let release = self
                            .entry
                            .kept()
                            .state
                            .as_ref()
                            .map(|st| st.release.clone())
                            .unwrap_or_default();
                        self.entry.kept().update_end = Some(UpdateEnd {
                            slot: r.slot.clone(),
                            confirmed: false,
                            release,
                            build: String::new(),
                            reason: r.reason.clone(),
                            code: r.code.clone(),
                            time: r.time,
                        });
                    }
                }
                self.read_update(s);
            }
            _ => return false,
        }
        true
    }
}

/// A system's note as the console keeps it.
pub(super) fn note(n: &qsi::host::model::Note) -> mgmtproto::kept::Note {
    mgmtproto::kept::Note {
        msgid: n.msgid.clone(),
        text: n.text.clone(),
        help: n.help.clone(),
    }
}
