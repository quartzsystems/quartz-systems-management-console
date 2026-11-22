// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! View system plan (`plan-view.html`): who made it, when and from
//! what; the managed system's table; then each partition's profiles as
//! nested property tables, the default first. Printable as it is: the
//! stylesheet's print rule is all it needs.

use std::fmt::Write as _;

use qsweb::frame::{Ctx, Page};
use qsweb::html::esc;
use qsweb::http::Response;
use qsweb::msg::Message;
use qsweb::partials::{Cell, Col, List, list_table};

use plan::doc::{Keylock, Mode, Plan, ProfilePlan, Sharing};
use plan::validate::{size, thousands};

use super::{NAV, down, route_of, when};
use crate::app::routes::Rq;

/// A profile's table.
#[derive(Clone, Debug, Default)]
pub struct Profile {
    /// `Partition 1: QSDEV01, profile default`.
    pub heading: String,
    /// Its rows: label and value.
    pub rows: Vec<(String, String)>,
}

/// What the page shows.
#[derive(Clone, Debug, Default)]
pub struct View {
    /// The plan's name.
    pub name: String,
    /// `Created 10/02/26 10:12 by qsadmin from QSHQVS1.`
    pub created: String,
    /// The managed system's row: name, type, type-model, processors,
    /// memory, storage pools, switches.
    pub system: [String; 7],
    /// Each partition's profiles.
    pub profiles: Vec<Profile>,
    /// The message rows.
    pub messages: Vec<Message>,
}

/// The page.
pub fn page(ctx: &Ctx, v: &View) -> Page {
    let at = |rest: &str| esc(&ctx.url(&route_of(&v.name, rest)));
    let mut body = format!(
        "<p class=\"small\">{} <a href=\"{}\">Deploy</a> &nbsp; <a href=\"{}\">Edit</a> &nbsp; <a href=\"{}\">Export</a> &nbsp; <a href=\"#\">Print</a></p><h2>Managed system</h2>",
        esc(&v.created),
        at("/deploy/step/1"),
        at("/edit"),
        at("/export"),
    );
    let s = &v.system;
    body.push_str(&list_table(&List {
        cols: [
            "Name",
            "Type",
            "Type-Model",
            "Processors",
            "Memory (MB)",
            "Storage pools",
            "Virtual switches",
        ]
        .map(Col::plain)
        .to_vec(),
        rows: vec![vec![
            Cell::mono(&s[0]),
            Cell::text(&s[1]),
            Cell::mono(&s[2]),
            Cell::text(&s[3]),
            Cell::text(&s[4]),
            Cell::text(&s[5]),
            Cell::text(&s[6]),
        ]],
        empty: String::new(),
        keep: Vec::new(),
    }));
    for p in &v.profiles {
        let _ = write!(
            body,
            "<h2>{}</h2><table class=\"props\" style=\"margin-left:12px\">",
            esc(&p.heading)
        );
        for (label, value) in &p.rows {
            let _ = write!(
                body,
                "<tr><th>{}</th><td class=\"mono\">{}</td></tr>",
                esc(label),
                esc(value)
            );
        }
        body.push_str("</table>");
    }
    Page {
        title: format!("System plan: {}", v.name),
        crumb: vec![NAV.into(), v.name.clone()],
        h1: format!("System plan: {}", v.name),
        messages: v.messages.clone(),
        body,
        help: "plans".into(),
        nav: Some((NAV, None)),
        ..Page::default()
    }
}

fn three<T: std::fmt::Display>(a: T, b: T, c: T) -> String {
    format!("{a} / {b} / {c}")
}

/// A profile's processors: `Shared, DefaultPool, 0.25 / 1.00 / 2.00
/// units, 1 / 2 / 4 virtual, uncapped 128`; `Dedicated, 2 / 2 / 4`.
fn processors(p: &ProfilePlan) -> String {
    let x = &p.processors;
    if x.mode != Mode::Shared {
        return format!("Dedicated, {}", three(x.minimum, x.desired, x.maximum));
    }
    let u = |v: Option<u32>| {
        let v = u64::from(v.unwrap_or(0));
        format!("{}.{:02}", v / 100, v % 100)
    };
    let sharing = match x.sharing {
        Some(Sharing::Uncapped) => format!("uncapped {}", x.weight.unwrap_or(128)),
        _ => "capped".to_string(),
    };
    format!(
        "Shared, {}, {} units, {} virtual, {sharing}",
        x.pool.as_deref().unwrap_or("default pool"),
        three(u(x.minimum_units), u(x.desired_units), u(x.maximum_units)),
        three(x.minimum, x.desired, x.maximum),
    )
}

