// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! Create partition (`create-1.html` to `create-9.html`): nine steps,
//! their entries kept in the session; the ID chosen in step 1 and one in
//! use refused there; physical adapters from what the host released;
//! the virtual adapters; optical and tape; boot settings; the summary as
//! the whole profile. Finish makes the partition at its ID and its first
//! profile, Not activated. Nothing is made before Finish, so Cancel at
//! any step leaves nothing but the session's entries, which it clears.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use qsweb::frame::{Ctx, Page};
use qsweb::html::esc;
use qsweb::http::{Form, Response};
use qsweb::msg::{Kind, Message};
use qsweb::partials::{Cell, Col, List, list_table, mono};

use mgmtproto::Kept;
use qsi::any::{AnyReply, AnyRequest};
use qsi::base::Keylock;
use qsi::caps;
use qsi::host::model::{
    AutoStart, Boot, ConsoleMode, EthernetAdapter, IplSource, IplType, Lun, MediaDevice, Memory,
    PhysicalIo, ProcessorMode, Processors, Profile, ScsiAdapter, Sharing,
};
use qsi::host::partition::{self, Settings, WithHost};
use qsi::host::profiles;
use webusers::Task;

use crate::app::routes::{Rq, lacks};

/// The steps' names.
pub const STEPS: [&str; 9] = [
    "Partition ID and name",
    "Profile name",
    "Processors",
    "Memory",
    "I/O",
    "Virtual adapters",
    "Optical and tape",
    "Boot settings",
    "Summary",
];

/// The fields each step posts, kept in the session.
const FIELDS: [&[&str]; 9] = [
    &["id", "n", "d"],
    &["p", "dft", "copy"],
    &[
        "m", "pool", "min", "des", "max", "vmin", "vdes", "vmax", "sm", "w",
    ],
    &["min", "des", "max"],
    &[],
    &[],
    &["opt", "iso", "tap", "fc"],
    &["src", "key", "bm", "as"],
    &[],
];

/// A step's checkboxes: kept as `on` or empty.
const CHECKS: [&str; 6] = ["dft", "opt", "tap", "fc", "as", "req"];

/// A virtual adapter of the wizard.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Adapter {
    /// Its slot.
    pub slot: u32,
    /// `ethernet`, `scsi`, `serial`.
    pub kind: String,
    /// The switch.
    pub switch: String,
    /// The VLAN.
    pub vlan: u32,
    /// The disk: `POOL1/VD06`.
    pub disk: String,
    /// Required.
    pub required: bool,
}

impl Adapter {
    fn line(&self) -> String {
        format!(
            "{}|{}|{}|{}|{}|{}",
            self.slot,
            self.kind,
            self.switch,
            self.vlan,
            self.disk,
            u8::from(self.required)
        )
    }

    fn parse(l: &str) -> Option<Self> {
        let w: Vec<&str> = l.split('|').collect();
        Some(Self {
            slot: w.first()?.parse().ok()?,
            kind: (*w.get(1)?).into(),
            switch: (*w.get(2)?).into(),
            vlan: w.get(3)?.parse().unwrap_or(0),
            disk: (*w.get(4)?).into(),
            required: *w.get(5)? == "1",
        })
    }
}

/// A row of step 5: a device the host released.
#[derive(Clone, Debug, Default)]
pub struct Device {
    /// Its location code.
    pub location: String,
    /// Its slot.
    pub slot: String,
    /// What it is.
    pub description: String,
    /// Who holds it now.
    pub owner: String,
    /// Its box's name.
    pub name: String,
    /// Chosen.
    pub chosen: bool,
}

/// What a step shows.
#[derive(Clone, Debug, Default)]
pub struct Wizard {
    /// The host.
    pub host: String,
    /// The step, 1 to 9.
    pub step: usize,
    /// The entries.
    pub v: BTreeMap<String, String>,
    /// The next free ID.
    pub next_id: u32,
    /// Step 2's Copy from: `QSTEST01 / install`.
    pub copies: Vec<String>,
    /// Step 3's pools: `DefaultPool (0) - 3.50 units available`.
    pub pools: Vec<String>,
    /// Step 4: `40,960 MB (region size 256 MB)`.
    pub available: String,
    /// Step 5's devices.
    pub devices: Vec<Device>,
    /// Step 6's adapters, with their words: (adapter, type, connects to).
    pub adapters: Vec<(Adapter, String, String)>,
    /// Step 7's images.
    pub images: Vec<String>,
    /// Step 7's words: the optical drive, the tape drive, a pass-through
    /// drive; empty for none.
    pub media: [String; 3],
    /// Step 9: (setting, value).
    pub summary: Vec<(String, String)>,
    /// The messages.
    pub messages: Vec<Message>,
}

impl Wizard {
    fn get(&self, k: &str) -> &str {
        self.v.get(k).map_or("", String::as_str)
    }
}

fn text(name: &str, value: &str, size: u32) -> String {
    format!(
        "<input type=\"text\" name=\"{name}\" value=\"{}\" size=\"{size}\">",
        esc(value)
    )
}

fn check(name: &str, on: bool) -> String {
    format!(
        "<input type=\"checkbox\" name=\"{name}\"{}>",
        if on { " checked" } else { "" }
    )
}

fn radio(name: &str, value: &str, on: bool) -> String {
    format!(
        "<input type=\"radio\" name=\"{name}\" value=\"{value}\"{}>",
        if on { " checked" } else { "" }
    )
}

/// A select of `options`, the chosen first when it is one of them.
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

fn props(rows: &[(&str, String)]) -> String {
    let mut s = String::from("<table class=\"props\">");
    for (l, v) in rows {
        let _ = write!(s, "<tr><th>{l}</th><td>{v}</td></tr>");
    }
    s.push_str("</table>");
    s
}

