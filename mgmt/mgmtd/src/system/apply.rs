// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! What a system says, kept: the replies to the thread's own reads and
//! every event. An event says what changed; where it does not say all
//! of it, the thing is read again.

use mgmtproto::calls::Row;
use mgmtproto::kept::{Kind, PartProfile};
use qsi::Empty;
use qsi::any::{AnyEvent, AnyReply, AnyRequest};
use qsi::base::{self, Replay};
use qsi::host::refcodes::{Posted, Source};
use qsi::host::{host, partitions, profiles, refcodes};
use qsi::{base::Refcode, caps};
use qsisess::{Event, Reply, Session};

use super::worker::{Mine, Pending, Worker};
use crate::peer::pair::refusal;

/// Codes read a page at a time.
const PAGE: u32 = 200;

impl Worker {
    /// Read everything the pages show.
    fn read_all(&mut self, s: &mut Session) {
        let has = s.peer().map_or(0, |h| h.caps);
        self.mine(
            s,
            Mine::Identify,
            &AnyRequest::Base(base::Request::Identify(Empty)),
        );
        self.mine(
            s,
            Mine::State,
            &AnyRequest::Base(base::Request::State(Empty)),
        );
        self.mine(
            s,
            Mine::BaseCodes,
            &AnyRequest::Base(base::Request::Refcodes(base::Refcodes {
                before: None,
                limit: PAGE,
            })),
        );
        if has & caps::HOST != 0 {
            self.mine(s, Mine::Host, &AnyRequest::Host(host::Request::Host(Empty)));
            self.mine(
                s,
                Mine::Hardware,
                &AnyRequest::Host(host::Request::Hardware(Empty)),
            );
        }
        if has & caps::PARTITIONS != 0 {
            self.mine(
                s,
                Mine::List,
                &AnyRequest::Partitions(partitions::Request::List(Empty)),
            );
        }
        self.read_resources(s);
        self.read_servicing(s);
    }

    fn read_list(&mut self, s: &mut Session) {
        if !self.asking(&Mine::List) {
            self.mine(
                s,
                Mine::List,
                &AnyRequest::Partitions(partitions::Request::List(Empty)),
            );
        }
    }

    fn read_state(&mut self, s: &mut Session) {
        if !self.asking(&Mine::State) {
            self.mine(
                s,
                Mine::State,
                &AnyRequest::Base(base::Request::State(Empty)),
            );
        }
    }

    fn rows(&self, partition: u32, name: &str, codes: Vec<Posted>) -> Vec<Row> {
        codes
            .into_iter()
            .map(|posted| Row {
                system: self.entry.name.clone(),
                partition,
                partition_name: name.into(),
                posted,
            })
            .collect()
    }

    fn keep_rows(&self, rows: Vec<Row>) {
        self.inner.history().add(&self.entry.name, rows);
    }

    fn part_name(&self, id: u32) -> String {
        self.entry
            .kept()
            .partitions
            .iter()
            .find(|p| p.id == id)
            .map(|p| p.name.clone())
            .unwrap_or_default()
    }

