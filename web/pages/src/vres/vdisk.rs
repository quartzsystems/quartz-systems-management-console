// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! Create virtual disk (`create-vdisk.html`) and a disk's properties
//! with its resize. A disk is made first; its assignment to a
//! partition is a second request, a LUN on a client adapter of the
//! partition's default profile, whose failure leaves the disk made and
//! says so.

use std::fmt::Write as _;

use qsweb::frame::{Ctx, Page};
use qsweb::html::esc;
use qsweb::http::Response;
use qsweb::msg::{Kind, Message};
use qsweb::partials::{Button, Prop, PropGroup, buttons, form, hidden, mono, props_table};

use mgmtproto::Kept;
use qsi::any::{AnyReply, AnyRequest};
use qsi::host::model::{Lun, ScsiAdapter};
use qsi::host::{partitions, profiles, vres};

use crate::app::routes::Rq;

/// The form.
#[derive(Clone, Debug, Default)]
pub struct Create {
    /// The host.
    pub host: String,
    /// The name typed.
    pub name: String,
    /// The size typed, GB.
    pub size: String,
    /// The pools: `POOL1 (2,530 GB free)`.
    pub pools: Vec<String>,
    /// The pool chosen.
    pub pool: String,
    /// Thin, not thick.
    pub thin: bool,
    /// The partitions to assign it to.
    pub partitions: Vec<String>,
    /// The partition chosen; `(none)`.
    pub assign: String,
    /// The client adapter's slot typed; `(next free)`.
    pub client: String,
    /// The messages.
    pub messages: Vec<Message>,
}

fn select(name: &str, options: &[String], chosen: &str) -> String {
    let mut s = format!("<select name=\"{name}\">");
    for o in options {
        let sel = if o == chosen && Some(o) != options.first() {
            " selected"
        } else {
            ""
        };
        let _ = write!(s, "<option{sel}>{}</option>", esc(o));
    }
    s.push_str("</select>");
    s
}

/// The page.
pub fn page(ctx: &Ctx, v: &Create) -> Page {
    let radio = |value: &str, on: bool| {
        let checked = if on { " checked" } else { "" };
        format!("<input type=\"radio\" name=\"t\" value=\"{value}\"{checked}>")
    };
    let mut assign = vec!["(none)".to_string()];
    assign.extend(v.partitions.iter().cloned());
    let groups = [PropGroup {
        title: None,
        rows: vec![
            Prop::new(
                "Name",
                format!(
                    "<input type=\"text\" name=\"n\" value=\"{}\" size=\"10\">",
                    esc(&v.name)
                ),
            )
            .required(),
            Prop::new(
                "Size (GB)",
                format!(
                    "<input type=\"text\" name=\"s\" value=\"{}\" size=\"8\">",
                    esc(&v.size)
                ),
            )
            .required(),
            Prop::new("Storage pool", select("p", &v.pools, &v.pool)),
            Prop::new(
                "Provisioning",
                format!(
                    "{} Thin {} Thick",
                    radio("thin", v.thin),
                    radio("thick", !v.thin)
                ),
            ),
            Prop::new("Assign to partition", select("a", &assign, &v.assign)),
            Prop::new(
                "Client adapter ID",
                format!(
                    "<input type=\"text\" name=\"c\" value=\"{}\" size=\"12\">",
                    esc(&v.client)
                ),
            ),
        ],
    }];
    let body = form(
        ctx,
        "",
        &format!(
            "{}\n{}",
            props_table(ctx.product, &groups, true),
            buttons(&[Button::new("ok", "OK"), Button::new("cancel", "Cancel")])
        ),
    );
    super::form_page(
        &v.host,
        "storage",
        "Create virtual disk",
        v.messages.clone(),
        body,
    )
}

/// A pool as the form offers it: `POOL1 (2,530 GB free)`.
fn pool_label(rq: &Rq<'_>, p: &vres::StoragePool) -> String {
    format!(
        "{} ({} GB free)",
        p.name,
        super::gb(rq, p.bytes.saturating_sub(p.allocated))
    )
}

/// The form of `k`, with the fields of `f` kept.
fn form_of(rq: &Rq<'_>, k: &Kept, f: &qsweb::http::Form, messages: Vec<Message>) -> Create {
    let pools: Vec<String> = k
        .resources
        .pools
        .iter()
        .map(|p| pool_label(rq, p))
        .collect();
    let next = (1..1000)
        .map(|n| format!("VD{n:02}"))
        .find(|n| !k.resources.vdisks.iter().any(|d| &d.name == n))
        .unwrap_or_default();
    let posted = rq.post();
    let keep = |name: &str, default: String| {
        if posted {
            f.str(name).to_string()
        } else {
            default
        }
    };
    Create {
        host: k.name.clone(),
        name: keep("n", next),
        size: keep("s", "120".into()),
        pool: keep("p", pools.first().cloned().unwrap_or_default()),
        pools,
        thin: !posted || f.str("t") != "thick",
        partitions: k.partitions.iter().map(|p| p.name.clone()).collect(),
        assign: keep("a", "(none)".into()),
        client: keep("c", "(next free)".into()),
        messages,
    }
}

