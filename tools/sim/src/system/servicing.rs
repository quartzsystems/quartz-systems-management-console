// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! A system's serviceability as the simulator plays it (`docs/SIM.md`):
//! its problems (QSI's service family, version 2), its own dumps and
//! each partition's with their space, a host's code update with its two
//! slots, and its fix level. What the next stage's verification and the
//! next activation do is told on standard input.

use qsi::any::{AnyEvent, AnyReply};
use qsi::base::{self, DumpKind, Level, Refcode, Resources, Space, SystemDump};
use qsi::host::dumps::{self, DumpInfo, PartitionSpace};
use qsi::host::model::Note;
use qsi::service::{
    self, Line, Noticed, Problem, ProblemStatus, Rated, RatedPage, RatedProblem, Resource,
    ResourceKind, Severity, Source, SourceKind, Summary,
};
use qsi::update::{self, Attempt, Image, Progress, Slot, StagedImage, Verification};
use qsi::{Empty, PeerKind};

use super::refuse::Refusal;
use super::{System, now_s};

/// What the next activation does after its restart.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum Outcome {
    /// The slot starts and confirms.
    #[default]
    Confirm,
    /// It does not: the host comes back on the other slot, saying why
    /// and with the code.
    RollBack(String, String),
}

/// A system's serviceability.
#[derive(Clone, Debug, Default)]
pub struct Servicing {
    /// Its problems, each with how grave it is, oldest first.
    pub problems: Vec<(Severity, Problem)>,
    /// The number the next problem takes.
    pub next: u64,
    /// Its own dumps, oldest first.
    pub dumps: Vec<SystemDump>,
    /// The most its own dumps may take.
    pub dump_limit: u64,
    /// A host's code slots, A then B.
    pub slots: Vec<Slot>,
    /// What is staged.
    pub staged: Option<StagedImage>,
    /// What the next stage's verification refuses with, if anything.
    pub refuse_stage: Option<(String, String)>,
    /// What the next activation does.
    pub outcome: Outcome,
    /// Its cumulative fix level; none for a host that has none.
    pub fix_level: Option<Level>,
    /// What a standalone system has and uses (base's `Resources`).
    pub resources: Resources,
}

/// Each partition's dumps, by ID, and their limit.
pub const PART_DUMP_LIMIT: u64 = 256 << 30;

/// The bytes of dump `name` from `offset`, `len` of them: the same for
/// the simulator and the harness, which checks a download by them.
pub fn content(name: &str, offset: u64, len: usize) -> Vec<u8> {
    let seed = name
        .bytes()
        .fold(7u64, |h, b| h.wrapping_mul(31).wrapping_add(u64::from(b)));
    (0..len as u64)
        .map(|i| {
            let x = (offset + i)
                .wrapping_add(seed.wrapping_mul(0x0100_0193))
                .wrapping_mul(0x9e37_79b9_7f4a_7c15);
            u8::try_from(x >> 56).unwrap_or(0)
        })
        .collect()
}

fn refuse(id: &str, text: &str) -> Refusal {
    vec![(id.into(), text.into())]
}

fn summary_of(p: &Problem) -> Summary {
    p.summary.clone()
}

impl System {
    /// Open a problem of `code` with `text`, about `resources`, on
    /// partition `id` (0 for the system): its number.
    pub fn open_problem(
        &mut self,
        id: u32,
        code: &str,
        text: &str,
        severity: Severity,
        resources: Vec<Resource>,
    ) -> u64 {
        let now = now_s();
        let number = self.servicing.next.max(1);
        self.servicing.next = number + 1;
        let source = if id == 0 {
            Source {
                kind: SourceKind::System,
                id: 0,
                name: String::new(),
            }
        } else {
            Source {
                kind: SourceKind::Partition,
                id,
                name: self.part_name(id),
            }
        };
        let mut log = vec![Line {
            time: now,
            text: format!("Opened by {code}: {text}"),
        }];
        for r in &resources {
            log.push(Line {
                time: now,
                text: format!("About {:?} {} {}", r.kind, r.name, r.location),
            });
        }
        let p = Problem {
            summary: Summary {
                number,
                status: ProblemStatus::Open,
                first_seen: now,
                last_seen: now,
                count: 1,
                refcode: Refcode {
                    seq: 0,
                    code: code.into(),
                    words: Vec::new(),
                    text: text.into(),
                    time_ms: now * 1000,
                },
                source,
                description: text.into(),
                msgid: if self.hello.kind == PeerKind::Host {
                    "QVS5130".into()
                } else {
                    "CPF0000".into()
                },
                holds: true,
                closed_by: String::new(),
                closed_at: 0,
            },
            resources,
            log,
        };
        let s = summary_of(&p);
        self.servicing.problems.push((severity, p));
        self.push(&AnyEvent::Service(service::Event::Opened(Noticed {
            problem: s,
            time: now,
        })));
        number
    }

