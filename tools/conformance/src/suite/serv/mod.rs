// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! Phase 6's suites: serviceable events, dumps and code updates through
//! the web interface and the local console of a console in QEMU, against
//! the simulator's `QSHQVS1` (the whole host family with the prototype's
//! event 1042) and `QSHQ400`, and a stream server of the harness's own
//! ([`crate::streams`]) the console is pointed at. The console's own
//! update installs the test image's variants (`build --test-image
//! --variant next|broken all`).

#![allow(
    clippy::many_single_char_names,
    reason = "a suite's browser, console and page are `b`, `c` and `t` throughout"
)]

mod audit;
pub(crate) mod console;
mod dumps;
mod events;
mod host;
mod local;
mod pages;
mod real;
mod streams;

use std::path::Path;
use std::time::Duration;

use super::base::SuiteFn;
use super::mgmt::{Fleet, HOST, keep, page, until};
use super::parts::{expect, post_form};
use super::web::{ADMIN, Console, PASSWORD, follow, message_of, signed_on, start};
use crate::streams::keys::test_key;
use crate::streams::server::{Release, Server};
use crate::web::Browser;

/// The suites, by the handoff's numbers (1 is every earlier phase's,
/// 17 the fuzzer's).
pub const SUITES: &[(&str, SuiteFn)] = &[
    ("serv-pages", pages::pages),
    ("serv-events", events::events),
    ("serv-own", events::own),
    ("serv-dumps", dumps::dumps),
    ("serv-streams", streams::streams),
    ("serv-host", host::host_update),
    ("serv-console", console::update),
    ("serv-refusals", console::refusals),
    ("serv-power", console::power),
    ("serv-local", local::local),
    ("serv-dumpfile", dumps::dump_file),
    ("serv-sched", host::sched),
    ("serv-roles", pages::roles),
    ("serv-audit", audit::audit),
];

/// The host of the suites.
pub const VS: &str = "QSHQVS1";
/// The standalone system beside it.
pub const STANDALONE: &str = "QSHQ400";
/// The level the `qsvs` stream offers.
pub const QSVS_NEXT: &str = "V1R0M1";
/// Its update image's name.
pub const QSVS_IMAGE: &str = "QSVS-V1R0M1-update-26300.upd";
/// Its notes.
pub const QSVS_NOTES: &str = "QSVS-V1R0M1-notes.txt";
/// The cumulative package QS/400's catalog offers.
pub const CUM: &str = "QS400-CUM-26300";
/// What the console's own row of Code updates holds.
pub const CONSOLE_ROW: &str = ">QS/MC</td>";
/// `VERSION`'s numbers.
fn version() -> [u32; 3] {
    let mut v = include_str!("../../../../../VERSION")
        .trim()
        .split('.')
        .map(|n| n.parse().unwrap_or(0));
    [(); 3].map(|()| v.next().unwrap_or(0))
}

/// The console's release, `VERSION`'s: `V1R1M2`.
pub fn qsmc_here() -> String {
    let [v, r, m] = version();
    format!("V{v}R{r}M{m}")
}

/// The release the test image's update variants are built as, one
/// modification on: `V1R1M3`.
pub fn qsmc_next() -> String {
    qsmc_on(1)
}

/// The release `n` modifications on.
pub fn qsmc_on(n: u32) -> String {
    let [v, r, m] = version();
    format!("V{v}R{r}M{}", m + n)
}

/// A console managing `QSHQVS1` and `QSHQ400`, pointed at a stream
/// server, signed on as the first Administrator.
pub struct Lab<'a> {
    /// The simulated systems: `QSHQVS1` first.
    pub fleet: Fleet,
    /// The console and its browser.
    pub c: Console<'a>,
    /// The stream server.
    pub streams: Server,
}

/// The `qsvs` stream's update image: a QS/VS image's magic, then bytes
/// the simulated host takes as its image (`tools/sim`).
pub fn qsvs_image() -> Vec<u8> {
    let mut v = b"QSVSUPDT".to_vec();
    v.extend((0u32..(1 << 20)).map(|i| (i.wrapping_mul(2_654_435_761) >> 24) as u8));
    v
}

/// The `qsvs` stream's release.
pub fn qsvs_release() -> Release {
    Release {
        release: QSVS_NEXT.into(),
        build: "26300".into(),
        updates_from: "V1R0M0".into(),
        update: (QSVS_IMAGE.into(), qsvs_image()),
        released: "2026-11-01".into(),
        notes: (
            QSVS_NOTES.into(),
            "QS/VS V1R0M1\n\nFixes QV00001: a pool unit's state after a restart.\n".into(),
        ),
    }
}

