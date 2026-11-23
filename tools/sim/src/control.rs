// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! What a harness or a person tells the simulator, a line at a time:
//! `<command> <system> [arguments]`, answered with one line (or a line
//! each of what is kept).

use qsi::base::PowerState;
use qsi::host::model::{IplSource, PartitionState};
use qsi::host::partition::{Activate, ShutdownKind};
use qsi::host::refcodes::Source;

use crate::fixture;
use crate::system::refuse::units;
use crate::system::{FamilyRefusal, Shared, System, codes, lock};

/// What `markup` and `lie` put where a system names things: every
/// character a page must escape.
pub const MARKUP: &str = "<script>alert(1)</script><img src=x onerror=1>\"'&";

/// The commands, for `help`.
pub const HELP: &str = "pair S | expire S | drop S | stall S on|off | refuse S on|off | down S | up S \
| refuse-power S ID TEXT... | refuse-panel S ID TEXT... | refuse-family S FAMILY[/OP] ID TEXT... \
| missing S NAME|none | allow S | nonsense S | oversize S | markup S | lie S | bad-screen S | flood S on|off \
| no-credit S on|off | renew S | activate S P [PROFILE] [a|b|d] | shutdown S P [delayed|system|immediate] \
| state S P not-activated|starting|running|shutting-down|error | fail S POOL UNIT \
| upload-rate S BYTES|stall|off | uploaded S | code S CODE TEXT... | pcode S P CODE TEXT... \
| keylock S P normal|manual | keys S P | lines S | pins S | withhold S BIT...|new \
| kept S pools|disks|vdisks|images|volumes|switches|procpools|profiles P|drives P \
| problem S open P CODE TEXT... | problem S seen|clear|close N | dump S P NAME MIB \
| stage-refuse S ID TEXT... | outcome S confirm | outcome S rollback CODE TEXT... | fixlevel S LEVEL";

fn on(w: Option<&&str>) -> bool {
    w.is_some_and(|w| *w == "on")
}

fn rest(words: &[&str], from: usize) -> String {
    words.get(from..).map(|w| w.join(" ")).unwrap_or_default()
}

/// A refusal's messages as one line.
fn said(notes: &[(String, String)]) -> String {
    notes
        .iter()
        .map(|(id, text)| format!("{id} {text}"))
        .collect::<Vec<_>>()
        .join(" / ")
}

/// A family by name or number.
fn family(w: &str) -> Option<u16> {
    use qsi::family as f;
    Some(match w {
        "base" => f::BASE,
        "host" => f::HOST,
        "partitions" => f::PARTITIONS,
        "partition" => f::PARTITION,
        "profiles" => f::PROFILES,
        "panel" => f::PANEL,
        "refcodes" => f::REFCODES,
        "dumps" => f::DUMPS,
        "console" => f::CONSOLE,
        "vres" => f::VRES,
        w => number(w)?,
    })
}

/// A number, decimal or `0x` hexadecimal.
fn number(w: &str) -> Option<u16> {
    match w.strip_prefix("0x") {
        Some(h) => u16::from_str_radix(h, 16).ok(),
        None => w.parse().ok(),
    }
}

fn state(w: &str) -> Option<PartitionState> {
    Some(match w {
        "not-activated" => PartitionState::NotActivated,
        "starting" => PartitionState::Starting,
        "running" => PartitionState::Running,
        "shutting-down" => PartitionState::ShuttingDown,
        "error" => PartitionState::Error,
        _ => return None,
    })
}