#[allow(clippy::too_many_lines, reason = "one arm a step")]
fn step_body(ctx: &Ctx, w: &Wizard) -> String {
    let g = |k: &str| w.get(k);
    let on = |k: &str| !w.get(k).is_empty();
    let req = " <span class=\"req\">*</span>";
    match w.step {
        1 => format!(
            "<h2>Partition ID and name</h2>{}",
            props(&[
                ("Managed system", mono(&w.host)),
                (
                    &format!("Partition ID{req}"),
                    format!(
                        "{} <span class=\"small grey\">next free: {}</span>",
                        text("id", g("id"), 4),
                        w.next_id
                    ),
                ),
                (&format!("Partition name{req}"), text("n", g("n"), 10)),
                ("Environment", mono("QS/400")),
                ("Description", text("d", g("d"), 40)),
            ])
        ),
        2 => {
            let mut copies = vec!["(none)".to_string()];
            copies.extend(w.copies.iter().cloned());
            format!(
                "<h2>Profile name</h2><p class=\"small grey\">A profile holds the resource configuration. A partition may have several; one is activated.</p>{}",
                props(&[
                    (&format!("Profile name{req}"), text("p", g("p"), 20)),
                    ("Make default", check("dft", on("dft"))),
                    ("Copy from", select("copy", &copies, g("copy"))),
                ])
            )
        }
        3 => format!(
            "<h2>Processors</h2>{}",
            props(&[
                (
                    "Mode",
                    format!(
                        "{} Shared {} Dedicated",
                        radio("m", "shared", g("m") != "ded"),
                        radio("m", "ded", g("m") == "ded")
                    ),
                ),
                ("Shared processor pool", select("pool", &w.pools, g("pool"))),
                (
                    "Processing units",
                    format!(
                        "Min {} Desired {} Max {}",
                        text("min", g("min"), 5),
                        text("des", g("des"), 5),
                        text("max", g("max"), 5)
                    ),
                ),
                (
                    "Virtual processors",
                    format!(
                        "Min {} Desired {} Max {}",
                        text("vmin", g("vmin"), 3),
                        text("vdes", g("vdes"), 3),
                        text("vmax", g("vmax"), 3)
                    ),
                ),
                (
                    "Sharing mode",
                    format!(
                        "{} weight {}",
                        select("sm", &["Uncapped".into(), "Capped".into()], g("sm")),
                        text("w", g("w"), 4)
                    ),
                ),
            ])
        ),
        4 => format!(
            "<h2>Memory</h2>{}",
            props(&[
                (
                    "Memory (MB)",
                    format!(
                        "Min {} Desired {} Max {}",
                        text("min", g("min"), 7),
                        text("des", g("des"), 7),
                        text("max", g("max"), 7)
                    ),
                ),
                (&format!("Available on {}", w.host), esc(&w.available)),
            ])
        ),
        5 => {
            let list = List {
                cols: [
                    "Select",
                    "Location code",
                    "Slot",
                    "Description",
                    "Current owner",
                ]
                .iter()
                .map(|c| Col::plain(c))
                .collect(),
                rows: w
                    .devices
                    .iter()
                    .map(|d| {
                        vec![
                            Cell::html(check(&d.name, d.chosen)),
                            Cell::mono(&d.location),
                            Cell::mono(&d.slot),
                            Cell::text(&d.description),
                            Cell::text(&d.owner),
                        ]
                    })
                    .collect(),
                empty: "The host has released no device for partitions. Release one at the QS/VS host: Service Tools option 10.".into(),
                keep: Vec::new(),
            };
            format!(
                "<h2>I/O</h2><p class=\"small grey\">Physical adapters to assign. Usually none; virtual resources are configured in the next steps.</p>{}",
                list_table(&list)
            )
        }
        6 => {
            let rows = w
                .adapters
                .iter()
                .map(|(a, kind, to)| {
                    let edit = ctx.url(&format!("/partitions/create/step/6?edit={}", a.slot));
                    let remove = ctx.url(&format!("/partitions/create/step/6?remove={}", a.slot));
                    vec![
                        Cell::act(format!(
                            "<a href=\"{}\">Edit</a> | <a href=\"{}\">Remove</a>",
                            esc(&edit),
                            esc(&remove)
                        )),
                        Cell::num(&a.slot.to_string()),
                        Cell::text(kind),
                        Cell::mono(to),
                        Cell::text(if a.required { "Yes" } else { "No" }),
                    ]
                })
                .collect();
            let list = List {
                cols: ["Actions", "Adapter ID", "Type", "Connects to", "Required"]
                    .iter()
                    .map(|c| Col::plain(c))
                    .collect(),
                rows,
                empty: String::new(),
                keep: Vec::new(),
            };
            format!(
                "<h2>Virtual adapters</h2>{}<div class=\"buttons\">Add: <select name=\"t\"><option>Ethernet</option><option>SCSI client (virtual disk)</option><option>Serial</option></select> <input type=\"submit\" name=\"add\" value=\"Add...\"></div>",
                list_table(&list)
            )
        }
        7 => {
            let mut rows: Vec<(&str, String)> = Vec::new();
            if !w.media[0].is_empty() {
                let mut images = w.images.clone();
                images.push("(none)".into());
                rows.push((
                    "Virtual optical drive",
                    format!(
                        "{} {} &nbsp; load image now: {}",
                        check("opt", on("opt")),
                        esc(&w.media[0]),
                        select("iso", &images, g("iso"))
                    ),
                ));
            }
            if !w.media[1].is_empty() {
                rows.push((
                    "Virtual tape drive",
                    format!("{} {}", check("tap", on("tap")), esc(&w.media[1])),
                ));
            }
            if !w.media[2].is_empty() {
                rows.push((
                    "Pass-through FC tape",
                    format!("{} {}", check("fc", on("fc")), esc(&w.media[2])),
                ));
            }
            format!("<h2>Optical and tape</h2>{}", props(&rows))
        }
        8 => format!(
            "<h2>Boot settings</h2>{}",
            props(&[
                (
                    "IPL source",
                    select(
                        "src",
                        &[
                            "D (optical/install) - first install from mounted ISO".into(),
                            "A".into(),
                            "B".into(),
                        ],
                        g("src"),
                    ),
                ),
                (
                    "Keylock position",
                    select("key", &["Manual".into(), "Normal".into()], g("key")),
                ),
                (
                    "Boot mode",
                    select(
                        "bm",
                        &["Normal".into(), "Service - manual IPL".into()],
                        g("bm"),
                    ),
                ),
                ("Auto start with managed system", check("as", on("as"))),
                (
                    "Console",
                    "Terminal window (virtual serial adapter 0)".into()
                ),
            ])
        ),
        _ => {
            let mut t = String::from(
                "<table class=\"list\" style=\"width:auto\"><tr><th>Setting</th><th>Value</th></tr>",
            );
            for (i, (s, v)) in w.summary.iter().enumerate() {
                let alt = if i % 2 == 1 { " class=\"alt\"" } else { "" };
                let _ = write!(
                    t,
                    "<tr{alt}><td>{}</td><td class=\"mono\">{}</td></tr>",
                    esc(s),
                    esc(v)
                );
            }
            t.push_str("</table>");
            format!(
                "<h2>Summary</h2><p class=\"small grey\">The full profile. Press Finish to create the partition and its profile; the partition is created Not activated.</p>{t}"
            )
        }
    }
}

