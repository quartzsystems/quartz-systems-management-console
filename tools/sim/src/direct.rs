// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! The managed-systems service's calls answered by simulated systems in
//! this process, with no QSI session between: for the unit tests of
//! what asks `mgmtd` (plans, the scheduler). Only what such a caller
//! asks is answered: `systems`, `system`, `partition` and `ask`, each
//! request answered as the system answers it on its session, in the
//! user's name.

use mgmtproto::kept::{Kind, Link};
use mgmtproto::{Answered, Calls, Kept, Refusal, Request};
use qsi::PeerKind;
use qsi::any::{AnyReply, AnyRequest};
use qsi::host::partitions;

use crate::system::answer::{Answer, answer};
use crate::{Shared, lock};

/// Simulated systems as `mgmtd` would show them, all connected.
#[derive(Clone)]
pub struct Direct {
    /// The systems.
    pub systems: Vec<Shared>,
}

impl Direct {
    /// Over `systems`.
    pub fn new(systems: Vec<Shared>) -> Self {
        Self { systems }
    }

    fn find(&self, name: &str) -> Option<&Shared> {
        self.systems.iter().find(|s| lock(s).name() == name)
    }

    fn kept(s: &Shared) -> Kept {
        let s = lock(s);
        let kind = if s.hello.kind == PeerKind::Host {
            Kind::Host
        } else {
            Kind::Standalone
        };
        Kept {
            name: s.name().to_string(),
            kind,
            serial: s.hello.serial.clone(),
            link: if s.up { Link::Connected } else { Link::Lost },
            reason: if s.up {
                String::new()
            } else {
                "it does not answer".into()
            },
            caps: s.hello.caps & mgmtproto::ask::CONSOLE_CAPS,
            identity: Some(qsi::base::Identity {
                kind: s.hello.kind,
                name: s.name().to_string(),
                release: s.hello.release.clone(),
                serial: s.hello.serial.clone(),
                model: s.model.clone(),
                build: s.build.clone(),
            }),
            ..Kept::default()
        }
    }

    fn answer(&self, name: &str, req: &AnyRequest, user: &str) -> Result<AnyReply, Refusal> {
        let s = self
            .find(name)
            .ok_or_else(|| Refusal::new("QMC1101", "There is no such managed system."))?;
        let mut g = lock(s);
        if !g.up {
            return Err(Refusal::new(
                "QMC1102",
                "The managed system is not connected.",
            ));
        }
        match answer(&mut g, req, user) {
            Answer::Reply(r) => Ok(r),
            Answer::Refuse(notes) => {
                let mut notes = notes.into_iter().map(|(id, text)| mgmtproto::Note {
                    msgid: id,
                    text,
                    help: String::new(),
                });
                let main = notes.next().unwrap_or_default();
                Err(Refusal {
                    main,
                    also: notes.collect(),
                })
            }
            other => Err(Refusal::new(
                "QMC1103",
                &format!("The request was not answered directly: {other:?}"),
            )),
        }
    }
}

fn other(what: &str) -> ipc::WireError {
    std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        format!("the direct service does not answer {what}"),
    )
    .into()
}

impl Calls for Direct {
    fn call(&mut self, req: &Request) -> Result<Vec<u8>, ipc::WireError> {
        Ok(match req {
            Request::Systems {} => {
                ipc::encode(&self.systems.iter().map(Self::kept).collect::<Vec<_>>())
            }
            Request::System { name } => ipc::encode(&self.find(name).map(Self::kept)),
            Request::Ask { name, req, user } => {
                let Some(q) = req.request() else {
                    return Err(other("a request that does not read"));
                };
                let r = self.answer(name, &q, user).map(|r| Answered::of(&r));
                ipc::encode(&r)
            }
            Request::Partition { name, id, user } => {
                let q =
                    AnyRequest::Partitions(partitions::Request::Get(partitions::Get { id: *id }));
                let r = self.answer(name, &q, user).and_then(|r| match r {
                    AnyReply::Partitions(partitions::Reply::Get(p)) => Ok(p),
                    _ => Err(Refusal::new("QMC1103", "another reply")),
                });
                ipc::encode(&r)
            }
            other_req => return Err(other(other_req.name())),
        })
    }
}
