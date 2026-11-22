// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! Installing a code update (`docs/UPDATES.md`): what an install of a
//! host's or the console's update would do, or why it cannot be, which
//! the Code updates page asks before its confirmation and a scheduled
//! install asks again at its time; and the scheduled install itself:
//! the downloaded image staged, verified by the system, installed into
//! its other slot, then activated.

use std::time::{Duration, Instant};

use fetchd::Stream;
use fetchd::proto::{Entry, FileState, Status as Fetched};
use mgmtproto::Kept;
use mgmtproto::kept::{Kind, Link};
use msg::Issued;
use qsi::caps;

/// How often a step's end is looked for.
const LOOK: Duration = Duration::from_secs(2);
/// How long staging, verifying or installing may take.
const STEP: Duration = Duration::from_mins(15);

fn issue(d: &qmcmsg::Def, values: &[&str]) -> Issued {
    qmcmsg::base_of(d).with(values)
}

/// An install the console can do.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Plan {
    /// What is updated: the host's name, or the console's.
    pub target: String,
    /// The release that runs there.
    pub from: String,
    /// What the stream offers it.
    pub entry: Entry,
    /// The downloaded image, readable by the services.
    pub path: String,
    /// What runs that an activation shuts down: each partition's ID and
    /// name, a host's.
    pub running: Vec<(u32, String)>,
}

/// The newest release `stream` offers `from`, and whether its image is
/// downloaded: its path.
fn offered(f: &Fetched, stream: Stream, from: vrm::Release) -> Result<(Entry, String), Issued> {
    let k = f
        .streams
        .iter()
        .find(|k| k.stream == stream && k.sequence > 0)
        .ok_or_else(|| issue(&qmcmsg::NOT_CHECKED, &[stream.name()]))?;
    let e = fetchd::proto::newest_for(&k.entries, from)
        .ok_or_else(|| issue(&qmcmsg::AT_NEWEST, &[&from.to_string(), stream.name()]))?
        .clone();
    let have = f
        .files
        .iter()
        .any(|x| x.stream == stream && x.name == e.update && x.state == FileState::Verified);
    if !have {
        return Err(issue(&qmcmsg::DOWNLOAD_FIRST, &[&e.update]));
    }
    let path = format!("{}/{}/{}", release::UPDATES_DIR, stream.name(), e.update);
    Ok((e, path))
}

/// What installing host `k`'s update would do, by what the download
/// service holds `f`.
pub fn host_plan(k: &Kept, f: &Fetched) -> Result<Plan, Issued> {
    if k.kind == Kind::Standalone {
        return Err(issue(&qmcmsg::NO_QSI_UPDATE, &[&k.name]));
    }
    if k.link != Link::Connected {
        let why = if k.reason.is_empty() {
            "the console has not reached it yet"
        } else {
            k.reason.as_str()
        };
        return Err(issue(&qmcmsg::NOT_CONNECTED, &[&k.name, why]));
    }
    if let Some(what) = mgmtproto::ask::lacks(k.caps, caps::UPDATE) {
        return Err(issue(&qmcmsg::LACKS, &[&k.name, what]));
    }
    let release = k
        .state
        .as_ref()
        .map(|s| s.release.clone())
        .or_else(|| k.identity.as_ref().map(|i| i.release.clone()))
        .unwrap_or_default();
    let from: vrm::Release = release
        .parse()
        .map_err(|_| issue(&qmcmsg::NOT_CHECKED, &[Stream::Qsvs.name()]))?;
    let (entry, path) = offered(f, Stream::Qsvs, from)?;
    let running = k
        .partitions
        .iter()
        .filter(|p| {
            !matches!(
                p.state,
                qsi::host::model::PartitionState::NotActivated
                    | qsi::host::model::PartitionState::Error
            )
        })
        .map(|p| (p.id, p.name.clone()))
        .collect();
    Ok(Plan {
        target: k.name.clone(),
        from: release,
        entry,
        path,
        running,
    })
}

/// What installing the console's own update would do: `name` is the
/// console's, `running` its release.
pub fn console_plan(name: &str, f: &Fetched, running: vrm::Release) -> Result<Plan, Issued> {
    let (entry, path) = offered(f, Stream::Qsmc, running)?;
    Ok(Plan {
        target: name.into(),
        from: running.to_string(),
        entry,
        path,
        running: Vec::new(),
    })
}

/// How the scheduled install reaches each service.
pub struct Reach<'a> {
    /// `mgmtd`.
    pub mgmt: &'a mut dyn mgmtproto::Calls,
    /// `fetchd`.
    pub fetch: &'a mut dyn fetchd::proto::Calls,
    /// The console's update service.
    pub updd: &'a mut dyn update::proto::Calls,
    /// How long to wait between looks; tests wait none.
    pub look: Duration,
}

fn down(what: &str, e: &ipc::WireError) -> Issued {
    issue(&qmcmsg::SERVICE_DOWN, &[what, &e.to_string()])
}

fn first(v: Vec<Issued>) -> Issued {
    v.into_iter().next().unwrap_or_default()
}

/// Wait until `done` says the step ended, `look` between: its outcome.
fn wait<T>(
    look: Duration,
    mut done: impl FnMut() -> Option<Result<T, Issued>>,
    late: &Issued,
) -> Result<T, Issued> {
    let until = Instant::now() + STEP;
    loop {
        if let Some(r) = done() {
            return r;
        }
        if Instant::now() >= until {
            return Err(late.clone());
        }
        std::thread::sleep(look);
    }
}