/// The page of a step.
pub fn page(ctx: &Ctx, w: &Wizard) -> Page {
    let mut steps = String::from("<td class=\"wsteps\">");
    for (i, s) in STEPS.iter().enumerate() {
        let cls = match (i + 1).cmp(&w.step) {
            std::cmp::Ordering::Less => "done",
            std::cmp::Ordering::Equal => "cur",
            std::cmp::Ordering::Greater => "",
        };
        let _ = write!(steps, "<div class=\"wstep {cls}\">{}. {s}</div>", i + 1);
    }
    steps.push_str("</td>");
    let back = if w.step == 1 { " disabled" } else { "" };
    let buttons = if w.step == 9 {
        "<div class=\"buttons\"><input type=\"submit\" name=\"back\" value=\"&lt; Back\"><input type=\"submit\" name=\"cancel\" value=\"Cancel\"><input type=\"submit\" name=\"finish\" value=\"Finish\"></div>".to_string()
    } else {
        format!(
            "<div class=\"buttons\"><input type=\"submit\" name=\"back\" value=\"&lt; Back\"{back}><input type=\"submit\" name=\"next\" value=\"Next &gt;\"><input type=\"submit\" name=\"cancel\" value=\"Cancel\"></div>"
        )
    };
    let body = format!(
        "<form method=\"post\">{}<table class=\"wizard\" cellspacing=\"0\"><tr>{steps}<td class=\"wbody\">{}{buttons}</td></tr></table></form>",
        ctx.csrf_field(),
        step_body(ctx, w),
    );
    Page {
        title: "Create partition".into(),
        crumb: vec![
            "Systems Management".into(),
            "Partitions".into(),
            "Create partition".into(),
        ],
        h1: "Create partition".into(),
        desc: Some(format!(
            "Step {} of 9 on managed system {}.",
            w.step, w.host
        )),
        messages: w.messages.clone(),
        body,
        help: "create-partition".into(),
        nav: Some(("Systems Management", Some("/partitions"))),
        ..Page::default()
    }
}

/// The session's entries of the wizard.
fn entries(rq: &Rq<'_>) -> BTreeMap<String, String> {
    rq.session
        .data
        .iter()
        .filter_map(|(k, v)| k.strip_prefix("wiz.").map(|k| (k.to_string(), v.clone())))
        .collect()
}

fn save(rq: &Rq<'_>, v: &BTreeMap<String, String>) {
    rq.app.sessions.update(&rq.session.id, |s| {
        s.data.retain(|k, _| !k.starts_with("wiz."));
        for (k, val) in v {
            s.data.insert(format!("wiz.{k}"), val.clone());
        }
    });
}

/// The key a step's field is kept under: steps 3 and 4 both have
/// `min`, so each is kept by its step.
fn key(step: usize, field: &str) -> String {
    match (step, field) {
        (3, "min" | "des" | "max") => format!("u{field}"),
        (4, "min" | "des" | "max") => format!("m{field}"),
        _ => field.to_string(),
    }
}

/// A step's fields as the page shows them, from the kept entries.
fn shown(step: usize, v: &BTreeMap<String, String>) -> BTreeMap<String, String> {
    let mut out = v.clone();
    for f in FIELDS[step - 1] {
        out.insert(
            (*f).to_string(),
            v.get(&key(step, f)).cloned().unwrap_or_default(),
        );
    }
    out
}

fn keep_posted(step: usize, f: &Form, v: &mut BTreeMap<String, String>) {
    for name in FIELDS[step - 1] {
        let val = if CHECKS.contains(name) {
            if f.has(name) {
                "on".into()
            } else {
                String::new()
            }
        } else {
            f.str(name).to_string()
        };
        v.insert(key(step, name), val);
    }
    if step == 5 {
        let chosen: Vec<&str> = f
            .pairs()
            .iter()
            .filter(|(n, _)| n.starts_with("io_"))
            .map(|(n, _)| n.trim_start_matches("io_"))
            .collect();
        v.insert("io".into(), chosen.join(","));
    }
}

fn adapters_of(v: &BTreeMap<String, String>) -> Vec<Adapter> {
    v.get("adapters")
        .map(|s| s.lines().filter_map(Adapter::parse).collect())
        .unwrap_or_default()
}

