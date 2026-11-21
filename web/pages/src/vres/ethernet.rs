// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! Virtual Ethernet (`vr-ethernet.html`): the host's switches with
//! their VLANs, uplink and the partitions on them, and each partition's
//! virtual Ethernet adapters as its profile has them.

use qsweb::frame::{Ctx, Page};
use qsweb::html::esc;
use qsweb::http::Response;
use qsweb::msg::Message;
use qsweb::partials::{Cell, Col, List, list_table};

use mgmtproto::Kept;
use qsi::host::vres::Switch;

use crate::app::routes::Rq;

/// A switch's row.
#[derive(Clone, Debug, Default)]
pub struct SwitchRow {
    /// Its name.
    pub name: String,
    /// Its VLANs: `10, 20, 30`.
    pub vlans: String,
    /// Its uplink: `C1 port 1 (10 GbE)`, `none (internal)`.
    pub uplink: String,
    /// `VLAN aware`, `Untagged`.
    pub mode: String,
    /// The partitions on it: `QSPROD01 (VLAN 10), QSDEV01 (VLAN 20)`.
    pub attached: String,
}

/// A virtual Ethernet adapter's row.
#[derive(Clone, Debug, Default)]
pub struct AdapterRow {
    /// The partition.
    pub partition: String,
    /// Its slot.
    pub slot: u32,
    /// The switch.
    pub switch: String,
    /// Its VLAN.
    pub vlan: u32,
    /// Its MAC address.
    pub mac: String,
    /// The partition does not activate without it.
    pub required: bool,
}

/// What the tab shows.
#[derive(Clone, Debug, Default)]
pub struct Ethernet {
    /// The host.
    pub host: String,
    /// Its switches.
    pub switches: Vec<SwitchRow>,
    /// The adapters.
    pub adapters: Vec<AdapterRow>,
    /// The messages.
    pub messages: Vec<Message>,
}

/// The page.
pub fn page(ctx: &Ctx, v: &Ethernet) -> Page {
    let switches = List {
        cols: [
            "Actions",
            "Switch",
            "VLAN IDs",
            "Uplink",
            "Mode",
            "Attached partitions",
        ]
        .iter()
        .map(|c| Col::plain(c))
        .collect(),
        rows: v
            .switches
            .iter()
            .map(|s| {
                let props = ctx.url(&format!(
                    "/server/{}/virtual/ethernet/switch?n={}",
                    v.host, s.name
                ));
                let remove = ctx.url(&format!(
                    "/confirm/switch-delete?ms={}&n={}",
                    v.host, s.name
                ));
                vec![
                    Cell::act(format!(
                        "<a href=\"{}\">Properties</a> | <a href=\"{}\">Remove</a>",
                        esc(&props),
                        esc(&remove)
                    )),
                    Cell::mono(&s.name),
                    Cell::mono(&s.vlans),
                    Cell::text(&s.uplink),
                    Cell::text(&s.mode),
                    Cell::text(&s.attached),
                ]
            })
            .collect(),
        empty: "The host has no virtual switches.".into(),
        keep: Vec::new(),
    };
    let adapters = List {
        cols: [
            "Partition",
            "Adapter ID",
            "Switch",
            "VLAN",
            "MAC address",
            "Required",
        ]
        .iter()
        .map(|c| Col::plain(c))
        .collect(),
        rows: v
            .adapters
            .iter()
            .map(|a| {
                vec![
                    Cell::mono(&a.partition),
                    Cell::num(&a.slot.to_string()),
                    Cell::mono(&a.switch),
                    Cell::num(&a.vlan.to_string()),
                    Cell::mono(&a.mac),
                    Cell::text(if a.required { "Yes" } else { "No" }),
                ]
            })
            .collect(),
        empty: "No partition has a virtual Ethernet adapter.".into(),
        keep: Vec::new(),
    };
    let body = format!(
        "<h2>Virtual switches</h2>{}<h2>Virtual Ethernet adapters</h2>{}<form method=\"post\">{}<div class=\"buttons\"><input type=\"submit\" value=\"Create virtual switch...\"></div></form>",
        list_table(&switches),
        list_table(&adapters),
        ctx.csrf_field(),
    );
    super::page(ctx, &v.host, "ethernet", v.messages.clone(), &body)
}

