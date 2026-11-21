// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! A managed system's properties (`system-general.html` and its six
//! tabs): from what the managed-systems service keeps of it, which is
//! what the system says of itself. A host's two power-on settings are
//! set over QSI where its hello offers it; what QSI cannot change (its
//! name, its IPL side, keylock and boot mode) answers with a message.

use std::fmt::Write as _;

use qsweb::frame::{Ctx, Page};
use qsweb::html::esc;
use qsweb::http::Response;
use qsweb::msg::{Kind, Message};
use qsweb::partials::{
    Button, Cell, Col, List, Prop, PropGroup, buttons, form, list_table, mono, props_table, status,
};

use mgmtproto::Kept;
use mgmtproto::kept::Kind as SystemKind;
use qsi::any::AnyRequest;
use qsi::caps;
use qsi::host::host::{self, PowerOff, PowerOn};
use qsi::host::model::{PartitionState, ProcessorMode};

use super::{system, when};
use crate::app::routes::Rq;
use crate::refcodes::explain;

/// The tabs, by route and label.
pub const TABS: [(&str, &str); 7] = [
    ("general", "General"),
    ("processors", "Processors"),
    ("memory", "Memory"),
    ("io", "I/O"),
    ("power", "Power-on parameters"),
    ("capabilities", "Capabilities"),
    ("refcode", "Reference code"),
];

/// General.
#[derive(Clone, Debug, Default)]
pub struct General {
    /// Its name.
    pub name: String,
    /// `QS/VS host`.
    pub kind: String,
    /// Its type and model.
    pub model: String,
    /// Its serial number.
    pub serial: String,
    /// The console's note about it.
    pub description: String,
    /// `Operating`, ...
    pub state: String,
    /// Its current code.
    pub code: String,
    /// The version of what answers QSI for it.
    pub service: String,
    /// `QS/VS version` or `QS/400 version`.
    pub version_label: String,
    /// Its release.
    pub release: String,
    /// Its build.
    pub build: String,
    /// Whether an update is available for it (Code updates, Phase 6).
    pub update: bool,
    /// When it last started, as its history says.
    pub since: String,
    /// `address:port`.
    pub address: String,
    /// Partitions defined and running.
    pub defined: usize,
    /// Running.
    pub running: usize,
}

/// Processors.
#[derive(Clone, Debug, Default)]
pub struct Processors {
    /// Installed, configurable, available, deconfigured.
    pub amounts: [String; 4],
    /// `6.00 (DefaultPool)`.
    pub pool: String,
    /// `2 (QSPROD01)`.
    pub dedicated: String,
    /// Each partition: name, mode, units, virtual processors.
    pub rows: Vec<[String; 4]>,
}

/// Memory.
#[derive(Clone, Debug, Default)]
pub struct Memory {
    /// Installed, configurable, available, firmware, region size,
    /// assigned: each with its unit.
    pub amounts: [String; 6],
    /// Each partition: name, current, minimum, desired, maximum.
    pub rows: Vec<[String; 5]>,
}

/// Power-on parameters.
#[derive(Clone, Debug)]
pub struct Power {
    /// `A` or `B`.
    pub side: String,
    /// `Normal`, `Manual`.
    pub keylock: String,
    /// The partitions marked for auto start are started with the host.
    pub start_partitions: bool,
    /// The default power off is immediate, not delayed.
    pub immediate: bool,
    /// A delayed power off's wait, seconds.
    pub delay: u32,
}

impl Default for Power {
    /// A host that does not say: the design's (auto start, delayed 300 s).
    fn default() -> Self {
        Self {
            side: String::new(),
            keylock: String::new(),
            start_partitions: true,
            immediate: false,
            delay: 300,
        }
    }
}

/// The current reference code.
#[derive(Clone, Debug, Default)]
pub struct Current {
    /// The code.
    pub code: String,
    /// What it means.
    pub meaning: String,
    /// When it was posted.
    pub since: String,
    /// Words 2 to 9.
    pub words: String,
}