fn set_adapters(v: &mut BTreeMap<String, String>, list: &[Adapter]) {
    v.insert(
        "adapters".into(),
        list.iter()
            .map(Adapter::line)
            .collect::<Vec<_>>()
            .join("\n"),
    );
}

/// The first values of a new wizard: the design's.
fn fresh(k: &Kept) -> BTreeMap<String, String> {
    let next = next_id(k);
    let mut v = BTreeMap::new();
    for (key, val) in [
        ("id", next.to_string()),
        ("n", String::new()),
        ("p", "DEFAULT".into()),
        ("dft", "on".into()),
        ("m", "shared".into()),
        ("umin", "0.25".into()),
        ("udes", "0.50".into()),
        ("umax", "2.00".into()),
        ("vmin", "1".into()),
        ("vdes", "1".into()),
        ("vmax", "2".into()),
        ("w", "128".into()),
        ("mmin", "4096".into()),
        ("mdes", "8192".into()),
        ("mmax", "16384".into()),
        ("opt", "on".into()),
        ("key", "Manual".into()),
        (
            "adapters",
            Adapter {
                slot: 0,
                kind: "serial".into(),
                required: true,
                ..Adapter::default()
            }
            .line(),
        ),
    ] {
        v.insert(key.to_string(), val);
    }
    v
}

/// The lowest ID no partition of `k` has.
fn next_id(k: &Kept) -> u32 {
    (1..1000)
        .find(|n| !k.partitions.iter().any(|p| p.id == *n))
        .unwrap_or(1)
}

/// QS/VS's naming rule for partitions and profiles: 1 to 10 of `A` to
/// `Z`, `0` to `9` and `_`, a letter first.
fn host_name_ok(n: &str) -> bool {
    (1..=10).contains(&n.len())
        && n.starts_with(|c: char| c.is_ascii_uppercase())
        && n.bytes()
            .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'_')
}

fn invalid(what: &str) -> Vec<Message> {
    vec![qmcmsg::message(Kind::Error, &qmcmsg::NOT_VALID, &[what])]
}

/// The checks of step `step`'s entries.
fn validate(k: &Kept, step: usize, v: &BTreeMap<String, String>) -> Result<(), Vec<Message>> {
    let g = |key: &str| v.get(key).map_or("", String::as_str).trim();
    let units = |key: &str| crate::vres::parse_units(g(key));
    let whole = |key: &str| g(key).replace(',', "").parse::<u64>().ok();
    match step {
        1 => {
            let id: u32 = g("id")
                .parse()
                .ok()
                .filter(|i| (1..=999).contains(i))
                .ok_or_else(|| invalid("The partition ID is a number from 1 to 999."))?;
            if let Some(p) = k.partitions.iter().find(|p| p.id == id) {
                return Err(vec![qmcmsg::message(
                    Kind::Error,
                    &qmcmsg::ID_IN_USE,
                    &[&id.to_string(), &p.name],
                )]);
            }
            if g("n").is_empty() || g("n").len() > 32 {
                return Err(invalid("Type the partition's name, 32 characters at most."));
            }
            if k.partitions
                .iter()
                .any(|p| p.name.eq_ignore_ascii_case(g("n")))
            {
                return Err(invalid("Another partition of the host has that name."));
            }
        }
        2 if g("p").is_empty() => return Err(invalid("Type the profile's name.")),
        2 if !host_name_ok(&g("p").to_uppercase()) => {
            return Err(invalid(
                "A profile's name is 1 to 10 letters, digits and _, a letter first.",
            ));
        }
        6 if !adapters_of(v)
            .iter()
            .any(|a| a.kind == "scsi" && !a.disk.is_empty()) =>
        {
            return Err(invalid(
                "The partition needs a load source: add a SCSI client adapter with a virtual disk.",
            ));
        }
        3 => {
            let v3 = |key: &str| g(key).parse::<u32>().ok();
            let (Some(a), Some(b), Some(c)) = (v3("vmin"), v3("vdes"), v3("vmax")) else {
                return Err(invalid("Virtual processors are whole numbers."));
            };
            if !(a <= b && b <= c && a > 0) {
                return Err(invalid(
                    "Virtual processors: minimum, desired and maximum in order, at least 1.",
                ));
            }
            if g("m") != "ded" {
                let (Some(x), Some(y), Some(z)) = (units("umin"), units("udes"), units("umax"))
                else {
                    return Err(invalid(
                        "Processing units are hundredths of a processor: 0.50.",
                    ));
                };
                if !(x <= y && y <= z && x > 0) {
                    return Err(invalid(
                        "Processing units: minimum, desired and maximum in order.",
                    ));
                }
            }
        }
        4 => {
            let (Some(a), Some(b), Some(c)) = (whole("mmin"), whole("mdes"), whole("mmax")) else {
                return Err(invalid("Memory is whole megabytes."));
            };
            if !(a <= b && b <= c && a > 0) {
                return Err(invalid("Memory: minimum, desired and maximum in order."));
            }
        }
        _ => {}
    }
    Ok(())
}

