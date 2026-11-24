// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! The forms the design links to and does not draw (`docs/OPEN_QUESTIONS.md`,
//! item 37): a switch's create and properties, a storage pool's create
//! and properties, the tape volumes, a processor pool's change. Each is
//! the shared properties pattern with the fields of its requests only.

use std::fmt::Write as _;

use qsweb::html::esc;
use qsweb::http::Response;
use qsweb::msg::{Kind, Message};
use qsweb::partials::{
    Button, Cell, Col, List, Prop, PropGroup, buttons, form, hidden, list_table, mono, props_table,
};

use mgmtproto::Kept;
use qsi::any::{AnyReply, AnyRequest};
use qsi::host::model::MediaDevice;
use qsi::host::vres::{self, Bond, Protection};
use qsi::host::{partitions, profiles};
use webusers::Task;

use crate::app::routes::{Rq, lacks};

/// The interfaces of `form`'s checkboxes (`u`), each once; a value
/// with commas is several, as a typed list was.
fn chosen(form: &qsweb::http::Form) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for w in form.all("u").iter().flat_map(|v| v.split(',')) {
        let w = w.trim();
        if !w.is_empty() && !out.iter().any(|o| o == w) {
            out.push(w.to_string());
        }
    }
    out
}

/// The uplink's interfaces as a list that drops down, a checkbox each:
/// the host's interfaces no one owns and those of `switch`'s uplink,
/// never the management interface, another switch's or a partition's.
fn uplink_picker(k: &Kept, switch: &str, chosen: &[String]) -> String {
    use qsi::host::host::{ItemKind, Owner};
    let mut rows: Vec<(String, String)> = k
        .hardware
        .as_ref()
        .map(|h| {
            h.items
                .iter()
                .filter(|i| i.kind == ItemKind::Interface)
                .filter(|i| {
                    i.owner == Owner::None
                        || (i.owner == Owner::Switch
                            && !switch.is_empty()
                            && i.owner_name == switch)
                })
                .map(|i| {
                    let mut about = vec![i.name.clone()];
                    if !i.description.is_empty() {
                        about.push(i.description.clone());
                    }
                    about.push(if i.carrier && i.speed_mbps > 0 {
                        format!("{} Mb/s", i.speed_mbps)
                    } else {
                        "no link".into()
                    });
                    (i.location.clone(), about.join(", "))
                })
                .collect()
        })
        .unwrap_or_default();
    // One chosen before that the host no longer offers stays shown.
    for c in chosen {
        if !rows.iter().any(|(l, _)| l == c) {
            rows.push((c.clone(), "not offered by the host now".into()));
        }
    }
    let summary = if chosen.is_empty() {
        "None (a switch inside the host)".to_string()
    } else {
        chosen.join(", ")
    };
    let mut s = format!(
        "<details class=\"pick\"><summary>{}</summary>",
        esc(&summary)
    );
    if rows.is_empty() {
        s.push_str("<div class=\"grey\">The host has no free interface.</div>");
    }
    for (loc, about) in &rows {
        let on = if chosen.contains(loc) {
            " checked=\"checked\""
        } else {
            ""
        };
        let _ = write!(
            s,
            "<label><input type=\"checkbox\" name=\"u\" value=\"{}\"{on}> <span class=\"mono\">{}</span> <span class=\"grey\">{}</span></label>",
            esc(loc),
            esc(loc),
            esc(about)
        );
    }
    s.push_str("</details>");
    s
}

fn field(name: &str, value: &str, size: u32) -> String {
    format!(
        "<input type=\"text\" name=\"{name}\" value=\"{}\" size=\"{size}\">",
        esc(value)
    )
}

fn invalid(what: &str) -> Vec<Message> {
    vec![qmcmsg::message(Kind::Error, &qmcmsg::NOT_VALID, &[what])]
}

/// Numbers separated by commas or spaces.
fn numbers<T: std::str::FromStr>(text: &str) -> Option<Vec<T>> {
    text.split([',', ' '])
        .filter(|w| !w.is_empty())
        .map(|w| w.parse().ok())
        .collect()
}

/// The value of `name`: posted, else in the query.
fn value(rq: &Rq<'_>, f: &qsweb::http::Form, name: &str) -> String {
    if rq.post() {
        f.str(name).to_string()
    } else {
        rq.q(name)
    }
}