/// What a tab shows.
#[derive(Clone, Debug)]
pub enum Tab {
    /// General.
    General(General),
    /// Processors.
    Processors(Processors),
    /// Memory.
    Memory(Memory),
    /// I/O: location, slot, description, owner, bus.
    Io(Vec<[String; 5]>),
    /// Power-on parameters.
    Power(Power),
    /// Capabilities: each with its value.
    Capabilities(Vec<(String, String)>),
    /// Reference code.
    Refcode(Current),
}

/// The page: a system's name, its tab and the messages.
#[derive(Clone, Debug)]
pub struct Props {
    /// The system's name.
    pub name: String,
    /// The tab.
    pub tab: Tab,
    /// The message rows.
    pub messages: Vec<Message>,
}

fn tabs(ctx: &Ctx, name: &str, current: &str) -> String {
    let links: Vec<String> = TABS
        .iter()
        .map(|(r, l)| {
            if *r == current {
                format!("<b>{}</b>", esc(l))
            } else {
                format!(
                    "<a href=\"{}\">{}</a>",
                    ctx.url(&format!("/server/{}/{r}", esc(name))),
                    esc(l)
                )
            }
        })
        .collect();
    format!("<div class=\"tabs\">{}</div>", links.join(" | "))
}

fn inner_list(head: &[&str], rows: &[Vec<(bool, String)>]) -> String {
    let mut s = String::from("<table class=\"list\" style=\"width:auto\"><tr>");
    for h in head {
        let _ = write!(s, "<th>{h}</th>");
    }
    s.push_str("</tr>");
    for (i, r) in rows.iter().enumerate() {
        s.push_str(if i % 2 == 1 {
            "<tr class=\"alt\">"
        } else {
            "<tr>"
        });
        for (num, v) in r {
            let class = if *num { "num" } else { "mono" };
            let _ = write!(s, "<td class=\"{class}\">{}</td>", esc(v));
        }
        s.push_str("</tr>");
    }
    s.push_str("</table>");
    s
}

fn refcode_link(ctx: &Ctx, name: &str, code: &str) -> String {
    format!(
        "<a class=\"mono\" href=\"{}\">{}</a>",
        ctx.url(&format!("/refcode/{}?ms={}", esc(code), esc(name))),
        esc(code)
    )
}

fn general(ctx: &Ctx, g: &General) -> String {
    let url = |r: &str| ctx.url(r);
    let n = esc(&g.name);
    let update = if g.update {
        format!(
            " &nbsp; <a href=\"{}\">Update available</a>",
            url("/updates/code")
        )
    } else {
        String::new()
    };
    let groups = [
        PropGroup {
            title: Some("Identification".into()),
            rows: vec![
                Prop::new(
                    "Name",
                    format!("<input type=\"text\" name=\"n\" value=\"{n}\" size=\"20\">"),
                ),
                Prop::new("Type", esc(&g.kind)),
                Prop::new("Type-Model", mono(&g.model)),
                Prop::new("Serial number", mono(&g.serial)),
                Prop::new(
                    "Description",
                    format!(
                        "<input type=\"text\" name=\"d\" value=\"{}\" size=\"40\">",
                        esc(&g.description)
                    ),
                ),
            ],
        },
        PropGroup {
            title: Some("State".into()),
            rows: vec![
                Prop::new("State", status(&g.state, Some(g.state == "Operating"))),
                Prop::new("Reference code", refcode_link(ctx, &g.name, &g.code)),
                Prop::new("Service processor version", mono(&g.service)),
                Prop::new(
                    &g.version_label,
                    format!("{} build {}{update}", mono(&g.release), esc(&g.build)),
                ),
                Prop::new("Power on since", esc(&g.since)),
                Prop::new(
                    "Service interface",
                    format!(
                        "{} &nbsp; <a href=\"{}\">Connectivity</a>",
                        mono(&g.address),
                        url("/connectivity")
                    ),
                ),
            ],
        },
        PropGroup {
            title: Some("Partitions".into()),
            rows: vec![
                Prop::new(
                    "Defined / running",
                    format!(
                        "<a href=\"{}\">{} / {}</a>",
                        url(&format!("/partitions?ms={n}")),
                        g.defined,
                        g.running
                    ),
                ),
                Prop::new(
                    "Virtual resources",
                    format!(
                        "<a href=\"{}\">Virtual Ethernet, storage, optical, tape, pools</a>",
                        url(&format!("/server/{n}/virtual/ethernet"))
                    ),
                ),
            ],
        },
    ];
    form(
        ctx,
        "",
        &format!(
            "{}\n{}",
            props_table(ctx.product, &groups, true),
            buttons(&[
                Button::new("apply", "Apply"),
                Button::new("ok", "OK"),
                Button::new("cancel", "Cancel")
            ])
        ),
    )
}