/// A profile's adapters by slot: `0 Serial; 2 Ethernet ETHSW0 VLAN 20;
/// 3 SCSI VD03 120 GB; 5 Optical QS400_V1R0M0_INSTALL.iso; 6 Tape VTAP01`.
fn adapters(plan: &Plan, p: &ProfilePlan) -> String {
    let mut by_slot: Vec<(u32, String)> = vec![(0, "Serial".into())];
    for e in &p.ethernet {
        by_slot.push((
            e.slot,
            format!(
                "Ethernet {} VLAN {}",
                e.switch.as_deref().unwrap_or("(none)"),
                e.vlan
            ),
        ));
    }
    for a in &p.scsi {
        let names: Vec<&str> = a
            .luns
            .iter()
            .map(|l| plan::Plan::backing(&l.backing).map_or(l.backing.as_str(), |(_, n)| n))
            .collect();
        let mib: u64 = a
            .luns
            .iter()
            .filter_map(|l| plan::Plan::backing(&l.backing))
            .filter_map(|(pool, n)| plan.vdisks.iter().find(|d| d.pool == pool && d.name == n))
            .map(|d| d.mib)
            .sum();
        by_slot.push((a.slot, format!("SCSI {} {}", names.join("+"), size(mib))));
    }
    for m in &p.optical {
        by_slot.push((
            m.slot,
            format!("Optical {}", m.media.as_deref().unwrap_or("(empty)")),
        ));
    }
    for m in &p.tape {
        let what = match m.media.as_deref() {
            None => "Tape (empty)".to_string(),
            Some(x) => x.strip_prefix(qsi::host::model::DRIVE_MARK).map_or_else(
                || format!("Tape {x}"),
                |l| format!("Tape drive {} pass-through", plan::view::shown(l)),
            ),
        };
        by_slot.push((m.slot, what));
    }
    by_slot.sort_by_key(|(s, _)| *s);
    by_slot
        .into_iter()
        .map(|(s, w)| format!("{s} {w}"))
        .collect::<Vec<_>>()
        .join("; ")
}

/// The page's view of `plan`, its times in `rq`'s zone.
pub fn view_of(rq: &Rq<'_>, plan: &Plan) -> View {
    let h = &plan.host;
    let from = if plan.system.is_empty() {
        ", a template".to_string()
    } else {
        format!(" from {}", plan.system)
    };
    let created = format!("Created {} by {}{from}.", when(rq, plan.created), plan.by);
    let pools = plan
        .pools
        .iter()
        .map(|p| format!("{} ({})", p.name, size(p.bytes >> 20)))
        .collect::<Vec<_>>()
        .join(", ");
    let switches = plan
        .switches
        .iter()
        .map(|s| {
            let v: Vec<String> = s.vlans.iter().map(ToString::to_string).collect();
            format!("{} (VLAN {})", s.name, v.join(","))
        })
        .collect::<Vec<_>>()
        .join(", ");
    let system = [
        if h.name.is_empty() {
            "(template)".to_string()
        } else {
            h.name.clone()
        },
        "QS/VS host".to_string(),
        h.model.clone(),
        h.cores.to_string(),
        thousands(h.memory_mib),
        pools,
        switches,
    ];
    let mut profiles = Vec::new();
    for part in &plan.partitions {
        let mut ordered: Vec<&ProfilePlan> = part.profiles.iter().collect();
        ordered.sort_by_key(|p| p.name != part.default_profile);
        for pr in ordered {
            let m = pr.memory;
            let mut rows = vec![
                ("Processors".to_string(), processors(pr)),
                (
                    "Memory (MB)".to_string(),
                    three(
                        thousands(m.minimum_mib),
                        thousands(m.desired_mib),
                        thousands(m.maximum_mib),
                    ),
                ),
            ];
            if !pr.physical_io.is_empty() {
                rows.push((
                    "Physical I/O".to_string(),
                    pr.physical_io
                        .iter()
                        .map(|io| plan::view::shown(&io.location))
                        .collect::<Vec<_>>()
                        .join(", "),
                ));
            }
            rows.push(("Virtual adapters".to_string(), adapters(plan, pr)));
            let keylock = if part.keylock == Keylock::Manual {
                "Manual"
            } else {
                "Normal"
            };
            rows.push((
                "Boot".to_string(),
                format!(
                    "{}, {keylock}, auto start {}",
                    pr.boot.source.word(),
                    if part.auto_start.enabled { "Yes" } else { "No" }
                ),
            ));
            profiles.push(Profile {
                heading: format!("Partition {}: {}, profile {}", part.id, part.name, pr.name),
                rows,
            });
        }
    }
    View {
        name: plan.name.clone(),
        created,
        system,
        profiles,
        messages: Vec::new(),
    }
}

/// The plan `name` read from the scheduler.
pub fn read(rq: &Rq<'_>, name: &str) -> Result<Option<Plan>, Message> {
    let text = rq
        .app
        .console
        .sched()
        .plan(name.to_string())
        .map_err(|e| down(&e))?;
    Ok(text.and_then(|t| plan::doc::text::read(t.as_bytes()).ok()))
}

/// `GET /plan/<name>`.
pub fn route(rq: &Rq<'_>, name: &str) -> Response {
    match read(rq, name) {
        Ok(Some(p)) => rq.page(page(&rq.ctx, &view_of(rq, &p))),
        Ok(None) => crate::app::routes::error(rq, 404, &qmcmsg::NO_SUCH_PLAN, &[name]),
        Err(m) => rq.done("/plans", m),
    }
}
