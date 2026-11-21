// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! What a harness or a person tells the simulator, a line at a time:
//! `<command> <system> [arguments]`, answered with one line.

use qsi::base::PowerState;
use qsi::host::model::PartitionState;
use qsi::host::refcodes::Source;

use crate::fixture;
use crate::system::{Shared, codes, lock};

/// The commands, for `help`.
pub const HELP: &str = "pair S | expire S | drop S | stall S on|off | refuse S on|off | down S | up S \
| refuse-power S ID TEXT... | refuse-panel S ID TEXT... | allow S | nonsense S | oversize S | flood S on|off \
| no-credit S on|off | renew S | activate S P | shutdown S P | code S CODE TEXT... | pcode S P CODE TEXT... \
| keylock S P normal|manual | keys S P | lines S | pins S | withhold S BIT...|new";

fn on(w: Option<&&str>) -> bool {
    w.is_some_and(|w| *w == "on")
}

fn rest(words: &[&str], from: usize) -> String {
    words.get(from..).map(|w| w.join(" ")).unwrap_or_default()
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
        "allow" => {
            s.faults.refuse_power = None;
            s.faults.refuse_panel = None;
            "allowing".into()
        }
        "nonsense" => {
            s.faults.nonsense = true;
            "nonsense".into()
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
            codes::activate(&mut s, part(2));
            "activated".into()
        }
        "shutdown" => {
            codes::end(&mut s, part(2));
            s.set_state(part(2), PartitionState::NotActivated);
            "shut down".into()
        }
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