fn plain(ctx: &Ctx, groups: &[PropGroup], extra: &[Button]) -> String {
    let mut bs = vec![Button::new("refresh", "Refresh")];
    bs.extend_from_slice(extra);
    form(
        ctx,
        "",
        &format!(
            "{}\n{}",
            props_table(ctx.product, groups, true),
            buttons(&bs)
        ),
    )
}

fn processors(ctx: &Ctx, name: &str, p: &Processors) -> String {
    let rows: Vec<Vec<(bool, String)>> = p
        .rows
        .iter()
        .map(|r| {
            vec![
                (false, r[0].clone()),
                (false, r[1].clone()),
                (true, r[2].clone()),
                (true, r[3].clone()),
            ]
        })
        .collect();
    let table = inner_list(&["Partition", "Mode", "Units", "Virtual processors"], &rows)
        .replace("<td class=\"mono\">Dedicated", "<td>Dedicated")
        .replace("<td class=\"mono\">Shared", "<td>Shared");
    let groups = [
        PropGroup {
            title: Some("Processors".into()),
            rows: vec![
                Prop::new("Installed", esc(&p.amounts[0])),
                Prop::new("Configurable", esc(&p.amounts[1])),
                Prop::new("Available", esc(&p.amounts[2])),
                Prop::new("Deconfigured", esc(&p.amounts[3])),
                Prop::new(
                    "Shared processor pool size",
                    format!(
                        "{} &nbsp; <a href=\"{}\">Shared processor pools</a>",
                        esc(&p.pool),
                        ctx.url(&format!("/server/{}/virtual/pools", esc(name)))
                    ),
                ),
                Prop::new("Dedicated to partitions", esc(&p.dedicated)),
            ],
        },
        PropGroup {
            title: Some("Current assignment".into()),
            rows: vec![Prop::new("", table)],
        },
    ];
    plain(ctx, &groups, &[Button::new("ok", "OK")])
}

fn memory(ctx: &Ctx, m: &Memory) -> String {
    let rows: Vec<Vec<(bool, String)>> = m
        .rows
        .iter()
        .map(|r| {
            let mut v = vec![(false, r[0].clone())];
            v.extend(r[1..].iter().map(|c| (true, c.clone())));
            v
        })
        .collect();
    let table = inner_list(
        &["Partition", "Current (MB)", "Min", "Desired", "Max"],
        &rows,
    );
    let labels = [
        "Installed",
        "Configurable",
        "Available",
        "Firmware memory",
        "Memory region size",
        "Assigned to partitions",
    ];
    let groups = [
        PropGroup {
            title: Some("Memory".into()),
            rows: labels
                .iter()
                .zip(&m.amounts)
                .map(|(l, v)| Prop::new(l, esc(v)))
                .collect(),
        },
        PropGroup {
            title: Some("Current assignment".into()),
            rows: vec![Prop::new("", table)],
        },
    ];
    plain(ctx, &groups, &[Button::new("ok", "OK")])
}

fn io(rows: &[[String; 5]]) -> String {
    let list = List {
        cols: [
            "Location code",
            "Slot",
            "Description",
            "Owning partition",
            "Bus",
        ]
        .iter()
        .map(|c| Col::plain(c))
        .collect(),
        rows: rows
            .iter()
            .map(|r| {
                vec![
                    Cell::mono(&r[0]),
                    Cell::mono(&r[1]),
                    Cell::text(&r[2]),
                    Cell::text(&r[3]),
                    Cell::text(&r[4]),
                ]
            })
            .collect(),
        empty: "The system reports no adapters.".into(),
        keep: Vec::new(),
    };
    format!(
        "{}<p class=\"small grey\">Physical adapters are assigned to a partition from the partition's Hardware &gt; I/O page. The host keeps adapters that back virtual resources.</p>",
        list_table(&list)
    )
}

