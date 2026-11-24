// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! Deploy system plan (`plan-deploy-1.html` to `-3.html`,
//! `plan-deploy-progress.html`): the target with its partitions counted,
//! the validation of every item against it now, the partitions to
//! create with "Activate created partitions", the confirmation
//! (`/confirm/plan-deploy`), then the progress page with its steps,
//! reloading every 10 seconds, with Cancel deployment and Close. The
//! wizard's choices travel in the query string (`?t=` the target,
//! `&c=` the partitions, `&act=1`), so no step holds anything.

use std::fmt::Write as _;

use qsweb::frame::{Ctx, Page};
use qsweb::html::esc;
use qsweb::http::Response;
use qsweb::msg::{Kind, Message};
use qsweb::partials::{Button, Cell, Col, List, buttons, list_table, message_row, status, wizard};

use plan::Status;
use plan::doc::{Mode, Plan};
use plan::job::{Job, JobState, StepState};
use plan::validate::{Item, Kind as ItemKind, thousands};

use super::{NAV, TITLE, clock, down, issued, route_of};
use crate::app::routes::Rq;

/// The wizard's steps.
pub const STEPS: [&str; 4] = [
    "Target managed system",
    "Validation",
    "Partitions to create",
    "Deploy",
];

fn frame(name: &str, step: usize, desc: &str, body: String, messages: Vec<Message>) -> Page {
    let mut crumb = vec![
        NAV.to_string(),
        TITLE.to_string(),
        name.to_string(),
        "Deploy".to_string(),
    ];
    if step == 4 {
        crumb.push("Progress".into());
    }
    Page {
        title: "Deploy system plan".into(),
        crumb,
        h1: "Deploy system plan".into(),
        desc: Some(desc.into()),
        messages,
        body,
        help: "plans".into(),
        nav: Some((NAV, Some("/plans"))),
        refresh: (step == 4).then_some(10),
        ..Page::default()
    }
}

/// Step 1's view.
#[derive(Clone, Debug, Default)]
pub struct Target {
    /// The plan.
    pub plan: String,
    /// Each QS/VS host, `QSHQVS2 (QS/VS host, 0 partitions)`.
    pub options: Vec<String>,
    /// The one chosen; the first when none.
    pub chosen: String,
    /// The message rows.
    pub messages: Vec<Message>,
}

/// Step 1.
pub fn target_page(ctx: &Ctx, v: &Target) -> Page {
    let mut options = String::new();
    for o in &v.options {
        let sel = if o.split(' ').next() == Some(v.chosen.as_str()) && !v.chosen.is_empty() {
            " selected"
        } else {
            ""
        };
        let _ = write!(options, "<option{sel}>{}</option>", esc(o));
    }
    let body = format!(
        "<h2>Target managed system</h2><table class=\"props\"><tr><th>Plan</th><td class=\"mono\">{}</td></tr><tr><th>Deploy to</th><td><select name=\"t\">{options}</select></td></tr><tr><th>Existing partitions</th><td><input type=\"radio\" name=\"e\" value=\"keep\" checked> Keep; only create partitions not present<br><input type=\"radio\" name=\"e\" value=\"replace\"> Replace matching partitions (destructive, confirmed later)</td></tr></table>",
        esc(&v.plan)
    );
    let body = wizard(
        ctx,
        &STEPS,
        1,
        &body,
        &[
            Button::new("back", "< Back").disabled(),
            Button::new("next", "Next >"),
            Button::new("cancel", "Cancel"),
        ],
    );
    frame(&v.plan, 1, "Step 1 of 4.", body, v.messages.clone())
}

/// A validation row as the page shows it.
#[derive(Clone, Debug, Default)]
pub struct Row {
    /// Plan item.
    pub item: String,
    /// Its status.
    pub status: Status,
    /// Its message.
    pub message: String,
}

/// Step 2's view.
#[derive(Clone, Debug, Default)]
pub struct Validated {
    /// The plan.
    pub plan: String,
    /// The rows.
    pub rows: Vec<Row>,
    /// The message rows.
    pub messages: Vec<Message>,
}

/// A status as the tables show it: OK and Error coloured, Warning not.
pub fn word(s: Status) -> String {
    match s {
        Status::Ok => status("OK", Some(true)),
        Status::Warning => "Warning".into(),
        Status::Error => status("Error", Some(false)),
    }
}