    fn problem(&mut self, number: u64) -> Option<&mut (Severity, Problem)> {
        self.servicing
            .problems
            .iter_mut()
            .find(|(_, p)| p.summary.number == number)
    }

    /// Problem `number`'s condition seen again, or cleared: what changed.
    pub fn problem_seen(&mut self, number: u64, cleared: bool) -> Result<(), String> {
        let now = now_s();
        let (_, p) = self
            .problem(number)
            .ok_or_else(|| format!("no problem {number}"))?;
        if p.summary.status == ProblemStatus::Closed {
            return Err(format!("problem {number} is closed"));
        }
        if cleared {
            p.summary.holds = false;
            p.log.push(Line {
                time: now,
                text: "Its condition cleared".into(),
            });
        } else {
            p.summary.count += 1;
            p.summary.last_seen = now;
            p.summary.holds = true;
            p.log.push(Line {
                time: now,
                text: "Seen again".into(),
            });
        }
        let s = summary_of(p);
        let e = if cleared {
            service::Event::Cleared(Noticed {
                problem: s,
                time: now,
            })
        } else {
            service::Event::Seen(Noticed {
                problem: s,
                time: now,
            })
        };
        self.push(&AnyEvent::Service(e));
        Ok(())
    }

    /// Close problem `number` for `user`.
    pub fn close_problem(
        &mut self,
        number: u64,
        anyway: bool,
        user: &str,
    ) -> Result<Summary, Refusal> {
        let now = now_s();
        let standalone = self.hello.kind == PeerKind::Standalone;
        let Some((_, p)) = self.problem(number) else {
            return Err(refuse("QSI1002", &format!("There is no problem {number}.")));
        };
        if p.summary.status == ProblemStatus::Closed {
            return Err(refuse(
                "QSI1003",
                &format!(
                    "Problem {number} was closed already, by {}.",
                    p.summary.closed_by
                ),
            ));
        }
        if p.summary.holds && !anyway && !standalone {
            let mut r = refuse("QSI1001", "The problem's condition still holds.");
            r.push((
                "QVS5003".into(),
                format!("The condition of problem {number} still holds."),
            ));
            return Err(r);
        }
        p.summary.status = ProblemStatus::Closed;
        p.summary.closed_by = user.into();
        p.summary.closed_at = now;
        p.log.push(Line {
            time: now,
            text: format!(
                "Closed by {user}{}",
                if anyway { " while it holds" } else { "" }
            ),
        });
        let s = summary_of(p);
        self.push(&AnyEvent::Service(service::Event::Closed(Noticed {
            problem: s.clone(),
            time: now,
        })));
        Ok(s)
    }

    fn note_problem(&mut self, number: u64, text: &str, user: &str) -> Result<Line, Refusal> {
        let now = now_s();
        let Some((_, p)) = self.problem(number) else {
            return Err(refuse("QSI1002", &format!("There is no problem {number}.")));
        };
        if p.summary.status == ProblemStatus::Closed {
            return Err(refuse(
                "QSI1003",
                &format!("Problem {number} is closed: it takes no note."),
            ));
        }
        let line = Line {
            time: now,
            text: format!("Note by {user}: {text}"),
        };
        p.log.push(line.clone());
        self.push(&AnyEvent::Service(service::Event::Noted(service::Noted {
            number,
            line: line.clone(),
            by: user.into(),
            time: now,
        })));
        Ok(line)
    }