/// The whole profile the entries make.
#[allow(clippy::too_many_lines, reason = "the profile, a field a line")]
pub fn profile_of(k: &Kept, v: &BTreeMap<String, String>) -> Profile {
    let g = |key: &str| v.get(key).map_or("", String::as_str).trim().to_string();
    let on = |key: &str| !g(key).is_empty();
    let n = |key: &str| g(key).parse::<u32>().unwrap_or(0);
    let units = |key: &str| crate::vres::parse_units(&g(key));
    let mib = |key: &str| g(key).replace(',', "").parse::<u64>().unwrap_or(0);
    let shared = g("m") != "ded";
    let adapters = adapters_of(v);
    let used: Vec<u32> = adapters.iter().map(|a| a.slot).collect();
    let mut next = (2..1000).filter(|s| !used.contains(s) && *s != 1);
    let optical = if on("opt") {
        let iso = g("iso");
        vec![MediaDevice {
            slot: next.next().unwrap_or(5),
            media: Some(iso).filter(|i| !i.is_empty() && i != "(none)"),
        }]
    } else {
        Vec::new()
    };
    let mut tape = Vec::new();
    if on("tap") {
        tape.push(MediaDevice {
            slot: next.next().unwrap_or(6),
            media: Some(format!("{}T", g("n"))),
        });
    }
    if on("fc")
        && let Some(d) = k.resources.tape_drives.iter().find(|d| d.released)
    {
        tape.push(MediaDevice::drive(next.next().unwrap_or(7), &d.location));
    }
    Profile {
        name: g("p").to_uppercase(),
        processors: Processors {
            mode: if shared {
                ProcessorMode::Shared
            } else {
                ProcessorMode::Dedicated
            },
            minimum: n("vmin"),
            desired: n("vdes"),
            maximum: n("vmax"),
            minimum_units: if shared { units("umin") } else { None },
            desired_units: if shared { units("udes") } else { None },
            maximum_units: if shared { units("umax") } else { None },
            sharing: shared.then(|| {
                if g("sm") == "Capped" {
                    Sharing::Capped
                } else {
                    Sharing::Uncapped
                }
            }),
            weight: (shared && g("sm") != "Capped").then(|| n("w")),
            pool: if shared {
                g("pool").split(' ').next().map(str::to_string)
            } else {
                None
            },
        },
        memory: Memory {
            minimum_mib: mib("mmin"),
            desired_mib: mib("mdes"),
            maximum_mib: mib("mmax"),
        },
        boot: Boot {
            source: match g("src").chars().next() {
                Some('A') => IplSource::A,
                Some('B') => IplSource::B,
                _ => IplSource::D,
            },
            ipl_type: if g("bm") == "Service - manual IPL" {
                IplType::Attended
            } else {
                IplType::Unattended
            },
            console: ConsoleMode::Partition,
        },
        scsi: adapters
            .iter()
            .filter(|a| a.kind == "scsi")
            .map(|a| ScsiAdapter {
                slot: a.slot,
                luns: vec![Lun {
                    lun: 0,
                    backing: a.disk.clone(),
                    serial: None,
                }],
            })
            .collect(),
        ethernet: adapters
            .iter()
            .filter(|a| a.kind == "ethernet")
            .map(|a| EthernetAdapter {
                slot: a.slot,
                switch: Some(a.switch.clone()),
                vlan: a.vlan,
                tagged: Vec::new(),
                mac: None,
                required: a.required,
            })
            .collect(),
        optical,
        tape,
        physical_io: g("io")
            .split(',')
            .filter(|l| !l.is_empty())
            .map(|l| PhysicalIo {
                location: l.into(),
                required: false,
            })
            .collect(),
    }
}

/// An adapter's words on step 6: (type, connects to).
fn words(k: &Kept, a: &Adapter) -> (String, String) {
    match a.kind.as_str() {
        "ethernet" => ("Ethernet".into(), format!("{} / VLAN {}", a.switch, a.vlan)),
        "scsi" => {
            let (pool, name) = a.disk.split_once('/').unwrap_or(("", &a.disk));
            let d = k
                .resources
                .vdisks
                .iter()
                .find(|d| d.pool == pool && d.name == name);
            let size = d
                .map(|d| format!("{} GB, ", d.bytes / 1_000_000_000))
                .unwrap_or_default();
            let held = if d.is_some_and(|d| d.holder.partition == 0) {
                ", unassigned"
            } else {
                ""
            };
            (
                "SCSI client".into(),
                format!("Virtual disk {name} ({size}{pool}{held})"),
            )
        }
        _ => (
            "Serial (console)".into(),
            "QS/VS host / 0 - terminal window".into(),
        ),
    }
}