/// Step 2.
pub fn validated_page(ctx: &Ctx, v: &Validated) -> Page {
    let rows = v
        .rows
        .iter()
        .map(|r| {
            vec![
                Cell::text(&r.item),
                Cell::html(word(r.status)),
                Cell::text(&r.message),
            ]
        })
        .collect();
    let body = format!(
        "<h2>Validation</h2>{}<p class=\"small grey\">Items in Error are skipped at deployment unless corrected. Items with a Warning are deployed with the noted change.</p>",
        list_table(&List {
            cols: ["Plan item", "Status", "Message"].map(Col::plain).to_vec(),
            rows,
            empty: "The plan has nothing to validate.".into(),
            keep: Vec::new(),
        })
    );
    let body = wizard(
        ctx,
        &STEPS,
        2,
        &body,
        &[
            Button::new("back", "< Back"),
            Button::new("next", "Next >"),
            Button::new("cancel", "Cancel"),
        ],
    );
    frame(&v.plan, 2, "Step 2 of 4.", body, v.messages.clone())
}

/// A partition of step 3.
#[derive(Clone, Debug, Default)]
pub struct Part {
    /// Its ID.
    pub id: u32,
    /// Its name.
    pub name: String,
    /// Its default profile.
    pub profile: String,
    /// `1.00 shared`, `2 dedicated`.
    pub processors: String,
    /// `16,384`.
    pub memory: String,
    /// Its validation's status.
    pub status: Status,
    /// What its Warning or Error is about: `POOL2, C3`.
    pub about: String,
    /// Ticked.
    pub checked: bool,
}

/// Step 3's view.
#[derive(Clone, Debug, Default)]
pub struct Choice {
    /// The plan.
    pub plan: String,
    /// Its partitions.
    pub parts: Vec<Part>,
    /// Activate created partitions, ticked.
    pub activate: bool,
    /// The message rows.
    pub messages: Vec<Message>,
}

/// Step 3.
pub fn choice_page(ctx: &Ctx, v: &Choice) -> Page {
    let rows = v
        .parts
        .iter()
        .map(|p| {
            let validation = match (p.status, p.about.is_empty()) {
                (Status::Ok, _) => word(Status::Ok),
                (s, true) => word(s),
                (Status::Warning, false) => esc(&format!("Warning ({})", p.about)),
                (Status::Error, false) => format!("{} ({})", word(Status::Error), esc(&p.about)),
            };
            let disabled = if p.status == Status::Error {
                " disabled"
            } else {
                ""
            };
            vec![
                Cell::html(format!(
                    "<input type=\"checkbox\" name=\"p{}\"{}{disabled}>",
                    p.id,
                    if p.checked { " checked" } else { "" }
                )),
                Cell::mono(&p.name),
                Cell::num(&p.id.to_string()),
                Cell::mono(&p.profile),
                Cell::text(&p.processors),
                Cell::text(&p.memory),
                Cell::html(validation),
            ]
        })
        .collect();
    let body = format!(
        "<h2>Partitions to create</h2>{}<table class=\"props\"><tr><th>After deployment</th><td><input type=\"checkbox\" name=\"act\"{}> Activate created partitions</td></tr></table>",
        list_table(&List {
            cols: [
                "Create",
                "Partition",
                "ID",
                "Profile",
                "Processors",
                "Memory (MB)",
                "Validation"
            ]
            .map(Col::plain)
            .to_vec(),
            rows,
            empty: "The plan has no partitions.".into(),
            keep: Vec::new(),
        }),
        if v.activate { " checked" } else { "" }
    );
    let body = wizard(
        ctx,
        &STEPS,
        3,
        &body,
        &[
            Button::new("back", "< Back"),
            Button::new("cancel", "Cancel"),
            Button::new("finish", "Deploy"),
        ],
    );
    frame(
        &v.plan,
        3,
        "Step 3 of 4. Deploy opens the confirmation page, then the progress page.",
        body,
        v.messages.clone(),
    )
}

/// A step of the progress page.
#[derive(Clone, Debug, Default)]
pub struct StepRow {
    /// What it is.
    pub what: String,
    /// How it stands.
    pub state: StepState,
    /// When it began, `HH:MM:SS`.
    pub started: String,
    /// When it ended.
    pub ended: String,
    /// What it said.
    pub message: String,
}