    /// A reply to one of the thread's own reads.
    #[allow(clippy::too_many_lines, reason = "one arm a read")]
    pub(super) fn reply(&mut self, s: &mut Session, what: Mine, r: &Reply) {
        if let Mine::Attach(id) = what {
            let answer = if let Some(no) = refusal(r) {
                Err(no)
            } else {
                let serial = matches!(
                    r.typed::<qsi::host::console::Reply>(),
                    Ok(Some(qsi::host::console::Reply::Attach(a))) if a.serial
                );
                r.stream
                    .map(|s| (s, serial))
                    .ok_or_else(|| mgmtproto::Refusal::new("QSI0106", "no stream was opened"))
            };
            self.consoles.attached(id, answer);
            return;
        }
        if let Mine::Stage(name) = &what {
            let answer = match refusal(r) {
                Some(no) => Err(no),
                None => r
                    .stream
                    .ok_or_else(|| mgmtproto::Refusal::new("QSI0106", "no stream was opened")),
            };
            if let Err(no) = &answer
                && let Some(g) = self
                    .entry
                    .kept()
                    .staging
                    .as_mut()
                    .filter(|g| &g.name == name)
            {
                g.error = Some(no.main.clone());
            }
            self.stages.opened(name, answer);
            return;
        }
        if let Mine::Download(key) = what {
            let bytes = match r.decode() {
                Ok(Some(AnyReply::Base(base::Reply::DownloadDump(d)))) => d.bytes,
                Ok(Some(AnyReply::Dumps(qsi::host::dumps::Reply::Download(d)))) => d.bytes,
                _ => 0,
            };
            let answer = match refusal(r) {
                Some(no) => Err(no),
                None => r
                    .stream
                    .map(|s| (s, bytes))
                    .ok_or_else(|| mgmtproto::Refusal::new("QSI0106", "no stream was opened")),
            };
            self.downloads.opened(key, answer);
            return;
        }
        if let Mine::Upload(name) = &what {
            let answer = match refusal(r) {
                Some(no) => Err(no),
                None => r
                    .stream
                    .ok_or_else(|| mgmtproto::Refusal::new("QSI0106", "no stream was opened")),
            };
            self.uploads.opened(name, answer);
            return;
        }
        if refusal(r).is_some() {
            return;
        }
        let Ok(Some(any)) = r.decode() else {
            return;
        };
        match (what, any) {
            (Mine::Subscribe, AnyReply::Base(base::Reply::Subscribe(sub))) => {
                let nothing = self.entry.kept().identity.is_none();
                if nothing || sub.replay != Replay::Resumed {
                    self.read_all(s);
                }
            }
            (Mine::Identify, AnyReply::Base(base::Reply::Identify(i))) => {
                let mut k = self.entry.kept();
                k.kind = Kind::of(i.kind);
                k.identity = Some(i);
            }
            (Mine::State, AnyReply::Base(base::Reply::State(s))) => {
                self.entry.kept().state = Some(s);
            }
            (Mine::Host, AnyReply::Host(host::Reply::Host(h))) => {
                self.entry.kept().host = Some(h);
            }
            (Mine::Hardware, AnyReply::Host(host::Reply::Hardware(h))) => {
                self.entry.kept().hardware = Some(h);
            }
            (Mine::List, AnyReply::Partitions(partitions::Reply::List(list))) => {
                for p in &list {
                    if !p.profile.is_empty() {
                        self.mine(
                            s,
                            Mine::Profile(p.id),
                            &AnyRequest::Profiles(profiles::Request::Get(profiles::Get {
                                id: p.id,
                                name: p.profile.clone(),
                            })),
                        );
                    }
                    let fresh = self.inner.history().last_seq(&self.entry.name, p.id) == 0;
                    let had = self.entry.kept().partitions.iter().any(|o| o.id == p.id);
                    if fresh || !had {
                        self.mine(
                            s,
                            Mine::PartCodes(p.id),
                            &AnyRequest::Refcodes(refcodes::Request::History(refcodes::History {
                                id: p.id,
                                before: None,
                                limit: PAGE,
                            })),
                        );
                    }
                }
                self.read_parts(s, &list);
                for p in &list {
                    self.read_part_dumps(s, p.id);
                }
                let mut k = self.entry.kept();
                k.profiles.retain(|pp| list.iter().any(|p| p.id == pp.id));
                k.partitions = list;
            }
            (Mine::Profile(id), AnyReply::Profiles(profiles::Reply::Get(rev))) => {
                let mut k = self.entry.kept();
                k.profiles.retain(|p| p.id != id);
                k.profiles.push(PartProfile {
                    id,
                    profile: rev.profile,
                });
            }
            (Mine::BaseCodes, AnyReply::Base(base::Reply::Refcodes(list))) => {
                let known = self.inner.history().last_seq(&self.entry.name, 0);
                let done = list.codes.iter().any(|c| c.seq <= known);
                let posted = list
                    .codes
                    .into_iter()
                    .filter(|c| c.seq > known)
                    .map(|refcode| Posted {
                        refcode,
                        source: Source::Host,
                    })
                    .collect();
                let rows = self.rows(0, "", posted);
                self.keep_rows(rows);
                if let Some(before) = list.more.filter(|_| !done) {
                    self.mine(
                        s,
                        Mine::BaseCodes,
                        &AnyRequest::Base(base::Request::Refcodes(base::Refcodes {
                            before: Some(before),
                            limit: PAGE,
                        })),
                    );
                }
            }
            (Mine::PartCodes(id), AnyReply::Refcodes(refcodes::Reply::History(page))) => {
                let known = self.inner.history().last_seq(&self.entry.name, id);
                let done = page.codes.iter().any(|c| c.refcode.seq <= known);
                let name = self.part_name(id);
                let posted = page
                    .codes
                    .into_iter()
                    .filter(|c| c.refcode.seq > known)
                    .collect();
                let rows = self.rows(id, &name, posted);
                self.keep_rows(rows);
                if let Some(before) = page.more.filter(|_| !done) {
                    self.mine(
                        s,
                        Mine::PartCodes(id),
                        &AnyRequest::Refcodes(refcodes::Request::History(refcodes::History {
                            id,
                            before: Some(before),
                            limit: PAGE,
                        })),
                    );
                } else if self.stale.remove(&id) {
                    self.part_codes(s, id);
                }
            }
            (
                what @ (Mine::Problems
                | Mine::SystemDumps
                | Mine::SystemSpace
                | Mine::PartDumps(_)
                | Mine::PartSpace
                | Mine::UpdateStatus
                | Mine::FixLevel),
                any,
            ) => {
                self.servicing_reply(s, &what, any);
            }
            (what, any) => {
                self.resources_reply(s, &what, any);
            }
        }
    }

