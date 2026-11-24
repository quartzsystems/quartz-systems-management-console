// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! A virtual adapter made or edited, and a physical adapter added, in a
//! partition's default profile (`docs/OPEN_QUESTIONS.md`, item 37: the
//! design links to these and does not draw them). Ethernet: a switch,
//! a VLAN, tagged VLANs, a MAC address, required. SCSI client: virtual
//! disks of a pool. Serial: the console, shown and fixed. Physical: a
//! device the host has released for partitions.

use std::fmt::Write as _;

use qsweb::html::esc;
use qsweb::http::{Form, Response};
use qsweb::msg::{Kind, Message};
use qsweb::partials::{Button, Prop, PropGroup, buttons, form, hidden, props_table};

use mgmtproto::Kept;
use qsi::host::model::{EthernetAdapter, Lun, Profile, ScsiAdapter};
use webusers::Task;

use crate::app::routes::{Rq, lacks};

fn invalid(what: &str) -> Vec<Message> {
    vec![qmcmsg::message(Kind::Error, &qmcmsg::NOT_VALID, &[what])]
}

/// The slots a profile uses.
pub(crate) fn used(p: &Profile) -> Vec<u32> {
    let mut v: Vec<u32> = vec![0, 1];
    v.extend(p.ethernet.iter().map(|a| a.slot));
    v.extend(p.scsi.iter().map(|a| a.slot));
    v.extend(p.optical.iter().map(|a| a.slot));
    v.extend(p.tape.iter().map(|a| a.slot));
    v
}

fn field(name: &str, value: &str, size: u32) -> String {
    format!(
        "<input type=\"text\" name=\"{name}\" value=\"{}\" size=\"{size}\">",
        esc(value)
    )
}

/// Apply the form `f` of an adapter of `kind` at `slot` to `p`.
fn apply(k: &Kept, p: &mut Profile, kind: &str, slot: u32, f: &Form) -> Result<(), Vec<Message>> {
    match kind {
        "ethernet" => {
            let vlan: u32 = f
                .str("vlan")
                .trim()
                .parse()
                .map_err(|_| invalid("The VLAN is a number from 1 to 4094."))?;
            let tagged: Vec<u32> = f
                .str("tag")
                .split([',', ' '])
                .filter(|w| !w.is_empty())
                .map(|w| w.parse().map_err(|_| invalid("Tagged VLANs are numbers.")))
                .collect::<Result<_, _>>()?;
            let a = EthernetAdapter {
                slot,
                switch: Some(f.str("sw").to_string()).filter(|s| !s.is_empty()),
                vlan,
                tagged,
                mac: Some(f.str("mac").trim().to_string()).filter(|m| !m.is_empty()),
                required: f.has("req"),
            };
            p.ethernet.retain(|x| x.slot != slot);
            p.ethernet.push(a);
        }
        "scsi" => {
            let luns: Vec<Lun> = k
                .resources
                .vdisks
                .iter()
                .filter(|d| f.has(&format!("d_{}_{}", d.pool, d.name)))
                .enumerate()
                .map(|(i, d)| Lun {
                    lun: u32::try_from(i).unwrap_or(0),
                    backing: format!("{}/{}", d.pool, d.name),
                    serial: None,
                })
                .collect();
            if luns.is_empty() {
                return Err(invalid(
                    "Choose the virtual disks the adapter gives the partition.",
                ));
            }
            p.scsi.retain(|x| x.slot != slot);
            p.scsi.push(ScsiAdapter { slot, luns });
        }
        "io" => {
            let at = f.str("loc");
            if !k
                .resources
                .devices
                .iter()
                .any(|d| d.location == at && d.released)
            {
                return Err(invalid(
                    "Choose a device the host has released for partitions.",
                ));
            }
            p.physical_io.retain(|x| x.location != at);
            p.physical_io.push(super::props::physical(at, f.has("req")));
        }
        _ => {
            return Err(vec![qmcmsg::message(
                Kind::Error,
                &qmcmsg::SERIAL_FIXED,
                &[],
            )]);
        }
    }
    Ok(())
}