/// The progress page's view.
#[derive(Clone, Debug, Default)]
pub struct Progress {
    /// The plan.
    pub plan: String,
    /// The message the body begins with: deploying, or how it ended.
    pub head: Option<Message>,
    /// The steps.
    pub steps: Vec<StepRow>,
    /// The page's message rows.
    pub messages: Vec<Message>,
}

/// A step's state as the table shows it.
fn state_word(s: StepState) -> String {
    match s {
        StepState::Pending => "Pending".into(),
        StepState::Going => "In progress".into(),
        StepState::Complete => status("Complete", Some(true)),
        StepState::Skipped => "Skipped".into(),
        StepState::Failed => status("Failed", Some(false)),
        StepState::Interrupted => status("Interrupted", Some(false)),
    }
}

/// The progress page.
pub fn progress_page(ctx: &Ctx, v: &Progress) -> Page {
    let rows = v
        .steps
        .iter()
        .map(|s| {
            vec![
                Cell::text(&s.what),
                Cell::html(state_word(s.state)),
                Cell::text(&s.started),
                Cell::text(&s.ended),
                Cell::text(&s.message),
            ]
        })
        .collect();
    let head = v
        .head
        .as_ref()
        .map(|m| message_row(ctx.product, m))
        .unwrap_or_default();
    let body = format!(
        "<h2>Deploy</h2>{head}{}{}",
        list_table(&List {
            cols: ["Step", "Status", "Started", "Ended", "Message"]
                .map(Col::plain)
                .to_vec(),
            rows,
            empty: "The deployment has no steps.".into(),
            keep: Vec::new(),
        }),
        buttons(&[
            Button::bare("Refresh"),
            Button::new("stop", "Cancel deployment")
        ])
    );
    let body = wizard(
        ctx,
        &STEPS,
        4,
        &body,
        &[
            Button::new("back", "< Back").disabled(),
            Button::new("cancel", "Cancel"),
            Button::new("finish", "Close"),
        ],
    );
    frame(&v.plan, 4, "Step 4 of 4.", body, v.messages.clone())
}

/// The target of the query, the partitions chosen and Activate.
struct Asked {
    target: String,
    chosen: Vec<u32>,
    activate: bool,
}

fn asked(rq: &Rq<'_>) -> Asked {
    Asked {
        target: rq.q("t"),
        chosen: rq
            .q("c")
            .split(',')
            .filter_map(|x| x.parse().ok())
            .collect(),
        activate: rq.q("act") == "1",
    }
}

fn at(name: &str, step: usize, a: &Asked) -> String {
    let mut q = format!(
        "{}?t={}",
        route_of(name, &format!("/deploy/step/{step}")),
        qsweb::http::form::encode(&a.target)
    );
    if !a.chosen.is_empty() {
        let c: Vec<String> = a.chosen.iter().map(ToString::to_string).collect();
        let _ = write!(q, "&c={}", c.join(","));
    }
    if a.activate {
        q.push_str("&act=1");
    }
    q
}

/// The hosts a plan may be deployed to: every QS/VS host, the plan's
/// own last.
fn hosts(rq: &Rq<'_>, plan: &str) -> Vec<(String, usize)> {
    let own = super::view::read(rq, plan)
        .ok()
        .flatten()
        .map(|p| p.system)
        .unwrap_or_default();
    let mut v: Vec<(String, usize)> = crate::systems::systems(rq)
        .unwrap_or_default()
        .into_iter()
        .filter(|k| k.kind == mgmtproto::kept::Kind::Host)
        .map(|k| (k.name.clone(), k.partitions.len()))
        .collect();
    v.sort_by(|a, b| (a.0 == own).cmp(&(b.0 == own)).then(a.0.cmp(&b.0)));
    v
}

fn validate(rq: &Rq<'_>, name: &str, target: &str, chosen: &[u32]) -> Result<Vec<Item>, Message> {
    rq.app
        .console
        .sched()
        .validate(
            name.to_string(),
            target.to_string(),
            chosen.to_vec(),
            rq.user().to_string(),
        )
        .map_err(|e| down(&e))?
        .map_err(|i| issued(Kind::Error, &i))
}