    /// An event.
    pub(super) fn event(&mut self, s: &mut Session, e: &Event) {
        let Ok(Some(any)) = e.decode() else {
            return;
        };
        if self.servicing_event(s, &any) || self.resources_event(s, &any) {
            return;
        }
        match any {
            AnyEvent::Base(base::Event::State(s)) => self.entry.kept().state = Some(s),
            AnyEvent::Base(base::Event::Identity(i)) => self.entry.kept().identity = Some(i),
            AnyEvent::Base(base::Event::Refcode(r)) => {
                self.code(0, "", r, Source::Host);
                self.read_state(s);
            }
            AnyEvent::Host(host::Event::Resources(h)) => self.entry.kept().host = Some(h),
            AnyEvent::Host(_) => {
                if !self.asking(&Mine::Hardware) {
                    self.mine(
                        s,
                        Mine::Hardware,
                        &AnyRequest::Host(host::Request::Hardware(Empty)),
                    );
                }
            }
            AnyEvent::Partitions(_) | AnyEvent::Profiles(_) => self.read_list(s),
            AnyEvent::Refcodes(refcodes::Event::Code(c)) => {
                if c.current {
                    let mut k = self.entry.kept();
                    if let Some(p) = k.partitions.iter_mut().find(|p| p.id == c.id) {
                        p.code.clone_from(&c.posted.refcode.code);
                        p.text.clone_from(&c.posted.refcode.text);
                    }
                }
                if c.posted.refcode.seq > 0 {
                    let rows = self.rows(c.id, &c.name, vec![c.posted]);
                    self.keep_rows(rows);
                } else {
                    // A host numbers a partition's codes only in its
                    // history: read its newest page.
                    self.part_codes(s, c.id);
                }
            }
            _ => {}
        }
    }

    /// Read the newest page of partition `id`'s history, or once more
    /// after the read that is out.
    fn part_codes(&mut self, s: &mut Session, id: u32) {
        let out = self
            .pending
            .values()
            .any(|p| matches!(p, Pending::Mine(Mine::PartCodes(i)) if *i == id));
        if out {
            self.stale.insert(id);
            return;
        }
        self.mine(
            s,
            Mine::PartCodes(id),
            &AnyRequest::Refcodes(refcodes::Request::History(refcodes::History {
                id,
                before: None,
                limit: PAGE,
            })),
        );
    }

    fn code(&self, partition: u32, name: &str, r: Refcode, source: Source) {
        let rows = self.rows(partition, name, vec![Posted { refcode: r, source }]);
        self.keep_rows(rows);
    }
}