/// The summary of the entries, as the whole profile.
#[allow(
    clippy::too_many_lines,
    reason = "one form and its request, step by step"
)]
fn summary(k: &Kept, v: &BTreeMap<String, String>) -> Vec<(String, String)> {
    let p = profile_of(k, v);
    let g = |key: &str| v.get(key).map_or("", String::as_str).to_string();
    let n = |x: u64| crate::fmt::number(x, ",");
    let u = |x: Option<u32>| crate::systems::props::units(u64::from(x.unwrap_or(0)));
    let x = &p.processors;
    let procs = if x.mode == ProcessorMode::Dedicated {
        format!("Dedicated, {} / {} / {}", x.minimum, x.desired, x.maximum)
    } else {
        let sharing = match x.sharing {
            Some(Sharing::Capped) => "capped".to_string(),
            _ => format!("uncapped weight {}", x.weight.unwrap_or(128)),
        };
        format!(
            "Shared, {}, units {} / {} / {}, virtual {} / {} / {}, {sharing}",
            x.pool.clone().unwrap_or_default(),
            u(x.minimum_units),
            u(x.desired_units),
            u(x.maximum_units),
            x.minimum,
            x.desired,
            x.maximum
        )
    };
    let adapters: Vec<String> = adapters_of(v)
        .iter()
        .map(|a| match a.kind.as_str() {
            "ethernet" => format!("{} Ethernet {} VLAN {}", a.slot, a.switch, a.vlan),
            "scsi" => {
                let name = a.disk.split_once('/').map_or(a.disk.as_str(), |(_, n)| n);
                let size = k
                    .resources
                    .vdisks
                    .iter()
                    .find(|d| format!("{}/{}", d.pool, d.name) == a.disk)
                    .map(|d| format!(" ({} GB)", d.bytes / 1_000_000_000))
                    .unwrap_or_default();
                format!("{} SCSI client -> {name}{size}", a.slot)
            }
            _ => format!("{} Serial console", a.slot),
        })
        .collect();
    let optical = p.optical.first().map_or_else(
        || "None".to_string(),
        |o| match &o.media {
            Some(m) => format!("OPT01 (ID {}), {m} loaded", o.slot),
            None => format!("OPT01 (ID {}), empty", o.slot),
        },
    );
    let tape = if p.tape.is_empty() {
        "None".to_string()
    } else {
        p.tape
            .iter()
            .map(|t| format!("ID {}, {}", t.slot, t.media.clone().unwrap_or_default()))
            .collect::<Vec<_>>()
            .join("; ")
    };
    let dft = if g("dft").is_empty() {
        ""
    } else {
        " (default)"
    };
    vec![
        ("Managed system".into(), k.name.clone()),
        (
            "Partition ID / name".into(),
            format!("{} / {}", g("id"), g("n")),
        ),
        ("Environment".into(), "QS/400".into()),
        ("Profile".into(), format!("{}{dft}", p.name)),
        ("Processors".into(), procs),
        (
            "Memory (MB)".into(),
            format!(
                "{} / {} / {}",
                n(p.memory.minimum_mib),
                n(p.memory.desired_mib),
                n(p.memory.maximum_mib)
            ),
        ),
        (
            "Physical I/O".into(),
            if p.physical_io.is_empty() {
                "None".into()
            } else {
                p.physical_io
                    .iter()
                    .map(|x| x.location.clone())
                    .collect::<Vec<_>>()
                    .join(", ")
            },
        ),
        ("Virtual adapters".into(), adapters.join("; ")),
        ("Optical".into(), optical),
        ("Tape".into(), tape),
        (
            "Boot".into(),
            format!(
                "IPL source {}, keylock {}, boot mode {}, auto start {}",
                g("src").chars().next().unwrap_or('D'),
                if g("key").is_empty() {
                    "Manual".into()
                } else {
                    g("key")
                },
                if g("bm").is_empty() {
                    "Normal".into()
                } else {
                    g("bm")
                },
                if g("as").is_empty() { "No" } else { "Yes" }
            ),
        ),
    ]
}

/// The view of step `step` from the entries.
fn view(
    rq: &Rq<'_>,
    k: &Kept,
    step: usize,
    v: &BTreeMap<String, String>,
    messages: Vec<Message>,
) -> Wizard {
    let host = k.host.as_ref();
    let copies = k
        .partitions
        .iter()
        .flat_map(|p| {
            k.profile_list(p.id)
                .iter()
                .map(move |e| format!("{} / {}", p.name, e.name))
        })
        .collect();
    let pools = host
        .map(|h| {
            h.pools
                .iter()
                .enumerate()
                .map(|(i, p)| {
                    format!(
                        "{} ({i}) - {} units available",
                        p.name,
                        crate::systems::props::units(u64::from(p.available))
                    )
                })
                .collect()
        })
        .unwrap_or_default();
    let io: Vec<String> = v
        .get("io")
        .map(|s| s.split(',').map(str::to_string).collect())
        .unwrap_or_default();
    let devices = k
        .resources
        .devices
        .iter()
        .filter(|d| d.released)
        .map(|d| Device {
            location: d.location.clone(),
            slot: d.location.rsplit('-').next().unwrap_or("").into(),
            description: d.description.clone(),
            owner: d
                .holder
                .as_ref()
                .map(|h| h.name.clone())
                .unwrap_or_default(),
            name: format!("io_{}", d.location),
            chosen: io.contains(&d.location),
        })
        .collect();
    let adapters = adapters_of(v)
        .into_iter()
        .map(|a| {
            let (t, to) = words(k, &a);
            (a, t, to)
        })
        .collect();
    let fc = k
        .resources
        .tape_drives
        .iter()
        .find(|d| d.released)
        .map(|d| format!("{} {}", d.location, d.product))
        .unwrap_or_default();
    Wizard {
        host: k.name.clone(),
        step,
        v: shown(step, v),
        next_id: next_id(k),
        copies,
        pools,
        available: host.map_or_else(String::new, |h| {
            format!(
                "{} MB (region size {} MB)",
                crate::fmt::number(h.memory.available, &rq.app.settings().number_format),
                h.memory_block_mib
            )
        }),
        devices,
        adapters,
        images: k.resources.images.iter().map(|i| i.name.clone()).collect(),
        media: [
            "OPT01 (virtual optical)".into(),
            "a tape volume of its own in the media library".into(),
            fc,
        ],
        summary: summary(k, v),
        messages,
    }
}

/// The add or edit form of step 6's adapter.
fn adapter_form(
    rq: &Rq<'_>,
    k: &Kept,
    slot: u32,
    kind: &str,
    v: &BTreeMap<String, String>,
) -> Response {
    let had = adapters_of(v).into_iter().find(|a| a.slot == slot);
    let switches: Vec<&str> = k
        .resources
        .switches
        .iter()
        .map(|s| s.name.as_str())
        .collect();
    let disks: Vec<String> = k
        .resources
        .vdisks
        .iter()
        .filter(|d| d.holder.partition == 0)
        .map(|d| format!("{}/{}", d.pool, d.name))
        .collect();
    let disk_opts: Vec<&str> = disks.iter().map(String::as_str).collect();
    let rows = match kind {
        "scsi" => vec![(
            "Virtual disk",
            qsweb::partials::select(
                "disk",
                &disk_opts,
                had.as_ref().map_or("", |a| a.disk.as_str()),
            ),
        )],
        "ethernet" => vec![
            (
                "Switch",
                qsweb::partials::select(
                    "sw",
                    &switches,
                    had.as_ref().map_or("", |a| a.switch.as_str()),
                ),
            ),
            (
                "VLAN",
                text(
                    "vlan",
                    &had.as_ref().map_or("1".into(), |a| a.vlan.to_string()),
                    5,
                ),
            ),
        ],
        _ => vec![(
            "Serial adapter",
            "The console is adapter 0, the terminal window. It is fixed.".into(),
        )],
    };
    let mut rows = rows;
    rows.insert(0, ("Adapter ID", text("slot", &slot.to_string(), 4)));
    rows.push((
        "Required",
        check("req", had.as_ref().is_none_or(|a| a.required)),
    ));
    let body = format!(
        "<form method=\"post\">{}{}{}<div class=\"buttons\"><input type=\"submit\" name=\"keep\" value=\"OK\"><input type=\"submit\" name=\"next6\" value=\"Cancel\"></div></form>",
        rq.ctx.csrf_field(),
        qsweb::partials::hidden("kind", kind),
        props(&rows)
    );
    rq.page(Page {
        title: "Create partition".into(),
        crumb: vec![
            "Systems Management".into(),
            "Partitions".into(),
            "Create partition".into(),
        ],
        h1: "Virtual adapter".into(),
        body,
        help: "create-partition".into(),
        nav: Some(("Systems Management", Some("/partitions"))),
        ..Page::default()
    })
}