/// Give disk `pool/name` to partition `id` as a LUN on client adapter
/// `slot` (`None`: a new adapter at the next free slot) of its default
/// profile, put back with the revision read.
fn assign(
    rq: &Rq<'_>,
    k: &Kept,
    id: u32,
    pool: &str,
    name: &str,
    slot: Option<u32>,
) -> Result<(), Vec<Message>> {
    let get = AnyRequest::Partitions(partitions::Request::Get(partitions::Get { id }));
    let def = match crate::systems::ask(rq, &k.name, "Read partition", &get)? {
        AnyReply::Partitions(partitions::Reply::Get(p)) => p.definition,
        _ => return Err(Vec::new()),
    };
    let read = AnyRequest::Profiles(profiles::Request::Get(profiles::Get {
        id,
        name: def.default_profile.clone(),
    }));
    let AnyReply::Profiles(profiles::Reply::Get(rev)) =
        crate::systems::ask(rq, &k.name, "Read profile", &read)?
    else {
        return Err(Vec::new());
    };
    let mut profile = rev.profile;
    let used: Vec<u32> = profile
        .scsi
        .iter()
        .map(|a| a.slot)
        .chain(profile.ethernet.iter().map(|a| a.slot))
        .chain(profile.optical.iter().map(|a| a.slot))
        .chain(profile.tape.iter().map(|a| a.slot))
        .collect();
    let slot = slot.unwrap_or_else(|| (2..1000).find(|s| !used.contains(s)).unwrap_or(2));
    let backing = format!("{pool}/{name}");
    match profile.scsi.iter_mut().find(|a| a.slot == slot) {
        Some(a) => {
            let lun = a.luns.iter().map(|l| l.lun + 1).max().unwrap_or(0);
            a.luns.push(Lun {
                lun,
                backing,
                serial: None,
            });
        }
        None => profile.scsi.push(ScsiAdapter {
            slot,
            luns: vec![Lun {
                lun: 0,
                backing,
                serial: None,
            }],
        }),
    }
    let put = AnyRequest::Profiles(profiles::Request::Put(profiles::Put {
        id,
        profile,
        revision: Some(rev.revision),
    }));
    crate::systems::ask(rq, &k.name, "Assign virtual disk", &put).map(|_| ())
}

/// `GET`, `POST /server/<host>/virtual/storage/create`.
pub fn create(rq: &Rq<'_>, k: &Kept) -> Response {
    let f = rq.req.form();
    let back = format!("/server/{}/virtual/storage", k.name);
    if !rq.post() || f.has("vdisk") {
        let mut ms = Vec::new();
        if let Some(m) = super::lacking(k, qsi::caps::VRES) {
            ms.push(m);
        }
        return rq.page(page(
            &rq.ctx,
            &form_of(rq, k, &qsweb::http::Form::default(), ms),
        ));
    }
    if f.has("pool") {
        return Response::redirect(&rq.ctx.url(&format!("{back}/pool")));
    }
    if f.has("cancel") {
        return Response::redirect(&rq.ctx.url(&back));
    }
    if let Some(no) = crate::app::routes::lacks(rq, webusers::Task::VirtualResources) {
        return no;
    }
    let name = f.str("n").trim().to_uppercase();
    let gb: Option<u64> = f
        .str("s")
        .trim()
        .replace(',', "")
        .parse()
        .ok()
        .filter(|g| *g > 0);
    let pool = f.str("p").split(' ').next().unwrap_or("").to_string();
    let invalid = |what: &str| {
        let m = qmcmsg::message(Kind::Error, &qmcmsg::NOT_VALID, &[what]);
        rq.page(page(&rq.ctx, &form_of(rq, k, &f, vec![m])))
    };
    if name.is_empty() {
        return invalid("Type the virtual disk's name.");
    }
    let Some(gb) = gb else {
        return invalid("Type the size in whole gigabytes.");
    };
    let slot = match f.str("c").trim() {
        "" | "(next free)" => None,
        c => match c.parse::<u32>() {
            Ok(s) if s > 0 => Some(s),
            _ => return invalid("Type the client adapter's ID, or leave it (next free)."),
        },
    };
    let req = AnyRequest::Vres(vres::Request::VdiskCreate(vres::VdiskCreate {
        pool: pool.clone(),
        name: name.clone(),
        mib: gb * 1_000_000_000 / (1 << 20),
        thick: f.str("t") == "thick",
    }));
    let action = format!("create virtual disk {pool}/{name} on {}", k.name);
    if let Err(ms) = super::act(rq, k, &action, &req) {
        return rq.page(page(&rq.ctx, &form_of(rq, k, &f, ms)));
    }
    let made = qmcmsg::message(Kind::Done, &qmcmsg::VDISK_CREATED, &[&name, &pool]);
    let to = f.str("a");
    let Some(p) = k.partitions.iter().find(|p| p.name == to) else {
        return rq.done(&back, made);
    };
    match assign(rq, k, p.id, &pool, &name, slot) {
        Ok(()) => {
            rq.audit(&format!("assign {pool}/{name} to {}", p.name), "done");
            rq.done_all(
                &back,
                vec![
                    made,
                    qmcmsg::message(Kind::Done, &qmcmsg::VDISK_ASSIGNED, &[&name, &p.name]),
                ],
            )
        }
        Err(mut ms) => {
            rq.audit(&format!("assign {pool}/{name} to {}", p.name), "refused");
            ms.insert(
                0,
                qmcmsg::message(Kind::Error, &qmcmsg::VDISK_NOT_ASSIGNED, &[&name, &p.name]),
            );
            ms.insert(0, made);
            rq.done_all(&back, ms)
        }
    }
}