    fn page(&self, list: &service::List) -> (Vec<(Severity, Summary)>, Option<u64>) {
        let f = &list.filter;
        let mut all: Vec<(Severity, Summary)> = self
            .servicing
            .problems
            .iter()
            .rev()
            .map(|(sev, p)| (*sev, p.summary.clone()))
            .filter(|(_, s)| f.status.is_none_or(|st| s.status == st))
            .filter(|(_, s)| {
                f.source
                    .as_ref()
                    .is_none_or(|o| o.kind == s.source.kind && o.id == s.source.id)
            })
            .filter(|(_, s)| f.since.is_none_or(|t| s.last_seen >= t))
            .filter(|(_, s)| list.before.is_none_or(|b| s.number < b))
            .collect();
        let limit = usize::try_from(list.limit.clamp(1, 500)).unwrap_or(500);
        let more = (all.len() > limit).then(|| all[limit - 1].1.number);
        all.truncate(limit);
        (all, more)
    }

    /// A service family request answered.
    pub fn service_answer(
        &mut self,
        req: &service::Request,
        user: &str,
    ) -> Result<AnyReply, Refusal> {
        use service::Reply as R;
        let get = |s: &Self, n: u64| {
            s.servicing
                .problems
                .iter()
                .find(|(_, p)| p.summary.number == n)
                .cloned()
                .ok_or_else(|| refuse("QSI1002", &format!("There is no problem {n}.")))
        };
        Ok(AnyReply::Service(match req {
            service::Request::List(l) => {
                let (rows, more) = self.page(l);
                R::List(service::Page {
                    problems: rows.into_iter().map(|(_, s)| s).collect(),
                    more,
                })
            }
            service::Request::ListRated(l) => {
                let (rows, more) = self.page(l);
                R::ListRated(RatedPage {
                    problems: rows
                        .into_iter()
                        .map(|(severity, summary)| Rated { severity, summary })
                        .collect(),
                    more,
                })
            }
            service::Request::Get(g) => R::Get(get(self, g.number)?.1),
            service::Request::GetRated(g) => {
                let (severity, problem) = get(self, g.number)?;
                R::GetRated(RatedProblem { severity, problem })
            }
            service::Request::Close(c) => R::Close(self.close_problem(c.number, c.anyway, user)?),
            service::Request::Note(n) => R::Note(self.note_problem(n.number, &n.text, user)?),
        }))
    }

    /// The space the system's own dumps take.
    pub fn dump_space(&self) -> Space {
        Space {
            bytes: self.servicing.dumps.iter().map(|d| d.bytes).sum(),
            limit: self.servicing.dump_limit,
            dumps: u32::try_from(self.servicing.dumps.len()).unwrap_or(u32::MAX),
            max_dumps: 8,
        }
    }

    /// Take a dump of the system's own of `bytes`, by `by`.
    pub fn take_own_dump(&mut self, name: &str, bytes: u64, by: &str) {
        let now = now_s();
        let d = SystemDump {
            name: name.into(),
            kind: DumpKind::ServiceData,
            kind_name: "service data".into(),
            bytes,
            time: now,
            by: by.into(),
        };
        self.servicing.dumps.push(d.clone());
        self.push(&AnyEvent::Base(base::Event::DumpTaken(base::DumpTaken {
            name: name.into(),
            dump: Some(d),
            error: None,
            time: now,
        })));
    }

    /// Partition `id`'s dump of `bytes`, taken for `reason`.
    pub fn take_part_dump(
        &mut self,
        id: u32,
        name: &str,
        bytes: u64,
        reason: &str,
    ) -> Result<(), String> {
        let now = now_s();
        let Some(p) = self.part(id) else {
            return Err(format!("no partition {id}"));
        };
        p.dumps.push(DumpInfo {
            name: name.into(),
            time: now,
            bytes,
            allocated: bytes / 2,
            code: p.status.code.clone(),
            reason: reason.into(),
        });
        self.push(&AnyEvent::Dumps(dumps::Event::Taken(dumps::Taken {
            id,
            name: name.into(),
            dump: name.into(),
            error: None,
            time: now,
        })));
        Ok(())
    }