/// The delayed power off's option.
fn delayed(secs: u32) -> String {
    format!("Shut down partitions first (delayed, {secs} s)")
}

fn power(ctx: &Ctx, p: &Power) -> String {
    let radio = |name: &str, value: &str, on: bool| {
        let checked = if on { " checked" } else { "" };
        format!("<input type=\"radio\" name=\"{name}\" value=\"{value}\"{checked}>")
    };
    let select = |name: &str, opts: &[&str], chosen: &str| {
        let mut s = format!("<select name=\"{name}\">");
        for o in opts {
            let sel = if *o == chosen && Some(o) != opts.first() {
                " selected"
            } else {
                ""
            };
            let _ = write!(s, "<option{sel}>{o}</option>");
        }
        s.push_str("</select>");
        s
    };
    let groups = [
        PropGroup {
            title: Some("Next power on".into()),
            rows: vec![
                Prop::new(
                    "IPL side",
                    format!(
                        "{} A (permanent)<br>{} B (temporary)",
                        radio("side", "A", p.side != "B"),
                        radio("side", "B", p.side == "B")
                    ),
                ),
                Prop::new(
                    "Keylock position",
                    select("key", &["Normal", "Manual"], &p.keylock),
                ),
                Prop::new(
                    "Boot mode",
                    select(
                        "boot",
                        &[
                            "Normal",
                            "Service - partition standby",
                            "Service - firmware only",
                        ],
                        "Normal",
                    ),
                ),
                Prop::new(
                    "Power on partitions",
                    format!(
                        "{} Auto-start partitions flagged for auto start<br>{} None - leave all partitions Not activated",
                        radio("auto", "auto", p.start_partitions),
                        radio("auto", "none", !p.start_partitions)
                    ),
                ),
            ],
        },
        PropGroup {
            title: Some("Power off".into()),
            rows: vec![Prop::new(
                "Default power off",
                select(
                    "off",
                    &[&delayed(p.delay), "Immediate"],
                    if p.immediate { "Immediate" } else { "" },
                ),
            )],
        },
    ];
    form(
        ctx,
        "",
        &format!(
            "{}\n{}",
            props_table(ctx.product, &groups, true),
            buttons(&[
                Button::new("apply", "Apply"),
                Button::new("ok", "OK"),
                Button::new("cancel", "Cancel")
            ])
        ),
    )
}

fn capabilities(rows: &[(String, String)]) -> String {
    list_table(&List {
        cols: vec![Col::plain("Capability"), Col::plain("Value")],
        rows: rows
            .iter()
            .map(|(c, v)| vec![Cell::text(c), Cell::text(v)])
            .collect(),
        empty: String::new(),
        keep: Vec::new(),
    })
}

fn refcode(ctx: &Ctx, name: &str, c: &Current) -> String {
    let groups = [PropGroup {
        title: Some("Current reference code".into()),
        rows: vec![
            Prop::new(
                "Reference code",
                format!(
                    "<span class=\"mono\" style=\"font-size:12pt\">{}</span>",
                    refcode_link(ctx, name, &c.code)
                ),
            ),
            Prop::new("Meaning", esc(&c.meaning)),
            Prop::new("Since", esc(&c.since)),
            Prop::new("Words 2-9", mono(&c.words)),
        ],
    }];
    format!(
        "{}<p><a href=\"{}\">Reference code history for {}</a></p>",
        plain(ctx, &groups, &[]),
        ctx.url(&format!("/refcodes?ms={}", esc(name))),
        esc(name)
    )
}