/// What is kept of `what`, a line each.
#[allow(clippy::too_many_lines, reason = "one arm a kind")]
fn kept(s: &System, what: &str, id: u32) -> String {
    let lines: Vec<String> = match what {
        "pools" => s
            .vres
            .pools
            .iter()
            .map(|p| {
                format!(
                    "{} {} {} bytes {} allocated {} provisioned {} units {}{}",
                    p.name,
                    p.protection,
                    p.state,
                    p.bytes,
                    p.allocated,
                    p.provisioned,
                    p.units.len(),
                    if p.repository { " repository" } else { "" }
                )
            })
            .collect(),
        "disks" => s
            .vres
            .disks
            .iter()
            .map(|d| {
                format!(
                    "{} {} pool {} free {} foreign {}",
                    d.name, d.what, d.pool, d.free, d.foreign
                )
            })
            .collect(),
        "vdisks" => s
            .vres
            .vdisks
            .iter()
            .map(|v| {
                format!(
                    "{}/{} {} bytes {} held {}/{}",
                    v.pool,
                    v.name,
                    if v.thick { "thick" } else { "thin" },
                    v.bytes,
                    v.holder.partition,
                    v.holder.slot
                )
            })
            .collect(),
        "images" => s
            .vres
            .images
            .iter()
            .map(|i| {
                format!(
                    "{} {} bytes {} held {}/{}",
                    i.name, i.label, i.bytes, i.holder.partition, i.holder.slot
                )
            })
            .collect(),
        "volumes" => s
            .vres
            .volumes
            .iter()
            .map(|v| {
                format!(
                    "{} {} {} MiB protected {} held {}/{}",
                    v.name,
                    v.identifier,
                    v.capacity_mib,
                    v.protected,
                    v.holder.partition,
                    v.holder.slot
                )
            })
            .collect(),
        "switches" => s
            .vres
            .switches
            .iter()
            .map(|w| {
                let vlans: Vec<String> = w.vlans.iter().map(u16::to_string).collect();
                format!(
                    "{} {} vlans {} mtu {} uplink {}",
                    w.name,
                    w.id,
                    vlans.join(","),
                    w.mtu,
                    w.uplink_state
                )
            })
            .collect(),
        "procpools" => s
            .host
            .iter()
            .flat_map(|h| h.pools.iter())
            .map(|p| {
                format!(
                    "{} cores {:?} maximum {} entitled {} available {} partitions {}",
                    p.name,
                    p.cores,
                    units(p.maximum),
                    units(p.entitled),
                    units(p.available),
                    p.partitions
                )
            })
            .collect(),
        "profiles" => s
            .part_ref(id)
            .map(|p| {
                p.profiles
                    .iter()
                    .map(|k| {
                        format!(
                            "{} revision {}{}",
                            k.profile.name,
                            k.revision,
                            if k.profile.name == p.definition.default_profile {
                                " default"
                            } else {
                                ""
                            }
                        )
                    })
                    .collect()
            })
            .unwrap_or_default(),
        "drives" => s
            .part_ref(id)
            .map(|p| {
                p.drives
                    .iter()
                    .map(|d| {
                        format!(
                            "{} {:?} {} prevented {}",
                            d.slot, d.kind, d.name, d.prevented
                        )
                    })
                    .collect()
            })
            .unwrap_or_default(),
        _ => return format!("kept: not a kind ({HELP})"),
    };
    lines.join("\n")
}