    /// A base family request of the system's own dumps or fix level;
    /// `None` when it is not one. A download is the connection's.
    pub fn own_dump_answer(
        &mut self,
        req: &base::Request,
        user: &str,
    ) -> Option<Result<AnyReply, Refusal>> {
        use base::Reply as R;
        Some(Ok(AnyReply::Base(match req {
            base::Request::FixLevel(_) => {
                R::FixLevel(self.servicing.fix_level.clone().unwrap_or(Level {
                    level: String::new(),
                    date: 0,
                }))
            }
            base::Request::Resources(_) => R::Resources(self.servicing.resources.clone()),
            base::Request::Dumps(_) => R::Dumps(self.servicing.dumps.clone()),
            base::Request::DumpSpace(_) => R::DumpSpace(self.dump_space()),
            base::Request::TakeDump(_) => {
                let name = format!("SPDUMP.{}.{}", self.name(), now_s());
                self.take_own_dump(&name, 4 << 20, user);
                R::TakeDump(base::DumpBegun { name })
            }
            base::Request::DeleteDump(d) => {
                let Some(i) = self.servicing.dumps.iter().position(|x| x.name == d.name) else {
                    return Some(Err(refuse("QSI1201", &format!("No dump {}.", d.name))));
                };
                self.servicing.dumps.remove(i);
                self.push(&AnyEvent::Base(base::Event::DumpRemoved(
                    base::DumpRemoved {
                        name: d.name.clone(),
                        time: now_s(),
                    },
                )));
                R::DeleteDump(Empty)
            }
            _ => return None,
        })))
    }

    /// The size of the system's own dump `name`, or why there is none.
    pub fn own_dump(&self, name: &str) -> Result<u64, Refusal> {
        self.servicing
            .dumps
            .iter()
            .find(|d| d.name == name)
            .map(|d| d.bytes)
            .ok_or_else(|| refuse("QSI1201", &format!("No dump {name}.")))
    }

    /// The size of partition `id`'s dump `name`, or why there is none.
    pub fn part_dump(&self, id: u32, name: &str) -> Result<u64, Refusal> {
        self.part_ref(id)
            .and_then(|p| p.dumps.iter().find(|d| d.name == name))
            .map(|d| d.bytes)
            .ok_or_else(|| refuse("QVS2312", &format!("Partition {id} has no dump {name}.")))
    }

    /// A dumps family request; a download is the connection's.
    pub fn dumps_answer(&mut self, req: &dumps::Request) -> Result<AnyReply, Refusal> {
        use dumps::Reply as R;
        Ok(AnyReply::Dumps(match req {
            dumps::Request::List(l) => R::List(
                self.part_ref(l.id)
                    .map(|p| p.dumps.clone())
                    .ok_or_else(|| {
                        refuse("QVS2001", &format!("There is no partition {}.", l.id))
                    })?,
            ),
            dumps::Request::Space(_) => R::Space(
                self.parts
                    .iter()
                    .filter(|p| p.definition.id != 0)
                    .map(|p| PartitionSpace {
                        id: p.definition.id,
                        space: Space {
                            bytes: p.dumps.iter().map(|d| d.allocated).sum(),
                            limit: PART_DUMP_LIMIT,
                            dumps: u32::try_from(p.dumps.len()).unwrap_or(u32::MAX),
                            max_dumps: 3,
                        },
                    })
                    .collect(),
            ),
            dumps::Request::Take(t) => {
                let name = format!("{}-{}.core", self.part_name(t.id), now_s());
                self.take_part_dump(t.id, &name, 64 << 20, "request")
                    .map_err(|e| refuse("QVS2001", &e))?;
                R::Take(dumps::Begun { name })
            }
            dumps::Request::Delete(d) => {
                let Some(p) = self.part(d.id) else {
                    return Err(refuse(
                        "QVS2001",
                        &format!("There is no partition {}.", d.id),
                    ));
                };
                let Some(i) = p.dumps.iter().position(|x| x.name == d.name) else {
                    return Err(refuse(
                        "QVS2312",
                        &format!("Partition {} has no dump {}.", d.id, d.name),
                    ));
                };
                p.dumps.remove(i);
                self.push(&AnyEvent::Dumps(dumps::Event::Removed(dumps::Removed {
                    id: d.id,
                    dump: d.name.clone(),
                    time: now_s(),
                })));
                R::Delete(Empty)
            }
            dumps::Request::Download(_) => {
                return Err(refuse("QSI0002", "a download opens a stream"));
            }
        }))
    }