/// Install host `system`'s update as `owner`: staged from its download,
/// verified, installed, then activated. Partitions that run are shut
/// down through their systems when `timeout` is past 0, each given that
/// long; the install is refused while they run otherwise. What ran.
pub fn install_host(
    r: &mut Reach<'_>,
    system: &str,
    owner: &str,
    timeout: u32,
) -> Result<String, Issued> {
    let k = r
        .mgmt
        .system(system.to_string())
        .map_err(|e| down("the managed-systems service", &e))?
        .ok_or_else(|| issue(&qmcmsg::NO_SUCH_SYSTEM, &[system]))?;
    let f = fetchd::proto::Calls::status(r.fetch).map_err(|e| down("the download service", &e))?;
    let plan = host_plan(&k, &f)?;
    if !plan.running.is_empty() && timeout == 0 {
        let names: Vec<&str> = plan.running.iter().map(|(_, n)| n.as_str()).collect();
        return Err(issue(&qmcmsg::PARTITIONS_RUN, &[system, &names.join(", ")]));
    }
    let refused = |n: &mgmtproto::Refusal| Issued {
        id: n.main.msgid.clone(),
        text: n.main.text.clone(),
        help: n.main.help.clone(),
    };
    r.mgmt
        .stage(system.to_string(), plan.path.clone(), owner.to_string())
        .map_err(|e| down("the managed-systems service", &e))?
        .map_err(|n| refused(&n))?;
    let late = issue(&qmcmsg::NO_REPLY, &[system]);
    let look = r.look;
    let mgmt = &mut *r.mgmt;
    wait(
        look,
        || {
            let k = mgmt.system(system.to_string()).ok().flatten()?;
            if let Some(n) = k.staging.as_ref().and_then(|g| g.error.clone()) {
                return Some(Err(Issued {
                    id: n.msgid,
                    text: n.text,
                    help: n.help,
                }));
            }
            let staged = k.update.as_ref()?.staged.as_ref()?;
            match staged.verification {
                qsi::update::Verification::Verified => Some(Ok(())),
                qsi::update::Verification::Refused => Some(Err(staged.error.as_ref().map_or_else(
                    || late.clone(),
                    |n| Issued {
                        id: n.msgid.clone(),
                        text: n.text.clone(),
                        help: n.help.clone(),
                    },
                ))),
                _ => None,
            }
        },
        &late,
    )?;
    let ask = |mgmt: &mut dyn mgmtproto::Calls, req: qsi::update::Request| {
        let asked = mgmtproto::Asked::of(&qsi::any::AnyRequest::Update(req));
        mgmt.ask(system.to_string(), asked, owner.to_string())
            .map_err(|e| down("the managed-systems service", &e))?
            .map_err(|n| refused(&n))
    };
    ask(&mut *r.mgmt, qsi::update::Request::Install(qsi::Empty))?;
    let mgmt = &mut *r.mgmt;
    wait(
        look,
        || {
            let k = mgmt.system(system.to_string()).ok().flatten()?;
            if let Some(l) = k.update_log.last().filter(|l| l.step == "install")
                && l.failed
            {
                return Some(Err(Issued {
                    id: String::new(),
                    text: l.text.clone(),
                    help: String::new(),
                }));
            }
            let u = k.update.as_ref()?;
            let pending = u.installing.is_none()
                && u.slots
                    .iter()
                    .any(|s| s.last == qsi::update::Attempt::Pending && !s.running);
            pending.then_some(Ok(()))
        },
        &late,
    )?;
    let partitions = if timeout > 0 {
        qsi::update::Partitions::ShutDown
    } else {
        qsi::update::Partitions::Refuse
    };
    ask(
        &mut *r.mgmt,
        qsi::update::Request::Activate(qsi::update::Activate {
            partitions,
            timeout,
        }),
    )?;
    Ok(format!(
        "{} {} installed on {system} and activated",
        Stream::Qsvs.product(),
        plan.entry.release
    ))
}

/// Install the console's own update as `owner`: staged from its
/// download, installed into the other slot, activated by a restart.
pub fn install_console(r: &mut Reach<'_>, name: &str, owner: &str) -> Result<String, Issued> {
    let f = fetchd::proto::Calls::status(r.fetch).map_err(|e| down("the download service", &e))?;
    let plan = console_plan(name, &f, release::RELEASE)?;
    // An image left staged by an install that never ended (the power
    // lost) is put out: this one is staged again from its download.
    let _ = r.updd.discard();
    let staged = r
        .updd
        .stage_file(plan.path.clone(), false)
        .map_err(|e| down("the update service", &e))?
        .map_err(first)?;
    if staged.verification != update::proto::Verification::Verified {
        return Err(staged.error.into_iter().next().unwrap_or_else(|| {
            issue(
                &qmcmsg::UPD_NOT_VERIFIED,
                &[&staged.name, "it did not verify"],
            )
        }));
    }
    r.updd
        .install()
        .map_err(|e| down("the update service", &e))?
        .map_err(first)?;
    let late = issue(
        &qmcmsg::SERVICE_DOWN,
        &["the update service", "the install did not end"],
    );
    let look = r.look;
    let updd = &mut *r.updd;
    wait(
        look,
        || {
            let s = updd.status().ok()?;
            let pending = s.installing.is_none()
                && s.slots
                    .iter()
                    .any(|x| x.last == update::proto::Attempt::Pending && !x.running);
            pending.then_some(Ok(()))
        },
        &late,
    )?;
    r.updd
        .activate(false, 0, owner.to_string())
        .map_err(|e| down("the update service", &e))?
        .map_err(first)?;
    Ok(format!(
        "{} {} installed on the console, which is restarting into it",
        Stream::Qsmc.product(),
        plan.entry.release
    ))
}

/// Wait no time between looks: for tests.
pub const NOW: Duration = Duration::ZERO;
/// The usual wait.
pub const USUAL: Duration = LOOK;
