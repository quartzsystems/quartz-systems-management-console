// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! A partition's properties (`partition-general.html` and its tabs).
//! Each field is its definition's, its default profile's or its
//! settings' (`docs/PARTITIONS.md`): the page reads them when it opens,
//! keeps the profile's revision in the session, and puts a change back
//! with it. A put refused as stale shows the page again with the
//! operator's entries and the new revision. A change to a running
//! partition's profile takes effect at its next activation, and the
//! page says so; a running shared partition's units, capping and weight
//! are also changed now.

use std::fmt::Write as _;

use qsweb::frame::{Ctx, Page};
use qsweb::html::esc;
use qsweb::http::{Form, Response};
use qsweb::msg::{Kind, Message};
use qsweb::partials::{
    Button, Cell, Col, List, Prop, PropGroup, buttons, form, list_table, mono, props_table, status,
};

use mgmtproto::Kept;
use qsi::any::{AnyReply, AnyRequest};
use qsi::base::Keylock;
use qsi::caps;
use qsi::host::model::{
    ConsoleMode, Definition, IplSource, IplType, PartitionState, PhysicalIo, ProcessorMode,
    Profile, Sharing,
};
use qsi::host::partition::{self, Settings, WithHost};
use qsi::host::partitions::{self, Status};
use qsi::host::profiles;
use webusers::Task;

use super::list::{good, state_word};
use crate::app::routes::{Rq, lacks};
use crate::systems::props::units;

/// General.
#[derive(Clone, Debug, Default)]
pub struct General {
    /// Its name.
    pub name: String,
    /// Its ID.
    pub id: u32,
    /// `QS/400`.
    pub environment: String,
    /// Its host.
    pub host: String,
    /// Its description.
    pub description: String,
    /// `Running`.
    pub state: String,
    /// Its current code.
    pub code: String,
    /// `Configured`.
    pub configuration: String,
    /// Its release: `V1R0M0`; empty when its system has not said.
    pub os: String,
    /// Its cumulative fix level; empty when not said.
    pub fix: String,
    /// The profile it runs with.
    pub profile: String,
    /// When its activation began; empty when not activated.
    pub since: String,
    /// `Normal`, `Service - manual IPL`.
    pub boot: String,
    /// `Normal`, `Manual`.
    pub keylock: String,
}

/// Processors.
#[derive(Clone, Debug, Default)]
#[allow(clippy::struct_excessive_bools, reason = "one switch a field")]
pub struct Processors {
    /// Shared, not dedicated.
    pub shared: bool,
    /// The pools: `DefaultPool (0)`.
    pub pools: Vec<String>,
    /// The pool chosen.
    pub pool: String,
    /// Units: minimum, desired, maximum, current.
    pub units: [String; 4],
    /// Virtual processors: minimum, desired, maximum, current.
    pub virtuals: [String; 4],
    /// Uncapped, not capped.
    pub uncapped: bool,
    /// The uncapped weight.
    pub weight: String,
}

/// Memory.
#[derive(Clone, Debug, Default)]
pub struct Memory {
    /// Minimum, desired, maximum, MB, as typed.
    pub mb: [String; 3],
    /// Held now: `16,384`.
    pub current: String,
    /// The host's region size: `256 MB`.
    pub region: String,
    /// What the host has free: `40,960 MB`.
    pub available: String,
}

/// A physical adapter's row.
#[derive(Clone, Debug, Default)]
pub struct IoRow {
    /// Its location code.
    pub location: String,
    /// Its slot.
    pub slot: String,
    /// What it is.
    pub description: String,
    /// It is given to the partition now.
    pub assigned: bool,
    /// The partition does not activate without it.
    pub required: bool,
}

/// A virtual adapter's row.
#[derive(Clone, Debug, Default)]
pub struct AdapterRow {
    /// Its slot.
    pub slot: u32,
    /// `Ethernet`, `SCSI client`, `Serial (console)`.
    pub kind: String,
    /// `ETHSW0 / VLAN 20`, `QS/VS host / 13`.
    pub connecting: String,
    /// `MAC 02:51:53:00:01:02`, `Virtual disk VD03 (120 GB, POOL1)`.
    pub backing: String,
    /// Required.
    pub required: bool,
    /// Shown and fixed: the console.
    pub fixed: bool,
}

/// Settings.
#[derive(Clone, Debug, Default)]
#[allow(clippy::struct_excessive_bools, reason = "one switch a field")]
pub struct Setting {
    /// Started with its host.
    pub auto_start: bool,
    /// `Normal`, `Service - manual IPL`.
    pub boot: String,
    /// `A`, `B`, `D (optical/install)`.
    pub source: String,
    /// `Normal`, `Manual`.
    pub keylock: String,
    /// The terminal window is its console.
    pub terminal: bool,
    /// Control panel functions from a console.
    pub panel: bool,
    /// Shut down delayed, not immediate, with its host.
    pub delayed: bool,
    /// The delay, seconds.
    pub delay: u32,
}

/// Other: the definition's facts.
#[derive(Clone, Debug, Default)]
pub struct Other {
    /// Its serial number.
    pub serial: String,
    /// When it was made.
    pub created: String,
    /// Its default profile.
    pub default_profile: String,
    /// Its place in the host's start: `order 2, after 30 s`.
    pub start: String,
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
    /// I/O.
    Io(Vec<IoRow>),
    /// Virtual adapters.
    Adapters(Vec<AdapterRow>),
    /// Settings.
    Settings(Setting),
    /// Other.
    Other(Other),
}