    /// The update family's status.
    pub fn update_status(&self) -> update::Status {
        update::Status {
            slots: self.servicing.slots.clone(),
            staged: self.servicing.staged.clone(),
            installing: None,
        }
    }

    /// A staged image arrived whole (`bytes`, its first bytes `head`) or
    /// not: verified, or refused as told.
    pub fn staged(&mut self, name: &str, bytes: u64, whole: bool, head: &[u8]) {
        let now = now_s();
        if !whole {
            self.servicing.staged = None;
            self.push(&AnyEvent::Update(update::Event::Staged(update::Staged {
                name: name.into(),
                bytes,
                error: Some(Note {
                    msgid: "QSI1110".into(),
                    text: format!("Update image {name} ended after {bytes} bytes."),
                    help: String::new(),
                }),
                time: now,
            })));
            return;
        }
        self.push(&AnyEvent::Update(update::Event::Staged(update::Staged {
            name: name.into(),
            bytes,
            error: None,
            time: now,
        })));
        let refused = self.servicing.refuse_stage.take().or_else(|| {
            (!head.starts_with(b"QSVSUPDT")).then(|| {
                (
                    "QSI1102".to_string(),
                    format!("Update image {name} is damaged: it is not an update image."),
                )
            })
        });
        let (verification, image, error) = match refused {
            Some((id, text)) => (
                Verification::Refused,
                None,
                Some(Note {
                    msgid: id,
                    text,
                    help: String::new(),
                }),
            ),
            None => (
                Verification::Verified,
                Some(Image {
                    product: "QS/VS".into(),
                    release: "V1R0M1".into(),
                    build: "26300".into(),
                    fixes: vec!["QV00001".into()],
                    bytes,
                }),
                None,
            ),
        };
        self.servicing.staged = Some(StagedImage {
            name: name.into(),
            bytes,
            verification,
            image: image.clone(),
            error: error.clone(),
            time: now,
        });
        if verification == Verification::Refused {
            self.servicing.staged = None;
        }
        self.push(&AnyEvent::Update(update::Event::Verified(
            update::Verified {
                name: name.into(),
                image,
                error,
                time: now,
            },
        )));
    }

    /// An update family request but a stage and an activation, which are
    /// the connection's.
    pub fn update_answer(&mut self, req: &update::Request) -> Result<AnyReply, Refusal> {
        use update::Reply as R;
        let now = now_s();
        let pending = |s: &Self| {
            s.servicing
                .slots
                .iter()
                .any(|x| x.last == Attempt::Pending && !x.running)
        };
        Ok(AnyReply::Update(match req {
            update::Request::Status(_) => R::Status(self.update_status()),
            update::Request::Discard(_) => {
                if self.servicing.staged.take().is_none() {
                    return Err(refuse("QSI1106", "No update image is staged."));
                }
                R::Discard(Empty)
            }
            update::Request::Install(_) => {
                if pending(self) {
                    return Err(refuse("QSI1114", "A code slot is pending already."));
                }
                let Some(staged) = self.servicing.staged.take() else {
                    return Err(refuse("QSI1106", "No update image is staged."));
                };
                let Some(image) = staged
                    .image
                    .filter(|_| staged.verification == Verification::Verified)
                else {
                    return Err(refuse("QSI1108", "The staged image has not verified."));
                };
                let target = self
                    .servicing
                    .slots
                    .iter()
                    .find(|x| !x.running)
                    .map_or_else(|| "B".to_string(), |x| x.name.clone());
                for percent in [50, 100] {
                    self.push(&AnyEvent::Update(update::Event::Installing(Progress {
                        slot: target.clone(),
                        percent,
                        time: now,
                    })));
                }
                if let Some(x) = self.servicing.slots.iter_mut().find(|x| x.name == target) {
                    x.release.clone_from(&image.release);
                    x.build.clone_from(&image.build);
                    x.fixes.clone_from(&image.fixes);
                    x.last = Attempt::Pending;
                    x.time = now;
                }
                self.push(&AnyEvent::Update(update::Event::Installed(
                    update::Installed {
                        slot: target.clone(),
                        image: Some(image),
                        error: None,
                        time: now,
                    },
                )));
                R::Install(update::Target { slot: target })
            }
            update::Request::Cancel(_) => {
                let Some(x) = self
                    .servicing
                    .slots
                    .iter_mut()
                    .find(|x| x.last == Attempt::Pending && !x.running)
                else {
                    return Err(refuse("QSI1112", "No code slot is pending."));
                };
                x.last = Attempt::Never;
                R::Cancel(Empty)
            }
            update::Request::Stage(_) | update::Request::Activate(_) => {
                return Err(refuse("QSI0002", "the connection's"));
            }
        }))
    }