/// Finish: the partition at its ID with its settings, then its first
/// profile, its default, its keylock and auto start; a tape volume of
/// its own when asked.
#[allow(
    clippy::too_many_lines,
    reason = "one form and its request, step by step"
)]
fn finish(rq: &Rq<'_>, k: &Kept, v: &BTreeMap<String, String>) -> Result<Message, Vec<Message>> {
    let g = |key: &str| v.get(key).map_or("", String::as_str).trim().to_string();
    let id: u32 = g("id").parse().unwrap_or(0);
    let name = g("n").to_uppercase();
    let what = format!("create partition {name} (ID {id}) on {}", k.name);
    let settings = Settings {
        description: g("d"),
        panel: true,
        with_host: WithHost::Delayed,
        delay: 300,
    };
    let req = if k.offers(caps::PARTITION | caps::PARTITION_V3) {
        AnyRequest::Partition(partition::Request::CreateWith(partition::CreateWith {
            id: Some(id),
            name: name.clone(),
            settings,
        }))
    } else {
        let lack = mgmtproto::ask::lacks(k.caps, caps::PARTITION_V3).unwrap_or("");
        return Err(vec![qmcmsg::message(
            Kind::Error,
            &qmcmsg::LACKS,
            &[&k.name, lack],
        )]);
    };
    let def = match crate::systems::ask(rq, &k.name, "Create partition", &req) {
        Ok(AnyReply::Partition(partition::Reply::CreateWith(d))) => d,
        Ok(_) => return Err(Vec::new()),
        Err(ms) => {
            rq.audit(&what, "refused");
            return Err(ms);
        }
    };
    rq.audit(&what, "created");
    let mut profile = profile_of(k, v);
    let mut volume = None;
    if !g("tap").is_empty() {
        let vol = format!("{name}T");
        volume = Some(vol.clone());
        let _ = crate::systems::ask(
            rq,
            &k.name,
            "Create tape volume",
            &AnyRequest::Vres(qsi::host::vres::Request::VolumeCreate(
                qsi::host::vres::VolumeCreate {
                    name: vol.clone(),
                    identifier: vol.chars().take(6).collect(),
                    mib: 200 * 1024,
                },
            )),
        );
        for t in &mut profile.tape {
            if t.media.as_deref() == Some(&format!("{}T", g("n"))) {
                t.media = Some(vol.clone());
            }
        }
    }
    let pname = profile.name.clone();
    let put = AnyRequest::Profiles(profiles::Request::Put(profiles::Put {
        id: def.id,
        profile,
        revision: None,
    }));
    let made = qmcmsg::message(
        Kind::Done,
        &qmcmsg::PARTITION_CREATED,
        &[&def.name, &def.id.to_string()],
    );
    if let Err(mut ms) = crate::systems::ask(rq, &k.name, "Create profile", &put) {
        rq.audit(&format!("{what}: profile {pname}"), "refused");
        // All or nothing: the partition made for the profile goes again.
        let undo = AnyRequest::Partition(partition::Request::Delete(partition::Delete {
            id: def.id,
            disks: false,
        }));
        if let Err(more) = crate::systems::ask(rq, &k.name, "Delete partition", &undo) {
            rq.audit(&what, "left without a profile: its delete refused");
            ms.insert(0, made);
            ms.extend(more);
            return Err(ms);
        }
        if let Some(vol) = volume {
            let _ = crate::systems::ask(
                rq,
                &k.name,
                "Delete tape volume",
                &AnyRequest::Vres(qsi::host::vres::Request::VolumeDelete(
                    qsi::host::vres::VolumeDelete { name: vol },
                )),
            );
        }
        rq.audit(&what, "deleted again: its profile was refused");
        ms.insert(
            0,
            qmcmsg::message(Kind::Error, &qmcmsg::NOT_CREATED, &[&def.name]),
        );
        return Err(ms);
    }
    rq.audit(&format!("{what}: profile {pname}"), "created");
    let mut d = def.clone();
    if !g("dft").is_empty() || d.default_profile.is_empty() {
        d.default_profile.clone_from(&pname);
    }
    d.keylock = if g("key") == "Normal" && g("bm") != "Service - manual IPL" {
        Keylock::Normal
    } else {
        Keylock::Manual
    };
    d.auto_start = AutoStart {
        enabled: !g("as").is_empty(),
        ..def.auto_start.clone()
    };
    if d != def {
        if d.default_profile != def.default_profile {
            let _ = crate::systems::ask(
                rq,
                &k.name,
                "Set default profile",
                &AnyRequest::Profiles(profiles::Request::SetDefault(profiles::SetDefault {
                    id: def.id,
                    name: pname.clone(),
                })),
            );
        }
        let change = AnyRequest::Partition(partition::Request::Change(partition::Change {
            definition: d,
        }));
        if let Err(mut ms) = crate::systems::ask(rq, &k.name, "Change partition", &change) {
            ms.insert(0, made);
            return Err(ms);
        }
    }
    Ok(made)
}