fn ok_cancel(ok: &str) -> String {
    buttons(&[Button::new("ok", ok), Button::new("cancel", "Cancel")])
}

#[allow(clippy::too_many_lines, reason = "one form, its fields in order")]
/// `GET`, `POST .../virtual/ethernet/switch?n=`: a switch made, or one
/// changed, renamed, given or cleared an uplink.
pub fn switch(rq: &Rq<'_>, k: &Kept) -> Response {
    let f = rq.req.form();
    let back = format!("/server/{}/virtual/ethernet", k.name);
    let name = value(rq, &f, "n");
    let had = k
        .resources
        .switches
        .iter()
        .find(|s| s.name == name)
        .cloned();
    let mut messages = Vec::new();
    if rq.post() {
        if f.has("cancel") {
            return Response::redirect(&rq.ctx.url(&back));
        }
        if let Some(no) = lacks(rq, Task::VirtualResources) {
            return no;
        }
        match switch_post(rq, k, &f, had.as_ref()) {
            Ok(m) => return rq.done(&back, m),
            Err(ms) => messages = ms,
        }
    }
    let keep = |n: &str, d: String| if rq.post() { f.str(n).to_string() } else { d };
    let (s_name, s_vlans, s_mtu) = had.as_ref().map_or((String::new(), Vec::new(), 0), |s| {
        (s.name.clone(), s.vlans.clone(), s.mtu)
    });
    let vlans = keep(
        "v",
        s_vlans
            .iter()
            .map(u16::to_string)
            .collect::<Vec<_>>()
            .join(", "),
    );
    let up = had
        .as_ref()
        .and_then(|s| s.uplink.clone())
        .unwrap_or(vres::Uplink {
            interfaces: Vec::new(),
            bond: None,
            native_vlan: None,
        });
    let bond = keep(
        "b",
        if up.bond == Some(Bond::Lacp) {
            "LACP".into()
        } else {
            "Active-backup".into()
        },
    );
    let groups = [
        PropGroup {
            title: None,
            rows: vec![
                Prop::new("Name", field("to", &keep("to", s_name.clone()), 16)).required(),
                Prop::new("VLAN IDs", field("v", &vlans, 30)),
                Prop::new(
                    "MTU",
                    field(
                        "mtu",
                        &keep(
                            "mtu",
                            if s_mtu == 0 {
                                String::new()
                            } else {
                                s_mtu.to_string()
                            },
                        ),
                        6,
                    ),
                ),
            ],
        },
        PropGroup {
            title: Some("Uplink".into()),
            rows: vec![
                Prop::new(
                    "Interfaces",
                    uplink_picker(
                        k,
                        &s_name,
                        &if rq.post() {
                            chosen(&f)
                        } else {
                            up.interfaces.clone()
                        },
                    ),
                ),
                Prop::new(
                    "Bond",
                    qsweb::partials::select("b", &["Active-backup", "LACP"], &bond),
                ),
                Prop::new(
                    "Native VLAN",
                    field(
                        "nv",
                        &keep(
                            "nv",
                            up.native_vlan.map(|v| v.to_string()).unwrap_or_default(),
                        ),
                        5,
                    ),
                ),
            ],
        },
    ];
    let title = if had.is_some() {
        format!("Virtual switch {name}")
    } else {
        "Create virtual switch".into()
    };
    let body = form(
        &rq.ctx,
        "",
        &format!(
            "{}{}\n{}<p class=\"small grey\">Choose the host's interfaces for the uplink; none for a switch inside the host. Only interfaces free or already this switch's are offered: never the host's management interface.</p>",
            hidden("n", &name),
            props_table(rq.ctx.product, &groups, true),
            ok_cancel("OK")
        ),
    );
    rq.page(super::form_page(
        &k.name, "ethernet", &title, messages, body,
    ))
}