/// `GET`, `POST /server/<host>/virtual/storage/resize?p=&n=`: a disk's
/// properties, and its size grown.
pub fn resize(rq: &Rq<'_>, k: &Kept) -> Response {
    let f = rq.req.form();
    let get = |n: &str| {
        if rq.post() {
            f.str(n).to_string()
        } else {
            rq.q(n)
        }
    };
    let (pool, name) = (get("p"), get("n"));
    let back = format!("/server/{}/virtual/storage", k.name);
    let Some(d) = k
        .resources
        .vdisks
        .iter()
        .find(|d| d.pool == pool && d.name == name)
    else {
        return crate::app::routes::error(rq, 404, &qmcmsg::NOT_FOUND, &[&rq.req.path]);
    };
    let mut messages = Vec::new();
    if rq.post() {
        if f.has("cancel") {
            return Response::redirect(&rq.ctx.url(&back));
        }
        if let Some(no) = crate::app::routes::lacks(rq, webusers::Task::VirtualResources) {
            return no;
        }
        match f.str("s").trim().replace(',', "").parse::<u64>() {
            Ok(gb) if gb * 1_000_000_000 > d.bytes => {
                let req = AnyRequest::Vres(vres::Request::VdiskResize(vres::VdiskResize {
                    pool: pool.clone(),
                    name: name.clone(),
                    mib: gb * 1_000_000_000 / (1 << 20),
                }));
                let action = format!("resize virtual disk {pool}/{name} on {}", k.name);
                match super::act(rq, k, &action, &req) {
                    Ok(_) => {
                        let m = qmcmsg::message(
                            Kind::Done,
                            &qmcmsg::VDISK_RESIZED,
                            &[&name, &gb.to_string()],
                        );
                        return rq.done(&back, m);
                    }
                    Err(ms) => messages = ms,
                }
            }
            _ => messages.push(qmcmsg::message(
                Kind::Error,
                &qmcmsg::NOT_VALID,
                &["Type a size larger than the disk's, in whole gigabytes: a disk only grows."],
            )),
        }
    }
    let holder = k
        .partitions
        .iter()
        .find(|p| p.id == d.holder.partition && d.holder.partition != 0)
        .map_or_else(
            || "(unassigned)".to_string(),
            |p| format!("{} (adapter {})", p.name, d.holder.slot),
        );
    let groups = [PropGroup {
        title: None,
        rows: vec![
            Prop::new("Name", mono(&d.name)),
            Prop::new("Storage pool", mono(&d.pool)),
            Prop::new(
                "Size (GB)",
                format!(
                    "<input type=\"text\" name=\"s\" value=\"{}\" size=\"8\">",
                    super::gb(rq, d.bytes)
                ),
            ),
            Prop::new("Allocated (GB)", super::gb(rq, d.allocated)),
            Prop::new(
                "Provisioning",
                if d.thick { "Thick" } else { "Thin" }.into(),
            ),
            Prop::new("Assigned partition", esc(&holder)),
        ],
    }];
    let body = form(
        &rq.ctx,
        "",
        &format!(
            "{}{}{}\n{}",
            hidden("p", &pool),
            hidden("n", &name),
            props_table(rq.ctx.product, &groups, true),
            buttons(&[Button::new("ok", "Resize"), Button::new("cancel", "Cancel")])
        ),
    );
    rq.page(super::form_page(
        &k.name,
        "storage",
        &format!("Virtual disk {name}"),
        messages,
        body,
    ))
}