/// Carry out `line` on `systems`.
#[allow(clippy::too_many_lines, reason = "one arm a command")]
pub fn command(systems: &[Shared], line: &str) -> String {
    let words: Vec<&str> = line.split_whitespace().collect();
    let Some(cmd) = words.first() else {
        return String::new();
    };
    if *cmd == "help" {
        return HELP.into();
    }
    let Some(name) = words.get(1) else {
        return format!("{cmd}: which system?");
    };
    let Some(sys) = systems
        .iter()
        .find(|s| lock(s).name().eq_ignore_ascii_case(name))
    else {
        return format!("no system {name}");
    };
    let mut s = lock(sys);
    let part = |i: usize| {
        words
            .get(i)
            .and_then(|w| w.parse::<u32>().ok())
            .unwrap_or(0)
    };
    match *cmd {
        "pair" => s.start_pairing(),
        "expire" => {
            s.expire_pairing();
            "expired".into()
        }
        "drop" => {
            s.generation += 1;
            "dropped".into()
        }
        "withhold" => {
            // The bits named (19 to 24, or `new` for all of them) taken
            // from the hello; the session drops and comes back without.
            let bits: u64 = words[2..]
                .iter()
                .map(|w| match *w {
                    "new" => crate::system::NEW_BITS,
                    w => w.parse::<u32>().ok().map_or(0, |b| 1u64 << b.min(63)),
                })
                .fold(0, |a, b| a | b);
            s.hello.caps &= !bits;
            s.generation += 1;
            format!("caps {:#x}", s.hello.caps)
        }
        "stall" => {
            s.faults.stall = on(words.get(2));
            format!("stall {}", s.faults.stall)
        }
        "refuse" => {
            s.faults.refuse_connections = on(words.get(2));
            format!("refuse {}", s.faults.refuse_connections)
        }
        "down" => {
            s.up = false;
            s.power = PowerState::Off;
            s.generation += 1;
            "down".into()
        }
        "up" => {
            s.up = true;
            s.power = PowerState::Running;
            "up".into()
        }
        "refuse-power" => {
            s.faults.refuse_power = Some((
                (*words.get(2).unwrap_or(&"QVS1010")).into(),
                rest(&words, 3),
            ));
            "refusing power".into()
        }
        "refuse-panel" => {
            s.faults.refuse_panel = Some((
                (*words.get(2).unwrap_or(&"QVS2306")).into(),
                rest(&words, 3),
            ));
            "refusing panel functions".into()
        }
        "refuse-family" => {
            let what = words.get(2).copied().unwrap_or_default();
            let (f, op) = what
                .split_once('/')
                .map_or((what, None), |(f, o)| (f, Some(o)));
            let Some(f) = family(f) else {
                return format!("refuse-family: no family {f}");
            };
            let op = match op.map(number) {
                None => None,
                Some(Some(o)) => Some(o),
                Some(None) => return format!("refuse-family: no operation {what}"),
            };
            s.faults.refuse_family.push(FamilyRefusal {
                family: f,
                op,
                id: (*words.get(3).unwrap_or(&"QSI0301")).into(),
                text: rest(&words, 4),
            });
            format!("refusing family {f:#06x}")
        }
        "slow" => {
            let what = words.get(2).copied().unwrap_or_default();
            let (f, op) = what
                .split_once('/')
                .map_or((what, None), |(f, o)| (f, Some(o)));
            let Some(f) = family(f) else {
                return format!("slow: no family {f}");
            };
            let op = match op.map(number) {
                None => None,
                Some(Some(o)) => Some(o),
                Some(None) => return format!("slow: no operation {what}"),
            };
            let Some(ms) = words.get(3).and_then(|m| m.parse().ok()) else {
                return "slow: how many milliseconds".into();
            };
            s.faults.slow.push((f, op, ms));
            format!("answering family {f:#06x} after {ms} ms")
        }
        "missing" => match words.get(2) {
            None | Some(&"none") => {
                s.faults.missing.clear();
                "nothing missing".into()
            }
            Some(n) => {
                s.faults.missing.push((*n).into());
                format!("missing {n}")
            }
        },
        "allow" => {
            s.faults.refuse_power = None;
            s.faults.refuse_panel = None;
            s.faults.refuse_family.clear();
            s.faults.missing.clear();
            s.faults.slow.clear();
            "allowing".into()
        }
        "nonsense" => {
            s.faults.nonsense = true;
            "nonsense".into()
        }
        "markup" => {
            // Names, a code's text and a console's cells full of markup:
            // the console shows each as text (`docs/SECURITY.md`).
            for p in &mut s.parts {
                p.definition.name = format!("<b>{}</b>\"'&", p.definition.id);
                p.status.name.clone_from(&p.definition.name);
                if let Some(r) = &mut p.screen {
                    for (c, ch) in r.frame.cells.iter_mut().zip(MARKUP.chars().cycle()) {
                        c.ch = ch;
                    }
                }
            }
            let ids: Vec<u32> = s.parts.iter().map(|p| p.definition.id).collect();
            for id in ids {
                s.resources_changed(id);
            }
            s.model = MARKUP.into();
            codes::post(&mut s, "B6000666", MARKUP);
            s.generation += 1;
            "markup".into()
        }
        "lie" => {
            // A hello that says what the system is not: another product's
            // release, a model and build of markup, no capability.
            s.hello.release = "V99R99M99<script>".into();
            s.build = MARKUP.into();
            s.hello.caps = 0;
            s.generation += 1;
            "lying".into()
        }
        "bad-screen" => {
            // A console's frame whose size is not its cells', its cursor
            // off the grid and a character past every code page.
            for p in &mut s.parts {
                if let Some(r) = &mut p.screen {
                    r.frame.rows = 200;
                    r.frame.cols = 250;
                    r.frame.status.row = 255;
                    r.frame.status.col = 255;
                    if let Some(c) = r.frame.cells.first_mut() {
                        c.ch = '\u{10FFFF}';
                        c.attr = u16::MAX;
                    }
                }
            }
            "bad screen".into()
        }
        "oversize" => {
            s.faults.oversize = true;
            "oversize".into()
        }
        "flood" => {
            s.faults.flood = on(words.get(2));
            format!("flood {}", s.faults.flood)
        }
        "no-credit" => {
            s.faults.no_credit = on(words.get(2));
            format!("no credit {}", s.faults.no_credit)
        }
        "renew" => {
            let turn = u32::try_from(s.generation % 1000).unwrap_or(0) + 1;
            s.identity = fixture::identity(&s.hello.name.clone(), &s.hello.serial.clone(), turn);
            s.generation += 1;
            s.identity.fingerprint().hex()
        }
        "activate" => {
            let mut profile = String::new();
            let mut source = None;
            for w in words.iter().skip(3) {
                match *w {
                    "a" => source = Some(IplSource::A),
                    "b" => source = Some(IplSource::B),
                    "d" => source = Some(IplSource::D),
                    p => profile = p.into(),
                }
            }
            let a = Activate {
                id: part(2),
                profile,
                source,
                keylock: None,
            };
            match s.activate(&a) {
                Ok(_) => "activated".into(),
                Err(n) => said(&n),
            }
        }
        "shutdown" => {
            let id = part(2);
            if !s.part_ref(id).is_some_and(crate::system::Part::active) {
                return "not activated".into();
            }
            match words.get(3).copied() {
                Some("delayed") => codes::shut_down(&mut s, id, ShutdownKind::Delayed, 0),
                Some("system") => codes::shut_down(&mut s, id, ShutdownKind::System, 0),
                Some("immediate") => codes::shut_down(&mut s, id, ShutdownKind::Immediate, 0),
                _ => codes::end(&mut s, id),
            }
            "shut down".into()
        }
        "state" => match words.get(3).and_then(|w| state(w)) {
            Some(st) if s.part_ref(part(2)).is_some() => {
                s.set_state(part(2), st);
                "set".into()
            }
            Some(_) => "no partition".into(),
            None => "state: not a state".into(),
        },
        "fail" => {
            let pool = words.get(2).copied().unwrap_or_default();
            let unit = part(3);
            let Some(p) = s.vres.pools.iter_mut().find(|p| p.name == pool) else {
                return format!("no pool {pool}");
            };
            let Some(u) = p.units.iter_mut().find(|u| u.number == unit) else {
                return format!("no unit {unit}");
            };
            u.state = "Failed".into();
            p.state = "Degraded".into();
            crate::system::vres::pool_changed(&mut s, pool);
            "failed".into()
        }
        "problem" => match words.get(2).copied() {
            Some("open") => {
                let code = words.get(4).copied().unwrap_or("A2007020");
                let resources = if part(3) == 0 {
                    Vec::new()
                } else {
                    vec![crate::system::servicing::resource(
                        qsi::service::ResourceKind::Partition,
                        &s.part_name(part(3)),
                        "",
                        "",
                    )]
                };
                let n = s.open_problem(
                    part(3),
                    code,
                    &rest(&words, 5),
                    qsi::service::Severity::Attention,
                    resources,
                );
                format!("problem {n} opened")
            }
            Some(w @ ("seen" | "clear")) => {
                match s.problem_seen(u64::from(part(3)), w == "clear") {
                    Ok(()) => format!("problem {} {w}", part(3)),
                    Err(e) => e,
                }
            }
            Some("close") => match s.close_problem(u64::from(part(3)), true, "sim") {
                Ok(_) => "closed".into(),
                Err(n) => said(&n),
            },
            _ => "problem: open, seen, clear or close".into(),
        },
        "dump" => {
            let name = words.get(3).copied().unwrap_or("DUMP");
            let mib: u64 = words.get(4).and_then(|w| w.parse().ok()).unwrap_or(1);
            if part(2) == 0 {
                s.take_own_dump(name, mib << 20, "sim");
                "dump taken".into()
            } else {
                match s.take_part_dump(part(2), name, mib << 20, "function22") {
                    Ok(()) => "dump taken".into(),
                    Err(e) => e,
                }
            }
        }
        "stage-refuse" => {
            s.servicing.refuse_stage = Some((
                (*words.get(2).unwrap_or(&"QSI1101")).into(),
                rest(&words, 3),
            ));
            "the next stage is refused".into()
        }
        "outcome" => {
            if words.get(2).copied() == Some("rollback") {
                s.servicing.outcome = crate::system::servicing::Outcome::RollBack(
                    rest(&words, 4),
                    (*words.get(3).unwrap_or(&"A2001035")).into(),
                );
                "the next activation rolls back".into()
            } else {
                s.servicing.outcome = crate::system::servicing::Outcome::Confirm;
                "the next activation confirms".into()
            }
        }
        "fixlevel" => {
            s.servicing.fix_level = Some(qsi::base::Level {
                level: rest(&words, 2),
                date: crate::system::now_s(),
            });
            "fix level set".into()
        }
        "upload-rate" => {
            match words.get(2).copied() {
                Some("stall") => s.upload.stalled = true,
                Some("off") | None => {
                    s.upload.stalled = false;
                    s.upload.rate = 0;
                }
                Some(n) => {
                    s.upload.stalled = false;
                    s.upload.rate = n.parse().unwrap_or(0);
                }
            }
            format!("upload rate {} stalled {}", s.upload.rate, s.upload.stalled)
        }
        "uploaded" => {
            let u = &s.upload;
            format!("{} {} {} {}", u.name, u.arrived, u.sha256, u.end)
                .trim_end()
                .to_string()
        }
        "kept" => kept(&s, words.get(2).copied().unwrap_or_default(), part(3)),
        "code" => {
            codes::post(
                &mut s,
                words.get(2).unwrap_or(&"00000000"),
                &rest(&words, 3),
            );
            "posted".into()
        }
        "pcode" => {
            codes::post_part(
                &mut s,
                part(2),
                words.get(3).unwrap_or(&"00000000"),
                &rest(&words, 4),
                Source::System,
                false,
            );
            "posted".into()
        }
        "keylock" => {
            let k = if words.get(3) == Some(&"manual") {
                qsi::base::Keylock::Manual
            } else {
                qsi::base::Keylock::Normal
            };
            match s.part(part(2)) {
                Some(p) => {
                    p.definition.keylock = k;
                    p.keylock_now = None;
                    "set".into()
                }
                None => "no partition".into(),
            }
        }
        "keys" => s
            .part(part(2))
            .map(|p| format!("{:?}", p.keys))
            .unwrap_or_default(),
        "lines" => s.lines.join("\n"),
        "pins" => s
            .pins
            .iter()
            .map(|(f, n)| format!("{n} {}", f.hex()))
            .collect::<Vec<_>>()
            .join("\n"),
        _ => format!("{cmd}: not a command ({HELP})"),
    }
}
