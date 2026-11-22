// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! The pages' requests carried as QSI encodes them (`mgmtproto::ask`):
//! only those the pages send, and only what the system's hello offers.
//! Pairing, a list of problems, a stage, a dump's download, a console's
//! stream and an upload are not carried here: each has a call of its
//! own.

use mgmtproto::ask::lacks_for;
use mgmtproto::{Answered, Asked, Refusal};
use qsi::any::AnyRequest;
use qsi::base;
use qsi::host::{dumps, panel, vres};
use qsi::{service, update};

use super::Inner;
use crate::words::qmc;

/// Whether the pages may send `req` through `ask`.
pub fn carried(req: &AnyRequest) -> bool {
    match req {
        AnyRequest::Host(_)
        | AnyRequest::Partitions(_)
        | AnyRequest::Partition(_)
        | AnyRequest::Profiles(_)
        | AnyRequest::Panel(panel::Request::Functions(_))
        // A system's own dumps listed, taken, deleted, their space; its
        // fix level.
        | AnyRequest::Base(
            base::Request::Dumps(_)
            | base::Request::TakeDump(_)
            | base::Request::DeleteDump(_)
            | base::Request::DumpSpace(_)
            | base::Request::FixLevel(_),
        ) => true,
        AnyRequest::Vres(v) => !matches!(v, vres::Request::ImageUpload(_)),
        // A problem closed, noted or read; the pages' lists are kept.
        AnyRequest::Service(s) => !matches!(
            s,
            service::Request::List(_) | service::Request::ListRated(_)
        ),
        // A host's update but the stage, which is a stream from a file.
        AnyRequest::Update(u) => !matches!(u, update::Request::Stage(_)),
        // Dumps listed, taken, deleted, their space; not downloaded.
        AnyRequest::Dumps(d) => !matches!(d, dumps::Request::Download(_)),
        _ => false,
    }
}

impl Inner {
    /// `asked` of `name` for `user`: refused when it is not carried or
    /// the system lacks it, else sent on its session.
    pub fn ask_carried(&self, name: &str, asked: &Asked, user: &str) -> Result<Answered, Refusal> {
        let e = self
            .entry(name)
            .ok_or_else(|| qmc(&qmcmsg::NO_SUCH_SYSTEM, &[name]))?;
        let req = asked
            .request()
            .filter(carried)
            .ok_or_else(|| qmc(&qmcmsg::NOT_CARRIED, &[&e.name]))?;
        let agreed = e.kept().caps;
        if let Some(what) = lacks_for(agreed, &req) {
            return Err(qmc(&qmcmsg::LACKS, &[&e.name, what]));
        }
        let r = self.ask(&e.name, req, user)?;
        match r.decode() {
            Ok(Some(reply)) => Ok(Answered::of(&reply)),
            _ => Err(qmc(&qmcmsg::NO_REPLY, &[&e.name])),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use qsi::Empty;
    use qsi::host::{host, partition, profiles};

    #[test]
    fn only_the_pages_requests_are_carried() {
        assert!(carried(&AnyRequest::Vres(vres::Request::Pools(Empty))));
        assert!(carried(&AnyRequest::Host(host::Request::PowerOn(Empty))));
        assert!(carried(&AnyRequest::Partition(
            partition::Request::Restart(partition::Restart { id: 1 })
        )));
        assert!(carried(&AnyRequest::Profiles(profiles::Request::List(
            profiles::List { id: 1 }
        ))));
        assert!(!carried(&AnyRequest::Vres(vres::Request::ImageUpload(
            vres::ImageUpload {
                name: "A.ISO".into(),
                bytes: 1,
            }
        ))));
        assert!(!carried(&AnyRequest::Base(qsi::base::Request::Time(Empty))));
        assert!(!carried(&AnyRequest::Panel(panel::Request::Function(
            panel::Function {
                id: 1,
                function: 8,
                args: panel::PanelArgs::default(),
            }
        ))));
    }
}