/// The page.
pub fn page(ctx: &Ctx, v: &Props) -> Page {
    let (route, body) = match &v.tab {
        Tab::General(g) => ("general", general(ctx, g)),
        Tab::Processors(p) => ("processors", processors(ctx, &v.name, p)),
        Tab::Memory(m) => ("memory", memory(ctx, m)),
        Tab::Io(rows) => ("io", io(rows)),
        Tab::Power(p) => ("power", power(ctx, p)),
        Tab::Capabilities(c) => ("capabilities", capabilities(c)),
        Tab::Refcode(c) => ("refcode", refcode(ctx, &v.name, c)),
    };
    let h1 = format!("Managed system: {}", v.name);
    let mut crumb = vec![
        "Systems Management".into(),
        "Servers".into(),
        v.name.clone(),
    ];
    if route != "general"
        && let Some((_, label)) = TABS.iter().find(|(r, _)| *r == route)
    {
        crumb.push((*label).into());
    }
    Page {
        title: h1.clone(),
        crumb,
        h1,
        messages: v.messages.clone(),
        body: format!("{}{body}", tabs(ctx, &v.name, route)),
        help: "system-properties".into(),
        nav: Some(("Systems Management", Some("/servers"))),
        ..Page::default()
    }
}

/// Hundredths of a core as `3.50`.
pub fn units(n: u64) -> String {
    format!("{}.{:02}", n / 100, n % 100)
}

fn mb(rq: &Rq<'_>, n: u64) -> String {
    format!(
        "{} MB",
        crate::fmt::number(n, &rq.app.settings().number_format)
    )
}

fn general_of(rq: &Rq<'_>, k: &Kept) -> General {
    let standalone = k.kind == SystemKind::Standalone;
    let ipl = if standalone { "C1001000" } else { "C2001000" };
    let history = rq.app.console.mgmt().history(
        mgmtproto::calls::Filter {
            system: k.name.clone(),
            partition: Some(0),
            code: ipl.into(),
            ..Default::default()
        },
        0,
        1,
    );
    let since = match history.ok().and_then(|p| p.rows.into_iter().next()) {
        Some(r) => when(rq, r.posted.refcode.time_ms / 1000),
        None => "not reported".into(),
    };
    General {
        name: k.name.clone(),
        kind: k.kind.words().into(),
        model: k.model().into(),
        serial: k
            .identity
            .as_ref()
            .map_or_else(|| k.serial.clone(), |i| i.serial.clone()),
        description: k.description.clone(),
        state: k.status().into(),
        code: k.code(),
        service: k.release().into(),
        version_label: if standalone {
            "QS/400 version"
        } else {
            "QS/VS version"
        }
        .into(),
        release: k.release().into(),
        build: k
            .identity
            .as_ref()
            .map(|i| i.build.clone())
            .unwrap_or_default(),
        update: false,
        since,
        address: format!("{}:{}", k.address, k.port),
        defined: k.partitions.len(),
        running: k.running(),
    }
}

fn processors_of(k: &Kept) -> Processors {
    let Some(h) = &k.host else {
        let none = || "not reported".to_string();
        return Processors {
            amounts: [none(), none(), none(), none()],
            pool: none(),
            dedicated: none(),
            rows: Vec::new(),
        };
    };
    let amount = &h.processors;
    let pool = h.pools.first().map_or_else(
        || "none".into(),
        |d| format!("{}.00 ({})", d.cores.len(), d.name),
    );
    let mut dedicated = 0;
    let mut names = Vec::new();
    let rows = k
        .partitions
        .iter()
        .map(|part| {
            let profile = k.profile(part.id);
            let (mode, units_, virtuals) = match &part.current {
                Some(c) if c.mode == ProcessorMode::Dedicated => {
                    dedicated += c.cores.len();
                    names.push(part.name.clone());
                    let n = u32::try_from(c.cores.len()).unwrap_or(u32::MAX);
                    ("Dedicated".to_string(), u64::from(n) * 100, n)
                }
                Some(c) => (
                    format!("Shared, {}", if c.capped { "capped" } else { "uncapped" }),
                    u64::from(c.units),
                    c.virtuals,
                ),
                None => match profile {
                    Some(pr) if pr.processors.mode == ProcessorMode::Dedicated => (
                        "Dedicated".into(),
                        u64::from(pr.processors.desired) * 100,
                        pr.processors.desired,
                    ),
                    Some(pr) => (
                        format!(
                            "Shared, {}",
                            if pr.processors.sharing == Some(qsi::host::model::Sharing::Capped) {
                                "capped"
                            } else {
                                "uncapped"
                            }
                        ),
                        u64::from(pr.processors.desired_units.unwrap_or(0)),
                        pr.processors.desired,
                    ),
                    None => ("not reported".into(), 0, 0),
                },
            };
            [part.name.clone(), mode, units(units_), virtuals.to_string()]
        })
        .collect();
    Processors {
        amounts: [
            (amount.installed / 100).to_string(),
            (amount.configurable / 100).to_string(),
            units(amount.available),
            ((amount.installed - amount.configurable.min(amount.installed)) / 100).to_string(),
        ],
        pool,
        dedicated: if names.is_empty() {
            "0".into()
        } else {
            format!("{dedicated} ({})", names.join(", "))
        },
        rows,
    }
}

