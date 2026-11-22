// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! Phase 7's suites: system plans through the web interface of a
//! console in QEMU, against the simulator's `QSHQVS1` (the plan's host),
//! `QSHQVS2` (the prototype's target: no disk where `POOL2`'s was, slot
//! `C3` empty, no partitions) and `QSHQ400`, which run in the harness's
//! process so a suite reads what each host keeps before and after.

#![allow(
    clippy::many_single_char_names,
    reason = "a suite's browser, console and page are `b`, `c` and `t` throughout"
)]

mod deploy;
mod pages;
mod real;
mod update;

use std::time::Duration;

use super::base::{Run, SuiteFn};
use super::mgmt::{Fleet, keep, page, until};
use super::parts::{expect, plain, post_form, post_on};
use super::web::{ADMIN, Console, PASSWORD, message_of, shows, signed_on, start};
use crate::web::Browser;

/// The suites, by the handoff's numbers (1 is every earlier phase's).
pub const SUITES: &[(&str, SuiteFn)] = &[
    ("plans-pages", pages::pages),
    ("plans-capture", deploy::capture),
    ("plans-validate", deploy::validate),
    ("plans-deploy", deploy::deploy),
    ("plans-restart", deploy::restart),
    ("plans-import", deploy::import),
    ("plans-update", update::before_update),
    ("plans-roles", pages::roles),
    ("plans-backup", update::backup),
];

/// The suites run by name only: suite 12 needs a QS/VS host's disk.
pub const NAMED: &[(&str, SuiteFn)] = &[("plans-real-host", real::real_host)];

/// The plan's host.
pub const VS1: &str = "QSHQVS1";
/// The target of the prototype's deployment.
pub const VS2: &str = "QSHQVS2";
/// The standalone system.
pub const STANDALONE: &str = "QSHQ400";

/// A console managing `QSHQVS1`, `QSHQVS2` and `QSHQ400`, signed on as
/// the first Administrator.
pub struct Lab<'a> {
    /// The simulated systems in that order.
    pub fleet: Fleet,
    /// The console and its browser.
    pub c: Console<'a>,
}

/// A lab, each system kept and read whole.
pub fn lab<'a>(run: &Run<'a>, tag: &str) -> Result<Lab<'a>, String> {
    let fleet = Fleet::start(vec![
        sim::fixture::qshqvs1(),
        sim::fixture::qshqvs2(),
        sim::fixture::qshq400(),
    ])?;
    let mut c = start(run, tag)?;
    signed_on(&mut c.browser, ADMIN, PASSWORD)?;
    for i in 0..3 {
        keep(&mut c.browser, &fleet, i)?;
    }
    super::parts::ready(&mut c.browser)?;
    let b = &mut c.browser;
    until("QSHQVS2 operating", Duration::from_secs(120), || {
        let t = page(b, "/qsmc/servers")?;
        Ok(t.split("<tr")
            .any(|r| r.contains(VS2) && r.contains("Operating")))
    })?;
    Ok(Lab { fleet, c })
}

/// The name in `text` after `before` and up to `after`.
fn between(text: &str, before: &str, after: &str) -> Option<String> {
    let t = text.split(before).nth(1)?;
    Some(t.split(after).next()?.trim().to_string())
}

/// Create plan from `system` on System plans: its name.
pub fn create(b: &mut Browser, system: &str) -> Result<String, String> {
    let token = b.csrf("/qsmc/plans")?;
    let a = b.upload(
        "/qsmc/plans",
        &[("csrf", &token), ("ms", system), ("create", "Create plan")],
        b"",
    )?;
    let t = plain(&expect(b, &a, qmcmsg::PLAN_CAPTURED.id)?);
    between(&t, "System plan ", " created from")
        .ok_or_else(|| format!("no plan named: {}", message_of(&t)))
}

/// Import `bytes` on System plans: the page it leads to.
pub fn import(b: &mut Browser, bytes: &[u8]) -> Result<String, String> {
    let token = b.csrf("/qsmc/plans")?;
    let a = b.upload(
        "/qsmc/plans",
        &[("csrf", &token), ("import", "Import")],
        bytes,
    )?;
    Ok(super::web::follow(b, &a)?.text())
}

/// Plan `name`'s Export: its document.
pub fn export(b: &mut Browser, name: &str) -> Result<Vec<u8>, String> {
    let a = b.get(&format!("/qsmc/plan/{name}/export"))?;
    if a.status != 200
        || !a
            .header("content-disposition")
            .is_some_and(|d| d.contains(&format!("{name}.qmcplan")))
    {
        return Err(format!("Export of {name} answered {}", a.status));
    }
    Ok(a.body)
}

/// A plan's document as exported.
pub fn read(b: &mut Browser, name: &str) -> Result<plan::Plan, String> {
    plan::doc::text::read(&export(b, name)?).map_err(|e| format!("{name} does not read: {e}"))
}