#[allow(
    clippy::too_many_lines,
    reason = "one form and its request, step by step"
)]
fn switch_post(
    rq: &Rq<'_>,
    k: &Kept,
    f: &qsweb::http::Form,
    had: Option<&vres::Switch>,
) -> Result<Message, Vec<Message>> {
    let to = f.str("to").trim().to_string();
    if to.is_empty() {
        return Err(invalid("Type the switch's name."));
    }
    let vlans: Vec<u16> =
        numbers(f.str("v")).ok_or_else(|| invalid("VLAN IDs are numbers from 1 to 4094."))?;
    let mtu: u32 = match f.str("mtu").trim() {
        "" => 0,
        m => m
            .parse()
            .map_err(|_| invalid("The MTU is a number of bytes."))?,
    };
    let interfaces = chosen(f);
    let native = match f.str("nv").trim() {
        "" => None,
        v => Some(
            v.parse()
                .map_err(|_| invalid("The native VLAN is a VLAN ID."))?,
        ),
    };
    let ask = |action: &str, req: AnyRequest| super::act(rq, k, action, &req).map(|_| ());
    let name = match had {
        None => {
            let spec = vres::SwitchSpec {
                name: to.clone(),
                vlans,
                mtu,
            };
            ask(
                &format!("create switch {to} on {}", k.name),
                AnyRequest::Vres(vres::Request::SwitchCreate(spec)),
            )?;
            to.clone()
        }
        Some(s) => {
            if s.vlans != vlans || s.mtu != mtu {
                let spec = vres::SwitchSpec {
                    name: s.name.clone(),
                    vlans,
                    mtu,
                };
                ask(
                    &format!("change switch {} on {}", s.name, k.name),
                    AnyRequest::Vres(vres::Request::SwitchChange(spec)),
                )?;
            }
            if s.name != to {
                ask(
                    &format!("rename switch {} to {to} on {}", s.name, k.name),
                    AnyRequest::Vres(vres::Request::SwitchRename(vres::SwitchRename {
                        name: s.name.clone(),
                        to: to.clone(),
                    })),
                )?;
            }
            to.clone()
        }
    };
    let before = had
        .and_then(|s| s.uplink.as_ref())
        .map(|u| u.interfaces.clone())
        .unwrap_or_default();
    if interfaces.is_empty() && !before.is_empty() {
        ask(
            &format!("clear the uplink of switch {name} on {}", k.name),
            AnyRequest::Vres(vres::Request::UplinkClear(vres::UplinkClear {
                switch: name.clone(),
            })),
        )?;
    } else if !interfaces.is_empty() {
        let bond = (interfaces.len() > 1).then_some(if f.str("b") == "LACP" {
            Bond::Lacp
        } else {
            Bond::ActiveBackup
        });
        ask(
            &format!("set the uplink of switch {name} on {}", k.name),
            AnyRequest::Vres(vres::Request::UplinkSet(vres::UplinkSet {
                switch: name.clone(),
                uplink: vres::Uplink {
                    interfaces,
                    bond,
                    native_vlan: native,
                },
            })),
        )?;
    }
    Ok(qmcmsg::message(
        Kind::Done,
        if had.is_some() {
            &qmcmsg::SWITCH_CHANGED
        } else {
            &qmcmsg::SWITCH_CREATED
        },
        &[&name, &k.name],
    ))
}

const PROTECTIONS: [&str; 4] = ["None", "Mirrored", "RAID-5", "RAID-6"];

fn protection(word: &str) -> Protection {
    match word {
        "Mirrored" => Protection::Mirrored,
        "RAID-5" => Protection::Raid5,
        "RAID-6" => Protection::Raid6,
        _ => Protection::None,
    }
}