/// The page's view.
#[derive(Clone, Debug)]
pub struct Props {
    /// The host.
    pub host: String,
    /// The partition's ID.
    pub id: u32,
    /// Its name.
    pub name: String,
    /// The tab.
    pub tab: Tab,
    /// The messages.
    pub messages: Vec<Message>,
}

fn text(name: &str, value: &str, size: u32) -> String {
    format!(
        "<input type=\"text\" name=\"{name}\" value=\"{}\" size=\"{size}\">",
        esc(value)
    )
}

fn radio(name: &str, value: &str, on: bool) -> String {
    let c = if on { " checked" } else { "" };
    format!("<input type=\"radio\" name=\"{name}\" value=\"{value}\"{c}>")
}

fn check(name: &str, on: bool) -> String {
    let c = if on { " checked" } else { "" };
    format!("<input type=\"checkbox\" name=\"{name}\"{c}>")
}

/// A select whose first option is not marked, as the design draws it.
fn select(name: &str, options: &[&str], chosen: &str) -> String {
    let mut s = format!("<select name=\"{name}\">");
    for o in options {
        let sel = if *o == chosen && Some(o) != options.first() {
            " selected"
        } else {
            ""
        };
        let _ = write!(s, "<option{sel}>{}</option>", esc(o));
    }
    s.push_str("</select>");
    s
}

fn apply_ok_cancel() -> String {
    buttons(&[
        Button::new("apply", "Apply"),
        Button::new("ok", "OK"),
        Button::new("cancel", "Cancel"),
    ])
}