/// `GET`, `POST /plan/<name>/deploy/step/<n>`.
#[allow(clippy::too_many_lines, reason = "a block a step of the wizard")]
pub fn step(rq: &Rq<'_>, name: &str, n: usize) -> Response {
    let mut a = asked(rq);
    let f = rq.req.form();
    if rq.post() && f.has("cancel") {
        return Response::redirect(&rq.ctx.url("/plans"));
    }
    match n {
        1 => {
            let hosts = hosts(rq, name);
            if rq.post() {
                let chosen = f.str("t").split(' ').next().unwrap_or("").to_string();
                if f.str("e") == "replace" {
                    let m = qmcmsg::message(Kind::Error, &qmcmsg::NO_REPLACE, &[]);
                    return rq.page(target_page(
                        &rq.ctx,
                        &Target {
                            plan: name.into(),
                            options: options(&hosts),
                            chosen,
                            messages: vec![m],
                        },
                    ));
                }
                if !hosts.iter().any(|(h, _)| *h == chosen) {
                    let m = qmcmsg::message(Kind::Error, &qmcmsg::DEPLOY_CHOOSE, &[name]);
                    return rq.done(&at(name, 1, &a), m);
                }
                a.target = chosen;
                return Response::redirect(&rq.ctx.url(&at(name, 2, &a)));
            }
            let mut messages = Vec::new();
            if hosts.is_empty() {
                messages.push(qmcmsg::message(
                    Kind::Error,
                    &qmcmsg::DEPLOY_CHOOSE,
                    &[name],
                ));
            }
            rq.page(target_page(
                &rq.ctx,
                &Target {
                    plan: name.into(),
                    options: options(&hosts),
                    chosen: a.target.clone(),
                    messages,
                },
            ))
        }
        2 => {
            if rq.post() {
                let to = if f.has("back") { 1 } else { 3 };
                return Response::redirect(&rq.ctx.url(&at(name, to, &a)));
            }
            let (rows, messages) = match validate(rq, name, &a.target, &[]) {
                Ok(items) => (
                    items
                        .into_iter()
                        .map(|i| Row {
                            item: i.label,
                            status: i.status,
                            message: i.message,
                        })
                        .collect(),
                    Vec::new(),
                ),
                Err(m) => (Vec::new(), vec![m]),
            };
            rq.page(validated_page(
                &rq.ctx,
                &Validated {
                    plan: name.into(),
                    rows,
                    messages,
                },
            ))
        }
        3 => {
            let plan = match super::view::read(rq, name) {
                Ok(Some(p)) => p,
                Ok(None) => {
                    return crate::app::routes::error(rq, 404, &qmcmsg::NO_SUCH_PLAN, &[name]);
                }
                Err(m) => return rq.done("/plans", m),
            };
            if rq.post() {
                if f.has("back") {
                    return Response::redirect(&rq.ctx.url(&at(name, 2, &a)));
                }
                a.chosen = plan
                    .partitions
                    .iter()
                    .map(|p| p.id)
                    .filter(|id| f.has(&format!("p{id}")))
                    .collect();
                a.activate = f.has("act");
                let c: Vec<String> = a.chosen.iter().map(ToString::to_string).collect();
                return Response::redirect(&rq.ctx.url(&format!(
                    "/confirm/plan-deploy?p={}&t={}&c={}{}",
                    qsweb::http::form::encode(name),
                    qsweb::http::form::encode(&a.target),
                    c.join(","),
                    if a.activate { "&act=1" } else { "" }
                )));
            }
            let (items, messages) = match validate(rq, name, &a.target, &[]) {
                Ok(i) => (i, Vec::new()),
                Err(m) => (Vec::new(), vec![m]),
            };
            let v = Choice {
                plan: name.into(),
                parts: parts(&plan, &items, &a.chosen),
                activate: a.activate,
                messages,
            };
            rq.page(choice_page(&rq.ctx, &v))
        }
        _ => crate::app::routes::error(rq, 404, &qmcmsg::NOT_FOUND, &[&rq.req.path]),
    }
}

fn options(hosts: &[(String, usize)]) -> Vec<String> {
    hosts
        .iter()
        .map(|(h, n)| {
            format!(
                "{h} (QS/VS host, {n} partition{})",
                if *n == 1 { "" } else { "s" }
            )
        })
        .collect()
}