#[allow(clippy::too_many_lines, reason = "one form, its fields in order")]
/// `GET`, `POST .../virtual/storage/pool?n=&extend=`: a storage pool
/// made from the host's free disks, or one's properties: renamed, its
/// media repository designated, extended (which a host whose pools have
/// a fixed size refuses). A disk that carries another system's label is
/// taken only after the confirmation page names it.
pub fn pool(rq: &Rq<'_>, k: &Kept) -> Response {
    let f = rq.req.form();
    let back = format!("/server/{}/virtual/storage", k.name);
    let name = value(rq, &f, "n");
    let had = k.resources.pools.iter().find(|p| p.name == name).cloned();
    if rq.post() && f.has("cancel") {
        return Response::redirect(&rq.ctx.url(&back));
    }
    // The candidates are read now: what is free, and what holds another
    // system's (`foreign`), taken only after its confirmation.
    let disks = match crate::systems::ask(
        rq,
        &k.name,
        "Read the host's disks",
        &AnyRequest::Vres(vres::Request::Disks(qsi::Empty)),
    ) {
        Ok(AnyReply::Vres(vres::Reply::Disks(d))) => d,
        Err(ms) if rq.post() => return rq.done_all(&back, ms),
        _ => Vec::new(),
    };
    let free: Vec<&vres::PoolDisk> = disks
        .iter()
        .filter(|d| d.free || (d.foreign && d.pool.is_empty()))
        .collect();
    let mut messages = Vec::new();
    if rq.post() {
        if let Some(no) = lacks(rq, Task::VirtualResources) {
            return no;
        }
        let chosen: Vec<String> = free
            .iter()
            .filter(|d| f.has(&format!("d_{}", d.name)))
            .map(|d| d.name.clone())
            .collect();
        let spares: Vec<String> = free
            .iter()
            .filter(|d| f.has(&format!("s_{}", d.name)))
            .map(|d| d.name.clone())
            .collect();
        let foreign: Vec<String> = free
            .iter()
            .filter(|d| d.foreign && (chosen.contains(&d.name) || spares.contains(&d.name)))
            .map(|d| d.name.clone())
            .collect();
        let confirmed: Vec<String> = f
            .str("confirm")
            .split(',')
            .filter(|w| !w.is_empty())
            .map(str::to_string)
            .collect();
        if !foreign.iter().all(|d| confirmed.contains(d)) {
            return confirm_foreign(rq, k, &f, &foreign, &free);
        }
        let result = match &had {
            None if f.has("ok") => {
                let to = f.str("to").trim().to_string();
                if to.is_empty() {
                    Err(invalid("Type the pool's name."))
                } else {
                    super::act(
                        rq,
                        k,
                        &format!("create storage pool {to} on {}", k.name),
                        &AnyRequest::Vres(vres::Request::PoolCreate(vres::PoolCreate {
                            name: to.clone(),
                            protection: protection(f.str("pr")),
                            disks: chosen,
                            spares,
                            confirm: confirmed,
                        })),
                    )
                    .map(|_| qmcmsg::message(Kind::Done, &qmcmsg::POOL_CREATED, &[&to, &k.name]))
                }
            }
            Some(p) if f.has("extend") => super::act(
                rq,
                k,
                &format!("extend storage pool {} on {}", p.name, k.name),
                &AnyRequest::Vres(vres::Request::PoolExtend(vres::PoolExtend {
                    pool: p.name.clone(),
                    disks: chosen,
                    confirm: confirmed,
                })),
            )
            .map(|_| qmcmsg::message(Kind::Done, &qmcmsg::POOL_CHANGED, &[&p.name, &k.name])),
            Some(p) => pool_change(rq, k, p, &f),
            None => Err(Vec::new()),
        };
        match result {
            Ok(m) => return rq.done(&back, m),
            Err(ms) => messages = ms,
        }
    }
    let rows: Vec<Vec<Cell>> = free
        .iter()
        .map(|d| {
            let checked = |p: &str| {
                if rq.post() && f.has(&format!("{p}_{}", d.name)) {
                    " checked"
                } else {
                    ""
                }
            };
            vec![
                Cell::act(format!(
                    "<input type=\"checkbox\" name=\"d_{}\"{}>",
                    esc(&d.name),
                    checked("d")
                )),
                Cell::act(format!(
                    "<input type=\"checkbox\" name=\"s_{}\"{}>",
                    esc(&d.name),
                    checked("s")
                )),
                Cell::mono(&d.location),
                Cell::mono(&d.name),
                Cell::mono(&d.serial),
                Cell::num(&super::gb(rq, d.bytes)),
                Cell::text(&if d.foreign {
                    format!("{} (another system's label)", d.what)
                } else {
                    d.what.clone()
                }),
            ]
        })
        .collect();
    let candidates = list_table(&List {
        cols: [
            "Use",
            "Spare",
            "Location",
            "Disk",
            "Serial",
            "Size (GB)",
            "What it holds",
        ]
        .iter()
        .map(|c| Col::plain(c))
        .collect(),
        rows,
        empty: "The host has no free disk.".into(),
        keep: Vec::new(),
    });
    let (title, groups, ok) = match &had {
        None => (
            "Create storage pool".to_string(),
            vec![PropGroup {
                title: None,
                rows: vec![
                    Prop::new("Name", field("to", f.str("to"), 16)).required(),
                    Prop::new(
                        "Protection",
                        qsweb::partials::select("pr", &PROTECTIONS, f.str("pr")),
                    ),
                ],
            }],
            "Create",
        ),
        Some(p) => (
            format!("Storage pool {}", p.name),
            vec![PropGroup {
                title: None,
                rows: vec![
                    Prop::new("Name", field("to", &p.name, 16)).required(),
                    Prop::new("Protection", esc(&p.protection)),
                    Prop::new("State", esc(&p.state)),
                    Prop::new("Capacity (GB)", super::gb(rq, p.bytes)),
                    Prop::new("Allocated (GB)", super::gb(rq, p.allocated)),
                    Prop::new(
                        "Units",
                        esc(&p
                            .units
                            .iter()
                            .map(|u| format!("{} {} {}", u.number, u.location, u.state))
                            .collect::<Vec<_>>()
                            .join("; ")),
                    ),
                    Prop::new(
                        "Media repository",
                        format!(
                            "<input type=\"checkbox\" name=\"repo\"{}> Holds the media library",
                            if p.repository { " checked" } else { "" }
                        ),
                    ),
                ],
            }],
            "OK",
        ),
    };
    let extend = if had.is_some() {
        Button::new("extend", "Extend with the disks chosen")
    } else {
        Button::new("ok", ok)
    };
    let body = form(
        &rq.ctx,
        "",
        &format!(
            "{}{}<h3>The host's free disks</h3>{candidates}\n{}",
            hidden("n", &name),
            props_table(rq.ctx.product, &groups, true),
            buttons(&if had.is_some() {
                vec![
                    Button::new("ok", "OK"),
                    extend,
                    Button::new("cancel", "Cancel"),
                ]
            } else {
                vec![extend, Button::new("cancel", "Cancel")]
            })
        ),
    );
    rq.page(super::form_page(&k.name, "storage", &title, messages, body))
}