fn general(ctx: &Ctx, v: &Props, g: &General) -> String {
    let os = if g.os.is_empty() {
        String::new()
    } else {
        let fix = if g.fix.is_empty() {
            String::new()
        } else {
            format!(", cumulative PTF level {}", esc(&g.fix))
        };
        format!(
            "{}{fix} &nbsp; <a href=\"{}\">Navigator PTFs</a>",
            mono(&g.os),
            esc(&super::href(ctx, &v.host, v.id, "navigator"))
        )
    };
    let groups = [
        PropGroup {
            title: Some("Identification".into()),
            rows: vec![
                Prop::new("Name", text("n", &g.name, 20)),
                Prop::new("ID", g.id.to_string()),
                Prop::new("Environment", mono(&g.environment)),
                Prop::new(
                    "Managed system",
                    format!(
                        "<a href=\"{}\">{}</a>",
                        esc(&ctx.url(&format!("/server/{}/general", g.host))),
                        esc(&g.host)
                    ),
                ),
                Prop::new("Description", text("d", &g.description, 40)),
            ],
        },
        PropGroup {
            title: Some("State".into()),
            rows: vec![
                Prop::new("State", status(&g.state, good(&g.state))),
                Prop::new(
                    "Reference code",
                    format!(
                        "<a class=\"mono\" href=\"{}\">{}</a>",
                        esc(&ctx.url(&format!("/refcode/{}?ms={}&p={}", g.code, g.host, g.id))),
                        esc(&g.code)
                    ),
                ),
                Prop::new("Resource configuration", esc(&g.configuration)),
                Prop::new("OS version", os),
                Prop::new(
                    "Active profile",
                    format!(
                        "<a href=\"{}\">{}</a>",
                        esc(&super::href(ctx, &v.host, v.id, "profiles")),
                        esc(&g.profile)
                    ),
                ),
                Prop::new("Running since", esc(&g.since)),
                Prop::new("Boot mode", esc(&g.boot)),
                Prop::new("Keylock position", esc(&g.keylock)),
                Prop::new(
                    "Terminal window",
                    format!(
                        "<a href=\"{}\">Open terminal window</a> &nbsp; <a href=\"{}\">Control panel functions</a>",
                        esc(&ctx.url(&format!("/terminal/{}/{}", v.host, v.id))),
                        esc(&super::href(ctx, &v.host, v.id, "panel"))
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
            apply_ok_cancel()
        ),
    )
}

fn processors(ctx: &Ctx, p: &Processors) -> String {
    let pools: Vec<&str> = p.pools.iter().map(String::as_str).collect();
    let groups = [
        PropGroup {
            title: Some("Processing mode".into()),
            rows: vec![
                Prop::new(
                    "Mode",
                    format!(
                        "{} Shared {} Dedicated",
                        radio("m", "shared", p.shared),
                        radio("m", "ded", !p.shared)
                    ),
                ),
                Prop::new("Shared processor pool", select("pool", &pools, &p.pool)),
            ],
        },
        PropGroup {
            title: Some("Processing units".into()),
            rows: vec![
                Prop::new("Minimum", text("min", &p.units[0], 6)),
                Prop::new("Desired", text("des", &p.units[1], 6)),
                Prop::new("Maximum", text("max", &p.units[2], 6)),
                Prop::new("Current", esc(&p.units[3])),
            ],
        },
        PropGroup {
            title: Some("Virtual processors".into()),
            rows: vec![
                Prop::new("Minimum", text("vmin", &p.virtuals[0], 4)),
                Prop::new("Desired", text("vdes", &p.virtuals[1], 4)),
                Prop::new("Maximum", text("vmax", &p.virtuals[2], 4)),
                Prop::new("Current", esc(&p.virtuals[3])),
            ],
        },
        PropGroup {
            title: Some("Sharing".into()),
            rows: vec![
                Prop::new(
                    "Sharing mode",
                    select(
                        "sm",
                        &["Uncapped", "Capped"],
                        if p.uncapped { "Uncapped" } else { "Capped" },
                    ),
                ),
                Prop::new(
                    "Uncapped weight",
                    format!("{} (0-255)", text("w", &p.weight, 4)),
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
            apply_ok_cancel()
        ),
    )
}

fn memory(ctx: &Ctx, host: &str, m: &Memory) -> String {
    let groups = [PropGroup {
        title: Some("Memory (MB)".into()),
        rows: vec![
            Prop::new("Minimum", text("min", &m.mb[0], 8)),
            Prop::new("Desired", text("des", &m.mb[1], 8)),
            Prop::new("Maximum", text("max", &m.mb[2], 8)),
            Prop::new("Current", esc(&m.current)),
            Prop::new(
                "Region size",
                format!("{} (set on the managed system)", esc(&m.region)),
            ),
            Prop::new(&format!("Available on {host}"), esc(&m.available)),
        ],
    }];
    form(
        ctx,
        "",
        &format!(
            "{}\n{}",
            props_table(ctx.product, &groups, true),
            apply_ok_cancel()
        ),
    )
}

fn io(ctx: &Ctx, rows: &[IoRow]) -> String {
    let list = List {
        cols: [
            "Select",
            "Location code",
            "Slot",
            "Description",
            "Assigned",
            "Required",
        ]
        .iter()
        .map(|c| Col::plain(c))
        .collect(),
        rows: rows
            .iter()
            .enumerate()
            .map(|(i, r)| {
                vec![
                    Cell::act(format!("<input type=\"checkbox\" name=\"a{i}\">")),
                    Cell::mono(&r.location),
                    Cell::num(&r.slot),
                    Cell::text(&r.description),
                    Cell::text(if r.assigned { "Yes" } else { "No" }),
                    Cell::text(if r.required { "Yes" } else { "No" }),
                ]
            })
            .collect(),
        empty: "No physical adapters are assigned to this partition. All I/O is virtual (see Virtual adapters).".into(),
        keep: Vec::new(),
    };
    format!(
        "{}<form method=\"post\">{}<div class=\"buttons\"><input type=\"submit\" name=\"add\" value=\"Add physical adapter...\"><input type=\"submit\" name=\"remove\" value=\"Remove\"></div></form>",
        list_table(&list),
        ctx.csrf_field()
    )
}

fn adapters(ctx: &Ctx, v: &Props, rows: &[AdapterRow]) -> String {
    let list = List {
        cols: vec![
            Col::plain("Actions"),
            Col::sorted("Adapter ID", "id"),
            Col::sorted("Type", "t"),
            Col::plain("Connecting partition / slot"),
            Col::plain("Backing resource"),
            Col::plain("Required"),
        ],
        rows: rows
            .iter()
            .map(|r| {
                let edit = ctx.url(&format!("/partition/{}/{}/edit?s={}", v.host, v.id, r.slot));
                let remove = ctx.url(&format!(
                    "/confirm/adapter-remove?ms={}&p={}&s={}",
                    v.host, v.id, r.slot
                ));
                vec![
                    Cell::act(format!(
                        "<a href=\"{}\">Edit</a> | <a href=\"{}\">Remove</a>",
                        esc(&edit),
                        esc(&remove)
                    )),
                    Cell::num(&r.slot.to_string()),
                    Cell::text(&r.kind),
                    Cell::mono(&r.connecting),
                    Cell::mono(&r.backing),
                    Cell::text(if r.required { "Yes" } else { "No" }),
                ]
            })
            .collect(),
        empty: "The partition has no virtual adapters.".into(),
        keep: Vec::new(),
    };
    format!(
        "{}<form method=\"post\">{}<div class=\"buttons\">Create: <select name=\"t\"><option>Ethernet</option><option>SCSI client</option><option>Serial</option></select> <input type=\"submit\" value=\"Create...\"></div></form><p class=\"small grey\">Changes to required adapters take effect at the next activation; others can be added while the partition runs.</p>",
        list_table(&list),
        ctx.csrf_field()
    )
}

fn settings(ctx: &Ctx, s: &Setting) -> String {
    let delayed = format!("Delayed (PWRDWNSYS *CNTRLD, {} s)", s.delay);
    let groups = [
        PropGroup {
            title: Some("Start".into()),
            rows: vec![
                Prop::new("Auto start with managed system", check("as", s.auto_start)),
                Prop::new(
                    "Boot mode",
                    select(
                        "bm",
                        &["Normal", "Service - manual IPL", "Open firmware"],
                        &s.boot,
                    ),
                ),
                Prop::new(
                    "IPL source",
                    select("src", &["A", "B", "D (optical/install)"], &s.source),
                ),
                Prop::new(
                    "Keylock position",
                    select("key", &["Normal", "Manual"], &s.keylock),
                ),
            ],
        },
        PropGroup {
            title: Some("Console".into()),
            rows: vec![
                Prop::new(
                    "Console device",
                    select(
                        "con",
                        &[
                            "Terminal window (virtual serial adapter 0)",
                            "Physical adapter (none assigned)",
                        ],
                        if s.terminal {
                            ""
                        } else {
                            "Physical adapter (none assigned)"
                        },
                    ),
                ),
                Prop::new("Allow control panel functions", check("cp", s.panel)),
            ],
        },
        PropGroup {
            title: Some("Power".into()),
            rows: vec![Prop::new(
                "Shut down with managed system",
                select(
                    "sd",
                    &[&delayed, "Immediate"],
                    if s.delayed { "" } else { "Immediate" },
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
            apply_ok_cancel()
        ),
    )
}

fn other(ctx: &Ctx, o: &Other) -> String {
    let groups = [PropGroup {
        title: Some("Definition".into()),
        rows: vec![
            Prop::new("Serial number", mono(&o.serial)),
            Prop::new("Created", esc(&o.created)),
            Prop::new("Default profile", mono(&o.default_profile)),
            Prop::new("Start with the host", esc(&o.start)),
        ],
    }];
    props_table(ctx.product, &groups, true)
}

/// The page.
pub fn page(ctx: &Ctx, v: &Props) -> Page {
    let (tab, body) = match &v.tab {
        Tab::General(g) => ("general", general(ctx, v, g)),
        Tab::Processors(p) => ("processors", processors(ctx, p)),
        Tab::Memory(m) => ("memory", memory(ctx, &v.host, m)),
        Tab::Io(rows) => ("io", io(ctx, rows)),
        Tab::Adapters(rows) => ("vadapters", adapters(ctx, v, rows)),
        Tab::Settings(s) => ("settings", settings(ctx, s)),
        Tab::Other(o) => ("other", other(ctx, o)),
    };
    super::page(ctx, &v.host, v.id, &v.name, tab, v.messages.clone(), &body)
}

/// What a partition's pages read when they open.
pub struct Loaded {
    /// Its definition.
    pub def: Definition,
    /// Its status.
    pub status: Status,
    /// Its default profile.
    pub profile: Profile,
    /// The profile's revision.
    pub revision: u64,
    /// Its settings, from a host that has them.
    pub settings: Option<Settings>,
}

/// Read partition `id` of `k`: its definition, its default profile with
/// its revision, and its settings.
pub fn load(rq: &Rq<'_>, k: &Kept, id: u32) -> Result<Loaded, Vec<Message>> {
    let ask = |what: &str, req: AnyRequest| crate::systems::ask(rq, &k.name, what, &req);
    let AnyReply::Partitions(partitions::Reply::Get(p)) = ask(
        "Read partition",
        AnyRequest::Partitions(partitions::Request::Get(partitions::Get { id })),
    )?
    else {
        return Err(Vec::new());
    };
    let AnyReply::Profiles(profiles::Reply::Get(rev)) = ask(
        "Read profile",
        AnyRequest::Profiles(profiles::Request::Get(profiles::Get {
            id,
            name: p.definition.default_profile.clone(),
        })),
    )?
    else {
        return Err(Vec::new());
    };
    let settings = if k.offers(caps::PARTITION | caps::PARTITION_V3) {
        match ask(
            "Read settings",
            AnyRequest::Partition(partition::Request::GetSettings(partition::GetSettings {
                id,
            })),
        )? {
            AnyReply::Partition(partition::Reply::GetSettings(s)) => Some(s),
            _ => None,
        }
    } else {
        None
    };
    Ok(Loaded {
        def: p.definition,
        status: p.status,
        profile: rev.profile,
        revision: rev.revision,
        settings,
    })
}

/// The session's key for the revision a form was read at.
fn rev_key(host: &str, id: u32, profile: &str) -> String {
    format!("rev:{host}:{id}:{profile}")
}

/// Keep the revision `l` was read at, for the form's put.
pub fn keep_revision(rq: &Rq<'_>, host: &str, l: &Loaded) {
    let key = rev_key(host, l.def.id, &l.profile.name);
    let rev = l.revision.to_string();
    rq.app.sessions.update(&rq.session.id, |s| {
        s.data.insert(key, rev);
    });
}

/// The revision the form was read at; the one read now when there is
/// none.
pub fn form_revision(rq: &Rq<'_>, host: &str, l: &Loaded) -> u64 {
    rq.session
        .data
        .get(&rev_key(host, l.def.id, &l.profile.name))
        .and_then(|r| r.parse().ok())
        .unwrap_or(l.revision)
}

/// Put `profile` of partition `id` at `revision`: `Ok` with the new
/// revision, or the messages; a stale one's first message says so.
pub fn put(
    rq: &Rq<'_>,
    k: &Kept,
    id: u32,
    profile: Profile,
    revision: u64,
) -> Result<u64, Vec<Message>> {
    let name = profile.name.clone();
    let req = AnyRequest::Profiles(profiles::Request::Put(profiles::Put {
        id,
        profile,
        revision: Some(revision),
    }));
    let what = format!("change profile {name} of {} partition {id}", k.name);
    match crate::systems::ask(rq, &k.name, "Change profile", &req) {
        Ok(AnyReply::Profiles(profiles::Reply::Put(s))) => {
            rq.audit(&what, &format!("changed, revision {}", s.revision));
            Ok(s.revision)
        }
        Ok(_) => Err(Vec::new()),
        Err(mut ms) => {
            let stale = ms
                .iter()
                .any(|m| m.id == qsi::host::profiles::msgid::CHANGED);
            rq.audit(&what, if stale { "refused: stale" } else { "refused" });
            if stale {
                ms.insert(
                    0,
                    qmcmsg::message(Kind::Error, &qmcmsg::PROFILE_STALE, &[&name]),
                );
            }
            Err(ms)
        }
    }
}

/// The definition changed by `edit`, sent: allowed only while the
/// partition is not activated, which the host says.
pub fn change_definition(
    rq: &Rq<'_>,
    k: &Kept,
    def: &Definition,
    edit: impl FnOnce(&mut Definition),
) -> Result<(), Vec<Message>> {
    let mut d = def.clone();
    edit(&mut d);
    if &d == def {
        return Ok(());
    }
    let req = AnyRequest::Partition(partition::Request::Change(partition::Change {
        definition: d,
    }));
    let r = crate::systems::ask(rq, &k.name, "Change partition", &req);
    rq.audit(
        &format!("change partition {} of {}", def.name, k.name),
        if r.is_ok() { "changed" } else { "refused" },
    );
    r.map(|_| ())
}

/// The settings changed by `edit`, sent: at any time, from a host that
/// has them.
pub fn change_settings(
    rq: &Rq<'_>,
    k: &Kept,
    l: &Loaded,
    edit: impl FnOnce(&mut Settings),
) -> Result<(), Vec<Message>> {
    let Some(before) = &l.settings else {
        let what = mgmtproto::ask::lacks(k.caps, caps::PARTITION_V3).unwrap_or("its settings");
        return Err(vec![qmcmsg::message(
            Kind::Error,
            &qmcmsg::LACKS,
            &[&k.name, what],
        )]);
    };
    let mut s = before.clone();
    edit(&mut s);
    if &s == before {
        return Ok(());
    }
    let req = AnyRequest::Partition(partition::Request::SetSettings(partition::SetSettings {
        id: l.def.id,
        settings: s,
    }));
    let r = crate::systems::ask(rq, &k.name, "Change settings", &req);
    rq.audit(
        &format!("change settings of {} of {}", l.def.name, k.name),
        if r.is_ok() { "changed" } else { "refused" },
    );
    r.map(|_| ())
}

/// A boot mode's word.
pub fn boot_word(keylock: Keylock, ipl: IplType) -> &'static str {
    if keylock == Keylock::Manual && ipl == IplType::Attended {
        "Service - manual IPL"
    } else {
        "Normal"
    }
}

/// An IPL source's word on the Settings tab.
pub fn source_word(s: IplSource) -> &'static str {
    match s {
        IplSource::B => "B",
        IplSource::D => "D (optical/install)",
        _ => "A",
    }
}

fn keylock_word(k: Keylock) -> &'static str {
    if k == Keylock::Manual {
        "Manual"
    } else {
        "Normal"
    }
}

/// The tab of `l`.
#[allow(clippy::too_many_lines, reason = "one arm a tab")]
pub fn tab_of(rq: &Rq<'_>, k: &Kept, l: &Loaded, tab: &str) -> Tab {
    let p = &l.profile;
    let current = l.status.current.as_ref();
    match tab {
        "processors" => {
            let pools: Vec<String> = k
                .host
                .as_ref()
                .map(|h| {
                    h.pools
                        .iter()
                        .enumerate()
                        .map(|(i, x)| format!("{} ({i})", x.name))
                        .collect()
                })
                .unwrap_or_default();
            let pool = pools
                .iter()
                .find(|x| x.split(' ').next() == p.processors.pool.as_deref())
                .cloned()
                .unwrap_or_default();
            let opt = |u: Option<u32>| u.map(|u| units(u64::from(u))).unwrap_or_default();
            let shared = p.processors.mode == ProcessorMode::Shared;
            Tab::Processors(Processors {
                shared,
                pools,
                pool,
                units: [
                    opt(p.processors.minimum_units),
                    opt(p.processors.desired_units),
                    opt(p.processors.maximum_units),
                    current.map_or_else(
                        || "0.00".into(),
                        |c| {
                            if c.mode == ProcessorMode::Dedicated {
                                units(u64::try_from(c.cores.len()).unwrap_or(0) * 100)
                            } else {
                                units(u64::from(c.units))
                            }
                        },
                    ),
                ],
                virtuals: [
                    p.processors.minimum.to_string(),
                    p.processors.desired.to_string(),
                    p.processors.maximum.to_string(),
                    current.map_or_else(
                        || "0".into(),
                        |c| {
                            if c.mode == ProcessorMode::Dedicated {
                                c.cores.len().to_string()
                            } else {
                                c.virtuals.to_string()
                            }
                        },
                    ),
                ],
                uncapped: p.processors.sharing != Some(Sharing::Capped),
                weight: p.processors.weight.unwrap_or(128).to_string(),
            })
        }
        "memory" => {
            let n = |v: u64| crate::fmt::number(v, &rq.app.settings().number_format);
            let host = k.host.as_ref();
            Tab::Memory(Memory {
                mb: [
                    p.memory.minimum_mib.to_string(),
                    p.memory.desired_mib.to_string(),
                    p.memory.maximum_mib.to_string(),
                ],
                current: n(current.map_or(0, |c| c.memory_mib)),
                region: format!("{} MB", host.map_or(0, |h| h.memory_block_mib)),
                available: format!("{} MB", n(host.map_or(0, |h| h.memory.available))),
            })
        }
        "io" => {
            let held = l.status.state == PartitionState::Running;
            Tab::Io(
                p.physical_io
                    .iter()
                    .map(|x| {
                        let d = k
                            .resources
                            .devices
                            .iter()
                            .find(|d| d.location == x.location);
                        IoRow {
                            location: x.location.clone(),
                            slot: d.map(|d| d.address.clone()).unwrap_or_default(),
                            description: d.map(|d| d.description.clone()).unwrap_or_default(),
                            assigned: held
                                && d.is_some_and(|d| {
                                    d.holder.as_ref().is_some_and(|h| h.partition == l.def.id)
                                }),
                            required: x.required,
                        }
                    })
                    .collect(),
            )
        }
        "vadapters" => Tab::Adapters(adapter_rows(rq, k, p)),
        "settings" => {
            let s = l.settings.clone();
            Tab::Settings(Setting {
                auto_start: l.def.auto_start.enabled,
                boot: boot_word(l.def.keylock, p.boot.ipl_type).into(),
                source: source_word(p.boot.source).into(),
                keylock: keylock_word(l.def.keylock).into(),
                terminal: true,
                panel: s.as_ref().is_none_or(|s| s.panel),
                delayed: s
                    .as_ref()
                    .is_none_or(|s| s.with_host != WithHost::Immediate),
                delay: s.as_ref().map_or(300, |s| s.delay),
            })
        }
        "other" => Tab::Other(Other {
            serial: l.def.serial.clone(),
            created: crate::systems::when(rq, l.def.created),
            default_profile: l.def.default_profile.clone(),
            start: if l.def.auto_start.enabled {
                format!(
                    "order {}, after {} s",
                    l.def.auto_start.order, l.def.auto_start.delay_secs
                )
            } else {
                "not started with the host".into()
            },
        }),
        _ => {
            let since = k
                .activations
                .iter()
                .find(|a| a.id == l.def.id)
                .and_then(|a| a.since)
                .map(|s| crate::systems::when(rq, s))
                .unwrap_or_default();
            Tab::General(General {
                name: l.def.name.clone(),
                id: l.def.id,
                environment: "QS/400".into(),
                host: k.name.clone(),
                description: l
                    .settings
                    .as_ref()
                    .map(|s| s.description.clone())
                    .unwrap_or_default(),
                state: state_word(l.status.state).into(),
                code: if l.status.code.is_empty() {
                    mgmtproto::kept::NO_CODE.into()
                } else {
                    l.status.code.clone()
                },
                configuration: "Configured".into(),
                os: l.status.channel.release.clone(),
                fix: l.status.channel.fix_level.clone(),
                profile: if l.status.profile.is_empty() {
                    l.def.default_profile.clone()
                } else {
                    l.status.profile.clone()
                },
                since,
                boot: boot_word(l.def.keylock, p.boot.ipl_type).into(),
                keylock: keylock_word(l.def.keylock).into(),
            })
        }
    }
}

/// A profile's virtual adapters as the page lists them: the console's
/// serial adapters first, fixed.
pub fn adapter_rows(rq: &Rq<'_>, k: &Kept, p: &Profile) -> Vec<AdapterRow> {
    let mut rows = vec![
        AdapterRow {
            slot: 0,
            kind: "Serial (console)".into(),
            connecting: "QS/VS host / 0".into(),
            backing: "Terminal window".into(),
            required: true,
            fixed: true,
        },
        AdapterRow {
            slot: 1,
            kind: "Serial".into(),
            connecting: "QS/VS host / 1".into(),
            backing: "(reserved)".into(),
            required: true,
            fixed: true,
        },
    ];
    for e in &p.ethernet {
        rows.push(AdapterRow {
            slot: e.slot,
            kind: "Ethernet".into(),
            connecting: format!("{} / VLAN {}", e.switch.clone().unwrap_or_default(), e.vlan),
            backing: e.mac.clone().filter(|m| !m.is_empty()).map_or_else(
                || "MAC (given at activation)".into(),
                |m| format!("MAC {m}"),
            ),
            required: e.required,
            fixed: false,
        });
    }
    for a in &p.scsi {
        let disks: Vec<String> = a
            .luns
            .iter()
            .map(|l| {
                let (pool, name) = l.backing.split_once('/').unwrap_or(("", &l.backing));
                let size = k
                    .resources
                    .vdisks
                    .iter()
                    .find(|d| d.pool == pool && d.name == name)
                    .map(|d| format!("{} GB, ", crate::vres::gb(rq, d.bytes)))
                    .unwrap_or_default();
                format!("Virtual disk {name} ({size}{pool})")
            })
            .collect();
        rows.push(AdapterRow {
            slot: a.slot,
            kind: "SCSI client".into(),
            connecting: "QS/VS host".into(),
            backing: disks.join(", "),
            required: true,
            fixed: false,
        });
    }
    for (i, o) in p.optical.iter().enumerate() {
        rows.push(AdapterRow {
            slot: o.slot,
            kind: "SCSI client".into(),
            connecting: "QS/VS host".into(),
            backing: format!("Virtual optical OPT{:02}", i + 1),
            required: false,
            fixed: false,
        });
    }
    for (i, t) in p.tape.iter().enumerate() {
        rows.push(AdapterRow {
            slot: t.slot,
            kind: "SCSI client".into(),
            connecting: "QS/VS host".into(),
            backing: format!("Virtual tape VTAP{:02}", i + 1),
            required: false,
            fixed: false,
        });
    }
    match rq.q("sort").as_str() {
        "t" => rows.sort_by(|a, b| a.kind.cmp(&b.kind).then(a.slot.cmp(&b.slot))),
        _ => rows.sort_by_key(|r| r.slot),
    }
    rows
}

fn num(f: &Form, name: &str) -> Option<u64> {
    f.str(name).trim().replace(',', "").parse().ok()
}

fn invalid(what: &str) -> Vec<Message> {
    vec![qmcmsg::message(Kind::Error, &qmcmsg::NOT_VALID, &[what])]
}

/// What a post of `tab` changes, with the messages of what was done.
#[allow(clippy::too_many_lines, reason = "one arm a tab")]
fn post(rq: &Rq<'_>, k: &Kept, l: &Loaded, tab: &str) -> Result<Vec<Message>, Vec<Message>> {
    let f = rq.req.form();
    let running = l.status.state != PartitionState::NotActivated;
    let rev = form_revision(rq, &k.name, l);
    let mut done = Vec::new();
    let later = |done: &mut Vec<Message>| {
        if running {
            done.push(qmcmsg::message(
                Kind::Info,
                &qmcmsg::AT_NEXT_ACTIVATION,
                &[&l.def.name],
            ));
        }
    };
    match tab {
        "general" => {
            let name = f.str("n").trim().to_uppercase();
            if name.is_empty() {
                return Err(invalid("Type the partition's name."));
            }
            change_definition(rq, k, &l.def, |d| d.name.clone_from(&name))?;
            change_settings(rq, k, l, |s| s.description = f.str("d").trim().to_string()).or_else(
                |e| {
                    if f.str("d").trim().is_empty() {
                        Ok(())
                    } else {
                        Err(e)
                    }
                },
            )?;
        }
        "processors" => {
            let mut p = l.profile.clone();
            let shared = f.str("m") != "ded";
            let u = |n: &str| crate::vres::parse_units(f.str(n));
            let v = |n: &str| f.str(n).trim().parse::<u32>().ok();
            let (Some(vmin), Some(vdes), Some(vmax)) = (v("vmin"), v("vdes"), v("vmax")) else {
                return Err(invalid("Virtual processors are whole numbers."));
            };
            p.processors.mode = if shared {
                ProcessorMode::Shared
            } else {
                ProcessorMode::Dedicated
            };
            (
                p.processors.minimum,
                p.processors.desired,
                p.processors.maximum,
            ) = (vmin, vdes, vmax);
            if shared {
                let (Some(a), Some(b), Some(c)) = (u("min"), u("des"), u("max")) else {
                    return Err(invalid(
                        "Processing units are hundredths of a processor: 1.00.",
                    ));
                };
                p.processors.minimum_units = Some(a);
                p.processors.desired_units = Some(b);
                p.processors.maximum_units = Some(c);
                p.processors.pool = f.str("pool").split(' ').next().map(str::to_string);
                p.processors.sharing = Some(if f.str("sm") == "Capped" {
                    Sharing::Capped
                } else {
                    Sharing::Uncapped
                });
                p.processors.weight = f.str("w").trim().parse().ok();
            }
            let units_now = u("des");
            put(rq, k, l.def.id, p, rev)?;
            later(&mut done);
            // A running shared partition's entitlement, changed now.
            if running
                && shared
                && l.status
                    .current
                    .as_ref()
                    .is_some_and(|c| c.mode == ProcessorMode::Shared)
            {
                done.extend(entitle(rq, k, l, units_now, &f));
            }
        }
        "memory" => {
            let mut p = l.profile.clone();
            let (Some(a), Some(b), Some(c)) = (num(&f, "min"), num(&f, "des"), num(&f, "max"))
            else {
                return Err(invalid("Memory is whole megabytes."));
            };
            (
                p.memory.minimum_mib,
                p.memory.desired_mib,
                p.memory.maximum_mib,
            ) = (a, b, c);
            put(rq, k, l.def.id, p, rev)?;
            later(&mut done);
        }
        "io" => {
            let mut p = l.profile.clone();
            let gone: Vec<String> = p
                .physical_io
                .iter()
                .enumerate()
                .filter(|(i, _)| f.has(&format!("a{i}")))
                .map(|(_, x)| x.location.clone())
                .collect();
            if gone.is_empty() {
                return Err(invalid("Select the adapters to remove."));
            }
            p.physical_io.retain(|x| !gone.contains(&x.location));
            put(rq, k, l.def.id, p, rev)?;
            later(&mut done);
        }
        "settings" => settings_post(rq, k, l, &f, rev, &mut done)?,
        _ => {}
    }
    Ok(done)
}

/// A running shared partition's units, capping and weight, changed now.
fn entitle(rq: &Rq<'_>, k: &Kept, l: &Loaded, units: Option<u32>, f: &Form) -> Vec<Message> {
    if let Some(m) = crate::vres::lacking(k, caps::PARTITION | caps::PARTITION_V2) {
        return vec![m];
    }
    let req = AnyRequest::Partition(partition::Request::Entitlement(partition::Entitlement {
        id: l.def.id,
        units,
        sharing: Some(if f.str("sm") == "Capped" {
            Sharing::Capped
        } else {
            Sharing::Uncapped
        }),
        weight: f.str("w").trim().parse().ok(),
    }));
    let what = format!("change the entitlement of {} on {}", l.def.name, k.name);
    match crate::systems::ask(rq, &k.name, "Change entitlement", &req) {
        Ok(_) => {
            rq.audit(&what, "changed");
            vec![qmcmsg::message(
                Kind::Done,
                &qmcmsg::ENTITLEMENT_CHANGED,
                &[&l.def.name],
            )]
        }
        Err(ms) => {
            rq.audit(&what, "refused");
            ms
        }
    }
}

fn settings_post(
    rq: &Rq<'_>,
    k: &Kept,
    l: &Loaded,
    f: &Form,
    rev: u64,
    done: &mut Vec<Message>,
) -> Result<(), Vec<Message>> {
    if f.str("bm") == "Open firmware" {
        return Err(vec![qmcmsg::message(
            Kind::Error,
            &qmcmsg::NO_OPEN_FIRMWARE,
            &[&k.name],
        )]);
    }
    if f.str("con").starts_with("Physical") {
        return Err(vec![qmcmsg::message(
            Kind::Error,
            &qmcmsg::CONSOLE_IS_TERMINAL,
            &[],
        )]);
    }
    let service = f.str("bm") == "Service - manual IPL";
    // Boot mode is the keylock and the IPL type together.
    let keylock = if service || f.str("key") == "Manual" {
        Keylock::Manual
    } else {
        Keylock::Normal
    };
    let auto = f.has("as");
    change_definition(rq, k, &l.def, |d| {
        d.keylock = keylock;
        d.auto_start.enabled = auto;
    })?;
    let mut p = l.profile.clone();
    p.boot.ipl_type = if service {
        IplType::Attended
    } else {
        IplType::Unattended
    };
    p.boot.source = match f.str("src") {
        "B" => IplSource::B,
        s if s.starts_with('D') => IplSource::D,
        _ => IplSource::A,
    };
    p.boot.console = ConsoleMode::Partition;
    if p != l.profile {
        put(rq, k, l.def.id, p, rev)?;
        if l.status.state != PartitionState::NotActivated {
            done.push(qmcmsg::message(
                Kind::Info,
                &qmcmsg::AT_NEXT_ACTIVATION,
                &[&l.def.name],
            ));
        }
    }
    let delayed = !f.str("sd").starts_with("Immediate");
    let panel = f.has("cp");
    if l.settings.is_some() {
        change_settings(rq, k, l, |s| {
            s.panel = panel;
            s.with_host = if delayed {
                WithHost::Delayed
            } else {
                WithHost::Immediate
            };
        })?;
    }
    Ok(())
}

/// The tab a post's entries are shown again in: what was typed kept.
fn kept_entries(tab: Tab, f: &Form) -> Tab {
    let s = |n: &str| f.str(n).to_string();
    match tab {
        Tab::General(mut g) => {
            g.name = s("n");
            g.description = s("d");
            Tab::General(g)
        }
        Tab::Processors(mut p) => {
            p.shared = f.str("m") != "ded";
            p.pool = s("pool");
            p.units[0] = s("min");
            p.units[1] = s("des");
            p.units[2] = s("max");
            p.virtuals[0] = s("vmin");
            p.virtuals[1] = s("vdes");
            p.virtuals[2] = s("vmax");
            p.uncapped = f.str("sm") != "Capped";
            p.weight = s("w");
            Tab::Processors(p)
        }
        Tab::Memory(mut m) => {
            m.mb = [s("min"), s("des"), s("max")];
            Tab::Memory(m)
        }
        Tab::Settings(mut x) => {
            x.auto_start = f.has("as");
            x.boot = s("bm");
            x.source = s("src");
            x.keylock = s("key");
            x.panel = f.has("cp");
            x.delayed = !f.str("sd").starts_with("Immediate");
            Tab::Settings(x)
        }
        other => other,
    }
}

/// `GET`, `POST /partition/<host>/<id>/<tab>`.
pub fn route(rq: &Rq<'_>, k: &Kept, id: u32, tab: &str) -> Response {
    let tab = if tab == "general" && rq.q("tab") == "other" {
        "other"
    } else {
        tab
    };
    let f = rq.req.form();
    let back = "/partitions";
    if rq.post() && f.has("cancel") {
        return Response::redirect(&rq.ctx.url(back));
    }
    if rq.post() && tab == "io" && f.has("add") {
        return Response::redirect(&rq.ctx.url(&format!("/partition/{}/{id}/edit?io=1", k.name)));
    }
    if rq.post() && tab == "vadapters" {
        let kind = match f.str("t") {
            "SCSI client" => "scsi",
            "Serial" => "serial",
            _ => "ethernet",
        };
        return Response::redirect(
            &rq.ctx
                .url(&format!("/partition/{}/{id}/edit?new={kind}", k.name)),
        );
    }
    let l = match load(rq, k, id) {
        Ok(l) => l,
        Err(ms) => return rq.done_all(back, ms),
    };
    let mut messages = Vec::new();
    if rq.post() {
        if let Some(no) = lacks(rq, Task::CreatePartitions) {
            return no;
        }
        match post(rq, k, &l, tab) {
            Ok(done) => {
                let saved = qmcmsg::message(Kind::Done, &qmcmsg::PARTITION_CHANGED, &[&l.def.name]);
                let mut all = vec![saved];
                all.extend(done);
                if f.has("ok") {
                    return rq.done_all(back, all);
                }
                return rq.done_all(&format!("/partition/{}/{id}/{tab}", k.name), all);
            }
            Err(ms) => messages = ms,
        }
    }
    // Read again after a refusal: a stale put shows the new revision
    // with the operator's entries.
    let l = if rq.post() {
        load(rq, k, id).unwrap_or(l)
    } else {
        l
    };
    keep_revision(rq, &k.name, &l);
    let mut view = tab_of(rq, k, &l, tab);
    if rq.post() {
        view = kept_entries(view, &f);
    }
    rq.page(page(
        &rq.ctx,
        &Props {
            host: k.name.clone(),
            id,
            name: l.def.name.clone(),
            tab: view,
            messages,
        },
    ))
}

/// The physical adapter `location` of `k`, as a profile names it.
pub fn physical(location: &str, required: bool) -> PhysicalIo {
    PhysicalIo {
        location: location.into(),
        required,
    }
}