/// A speed as the design writes it: `10 GbE`, `1 GbE`, `100 Mb/s`.
fn speed(mbps: u32) -> String {
    if mbps >= 1000 && mbps.is_multiple_of(1000) {
        format!("{} GbE", mbps / 1000)
    } else if mbps > 0 {
        format!("{mbps} Mb/s")
    } else {
        "no carrier".into()
    }
}

/// A switch's uplink in words.
fn uplink(s: &Switch) -> String {
    if s.uplink.is_none() {
        return "none (internal)".into();
    }
    let members: Vec<String> = s
        .members
        .iter()
        .map(|m| {
            let at = if m.name.is_empty() {
                &m.location
            } else {
                &m.name
            };
            format!("{at} ({})", speed(m.speed_mbps))
        })
        .collect();
    if members.is_empty() {
        s.uplink
            .as_ref()
            .map(|u| u.interfaces.join(", "))
            .unwrap_or_default()
    } else {
        members.join(", ")
    }
}

/// The tab of `k`, from what is kept.
pub fn view(k: &Kept, messages: Vec<Message>) -> Ethernet {
    let name_of = |id: u32| {
        k.partitions
            .iter()
            .find(|p| p.id == id)
            .map_or_else(|| id.to_string(), |p| p.name.clone())
    };
    let mut adapters = Vec::new();
    for p in &k.partitions {
        let Some(profile) = k.profile(p.id) else {
            continue;
        };
        for a in &profile.ethernet {
            // A MAC the host gives at activation is on its port.
            let mac = a.mac.clone().filter(|m| !m.is_empty()).unwrap_or_else(|| {
                k.resources
                    .ports
                    .iter()
                    .find(|o| o.partition == p.id && o.slot == a.slot)
                    .map(|o| o.mac.clone())
                    .unwrap_or_default()
            });
            adapters.push(AdapterRow {
                partition: p.name.clone(),
                slot: a.slot,
                switch: a.switch.clone().unwrap_or_default(),
                vlan: a.vlan,
                mac,
                required: a.required,
            });
        }
    }
    let switches = k
        .resources
        .switches
        .iter()
        .map(|s| {
            let on: Vec<&AdapterRow> = adapters.iter().filter(|a| a.switch == s.name).collect();
            let aware = s.vlans.len() > 1;
            let mut attached: Vec<String> = Vec::new();
            for a in on {
                let word = if aware {
                    format!("{} (VLAN {})", a.partition, a.vlan)
                } else {
                    a.partition.clone()
                };
                if !attached.contains(&word) {
                    attached.push(word);
                }
            }
            // Ports of partitions whose profile is not kept.
            for o in k.resources.ports.iter().filter(|o| o.switch == s.name) {
                let name = name_of(o.partition);
                if !attached.iter().any(|w| w.starts_with(&name)) {
                    attached.push(name);
                }
            }
            SwitchRow {
                name: s.name.clone(),
                vlans: s
                    .vlans
                    .iter()
                    .map(u16::to_string)
                    .collect::<Vec<_>>()
                    .join(", "),
                uplink: uplink(s),
                mode: if aware { "VLAN aware" } else { "Untagged" }.into(),
                attached: attached.join(", "),
            }
        })
        .collect();
    Ethernet {
        host: k.name.clone(),
        switches,
        adapters,
        messages,
    }
}

/// `GET`, `POST /server/<host>/virtual/ethernet`: the tab; its button
/// opens Create virtual switch.
pub fn route(rq: &Rq<'_>, k: &Kept) -> Response {
    if rq.post() {
        return Response::redirect(
            &rq.ctx
                .url(&format!("/server/{}/virtual/ethernet/switch", k.name)),
        );
    }
    let mut messages = Vec::new();
    if let Some(m) = super::lacking(k, qsi::caps::VRES) {
        messages.push(m);
    }
    rq.page(page(&rq.ctx, &view(k, messages)))
}