fn pool_change(
    rq: &Rq<'_>,
    k: &Kept,
    p: &vres::StoragePool,
    f: &qsweb::http::Form,
) -> Result<Message, Vec<Message>> {
    let to = f.str("to").trim().to_string();
    let mut name = p.name.clone();
    if !to.is_empty() && to != p.name {
        super::act(
            rq,
            k,
            &format!("rename storage pool {} to {to} on {}", p.name, k.name),
            &AnyRequest::Vres(vres::Request::PoolRename(vres::PoolRename {
                name: p.name.clone(),
                to: to.clone(),
            })),
        )?;
        name = to;
    }
    let repo = f.has("repo");
    if repo != p.repository {
        super::act(
            rq,
            k,
            &format!("media repository of {} on {}", name, k.name),
            &AnyRequest::Vres(vres::Request::Designate(vres::Designate {
                pool: if repo { name.clone() } else { String::new() },
            })),
        )?;
    }
    Ok(qmcmsg::message(
        Kind::Done,
        &qmcmsg::POOL_CHANGED,
        &[&name, &k.name],
    ))
}

/// The confirmation that a disk with another system's label is taken:
/// the form again, carried, with the disks' names to confirm.
fn confirm_foreign(
    rq: &Rq<'_>,
    k: &Kept,
    f: &qsweb::http::Form,
    foreign: &[String],
    free: &[&vres::PoolDisk],
) -> Response {
    let rows: Vec<Vec<Cell>> = free
        .iter()
        .filter(|d| foreign.contains(&d.name))
        .map(|d| {
            vec![
                Cell::mono(&d.location),
                Cell::mono(&d.name),
                Cell::mono(&d.serial),
                Cell::text(&d.what),
            ]
        })
        .collect();
    let table = list_table(&List {
        cols: ["Location", "Disk", "Serial", "What it holds"]
            .iter()
            .map(|c| Col::plain(c))
            .collect(),
        rows,
        empty: String::new(),
        keep: Vec::new(),
    });
    let mut carried: Vec<String> = vec!["n".into(), "to".into(), "pr".into(), "repo".into()];
    for d in free {
        carried.push(format!("d_{}", d.name));
        carried.push(format!("s_{}", d.name));
    }
    let names: Vec<&str> = carried
        .iter()
        .map(String::as_str)
        .filter(|n| f.has(n))
        .collect();
    let mut options = qsweb::partials::carry(f, &names);
    options.push_str(&hidden("confirm", &foreign.join(",")));
    if f.has("extend") {
        options.push_str(&hidden("extend", "1"));
    } else {
        options.push_str(&hidden("ok", "1"));
    }
    let body = qsweb::pages::confirm_body(
        &rq.ctx,
        "These disks carry another system's label. Everything on them is erased when the pool takes them. Press OK to take them, or Cancel to return without them.",
        &table,
        &options,
    );
    rq.page(super::form_page(
        &k.name,
        "storage",
        "Confirm Use of Disks",
        Vec::new(),
        body,
    ))
}