fn memory_of(rq: &Rq<'_>, k: &Kept) -> Memory {
    let Some(h) = &k.host else {
        let none = || "not reported".to_string();
        return Memory {
            amounts: [none(), none(), none(), none(), none(), none()],
            rows: Vec::new(),
        };
    };
    let n = |v: u64| crate::fmt::number(v, &rq.app.settings().number_format);
    let mut assigned = 0;
    let rows = k
        .partitions
        .iter()
        .map(|part| {
            let current = part
                .current
                .as_ref()
                .filter(|_| part.state != PartitionState::NotActivated)
                .map(|c| c.memory_mib);
            assigned += current.unwrap_or(0);
            let (min, des, max) =
                k.profile(part.id)
                    .map_or((String::new(), String::new(), String::new()), |p| {
                        (
                            n(p.memory.minimum_mib),
                            n(p.memory.desired_mib),
                            n(p.memory.maximum_mib),
                        )
                    });
            [
                part.name.clone(),
                current.map_or_else(|| "0 (not activated)".into(), n),
                min,
                des,
                max,
            ]
        })
        .collect();
    let m = &h.memory;
    Memory {
        amounts: [
            mb(rq, m.installed),
            mb(rq, m.configurable),
            mb(rq, m.available),
            mb(rq, h.reserved_memory_mib),
            mb(rq, h.memory_block_mib),
            mb(rq, assigned),
        ],
        rows,
    }
}

fn io_of(k: &Kept) -> Vec<[String; 5]> {
    use qsi::host::host::{ItemKind, Owner};
    k.hardware.as_ref().map_or_else(Vec::new, |h| {
        h.items
            .iter()
            .filter(|i| i.kind == ItemKind::Pci)
            .map(|i| {
                let who = match i.owner {
                    Owner::None => String::new(),
                    Owner::Partition => i.owner_name.clone(),
                    _ => "QS/VS host".into(),
                };
                let owner = if i.what.is_empty() || who.is_empty() {
                    who
                } else {
                    format!("{who} ({})", i.what)
                };
                [
                    i.location.clone(),
                    i.name.clone(),
                    i.description.clone(),
                    owner,
                    String::new(),
                ]
            })
            .collect()
    })
}