/// Step 3's rows: each partition of `plan` with its validation; ticked
/// when chosen before, else when nothing it needs is in Warning or Error.
pub fn parts(plan: &Plan, items: &[Item], chosen: &[u32]) -> Vec<Part> {
    plan.partitions
        .iter()
        .map(|p| {
            let item = items
                .iter()
                .find(|i| i.kind == ItemKind::Partition && i.key == p.id.to_string());
            let (status, about) = item.map_or((Status::Error, String::new()), |i| {
                (i.status, i.depends.join(", "))
            });
            let pr = p.profiles.iter().find(|x| x.name == p.default_profile);
            let processors = pr.map_or_else(String::new, |pr| {
                let x = &pr.processors;
                if x.mode == Mode::Shared {
                    let u = u64::from(x.desired_units.unwrap_or(0));
                    format!("{}.{:02} shared", u / 100, u % 100)
                } else {
                    format!("{} dedicated", x.desired)
                }
            });
            let checked = if chosen.is_empty() {
                status == Status::Ok || (status == Status::Warning && about.is_empty())
            } else {
                chosen.contains(&p.id)
            } && status != Status::Error;
            Part {
                id: p.id,
                name: p.name.clone(),
                profile: p.default_profile.clone(),
                processors,
                memory: pr.map_or_else(String::new, |pr| thousands(pr.memory.desired_mib)),
                status,
                about,
                checked,
            }
        })
        .collect()
}

/// `GET`, `POST /confirm/plan-deploy?p=&t=&c=&act=`.
#[allow(clippy::too_many_lines, reason = "the answer, then the page")]
pub fn confirm(rq: &Rq<'_>) -> Response {
    let f = rq.req.form();
    let q = |n: &str| {
        if rq.post() {
            f.str(n).to_string()
        } else {
            rq.q(n)
        }
    };
    let name = q("p");
    let a = Asked {
        target: q("t"),
        chosen: q("c").split(',').filter_map(|x| x.parse().ok()).collect(),
        activate: q("act") == "1",
    };
    let plan = match super::view::read(rq, &name) {
        Ok(Some(p)) => p,
        Ok(None) => return crate::app::routes::error(rq, 404, &qmcmsg::NO_SUCH_PLAN, &[&name]),
        Err(m) => return rq.done("/plans", m),
    };
    if rq.post() {
        if f.get("ok").is_none() {
            return Response::redirect(&rq.ctx.url(&at(&name, 3, &a)));
        }
        let begun = rq.app.console.sched().deploy(
            name.clone(),
            a.target.clone(),
            a.chosen.clone(),
            a.activate,
            rq.user().to_string(),
        );
        let to = format!(
            "{}?t={}",
            route_of(&name, "/deploy/progress"),
            qsweb::http::form::encode(&a.target)
        );
        return match begun {
            Ok(Ok(())) => {
                rq.audit(
                    &format!("deploy system plan {name} to {}", a.target),
                    "begun",
                );
                Response::redirect(&rq.ctx.url(&to))
            }
            Ok(Err(i)) => {
                rq.audit(
                    &format!("deploy system plan {name} to {}", a.target),
                    &format!("refused: {} {}", i.id, i.text),
                );
                rq.done(&at(&name, 3, &a), issued(Kind::Error, &i))
            }
            Err(e) => rq.done(&at(&name, 3, &a), down(&e)),
        };
    }
    let named: Vec<String> = plan
        .partitions
        .iter()
        .filter(|p| a.chosen.contains(&p.id))
        .map(|p| format!("{} ({})", p.name, p.id))
        .collect();
    let table = format!(
        "<table class=\"props\" cellspacing=\"0\"><tr><th>Plan</th><td>{}</td></tr><tr><th>Managed system</th><td>{}</td></tr><tr><th>Partitions to create</th><td>{}</td></tr><tr><th>After deployment</th><td>{}</td></tr></table>",
        qsweb::partials::mono(&name),
        qsweb::partials::mono(&a.target),
        esc(&if named.is_empty() {
            "None: the plan's resources only".to_string()
        } else {
            named.join(", ")
        }),
        if a.activate {
            "Activate created partitions"
        } else {
            "Leave them Not activated"
        }
    );
    let hidden = format!(
        "{}{}{}{}",
        qsweb::partials::hidden("p", &name),
        qsweb::partials::hidden("t", &a.target),
        qsweb::partials::hidden(
            "c",
            &a.chosen
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(",")
        ),
        qsweb::partials::hidden("act", if a.activate { "1" } else { "" })
    );
    let body = qsweb::pages::confirm_body(
        &rq.ctx,
        "The plan's switches, storage pools, processor pools, virtual disks and tape volumes that the validation says are made are made on the managed system, then the partitions chosen. Nothing that exists is deleted or changed, and no disk with a label is taken. Press OK to deploy, or Cancel to return.",
        &table,
        &hidden,
    );
    rq.page(Page {
        title: "Confirm Deploy System Plan".into(),
        crumb: vec![NAV.into(), TITLE.into(), name.clone(), "Deploy".into()],
        h1: "Confirm Deploy System Plan".into(),
        body,
        help: "plans".into(),
        nav: Some((NAV, Some("/plans"))),
        ..Page::default()
    })
}