#[allow(clippy::too_many_lines, reason = "one form, its fields in order")]
/// `GET`, `POST .../virtual/tape/volume?p=&s=`: the tape volumes of the
/// media library, one made, protected or not; and with a partition's
/// device (`p` and `s`), the volume it is given in its profile.
pub fn volume(rq: &Rq<'_>, k: &Kept) -> Response {
    let f = rq.req.form();
    let back = format!("/server/{}/virtual/tape", k.name);
    let (part, slot) = (value(rq, &f, "p"), value(rq, &f, "s"));
    let mut messages = Vec::new();
    if rq.post() {
        if f.has("cancel") {
            return Response::redirect(&rq.ctx.url(&back));
        }
        if let Some(no) = lacks(rq, Task::VirtualResources) {
            return no;
        }
        let r = if f.has("create") {
            volume_create(rq, k, &f)
        } else if f.has("add_drive") {
            device_volume(rq, k, f.str("dp"), "", f.str("dv"))
        } else if let Some(n) = f.get("protect") {
            let v = k.resources.volumes.iter().find(|v| v.name == n);
            let on = v.is_none_or(|v| !v.protected);
            super::act(
                rq,
                k,
                &format!("protect tape volume {n} on {}", k.name),
                &AnyRequest::Vres(vres::Request::VolumeProtect(vres::VolumeProtect {
                    name: n.into(),
                    on,
                })),
            )
            .map(|_| qmcmsg::message(Kind::Done, &qmcmsg::VOLUME_CHANGED, &[n, &k.name]))
        } else {
            device_volume(rq, k, &part, &slot, f.str("v"))
        };
        match r {
            Ok(m) => return rq.done(&back, m),
            Err(ms) => messages = ms,
        }
    }
    let rows = k
        .resources
        .volumes
        .iter()
        .map(|v| {
            let holder = k
                .partitions
                .iter()
                .find(|p| p.id == v.holder.partition && v.holder.partition != 0)
                .map(|p| p.name.clone())
                .unwrap_or_default();
            let delete = rq.ctx.url(&format!("/confirm/volume-delete?ms={}&n={}", k.name, v.name));
            vec![
                Cell::act(format!(
                    "<input type=\"submit\" name=\"protect\" value=\"{}\"> <a href=\"{}\">Delete</a>",
                    esc(&v.name),
                    esc(&delete)
                )),
                Cell::mono(&v.name),
                Cell::mono(&v.identifier),
                Cell::num(&crate::fmt::number(v.capacity_mib / 1024, &rq.app.settings().number_format)),
                Cell::num(&super::gb(rq, v.used)),
                Cell::text(if v.protected { "Yes" } else { "No" }),
                Cell::mono(&holder),
            ]
        })
        .collect();
    let list = list_table(&List {
        cols: [
            "Protect or not",
            "Volume",
            "Identifier",
            "Capacity (GB)",
            "Used (GB)",
            "Protected",
            "Mounted by",
        ]
        .iter()
        .map(|c| Col::plain(c))
        .collect(),
        rows,
        empty: "The media library has no tape volumes.".into(),
        keep: Vec::new(),
    });
    let mut device = String::new();
    if !part.is_empty() {
        let names: Vec<&str> = k
            .resources
            .volumes
            .iter()
            .map(|v| v.name.as_str())
            .collect();
        device = format!(
            "<h3>Tape device {slot} of {}</h3>{}{}",
            esc(&part),
            props_table(
                rq.ctx.product,
                &[PropGroup {
                    title: None,
                    rows: vec![Prop::new(
                        "Volume",
                        qsweb::partials::select("v", &names, names.first().copied().unwrap_or("")),
                    )],
                }],
                true
            ),
            buttons(&[Button::new("give", "Give it the volume")]),
        );
    }
    if part.is_empty() {
        let parts: Vec<&str> = k.partitions.iter().map(|p| p.name.as_str()).collect();
        let mut vols = vec![""];
        vols.extend(k.resources.volumes.iter().map(|v| v.name.as_str()));
        device = format!(
            "{}{}",
            props_table(
                rq.ctx.product,
                &[PropGroup {
                    title: Some("New tape drive".into()),
                    rows: vec![
                        Prop::new(
                            "Partition",
                            qsweb::partials::select("dp", &parts, f.str("dp")),
                        )
                        .required(),
                        Prop::new(
                            "Volume",
                            format!(
                                "{} <span class=\"small grey\">(blank: none yet)</span>",
                                qsweb::partials::select("dv", &vols, f.str("dv"))
                            ),
                        ),
                    ],
                }],
                true
            ),
            buttons(&[Button::new("add_drive", "Create drive")]),
        );
    }
    let create = props_table(
        rq.ctx.product,
        &[PropGroup {
            title: Some("New volume".into()),
            rows: vec![
                Prop::new("Name", field("vn", f.str("vn"), 16)).required(),
                Prop::new("Identifier", field("vi", f.str("vi"), 8)),
                Prop::new("Capacity (GB)", field("vs", f.str("vs"), 8)).required(),
            ],
        }],
        true,
    );
    let body = form(
        &rq.ctx,
        "",
        &format!(
            "{}{}{list}{device}{create}\n{}",
            hidden("p", &part),
            hidden("s", &slot),
            buttons(&[
                Button::new("create", "Create volume"),
                Button::new("cancel", "Cancel")
            ])
        ),
    );
    rq.page(super::form_page(
        &k.name,
        "tape",
        "Tape volumes",
        messages,
        body,
    ))
}

