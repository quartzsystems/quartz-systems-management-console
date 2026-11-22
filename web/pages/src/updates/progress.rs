// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! An install's progress (`code-update-progress.html`), for a host or
//! for the console across its own restart: its steps (download, stage,
//! verify, the system plan the design saves first, install, activate,
//! restart, confirm), each with when it began and ended, read from what
//! `mgmtd` keeps of the host or from the console's update service; the
//! page reloads every 10 seconds; View log shows what the update did.

use std::fmt::Write as _;

use fetchd::Stream;
use mgmtproto::kept::UpdateLine;
use qsweb::frame::{Ctx, Page};
use qsweb::html::esc;
use qsweb::http::Response;
use qsweb::msg::{Kind, Message};
use qsweb::partials::{Button, Cell, Col, List, buttons, list_table, status};

use super::{CONSOLE, Run, fetched, host_level, level, offered};
use crate::app::routes::Rq;

/// How a step stands.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Stand {
    /// Not begun.
    #[default]
    Pending,
    /// Under way.
    Going,
    /// Done.
    Complete,
    /// It failed.
    Failed,
    /// Not done in this release.
    Skipped,
}

/// One step.
#[derive(Clone, Debug, Default)]
pub struct Step {
    /// What it is.
    pub step: String,
    /// How it stands.
    pub stand: Stand,
    /// When it began, `HH:MM:SS`.
    pub started: String,
    /// When it ended.
    pub ended: String,
    /// What it said.
    pub message: String,
    /// What the step made, linked: its name and route (the plan saved).
    pub link: Option<(String, String)>,
}

/// What the page shows.
#[derive(Clone, Debug, Default)]
pub struct Progress {
    /// What is updated.
    pub target: String,
    /// The steps.
    pub steps: Vec<Step>,
    /// The message rows: what is installed where first.
    pub messages: Vec<Message>,
}

/// The page.
pub fn page(ctx: &Ctx, v: &Progress) -> Page {
    let cols = ["Step", "Status", "Started", "Ended", "Message"]
        .map(Col::plain)
        .to_vec();
    let rows = v
        .steps
        .iter()
        .map(|s| {
            let st = match s.stand {
                Stand::Pending => "Pending".to_string(),
                Stand::Going => "In progress".to_string(),
                Stand::Complete => status("Complete", Some(true)),
                Stand::Failed => status("Failed", Some(false)),
                Stand::Skipped => "Skipped".to_string(),
            };
            vec![
                match &s.link {
                    Some((label, route)) => Cell::html(format!(
                        "{} <a href=\"{}\">{}</a>",
                        esc(&s.step),
                        esc(&ctx.url(route)),
                        esc(label)
                    )),
                    None => Cell::text(&s.step),
                },
                Cell::html(st),
                Cell::text(&s.started),
                Cell::text(&s.ended),
                Cell::text(&s.message),
            ]
        })
        .collect();
    let body = format!(
        "{}{}",
        list_table(&List {
            cols,
            rows,
            ..List::default()
        }),
        qsweb::partials::form(
            ctx,
            "",
            &buttons(&[Button::bare("Refresh"), Button::new("log", "View log")])
        )
    );
    Page {
        title: format!("Install update: {}", v.target),
        crumb: vec![
            "Service Management".into(),
            "Code updates".into(),
            "Install".into(),
        ],
        h1: format!("Install update: {}", v.target),
        messages: v.messages.clone(),
        body,
        help: "code-updates".into(),
        nav: Some(("Service Management", Some("/updates/code"))),
        refresh: Some(10),
        ..Page::default()
    }
}

fn clock(rq: &Rq<'_>, secs: u64) -> String {
    if secs == 0 {
        return String::new();
    }
    let s = rq.app.settings();
    crate::fmt::time(
        i64::try_from(secs).unwrap_or(0),
        rq.app.host().zone(),
        &s.time_format,
        true,
    )
}

fn step(name: impl Into<String>, stand: Stand) -> Step {
    Step {
        step: name.into(),
        stand,
        ..Step::default()
    }
}

