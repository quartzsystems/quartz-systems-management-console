// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! The service pages' requests: the fleet's problems by the console's
//! numbers, one read whole from its system, and an update image brought
//! to a host from the download service's folder.

use std::path::Path;
use std::sync::mpsc::channel;
use std::time::Duration;

use mgmtproto::Refusal;
use mgmtproto::calls::{ProblemDetail, ProblemFilter, ProblemPage, ProblemRow};
use qsi::any::{AnyReply, AnyRequest};
use qsi::{caps, service};

use super::Inner;
use crate::system::Cmd;
use crate::words::qmc;

/// How long a host may take to open a stage's stream.
const STAGE: Duration = Duration::from_secs(60);

/// `path` when it is a downloaded host update in the download
/// service's folder `updates`.
fn downloaded<'a>(path: &'a str, updates: &Path) -> Option<&'a Path> {
    let p = Path::new(path);
    let dir = updates.join("qsvs");
    let name = p.file_name()?.to_str()?;
    let plain = !name.starts_with('.')
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-_.+".contains(&b));
    (p.parent() == Some(dir.as_path()) && plain).then_some(p)
}

impl Inner {
    /// A page of the kept problems.
    pub fn problems_page(&self, filter: &ProblemFilter, skip: u32, rows: u32) -> ProblemPage {
        self.problems().page(filter, skip, rows.clamp(1, 500))
    }

    /// Problem `number` read from its system now, for `user`; what is
    /// kept of it is brought up to date.
    pub fn problem_now(&self, number: u64, user: &str) -> Result<ProblemDetail, Refusal> {
        let row = self
            .problems()
            .get(number)
            .ok_or_else(|| qmc(&qmcmsg::PRB_NO_PROBLEM, &[&number.to_string()]))?;
        let agreed = self.entry(&row.system).map_or(0, |e| e.kept().caps);
        let get = service::Get {
            number: row.summary.number,
        };
        let req = if agreed & caps::PROBLEM_NOTES != 0 {
            service::Request::GetRated(get)
        } else {
            service::Request::Get(get)
        };
        let r = self.ask(&row.system, AnyRequest::Service(req), user)?;
        if let Some(no) = crate::peer::pair::refusal(&r) {
            return Err(no);
        }
        let (severity, problem) = match r.decode() {
            Ok(Some(AnyReply::Service(service::Reply::GetRated(rp)))) => {
                (Some(rp.severity), rp.problem)
            }
            Ok(Some(AnyReply::Service(service::Reply::Get(p)))) => (None, p),
            _ => return Err(qmc(&qmcmsg::NO_REPLY, &[&row.system])),
        };
        self.problems()
            .keep(&row.system, severity, problem.summary.clone());
        let row = self.problems().get(number).unwrap_or(ProblemRow {
            number,
            system: row.system,
            severity,
            summary: problem.summary.clone(),
        });
        Ok(ProblemDetail {
            row,
            resources: problem.resources,
            log: problem.log,
        })
    }

    /// Bring the image at `path` to host `name` for `user`: begun when the
    /// host opened its stream.
    pub fn stage_image(&self, name: &str, path: &str, user: &str) -> Result<(), Refusal> {
        let e = self
            .entry(name)
            .ok_or_else(|| qmc(&qmcmsg::NO_SUCH_SYSTEM, &[name]))?;
        if let Some(what) = mgmtproto::ask::lacks(e.kept().caps, caps::UPDATE) {
            return Err(qmc(&qmcmsg::LACKS, &[&e.name, what]));
        }
        let p = downloaded(path, &self.updates_dir)
            .ok_or_else(|| qmc(&qmcmsg::NOT_DOWNLOADED, &[path]))?;
        let file = std::fs::File::open(p).map_err(|_| qmc(&qmcmsg::NOT_DOWNLOADED, &[path]))?;
        let bytes = file
            .metadata()
            .map_err(|_| qmc(&qmcmsg::NOT_DOWNLOADED, &[path]))?
            .len();
        let image = p
            .file_name()
            .and_then(|f| f.to_str())
            .unwrap_or_default()
            .to_string();
        let (started, rx) = channel();
        let _ = e.tx.send(Cmd::Stage {
            name: image.clone(),
            file,
            bytes,
            user: user.into(),
            started,
        });
        let done = rx
            .recv_timeout(STAGE)
            .unwrap_or_else(|_| Err(qmc(&qmcmsg::NO_REPLY, &[&e.name])));
        self.log(&format!(
            "stage of {image} to {} for user {user}: {}",
            e.name,
            match &done {
                Ok(()) => format!("begun, {bytes} bytes"),
                Err(r) => format!("refused, {} {}", r.main.msgid, r.main.text),
            }
        ));
        done
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn only_a_downloaded_host_image_is_staged() {
        let u = std::path::Path::new("/data/updates");
        assert!(super::downloaded("/data/updates/qsvs/QSVS-V1R0M1-update-ab.upd", u).is_some());
        for bad in [
            "/data/updates/qsmc/QSMC-V1R0M1-update-ab.upd",
            "/data/updates/qsvs/../../state/identity.key",
            "/data/updates/qsvs/.partial",
            "/data/state/identity.key",
            "QSVS-V1R0M1-update-ab.upd",
        ] {
            assert!(super::downloaded(bad, u).is_none(), "{bad}");
        }
    }
}