/// The `qsmc` stream's release of the update image at `path`.
pub fn qsmc_release(path: &Path) -> Result<Release, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let next = qsmc_next();
    Ok(Release {
        release: next.clone(),
        build: "next".into(),
        updates_from: qsmc_here(),
        update: (name, bytes),
        released: "2026-11-20".into(),
        notes: (
            format!("QSMC-{next}-notes.txt"),
            format!("QS/MC {next}\n\nThe test release: web.toml of a newer schema.\n"),
        ),
    })
}

/// Publish the three streams at `sequence`: `qsvs` with
/// [`qsvs_release`], `qsmc` with `qsmc`, QS/400's catalog with [`CUM`].
pub fn publish(s: &Server, sequence: u64, qsmc: &[Release]) {
    s.publish("qsvs", sequence, &[qsvs_release()], &test_key("qsvs"));
    s.publish("qsmc", sequence, qsmc, &test_key("qsmc"));
    s.publish_catalog(sequence, "V1R2M0", CUM, &test_key("qs400"));
}

/// The address the console reaches `s` at.
pub fn server_url(s: &Server) -> String {
    format!("http://{HOST}:{}", s.port())
}

/// Point the console at `url` and check by hand only.
pub fn point(b: &mut Browser, url: &str) -> Result<(), String> {
    let r = post_form(
        b,
        "/qsmc/updates/code",
        ("apply", "Apply"),
        &[("s", url), ("a", "Never")],
    )?;
    expect(b, &r, qmcmsg::UPDATE_SETTINGS.id).map(|_| ())
}

/// Check now: the page it leads to.
pub fn check(b: &mut Browser) -> Result<String, String> {
    let r = post_form(b, "/qsmc/updates/code", ("check_now", "Check now"), &[])?;
    Ok(follow(b, &r)?.text())
}

/// The table row of `html` that holds `name`.
pub fn row<'t>(html: &'t str, name: &str) -> &'t str {
    html.split("<tr")
        .skip(1)
        .find(|r| r.contains(name))
        .and_then(|r| r.split("</tr>").next())
        .unwrap_or("")
}

/// Wait until `row`'s line of the Code updates page says `status`.
pub fn status_is(
    b: &mut Browser,
    row_of: &str,
    status: &str,
    within: Duration,
) -> Result<(), String> {
    until(&format!("{row_of} is {status}"), within, || {
        let t = page(b, "/qsmc/updates/code")?;
        Ok(row(&t, row_of).contains(status))
    })
}

/// Download what the stream offers `target`, from its row's link: the
/// row once it says Downloaded.
pub fn download(b: &mut Browser, target: &str, stream: &str, name: &str) -> Result<(), String> {
    let q = format!("/qsmc/updates/code/download?s={stream}&f={name}");
    let a = b.get(&q)?;
    expect(b, &a, qmcmsg::DOWNLOAD_BEGUN.id)?;
    status_is(b, target, "Downloaded", Duration::from_secs(180))
}

/// A console managing `QSHQVS1` and `QSHQ400`, its streams those of a
/// server of the harness at sequence 2 with `qsmc` the console's
/// releases, checked once.
pub fn lab<'a>(run: &super::base::Run<'a>, tag: &str, qsmc: &[Release]) -> Result<Lab<'a>, String> {
    let fleet = Fleet::start(vec![sim::fixture::qshqvs1(), sim::fixture::qshq400()])?;
    let mut c = start(run, tag)?;
    signed_on(&mut c.browser, ADMIN, PASSWORD)?;
    keep(&mut c.browser, &fleet, 0)?;
    keep(&mut c.browser, &fleet, 1)?;
    let streams = Server::start("127.0.0.1:0");
    publish(&streams, 2, qsmc);
    point(&mut c.browser, &server_url(&streams))?;
    let t = check(&mut c.browser)?;
    if !super::web::shows(&t, qmcmsg::CHECKED.id) {
        return Err(format!("Check now: {}", message_of(&t)));
    }
    ready(&mut c.browser)?;
    Ok(Lab { fleet, c, streams })
}