fn capabilities_of(k: &Kept) -> Vec<(String, String)> {
    use qsi::caps;
    let has = |c: u64| k.caps & c != 0;
    let yes = |b: bool| if b { "Yes" } else { "No" }.to_string();
    let host = k.kind == SystemKind::Host;
    let pools = k.host.as_ref().map_or(0, |h| h.pools.len());
    let max = k.host.as_ref().map_or(0, |h| h.max_partitions);
    vec![
        (
            "Partitioning".into(),
            if host {
                "Yes (QS/VS)".into()
            } else {
                "No".into()
            },
        ),
        (
            "Maximum partitions".into(),
            if host { max.to_string() } else { "1".into() },
        ),
        (
            "Shared processor pools".into(),
            if host && pools > 0 {
                format!("Yes, {pools} pools")
            } else {
                yes(false)
            },
        ),
        ("Dedicated processors".into(), yes(has(caps::PARTITION))),
        (
            "Virtual Ethernet".into(),
            if has(caps::VRES) {
                "Yes, 802.1Q VLAN aware".into()
            } else {
                yes(false)
            },
        ),
        ("Virtual storage (SCSI client)".into(), yes(has(caps::VRES))),
        ("Virtual optical".into(), yes(has(caps::VRES))),
        (
            "Virtual tape".into(),
            if has(caps::PHYSICAL_IO) {
                "Yes, including physical tape drives".into()
            } else {
                yes(has(caps::VRES))
            },
        ),
        ("Live partition migration".into(), "No".into()),
        ("Service processor redundancy".into(), "No".into()),
        (
            "Remote console (terminal window)".into(),
            yes(has(caps::CONSOLE)),
        ),
        (
            "Control panel functions".into(),
            if has(caps::PANEL) {
                "Yes, 01-03, 08, 11-22, 34".into()
            } else {
                yes(false)
            },
        ),
    ]
}

fn current_of(rq: &Rq<'_>, k: &Kept) -> Current {
    let code = k.code();
    let meaning = explain::explain(rq.app.catalogue(), &code).map_or_else(
        || k.refcode().map(|r| r.text.clone()).unwrap_or_default(),
        |e| e.meaning,
    );
    let r = k.refcode();
    Current {
        code,
        meaning,
        since: r.map_or_else(|| "not reported".into(), |r| when(rq, r.time_ms / 1000)),
        words: r.map_or_else(
            || ["00000000"; 8].join(" "),
            |r| {
                let mut w = r.words.clone();
                w.resize(8, "00000000".into());
                w.join(" ")
            },
        ),
    }
}

/// The Power-on parameters tab of `k`: its slot and keylock as its
/// state says, its power-on settings as kept.
fn power_of(k: &Kept) -> Power {
    let mut p = Power {
        side: k.state.as_ref().map(|s| s.slot.clone()).unwrap_or_default(),
        keylock: match k.state.as_ref().and_then(|s| s.keylock) {
            Some(qsi::base::Keylock::Manual) => "Manual".into(),
            _ => "Normal".into(),
        },
        ..Power::default()
    };
    if let Some(on) = k.power_on {
        p.start_partitions = on.start_partitions;
        p.immediate = on.power_off == PowerOff::Immediate;
        p.delay = on.delay;
    }
    p
}

/// A post of the Power-on parameters tab: the two power-on settings set
/// when they changed and the host offers it; the IPL side, keylock and
/// boot mode answered with their message when they were changed.
/// Whether the settings were set, and the messages.
fn set_power_on(rq: &Rq<'_>, k: &Kept, f: &qsweb::http::Form) -> (bool, Vec<Message>) {
    let shown = power_of(k);
    let mut messages = Vec::new();
    let side = if shown.side == "B" { "B" } else { "A" };
    let fixed_changed = (f.has("side") && f.str("side") != side)
        || (f.has("key") && f.str("key") != shown.keylock)
        || (f.has("boot") && f.str("boot") != "Normal");
    if fixed_changed {
        rq.audit(
            &format!("IPL side, keylock or boot mode of {}", k.name),
            "refused: set at the system",
        );
        messages.push(qmcmsg::message(
            Kind::Error,
            &qmcmsg::POWER_ON_PARAMETERS,
            &[&k.name],
        ));
    }
    let want = PowerOn {
        start_partitions: f.str("auto") != "none",
        power_off: if f.str("off") == "Immediate" {
            PowerOff::Immediate
        } else {
            PowerOff::Delayed
        },
        delay: shown.delay,
    };
    let now = PowerOn {
        start_partitions: shown.start_partitions,
        power_off: if shown.immediate {
            PowerOff::Immediate
        } else {
            PowerOff::Delayed
        },
        delay: shown.delay,
    };
    if !f.has("auto") || want == now {
        return (false, messages);
    }
    let what = format!("power-on settings of {}", k.name);
    if let Some(lacking) = mgmtproto::ask::lacks(k.caps, caps::HOST | caps::POWER_ON) {
        rq.audit(&what, &format!("refused: {lacking}"));
        messages.push(qmcmsg::message(
            Kind::Error,
            &qmcmsg::LACKS,
            &[&k.name, lacking],
        ));
        return (false, messages);
    }
    let req = AnyRequest::Host(host::Request::SetPowerOn(want));
    match super::ask(rq, &k.name, "Power-on settings", &req) {
        Ok(_) => {
            rq.audit(
                &what,
                &format!(
                    "set: start partitions {}, power off {:?}",
                    want.start_partitions, want.power_off
                ),
            );
            (true, messages)
        }
        Err(ms) => {
            rq.audit(&what, "refused");
            messages.extend(ms);
            (false, messages)
        }
    }
}