/// The message the progress page begins with, of `j`.
pub fn head(j: &Job) -> Message {
    let what = |s: StepState| {
        j.steps
            .iter()
            .find(|x| x.state == s)
            .map_or_else(String::new, |x| x.what.clone())
    };
    let count = |s: StepState| j.steps.iter().filter(|x| x.state == s).count().to_string();
    match j.state {
        JobState::Running => qmcmsg::message(Kind::Info, &qmcmsg::DEPLOYING, &[&j.plan, &j.target]),
        JobState::Done => qmcmsg::message(
            Kind::Done,
            &qmcmsg::DEPLOYED,
            &[
                &j.plan,
                &j.target,
                &count(StepState::Complete),
                &count(StepState::Skipped),
            ],
        ),
        JobState::Cancelled => {
            qmcmsg::message(Kind::Info, &qmcmsg::DEPLOY_CANCELLED, &[&j.plan, &j.target])
        }
        JobState::Failed => qmcmsg::message(
            Kind::Error,
            &qmcmsg::DEPLOY_FAILED,
            &[&j.plan, &j.target, &what(StepState::Failed)],
        ),
        JobState::Stopped => qmcmsg::message(
            Kind::Error,
            &qmcmsg::DEPLOY_STOPPED,
            &[&j.plan, &j.target, &what(StepState::Interrupted)],
        ),
    }
}

/// `GET`, `POST /plan/<name>/deploy/progress?t=`.
pub fn progress(rq: &Rq<'_>, name: &str) -> Response {
    let target = rq.q("t");
    let here = format!(
        "{}?t={}",
        route_of(name, "/deploy/progress"),
        qsweb::http::form::encode(&target)
    );
    let f = rq.req.form();
    let mut sched = rq.app.console.sched();
    if rq.post() {
        if f.has("cancel") || f.has("finish") {
            return Response::redirect(&rq.ctx.url("/plans"));
        }
        if f.has("stop") {
            return match sched.cancel(target.clone(), rq.user().to_string()) {
                Ok(Ok(())) => {
                    rq.audit(
                        &format!("cancel the deployment of system plan {name} to {target}"),
                        "asked",
                    );
                    rq.done(
                        &here,
                        qmcmsg::message(Kind::Info, &qmcmsg::CANCEL_ASKED, &[&target]),
                    )
                }
                Ok(Err(i)) => rq.done(&here, issued(Kind::Error, &i)),
                Err(e) => rq.done(&here, down(&e)),
            };
        }
        return Response::redirect(&rq.ctx.url(&here));
    }
    let mut messages = Vec::new();
    let job = match sched.job(target.clone()) {
        Ok(t) => t.and_then(|t| plan::job::read(&t)),
        Err(e) => {
            messages.push(down(&e));
            None
        }
    };
    let job = job.filter(|j| j.plan == name);
    if job.is_none() && messages.is_empty() {
        messages.push(qmcmsg::message(
            Kind::Info,
            &qmcmsg::NO_DEPLOYMENT,
            &[&target],
        ));
    }
    let v = Progress {
        plan: name.to_string(),
        head: job.as_ref().map(head),
        steps: job
            .as_ref()
            .map(|j| {
                j.steps
                    .iter()
                    .map(|s| StepRow {
                        what: s.what.clone(),
                        state: s.state,
                        started: clock(rq, s.started),
                        ended: clock(rq, s.ended),
                        message: s.message.clone(),
                    })
                    .collect()
            })
            .unwrap_or_default(),
        messages,
    };
    rq.page(progress_page(&rq.ctx, &v))
}