/// A stream server for a console already signed on in `b`: the three
/// streams at sequence 2, QS/400's catalog's package for `release`,
/// the console pointed at it and checked. The server, kept for as long
/// as it is used.
pub fn lab_streams(b: &mut Browser, release: &str) -> Result<Server, String> {
    let streams = Server::start("127.0.0.1:0");
    publish(&streams, 2, &[]);
    streams.publish_catalog(2, release, CUM, &test_key("qs400"));
    point(b, &server_url(&streams))?;
    let t = check(b)?;
    if !super::web::shows(&t, qmcmsg::CHECKED.id) {
        return Err(format!("Check now: {}", message_of(&t)));
    }
    Ok(streams)
}

/// The suites run by name only: a real host and a real standalone
/// system.
pub const NAMED: &[(&str, SuiteFn)] = &[
    ("serv-real-host", real::host),
    ("serv-real-standalone", real::standalone),
];

/// Wait until what `mgmtd` keeps of both systems' servicing is whole:
/// event 1042 listed, the dumps and the host's slots.
pub fn ready(b: &mut Browser) -> Result<(), String> {
    until(
        "the fleet's servicing read",
        Duration::from_secs(120),
        || {
            let events = page(b, "/qsmc/events?s=All")?;
            let dumps = page(b, "/qsmc/dumps")?;
            let code = page(b, "/qsmc/updates/code")?;
            Ok(events.contains("B6000100")
                && events.contains(STANDALONE)
                && dumps.contains("SYSDUMP.QSTEST01")
                && dumps.contains("FWDUMP.QSHQ400")
                && row(&code, VS).contains("V1R0M0"))
        },
    )
}

/// Restart the console through its control socket and boot it again:
/// the same disks and variable store, signed on again.
pub fn restarted<'a>(run: &super::base::Run<'a>, c: Console<'a>) -> Result<Console<'a>, String> {
    let Console {
        mut guest, machine, ..
    } = c;
    super::base::channel(&mut guest)?.tell(qsmchook::proto::command::RESTART)?;
    guest.wait_exit(std::time::Instant::now() + Duration::from_secs(90))?;
    drop(guest);
    let mut c = super::web::boot(run, machine)?;
    signed_on(&mut c.browser, ADMIN, PASSWORD)?;
    Ok(c)
}

/// The console's clock, Unix seconds.
pub fn clock(guest: &mut crate::qemu::session::Session) -> Result<i64, String> {
    super::base::fact_of(&super::base::channel(guest)?.status()?, release::fact::TIME)?
        .parse()
        .map_err(|_| "the status's time is not a number".to_string())
}

/// Set the console's clock to `secs`.
pub fn set_clock(guest: &mut crate::qemu::session::Session, secs: i64) -> Result<(), String> {
    super::base::channel(guest)?
        .ask(&format!("{} {secs}", qsmchook::proto::command::SET_CLOCK))
        .map(|_| ())
}

/// Kill service `name`; supervision starts it again.
pub fn kill(guest: &mut crate::qemu::session::Session, name: &str) -> Result<(), String> {
    // One killed a moment ago is started again first.
    let mut pid = 0;
    until(&format!("{name} running"), Duration::from_secs(60), || {
        let st = super::base::channel(guest)?.status()?;
        pid = st.service(name).map_or(0, |s| s.pid);
        Ok(pid != 0)
    })?;
    super::base::channel(guest)?.ask(&format!("{} {name}", qsmchook::proto::command::KILL))?;
    // Gone, so that a status read next is not the killed one's.
    until(
        &format!("{name} {pid} gone"),
        Duration::from_secs(30),
        || {
            let st = super::base::channel(guest)?.status()?;
            Ok(st.service(name).is_none_or(|s| s.pid != pid))
        },
    )
}

/// Post `code` through the control socket, about `about`
/// (`kind:name[:serial]`, or empty), with `text`.
pub fn post_code(
    guest: &mut crate::qemu::session::Session,
    code: &str,
    about: &str,
    text: &str,
) -> Result<(), String> {
    let line = if about.is_empty() {
        format!("{} {code} {text}", qsmchook::proto::command::POST)
    } else {
        format!("{} {code} {about} {text}", qsmchook::proto::command::POST)
    };
    super::base::channel(guest)?.ask(&line).map(|_| ())
}

/// The console's number of the event whose row holds `code` on the
/// list, with all statuses shown.
pub fn number_of(b: &mut Browser, code: &str) -> Result<u64, String> {
    let t = page(b, "/qsmc/events?s=All&rows=100")?;
    let r = row(&t, code);
    r.split("/qsmc/event/")
        .nth(1)
        .and_then(|rest| rest.split('"').next())
        .and_then(|n| n.parse().ok())
        .ok_or_else(|| format!("no event with {code} listed"))
}