/// A table's rows of `html` after the header cell `th`, each row's
/// cells as text.
pub fn table(html: &str, th: &str) -> Vec<Vec<String>> {
    let Some(rest) = html.split(&format!("<th>{th}</th>")).nth(1) else {
        return Vec::new();
    };
    let body = rest.split("</table>").next().unwrap_or("");
    body.split("<tr")
        .skip(1)
        .map(|row| {
            row.split("<td")
                .skip(1)
                .map(|cell| {
                    let inner = cell.split_once('>').map_or("", |(_, c)| c);
                    let inner = inner.split("</td>").next().unwrap_or("");
                    let mut text = String::new();
                    let mut tag = false;
                    for ch in inner.chars() {
                        match ch {
                            '<' => tag = true,
                            '>' => tag = false,
                            c if !tag => text.push(c),
                            _ => {}
                        }
                    }
                    plain(text.trim())
                })
                .collect()
        })
        .collect()
}

/// Deploy plan `name` to `target` through the wizard and its
/// confirmation: the partitions `chosen`, activated with `activate`.
pub fn deploy(
    b: &mut Browser,
    name: &str,
    target: &str,
    chosen: &[u32],
    activate: bool,
) -> Result<(), String> {
    let p1 = format!("/qsmc/plan/{name}/deploy/step/1");
    let r = post_form(b, &p1, ("next", "Next >"), &[("t", target), ("e", "keep")])?;
    let to2 = r.header("location").unwrap_or_default().to_string();
    if !to2.contains("/deploy/step/2") {
        return Err(format!("step 1 led to `{to2}` ({})", r.status));
    }
    let p2 = format!("/qsmc/plan/{name}/deploy/step/2?t={target}");
    let r = post_form(b, &p2, ("next", "Next >"), &[])?;
    if !r
        .header("location")
        .is_some_and(|l| l.contains("/deploy/step/3"))
    {
        return Err(format!("step 2 answered {}", r.status));
    }
    let p3 = format!("/qsmc/plan/{name}/deploy/step/3?t={target}");
    let token = b.csrf(&p3)?;
    let ticks: Vec<String> = chosen.iter().map(|id| format!("p{id}")).collect();
    let mut fields: Vec<(&str, &str)> = vec![("csrf", &token), ("finish", "Deploy")];
    for t in &ticks {
        fields.push((t, "on"));
    }
    if activate {
        fields.push(("act", "on"));
    }
    let r = b.post(&p3, &fields)?;
    let confirm = r.header("location").unwrap_or_default().to_string();
    if !confirm.contains("/confirm/plan-deploy") {
        return Err(format!("Deploy led to `{confirm}` ({})", r.status));
    }
    let confirm = confirm
        .split_once("/qsmc/")
        .map_or(confirm.clone(), |(_, p)| format!("/qsmc/{p}"));
    let html = page(b, &confirm)?;
    let r = post_on(b, &html, &confirm, ("ok", "OK"), &[])?;
    let to = r.header("location").unwrap_or_default().to_string();
    if !to.contains("/deploy/progress") {
        let t = super::web::follow(b, &r)?.text();
        return Err(format!(
            "the confirmation did not deploy: {}",
            message_of(&t)
        ));
    }
    Ok(())
}

/// The progress page of plan `name` on `target`.
pub fn progress_path(name: &str, target: &str) -> String {
    format!("/qsmc/plan/{name}/deploy/progress?t={target}")
}

/// Wait until the deployment of `name` to `target` ends: the message
/// it ended with and its steps (step, status, started, ended, message).
pub fn ended(
    b: &mut Browser,
    name: &str,
    target: &str,
    within: Duration,
) -> Result<(String, Vec<Vec<String>>), String> {
    let path = progress_path(name, target);
    let mut last = String::new();
    until("the deployment ends", within, || {
        last = page(b, &path)?;
        Ok(["QMC6002", "QMC6003", "QMC6004", "QMC6005"]
            .iter()
            .any(|id| shows(&last, id)))
    })
    .map_err(|e| format!("{e}: {}", message_of(&last)))?;
    let id = ["QMC6002", "QMC6003", "QMC6004", "QMC6005"]
        .into_iter()
        .find(|id| shows(&last, id))
        .unwrap_or_default()
        .to_string();
    Ok((id, table(&last, "Step")))
}

/// The steps of `steps` whose status is `status`.
pub fn with_status<'s>(steps: &'s [Vec<String>], status: &str) -> Vec<&'s str> {
    steps
        .iter()
        .filter(|r| r.get(1).is_some_and(|s| s == status))
        .filter_map(|r| r.first().map(String::as_str))
        .collect()
}

/// What of a host a deployment may not touch: its pools with their
/// units, its disks and their owners, its library, the definitions and
/// profiles of what was there.
pub fn untouchable(fleet: &Fleet, i: usize) -> String {
    let h = sim::lock(&fleet.systems[i]);
    let pools: Vec<String> = h
        .vres
        .pools
        .iter()
        .map(|p| {
            format!(
                "{} {} {} {:?}",
                p.name,
                p.protection,
                p.bytes,
                p.units.iter().map(|u| &u.disk).collect::<Vec<_>>()
            )
        })
        .collect();
    let disks: Vec<String> = h
        .vres
        .disks
        .iter()
        .map(|d| format!("{} {} {} {}", d.name, d.foreign, d.free, d.pool))
        .collect();
    let images: Vec<&str> = h.vres.images.iter().map(|x| x.name.as_str()).collect();
    format!("{pools:?} {disks:?} {images:?}")
}