/// `GET`, `POST /server/<name>/<tab>`.
pub fn route(rq: &Rq<'_>, name: &str, tab: &str) -> Response {
    let Some(k) = system(rq, name) else {
        return crate::app::routes::error(rq, 404, &qmcmsg::NO_SUCH_SYSTEM, &[name]);
    };
    let f = rq.req.form();
    let mut messages = Vec::new();
    if rq.post()
        && !f.has("refresh")
        && let Some(no) = crate::app::routes::lacks(rq, webusers::Task::Settings)
    {
        return no;
    }
    if rq.post() && !f.has("refresh") {
        if f.has("cancel") || (tab != "general" && tab != "power") {
            return Response::redirect(&rq.ctx.url("/servers"));
        }
        if tab == "general" {
            if f.str("n").trim() != k.name {
                messages.push(qmcmsg::message(
                    Kind::Error,
                    &qmcmsg::NAME_IS_THE_SYSTEMS,
                    &[&k.name],
                ));
            }
            let d = f.str("d").trim().to_string();
            if d != k.description {
                let done = rq
                    .app
                    .console
                    .mgmt()
                    .describe(k.name.clone(), d, rq.user().into());
                let ok = matches!(done, Ok(Ok(())));
                rq.audit(
                    &format!("describe {}", k.name),
                    if ok { "changed" } else { "not changed" },
                );
                if ok && messages.is_empty() && f.has("ok") {
                    return rq.done(
                        "/servers",
                        qmcmsg::message(Kind::Done, &qmcmsg::SYSTEM_DESCRIBED, &[&k.name]),
                    );
                }
                if ok {
                    messages.push(qmcmsg::message(
                        Kind::Done,
                        &qmcmsg::SYSTEM_DESCRIBED,
                        &[&k.name],
                    ));
                }
            } else if messages.is_empty() && f.has("ok") {
                return Response::redirect(&rq.ctx.url("/servers"));
            }
        } else {
            let (done, ms) = set_power_on(rq, &k, &f);
            if done && ms.is_empty() && f.has("ok") {
                let m = qmcmsg::message(Kind::Done, &qmcmsg::POWER_ON_SET, &[&k.name]);
                return rq.done("/servers", m);
            }
            if done {
                messages.push(qmcmsg::message(
                    Kind::Done,
                    &qmcmsg::POWER_ON_SET,
                    &[&k.name],
                ));
            }
            messages.extend(ms);
        }
    }
    let k = system(rq, name).unwrap_or(k);
    let tab = match tab {
        "general" => Tab::General(general_of(rq, &k)),
        "processors" => Tab::Processors(processors_of(&k)),
        "memory" => Tab::Memory(memory_of(rq, &k)),
        "io" => Tab::Io(io_of(&k)),
        "power" => Tab::Power(power_of(&k)),
        "capabilities" => Tab::Capabilities(capabilities_of(&k)),
        "refcode" => Tab::Refcode(current_of(rq, &k)),
        _ => return crate::app::routes::error(rq, 404, &qmcmsg::NOT_FOUND, &[&rq.req.path]),
    };
    rq.page(page(
        &rq.ctx,
        &Props {
            name: k.name.clone(),
            tab,
            messages,
        },
    ))
}
