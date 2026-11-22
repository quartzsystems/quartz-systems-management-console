// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! Code updates and Updates (`docs/UPDATES.md`): what each product
//! stream offers the console and each host, installs and their progress,
//! the fleet's levels on one page.
//!
//! - [`code`]: Code updates: the console and every host, the settings,
//!   Check now, Download, Install and its confirmation, Schedule, notes.
//! - [`progress`]: an install's steps, reloading, and its log.
//! - [`fleet`]: Updates: the console, hosts, partitions and standalone
//!   systems with their levels.
//!
//! Every byte of a stream comes through the download service; an install
//! is run in the background with the scheduler's own steps
//! (`schedd::update`), so the page and a scheduled install check alike.

pub mod code;
pub mod fleet;
pub mod progress;

use std::sync::Arc;

use fetchd::Stream;
use fetchd::proto::{Entry, FileState, Status as Fetched};
use mgmtproto::Kept;
use qsweb::msg::{Kind, Message};

use crate::app::routes::Rq;

/// An install begun from the page: who, when, and how it ended.
#[derive(Clone, Debug, Default)]
pub struct Run {
    /// The signed-on user.
    pub user: String,
    /// When it began, Unix seconds.
    pub began: u64,
    /// It ended: what ran, or why not.
    pub done: Option<Result<String, msg::Issued>>,
}

/// The target of the console's own install, in routes.
pub const CONSOLE: &str = "console";

/// What the download service says now, or why it could not be asked.
pub fn fetched(rq: &Rq<'_>) -> Result<Fetched, Message> {
    rq.app.console.fetch().status().map_err(|e| {
        qmcmsg::message(
            Kind::Error,
            &qmcmsg::SERVICE_DOWN,
            &["The download service", &e.to_string()],
        )
    })
}

/// `V1R0M0 build 26290`.
pub fn level(release: &str, build: &str) -> String {
    if build.is_empty() {
        release.to_string()
    } else {
        format!("{release} build {build}")
    }
}

/// What a stream knows, by name.
pub fn known(f: &Fetched, s: Stream) -> Option<&fetchd::proto::Known> {
    f.streams.iter().find(|k| k.stream == s && k.sequence > 0)
}

/// Whether `entry`'s image is downloaded.
pub fn downloaded(f: &Fetched, s: Stream, entry: &Entry) -> bool {
    f.files
        .iter()
        .any(|x| x.stream == s && x.name == entry.update && x.state == FileState::Verified)
}

/// Whether `entry`'s image is being downloaded.
pub fn downloading(f: &Fetched, s: Stream, entry: &Entry) -> bool {
    f.files
        .iter()
        .any(|x| x.stream == s && x.name == entry.update && x.state == FileState::Downloading)
}

/// A host's release and build that run, as it says them.
pub fn host_level(k: &Kept) -> (String, String) {
    if let Some(s) = k
        .update
        .as_ref()
        .and_then(|u| u.slots.iter().find(|s| s.running))
    {
        return (s.release.clone(), s.build.clone());
    }
    let release = k
        .state
        .as_ref()
        .map(|s| s.release.clone())
        .or_else(|| k.identity.as_ref().map(|i| i.release.clone()))
        .unwrap_or_default();
    let build = k
        .identity
        .as_ref()
        .map(|i| i.build.clone())
        .unwrap_or_default();
    (release, build)
}

/// What the stream offers a system at `release`: the newest entry
/// past it, when there is one.
pub fn offered<'a>(f: &'a Fetched, s: Stream, release: &str) -> Option<&'a Entry> {
    let r: vrm::Release = release.parse().ok()?;
    fetchd::proto::newest_for(&known(f, s)?.entries, r)
}

/// `YYYY-MM-DD` as the console shows a date.
pub fn date_of(rq: &Rq<'_>, ymd: &str) -> String {
    let mut parts = ymd.split('-').map(str::parse::<i64>);
    let (Some(Ok(year)), Some(Ok(month)), Some(Ok(day))) =
        (parts.next(), parts.next(), parts.next())
    else {
        return String::new();
    };
    let t = appliance::time::from_civil(year, month, day, 12, 0, 0);
    crate::fmt::date(
        t,
        appliance::time::Zone::UTC,
        &rq.app.settings().date_format,
    )
}

/// The installs begun from the pages, by target.
pub type Runs = std::sync::Mutex<std::collections::BTreeMap<String, Run>>;

/// Begin the install of `target` (a host's name, or [`CONSOLE`]) on a
/// thread of its own, for the signed-on user; `timeout` the seconds each
/// running partition is given to shut down, 0 to refuse while any runs.
pub fn begin(rq: &Rq<'_>, target: &str, timeout: u32) {
    let user = rq.user().to_string();
    let console_name = rq.app.console.name();
    let mut mgmt = rq.app.console.mgmt();
    let mut fetch = rq.app.console.fetch();
    let mut updd = rq.app.console.updd();
    let runs: Arc<Runs> = rq.app.installs.clone();
    let key = target.to_string();
    runs.lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .insert(
            key.clone(),
            Run {
                user: user.clone(),
                began: rq.app.console.now(),
                done: None,
            },
        );
    std::thread::spawn(move || {
        let mut reach = schedd::update::Reach {
            mgmt: &mut *mgmt,
            fetch: &mut *fetch,
            updd: &mut *updd,
            look: schedd::update::USUAL,
        };
        let done = if key == CONSOLE {
            schedd::update::install_console(&mut reach, &console_name, &user)
        } else {
            schedd::update::install_host(&mut reach, &key, &user, timeout)
        };
        if let Some(r) = runs
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get_mut(&key)
        {
            r.done = Some(done);
        }
    });
}