    /// Whether an activation may begin now: the pending slot, or why not.
    pub fn may_activate(&self, a: &update::Activate) -> Result<String, Refusal> {
        let slot = self
            .servicing
            .slots
            .iter()
            .find(|x| x.last == Attempt::Pending && !x.running)
            .map(|x| x.name.clone())
            .ok_or_else(|| refuse("QSI1112", "No code slot is pending."))?;
        let running: Vec<String> = self
            .parts
            .iter()
            .filter(|p| p.definition.id != 0 && p.active())
            .map(|p| p.definition.name.clone())
            .collect();
        if a.partitions == update::Partitions::Refuse && !running.is_empty() {
            let mut r = refuse(
                "QSI1113",
                "Partitions run and the activation said to refuse.",
            );
            r.push((
                "QVS5313".into(),
                format!("Partitions are running: {}.", running.join(", ")),
            ));
            return Err(r);
        }
        Ok(slot)
    }

    /// After the restart into `slot`: confirmed, or rolled back, as told.
    pub fn after_activation(&mut self, slot: &str) {
        let now = now_s();
        match std::mem::take(&mut self.servicing.outcome) {
            Outcome::Confirm => {
                let (release, build) = self
                    .servicing
                    .slots
                    .iter()
                    .find(|x| x.name == slot)
                    .map(|x| (x.release.clone(), x.build.clone()))
                    .unwrap_or_default();
                for x in &mut self.servicing.slots {
                    let this = x.name == slot;
                    x.running = this;
                    x.default = this;
                    if this {
                        x.last = Attempt::Confirmed;
                        x.time = now;
                    }
                }
                self.hello.release.clone_from(&release);
                self.build.clone_from(&build);
                self.push(&AnyEvent::Update(update::Event::Confirmed(
                    update::Confirmed {
                        slot: slot.into(),
                        release,
                        build,
                        time: now,
                    },
                )));
            }
            Outcome::RollBack(reason, code) => {
                if let Some(x) = self.servicing.slots.iter_mut().find(|x| x.name == slot) {
                    x.last = Attempt::RolledBack;
                    x.reason.clone_from(&reason);
                    x.time = now;
                }
                self.push(&AnyEvent::Update(update::Event::RolledBack(
                    update::RolledBack {
                        slot: slot.into(),
                        reason,
                        code,
                        time: now,
                    },
                )));
            }
        }
    }
}

/// A slot as a new host has it.
pub fn slot(name: &str, release: &str, build: &str, running: bool) -> Slot {
    Slot {
        name: name.into(),
        release: if running {
            release.into()
        } else {
            String::new()
        },
        build: if running { build.into() } else { String::new() },
        fixes: Vec::new(),
        running,
        default: running,
        last: if running {
            Attempt::Confirmed
        } else {
            Attempt::Never
        },
        reason: String::new(),
        time: 0,
    }
}

/// A resource of a problem.
pub fn resource(kind: ResourceKind, name: &str, location: &str, serial: &str) -> Resource {
    Resource {
        kind,
        name: name.into(),
        location: location.into(),
        serial: serial.into(),
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_dump_s_bytes_are_the_same_wherever_they_are_read() {
        let whole = super::content("D", 0, 64);
        assert_eq!(super::content("D", 10, 20), whole[10..30]);
        assert_ne!(super::content("E", 0, 64), whole);
    }
}