fn volume_create(rq: &Rq<'_>, k: &Kept, f: &qsweb::http::Form) -> Result<Message, Vec<Message>> {
    let name = f.str("vn").trim().to_uppercase();
    let gb: u64 = f
        .str("vs")
        .trim()
        .parse()
        .ok()
        .filter(|g| *g > 0)
        .ok_or_else(|| invalid("Type the volume's capacity in whole gigabytes."))?;
    if name.is_empty() {
        return Err(invalid("Type the volume's name."));
    }
    let identifier = match f.str("vi").trim() {
        "" => name.chars().take(6).collect(),
        i => i.to_uppercase(),
    };
    super::act(
        rq,
        k,
        &format!("create tape volume {name} on {}", k.name),
        &AnyRequest::Vres(vres::Request::VolumeCreate(vres::VolumeCreate {
            name: name.clone(),
            identifier,
            mib: gb * 1024,
        })),
    )
    .map(|_| qmcmsg::message(Kind::Done, &qmcmsg::VOLUME_CREATED, &[&name, &k.name]))
}

/// Give partition `part`'s tape device `slot` volume `v` in its default
/// profile, put with the revision read. No `slot`: a new device, in the
/// profile's first free slot; no `v`: a device with no volume.
fn device_volume(
    rq: &Rq<'_>,
    k: &Kept,
    part: &str,
    slot: &str,
    v: &str,
) -> Result<Message, Vec<Message>> {
    let Some(p) = k.partitions.iter().find(|p| p.name == part) else {
        return Err(invalid(if slot.is_empty() {
            "Choose the partition the drive is for."
        } else {
            "Choose a partition's tape device from Virtual Tape."
        }));
    };
    let new = slot.is_empty();
    let slot = if new {
        None
    } else {
        Some(
            slot.parse::<u32>()
                .map_err(|_| invalid("Choose a partition's tape device from Virtual Tape."))?,
        )
    };
    let media = (!v.is_empty()).then(|| v.to_string());
    let get = AnyRequest::Partitions(partitions::Request::Get(partitions::Get { id: p.id }));
    let AnyReply::Partitions(partitions::Reply::Get(def)) =
        crate::systems::ask(rq, &k.name, "Read partition", &get)?
    else {
        return Err(Vec::new());
    };
    let read = AnyRequest::Profiles(profiles::Request::Get(profiles::Get {
        id: p.id,
        name: def.definition.default_profile.clone(),
    }));
    let AnyReply::Profiles(profiles::Reply::Get(mut rev)) =
        crate::systems::ask(rq, &k.name, "Read profile", &read)?
    else {
        return Err(Vec::new());
    };
    let slot = slot.unwrap_or_else(|| {
        let u = crate::partitions::edit::used(&rev.profile);
        (2..1000).find(|s| !u.contains(s)).unwrap_or(2)
    });
    match rev.profile.tape.iter_mut().find(|t| t.slot == slot) {
        Some(t) => t.media.clone_from(&media),
        None => rev.profile.tape.push(MediaDevice {
            slot,
            media: media.clone(),
        }),
    }
    let what = if new {
        format!("add a tape device in slot {slot} to {part}")
    } else {
        format!("give {part}'s tape device {slot} volume {v}")
    };
    super::act(
        rq,
        k,
        &what,
        &AnyRequest::Profiles(profiles::Request::Put(profiles::Put {
            id: p.id,
            profile: rev.profile,
            revision: Some(rev.revision),
        })),
    )
    .map(|_| {
        if new {
            qmcmsg::message(Kind::Done, &qmcmsg::TAPE_ADDED, &[&slot.to_string(), part])
        } else {
            qmcmsg::message(Kind::Done, &qmcmsg::VOLUME_GIVEN, &[v, part])
        }
    })
}