/// The step of the plan saved before the install (`docs/PLANS.md`):
/// the host's newest automatic plan since its image's transfer began,
/// linked; under way once the image verified, until it is saved.
fn saved_plan(rq: &Rq<'_>, name: &str, k: &mgmtproto::Kept, verified: bool) -> Step {
    let began = k.staging.as_ref().map(|g| g.began);
    let saved = began.and_then(|began| {
        rq.app
            .console
            .sched()
            .plans()
            .ok()?
            .0
            .into_iter()
            .filter(|e| e.automatic && e.system == name && e.created >= began)
            .max_by_key(|e| e.created)
    });
    let mut s = step("Save system plan", Stand::Pending);
    if let Some(e) = saved {
        s.stand = Stand::Complete;
        s.started = clock(rq, e.created);
        s.ended = clock(rq, e.created);
        s.link = Some((e.name.clone(), crate::plans::route_of(&e.name, "")));
    } else if verified {
        s.stand = Stand::Going;
    }
    s
}

/// The last line of `step` in a host's log.
fn line<'a>(log: &'a [UpdateLine], step: &str) -> Option<&'a UpdateLine> {
    log.iter().rev().find(|l| l.step == step)
}

/// A host's steps, from what `mgmtd` keeps of it.
#[allow(clippy::too_many_lines, reason = "a step a block")]
fn host(rq: &Rq<'_>, name: &str, run: Option<&Run>) -> Result<Progress, Message> {
    let k = crate::systems::system(rq, name)
        .ok_or_else(|| qmcmsg::message(Kind::Error, &qmcmsg::NO_SUCH_SYSTEM, &[name]))?;
    let f = fetched(rq)?;
    let (release, build) = host_level(&k);
    let wanted = run.map_or(0, |r| r.began);
    let log: Vec<UpdateLine> = k
        .update_log
        .iter()
        .filter(|l| l.time >= wanted)
        .cloned()
        .collect();
    let entry = offered(&f, Stream::Qsvs, &release).cloned();
    let mut steps = Vec::new();
    let what = entry
        .as_ref()
        .map_or_else(|| level(&release, &build), |e| level(&e.release, &e.build));
    // Download: the confirmation needed it done.
    let mut dl = step(
        entry.as_ref().map_or_else(
            || "Download".to_string(),
            |e| {
                format!(
                    "Download {} ({} MB)",
                    level(&e.release, &e.build),
                    e.update_bytes.div_ceil(1_048_576)
                )
            },
        ),
        Stand::Complete,
    );
    if let Some(file) = entry
        .as_ref()
        .and_then(|e| f.files.iter().find(|x| x.name == e.update))
    {
        dl.ended = clock(rq, file.time);
    }
    steps.push(dl);
    let mut stage = step(format!("Transfer to {name}"), Stand::Pending);
    if let Some(g) = &k.staging {
        stage.started = clock(rq, g.began);
        stage.stand = if g.error.is_some() {
            Stand::Failed
        } else if g.done {
            Stand::Complete
        } else {
            Stand::Going
        };
        stage.message = g.error.as_ref().map_or_else(
            || {
                format!(
                    "{} of {} MB",
                    g.sent.div_ceil(1_048_576),
                    g.bytes.div_ceil(1_048_576)
                )
            },
            |n| format!("{} {}", n.msgid, n.text),
        );
    }
    if let Some(l) = line(&log, "stage") {
        stage.ended = clock(rq, l.time);
    }
    steps.push(stage);
    let mut verify = step("Verify signature", Stand::Pending);
    if let Some(l) = line(&log, "verify") {
        verify.stand = if l.failed {
            Stand::Failed
        } else {
            Stand::Complete
        };
        verify.ended = clock(rq, l.time);
        verify.message = if l.failed {
            l.text.clone()
        } else {
            "Signed by a key the host trusts".into()
        };
    } else if k.staging.as_ref().is_some_and(|g| g.done) {
        verify.stand = Stand::Going;
    }
    let verified = verify.stand == Stand::Complete;
    steps.push(verify);
    steps.push(saved_plan(rq, name, &k, verified));
    let mut install = step("Install on host", Stand::Pending);
    if let Some(p) = k.update.as_ref().and_then(|u| u.installing.as_ref()) {
        install.stand = Stand::Going;
        install.message = format!("{}% of slot {}", p.percent, p.slot);
    }
    if let Some(l) = line(&log, "install") {
        install.stand = if l.failed {
            Stand::Failed
        } else {
            Stand::Complete
        };
        install.ended = clock(rq, l.time);
        install.message.clone_from(&l.text);
    }
    steps.push(install);
    let mut activate = step(format!("Activate: restart {name}"), Stand::Pending);
    if let Some(l) = line(&log, "activate") {
        activate.stand = Stand::Complete;
        activate.ended = clock(rq, l.time);
        activate.message.clone_from(&l.text);
    }
    let activated = activate.stand == Stand::Complete;
    steps.push(activate);
    let mut confirm = step("Confirm", Stand::Pending);
    if let Some(e) = k.update_end.as_ref().filter(|e| e.time >= wanted) {
        confirm.stand = if e.confirmed {
            Stand::Complete
        } else {
            Stand::Failed
        };
        confirm.ended = clock(rq, e.time);
        confirm.message = if e.confirmed {
            format!("{} confirmed", level(&e.release, &e.build))
        } else {
            format!("rolled back: {}", e.reason)
        };
    } else if activated {
        confirm.stand = Stand::Going;
        confirm.message = "The host is restarting; it confirms or rolls back by itself".into();
    }
    let restarted = activated
        && (confirm.stand != Stand::Pending && k.link == mgmtproto::kept::Link::Connected);
    let mut restart = step(
        "Restart and reconnect",
        if restarted {
            Stand::Complete
        } else {
            Stand::Pending
        },
    );
    if activated && !restarted {
        restart.stand = Stand::Going;
    }
    steps.push(restart);
    steps.push(confirm);
    let mut messages = vec![qmcmsg::message(
        Kind::Info,
        &qmcmsg::INSTALLING,
        &[&format!("QS/VS {what}"), name],
    )];
    if let Some(Some(Err(i))) = run.map(|r| r.done.as_ref()) {
        messages.push(qsweb::msg::Message::new(Kind::Error, &i.id, &i.text));
    }
    Ok(Progress {
        target: name.into(),
        steps,
        messages,
    })
}