/// The form's rows for an adapter of `kind`.
#[allow(
    clippy::too_many_lines,
    reason = "one form and its request, step by step"
)]
fn rows(k: &Kept, p: &Profile, kind: &str, slot: u32) -> Vec<Prop> {
    match kind {
        "ethernet" => {
            let a = p.ethernet.iter().find(|x| x.slot == slot);
            let switches: Vec<&str> = k
                .resources
                .switches
                .iter()
                .map(|s| s.name.as_str())
                .collect();
            let chosen = a.and_then(|x| x.switch.clone()).unwrap_or_default();
            vec![
                Prop::new("Adapter ID", slot.to_string()),
                Prop::new("Switch", qsweb::partials::select("sw", &switches, &chosen)),
                Prop::new(
                    "VLAN",
                    field("vlan", &a.map_or("1".into(), |x| x.vlan.to_string()), 5),
                ),
                Prop::new(
                    "Tagged VLANs",
                    field(
                        "tag",
                        &a.map(|x| {
                            x.tagged
                                .iter()
                                .map(u32::to_string)
                                .collect::<Vec<_>>()
                                .join(", ")
                        })
                        .unwrap_or_default(),
                        20,
                    ),
                ),
                Prop::new(
                    "MAC address",
                    format!(
                        "{} <span class=\"small grey\">empty: the host gives one</span>",
                        field(
                            "mac",
                            &a.and_then(|x| x.mac.clone()).unwrap_or_default(),
                            18
                        )
                    ),
                ),
                Prop::new(
                    "Required",
                    format!(
                        "<input type=\"checkbox\" name=\"req\"{}>",
                        if a.is_none_or(|x| x.required) {
                            " checked"
                        } else {
                            ""
                        }
                    ),
                ),
            ]
        }
        "scsi" => {
            let have: Vec<String> = p
                .scsi
                .iter()
                .find(|x| x.slot == slot)
                .map(|a| a.luns.iter().map(|l| l.backing.clone()).collect())
                .unwrap_or_default();
            let disks = k.resources.vdisks.iter().fold(String::new(), |mut out, d| {
                    let b = format!("{}/{}", d.pool, d.name);
                    let on = if have.contains(&b) { " checked" } else { "" };
                    let _ = write!(
                        out,
                        "<input type=\"checkbox\" name=\"d_{}_{}\"{on}> <span class=\"mono\">{}</span><br>",
                        esc(&d.pool),
                        esc(&d.name),
                        esc(&b)
                    );
                    out
                });
            vec![
                Prop::new("Adapter ID", slot.to_string()),
                Prop::new("Virtual disks", disks),
            ]
        }
        "io" => {
            let released: Vec<String> = k
                .resources
                .devices
                .iter()
                .filter(|d| d.released)
                .map(|d| d.location.clone())
                .collect();
            let opts: Vec<&str> = released.iter().map(String::as_str).collect();
            vec![
                Prop::new(
                    "Device",
                    qsweb::partials::select("loc", &opts, opts.first().copied().unwrap_or("")),
                ),
                Prop::new("Required", "<input type=\"checkbox\" name=\"req\">".into()),
            ]
        }
        _ => vec![Prop::new(
            "Serial adapter",
            "The console: the terminal window, at adapter 0. It is fixed.".into(),
        )],
    }
}

/// `GET`, `POST /partition/<host>/<id>/edit?s=&new=&io=`.
pub fn route(rq: &Rq<'_>, k: &Kept, id: u32) -> Response {
    let f = rq.req.form();
    let get = |n: &str| {
        if rq.post() {
            f.str(n).to_string()
        } else {
            rq.q(n)
        }
    };
    let back = format!("/partition/{}/{id}/vadapters", k.name);
    if rq.post() && f.has("cancel") {
        return Response::redirect(&rq.ctx.url(&back));
    }
    let l = match super::props::load(rq, k, id) {
        Ok(l) => l,
        Err(ms) => return rq.done_all(&back, ms),
    };
    let p = &l.profile;
    let slot: u32 = get("s").parse().unwrap_or_else(|_| {
        let u = used(p);
        (2..1000).find(|s| !u.contains(s)).unwrap_or(2)
    });
    let kind = if get("io") == "1" {
        "io".to_string()
    } else if !get("new").is_empty() {
        get("new")
    } else if p.ethernet.iter().any(|a| a.slot == slot) {
        "ethernet".into()
    } else if p.scsi.iter().any(|a| a.slot == slot) {
        "scsi".into()
    } else {
        "serial".into()
    };
    let mut messages = Vec::new();
    if rq.post() {
        if let Some(no) = lacks(rq, Task::CreatePartitions) {
            return no;
        }
        let mut changed = p.clone();
        let rev = super::props::form_revision(rq, &k.name, &l);
        match apply(k, &mut changed, &kind, slot, &f)
            .and_then(|()| super::props::put(rq, k, id, changed, rev))
        {
            Ok(_) => {
                let mut ms = vec![qmcmsg::message(
                    Kind::Done,
                    &qmcmsg::PARTITION_CHANGED,
                    &[&l.def.name],
                )];
                if l.status.state != qsi::host::model::PartitionState::NotActivated {
                    ms.push(qmcmsg::message(
                        Kind::Info,
                        &qmcmsg::AT_NEXT_ACTIVATION,
                        &[&l.def.name],
                    ));
                }
                let to = if kind == "io" {
                    format!("/partition/{}/{id}/io", k.name)
                } else {
                    back
                };
                return rq.done_all(&to, ms);
            }
            Err(ms) => messages = ms,
        }
    }
    super::props::keep_revision(rq, &k.name, &l);
    let title = match kind.as_str() {
        "ethernet" => "Virtual Ethernet adapter",
        "scsi" => "Virtual SCSI client adapter",
        "io" => "Add physical adapter",
        _ => "Virtual serial adapter",
    };
    let groups = [PropGroup {
        title: None,
        rows: rows(k, p, &kind, slot),
    }];
    let body = form(
        &rq.ctx,
        "",
        &format!(
            "{}{}{}{}\n{}",
            hidden("s", &slot.to_string()),
            hidden("new", &get("new")),
            hidden("io", &get("io")),
            props_table(rq.ctx.product, &groups, true),
            buttons(&[Button::new("ok", "OK"), Button::new("cancel", "Cancel")])
        ),
    );
    rq.page(super::sub_page(&l.def.name, title, title, messages, body))
}