/// `GET`, `POST .../virtual/pools/change?id=`: a shared processor pool
/// made or changed: its name, its cores and its most units.
#[allow(
    clippy::too_many_lines,
    reason = "one form and its request, step by step"
)]
pub fn proc_pool(rq: &Rq<'_>, k: &Kept) -> Response {
    let f = rq.req.form();
    let back = format!("/server/{}/virtual/pools", k.name);
    let id: usize = value(rq, &f, "id").parse().unwrap_or(usize::MAX);
    let had = k.host.as_ref().and_then(|h| h.pools.get(id)).cloned();
    let mut messages = Vec::new();
    if rq.post() {
        if f.has("cancel") {
            return Response::redirect(&rq.ctx.url(&back));
        }
        if let Some(no) = lacks(rq, Task::VirtualResources) {
            return no;
        }
        let name = f.str("to").trim().to_string();
        let r = match (numbers::<u32>(f.str("c")), f.str("m").trim()) {
            (None, _) => Err(invalid("The cores are the host's core numbers.")),
            _ if name.is_empty() => Err(invalid("Type the pool's name.")),
            (Some(cores), m) => {
                let maximum_units = if m.is_empty() {
                    None
                } else {
                    match super::parse_units(m) {
                        Some(u) => Some(u),
                        None => {
                            return rq
                                .done_all(&back, invalid("The most units are hundredths: 2.00."));
                        }
                    }
                };
                let spec = vres::ProcPoolSpec {
                    name: name.clone(),
                    cores,
                    maximum_units,
                };
                let (req, word) = if had.is_some() {
                    (vres::Request::ProcPoolChange(spec), "change")
                } else {
                    (vres::Request::ProcPoolCreate(spec), "create")
                };
                super::act(
                    rq,
                    k,
                    &format!("{word} processor pool {name} on {}", k.name),
                    &AnyRequest::Vres(req),
                )
                .map(|_| qmcmsg::message(Kind::Done, &qmcmsg::PROC_POOL_CHANGED, &[&name, &k.name]))
            }
        };
        match r {
            Ok(m) => return rq.done(&back, m),
            Err(ms) => messages = ms,
        }
    }
    let (p_name, p_cores, p_max) = had.as_ref().map_or((String::new(), Vec::new(), 0), |p| {
        (p.name.clone(), p.cores.clone(), p.maximum)
    });
    let groups = [PropGroup {
        title: None,
        rows: vec![
            Prop::new("Name", field("to", &p_name, 16)).required(),
            Prop::new(
                "Cores",
                field(
                    "c",
                    &p_cores
                        .iter()
                        .map(u32::to_string)
                        .collect::<Vec<_>>()
                        .join(", "),
                    20,
                ),
            ),
            Prop::new(
                "Maximum units",
                field("m", &crate::systems::props::units(u64::from(p_max)), 6),
            ),
        ],
    }];
    let delete = if had.is_some() {
        format!(
            " <a href=\"{}\">Delete pool</a>",
            esc(&rq.ctx.url(&format!(
                "/confirm/proc-pool-delete?ms={}&n={p_name}",
                k.name
            )))
        )
    } else {
        String::new()
    };
    let body = form(
        &rq.ctx,
        "",
        &format!(
            "{}{}\n{}{delete}",
            hidden("id", &id.to_string()),
            props_table(rq.ctx.product, &groups, true),
            ok_cancel("OK")
        ),
    );
    let title = if had.is_some() {
        format!("Shared processor pool {p_name}")
    } else {
        "Create shared processor pool".into()
    };
    rq.page(super::form_page(&k.name, "pools", &title, messages, body))
}

/// A partition named `name`'s mono span, for confirmation tables.
pub fn named(name: &str) -> String {
    mono(name)
}