/// The console's own steps, from its update service, across its
/// restart.
#[allow(clippy::too_many_lines, reason = "a step a block")]
fn console(rq: &Rq<'_>, run: Option<&Run>) -> Result<Progress, Message> {
    let st = rq.app.console.updd().status().map_err(|e| {
        qmcmsg::message(
            Kind::Error,
            &qmcmsg::SERVICE_DOWN,
            &["The update service", &e.to_string()],
        )
    })?;
    let running = st
        .slots
        .iter()
        .find(|s| s.running)
        .cloned()
        .unwrap_or_default();
    let other = st
        .slots
        .iter()
        .find(|s| !s.running)
        .cloned()
        .unwrap_or_default();
    let name = rq.app.console.name();
    let mut steps = Vec::new();
    let staged = st.staged.clone();
    let what = staged
        .as_ref()
        .and_then(|s| s.image.as_ref())
        .map(|i| level(&i.release, &i.build))
        .or_else(|| {
            (other.last == update::proto::Attempt::Pending)
                .then(|| level(&other.release, &other.build))
        })
        .unwrap_or_else(|| level(&running.release, &running.build));
    steps.push(step(format!("Download {what}"), Stand::Complete));
    let pending = other.last == update::proto::Attempt::Pending;
    // After the restart the new release runs in the slot that was
    // pending, confirmed since the install began (or, its run forgotten
    // with the restart of the web server, within the day).
    let now = rq.app.console.now();
    let after = running.last == update::proto::Attempt::Confirmed
        && run.map_or(now.saturating_sub(running.time) < 86_400, |r| {
            running.time >= r.began
        });
    let (stage, verify) = match &staged {
        Some(s) => {
            let v = match s.verification {
                update::proto::Verification::Verified => Stand::Complete,
                update::proto::Verification::Refused => Stand::Failed,
                _ => Stand::Going,
            };
            (Stand::Complete, v)
        }
        None if pending || after => (Stand::Complete, Stand::Complete),
        None => (Stand::Pending, Stand::Pending),
    };
    let mut s = step("Stage on the console", stage);
    s.ended = staged
        .as_ref()
        .map(|x| clock(rq, x.time))
        .unwrap_or_default();
    steps.push(s);
    let mut v = step("Verify signature", verify);
    if let Some(e) = staged.as_ref().and_then(|x| x.error.first()) {
        v.message = format!("{} {}", e.id, e.text);
    } else if verify == Stand::Complete {
        v.message = "Signed by a key the console trusts".into();
    }
    steps.push(v);
    let installing = st.installing.is_some();
    let mut install = step(
        "Install into the other slot",
        if installing {
            Stand::Going
        } else if pending || after {
            Stand::Complete
        } else {
            Stand::Pending
        },
    );
    if let Some(p) = &st.installing {
        install.message = format!("{}% of slot {}", p.percent, p.slot);
    } else if pending {
        install.message = format!("slot {} is pending", other.name);
    }
    steps.push(install);
    let activated = run.and_then(|r| r.done.as_ref()).is_some_and(Result::is_ok);
    steps.push(step(
        "Activate: restart the console",
        if activated || after {
            Stand::Complete
        } else {
            Stand::Pending
        },
    ));
    steps.push(step(
        "Restart",
        if after {
            Stand::Complete
        } else if activated {
            Stand::Going
        } else {
            Stand::Pending
        },
    ));
    let mut confirm = step("Confirm", Stand::Pending);
    if other.last == update::proto::Attempt::RolledBack && run.is_none_or(|r| other.time >= r.began)
    {
        confirm.stand = Stand::Failed;
        confirm.ended = clock(rq, other.time);
        confirm.message = format!("rolled back: {}", other.reason);
    } else if after {
        confirm.stand = Stand::Complete;
        confirm.ended = clock(rq, running.time);
        confirm.message = format!("{} confirmed", level(&running.release, &running.build));
    }
    steps.push(confirm);
    let mut messages = vec![qmcmsg::message(
        Kind::Info,
        &qmcmsg::INSTALLING,
        &[&format!("QS/MC {what}"), &name],
    )];
    if let Some(Some(Err(i))) = run.map(|r| r.done.as_ref()) {
        messages.push(qsweb::msg::Message::new(Kind::Error, &i.id, &i.text));
    }
    Ok(Progress {
        target: name,
        steps,
        messages,
    })
}