/// `GET`, `POST /partitions/create/step/<n>?ms=`.
#[allow(clippy::too_many_lines, reason = "the wizard's moves")]
pub fn route(rq: &Rq<'_>, step: usize) -> Response {
    let f = rq.req.form();
    let mut v = entries(rq);
    // The host: chosen when the wizard begins, kept after.
    let host = if let Some(h) = v.get("ms") {
        h.clone()
    } else {
        let q = rq.q("ms");
        let kept = crate::systems::systems(rq).unwrap_or_default();
        kept.into_iter()
            .find(|k| (q.is_empty() || k.name == q) && k.offers(caps::PARTITION))
            .map(|k| k.name)
            .unwrap_or_default()
    };
    let Some(k) = crate::systems::system(rq, &host) else {
        let m = qmcmsg::message(Kind::Error, &qmcmsg::NO_HOST_FOR_PARTITION, &[]);
        return rq.done("/partitions", m);
    };
    if v.is_empty() || v.get("ms") != Some(&k.name) {
        v = fresh(&k);
        v.insert("ms".into(), k.name.clone());
        save(rq, &v);
    }
    let step = step.clamp(1, 9);
    let at = |n: usize| Response::redirect(&rq.ctx.url(&format!("/partitions/create/step/{n}")));
    if rq.post() {
        if let Some(no) = lacks(rq, Task::CreatePartitions) {
            return no;
        }
        if f.has("cancel") {
            let mut none = BTreeMap::new();
            none.clear();
            save(rq, &none);
            return Response::redirect(&rq.ctx.url("/partitions"));
        }
        // Step 6's adapter form.
        if f.has("keep") {
            let slot: u32 = f.str("slot").trim().parse().unwrap_or(0);
            let mut list = adapters_of(&v);
            list.retain(|a| a.slot != slot);
            let kind = f.str("kind").to_string();
            if kind != "serial" && slot > 1 {
                list.push(Adapter {
                    slot,
                    kind,
                    switch: f.str("sw").into(),
                    vlan: f.str("vlan").trim().parse().unwrap_or(1),
                    disk: f.str("disk").into(),
                    required: f.has("req"),
                });
            }
            list.sort_by_key(|a| a.slot);
            set_adapters(&mut v, &list);
            save(rq, &v);
            return at(6);
        }
        if f.has("next6") {
            return at(6);
        }
        keep_posted(step, &f, &mut v);
        save(rq, &v);
        if step == 6 && f.has("add") {
            let kind = match f.str("t") {
                "Ethernet" => "ethernet",
                "Serial" => "serial",
                _ => "scsi",
            };
            let used: Vec<u32> = adapters_of(&v).iter().map(|a| a.slot).collect();
            let slot = (2..1000).find(|s| !used.contains(s)).unwrap_or(2);
            return adapter_form(rq, &k, slot, kind, &v);
        }
        if f.has("back") {
            return at(step.saturating_sub(1).max(1));
        }
        if f.has("finish") && step == 9 {
            for s in 1..=8 {
                if let Err(ms) = validate(&k, s, &v) {
                    return rq.page(page(&rq.ctx, &view(rq, &k, s, &v, ms)));
                }
            }
            return match finish(rq, &k, &v) {
                Ok(m) => {
                    save(rq, &BTreeMap::new());
                    rq.done("/partitions", m)
                }
                Err(ms) => rq.page(page(&rq.ctx, &view(rq, &k, 9, &v, ms))),
            };
        }
        if let Err(ms) = validate(&k, step, &v) {
            return rq.page(page(&rq.ctx, &view(rq, &k, step, &v, ms)));
        }
        // Copy from: the profile's values, kept as the next steps' entries.
        if step == 2
            && let Some((part, prof)) = f.str("copy").split_once(" / ")
            && let Some(p) = k.partitions.iter().find(|p| p.name == part)
            && let Some(b) = k.profile_body(p.id, prof)
        {
            let x = &b.profile.processors;
            let u = |o: Option<u32>| crate::systems::props::units(u64::from(o.unwrap_or(0)));
            for (key, val) in [
                (
                    "m",
                    if x.mode == ProcessorMode::Dedicated {
                        "ded".into()
                    } else {
                        "shared".to_string()
                    },
                ),
                ("umin", u(x.minimum_units)),
                ("udes", u(x.desired_units)),
                ("umax", u(x.maximum_units)),
                ("vmin", x.minimum.to_string()),
                ("vdes", x.desired.to_string()),
                ("vmax", x.maximum.to_string()),
                ("mmin", b.profile.memory.minimum_mib.to_string()),
                ("mdes", b.profile.memory.desired_mib.to_string()),
                ("mmax", b.profile.memory.maximum_mib.to_string()),
            ] {
                v.insert(key.into(), val);
            }
            save(rq, &v);
        }
        return at(step + 1);
    }
    if step == 6 {
        if let Ok(s) = rq.q("edit").parse::<u32>() {
            let kind = adapters_of(&v)
                .into_iter()
                .find(|a| a.slot == s)
                .map_or_else(|| "ethernet".to_string(), |a| a.kind);
            return adapter_form(rq, &k, s, &kind, &v);
        }
        if let Ok(s) = rq.q("remove").parse::<u32>() {
            let mut list = adapters_of(&v);
            list.retain(|a| a.slot != s || a.slot == 0);
            set_adapters(&mut v, &list);
            save(rq, &v);
        }
    }
    rq.page(page(&rq.ctx, &view(rq, &k, step, &v, Vec::new())))
}