/// `/updates/code/<target>/progress`, and View log.
pub fn route(rq: &Rq<'_>, target: &str) -> Response {
    if rq.post() {
        let to = if rq.req.form().get("log").is_some() {
            format!("/updates/code/{target}/log")
        } else {
            format!("/updates/code/{target}/progress")
        };
        return Response::redirect(&rq.ctx.url(&to));
    }
    let run = rq
        .app
        .installs
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .get(target)
        .cloned();
    let v = if target == CONSOLE {
        console(rq, run.as_ref())
    } else {
        host(rq, target, run.as_ref())
    };
    match v {
        Ok(v) => rq.page(page(&rq.ctx, &v)),
        Err(m) => rq.done("/updates/code", m),
    }
}

/// `/updates/code/<target>/log`: what the update did, a line each.
pub fn log(rq: &Rq<'_>, target: &str) -> Response {
    let mut lines = Vec::new();
    if target == CONSOLE {
        if let Ok(st) = rq.app.console.updd().status() {
            for s in &st.slots {
                lines.push(format!(
                    "slot {}: {} build {}{}, last activation {:?}{}",
                    s.name,
                    s.release,
                    s.build,
                    if s.running { ", running" } else { "" },
                    s.last,
                    if s.reason.is_empty() {
                        String::new()
                    } else {
                        format!(": {}", s.reason)
                    }
                ));
            }
            if let Some(s) = &st.staged {
                lines.push(format!(
                    "staged {} ({} bytes): {:?}",
                    s.name, s.bytes, s.verification
                ));
                for e in &s.error {
                    lines.push(format!("  {} {}", e.id, e.text));
                }
            }
        }
    } else if let Some(k) = crate::systems::system(rq, target) {
        for l in &k.update_log {
            lines.push(format!(
                "{}  {:<8} {}{}",
                crate::systems::when(rq, l.time),
                l.step,
                l.text,
                if l.failed { " (failed)" } else { "" }
            ));
        }
    }
    if let Some(r) = rq
        .app
        .installs
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .get(target)
    {
        let _ = r.done.as_ref().map(|d| {
            lines.push(match d {
                Ok(what) => format!("by {}: {what}", r.user),
                Err(i) => format!("by {}: {} {}", r.user, i.id, i.text),
            });
        });
    }
    if lines.is_empty() {
        lines.push("Nothing is known yet of an update here.".into());
    }
    let mut body = format!(
        "<pre class=\"out\">{}</pre>",
        lines.iter().map(|l| esc(l)).collect::<Vec<_>>().join("\n")
    );
    let _ = write!(
        body,
        "<p><a href=\"{}\">Back to the progress page</a></p>",
        esc(&rq.ctx.url(&format!("/updates/code/{target}/progress")))
    );
    rq.page(Page {
        title: format!("Update log: {target}"),
        crumb: vec![
            "Service Management".into(),
            "Code updates".into(),
            "Log".into(),
        ],
        h1: format!("Update log: {target}"),
        body,
        help: "code-updates".into(),
        nav: Some(("Service Management", Some("/updates/code"))),
        ..Page::default()
    })
}
