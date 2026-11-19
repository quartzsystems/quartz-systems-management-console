// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! `Display console status`: what PID 1's status says, each network
//! interface by its role, what the processor has of what QSI and the
//! web server need, and how long the console has run. Drawn again
//! every second; F7 and F8 page it.

use console::Screen;
use netd::control::NetStatus;
use panels::{Flow, Help, Named, Panel, kit};
use release::fact;
use supervise::control::HostStatus;

use crate::env::Env;
use crate::menu::{Outcome, Step};

/// Rows shown at once.
const ROWS: usize = 14;
/// The width of a row's label, to its colon.
const LABEL: usize = 17;

/// `Display console status`.
#[derive(Default)]
pub struct Status {
    top: usize,
}

/// Seconds since the kernel started, as `3 d 04:05:06`.
pub fn uptime(secs: u64) -> String {
    let (days, rest) = (secs / 86_400, secs % 86_400);
    let clock = format!(
        "{:02}:{:02}:{:02}",
        rest / 3600,
        rest % 3600 / 60,
        rest % 60
    );
    if days == 0 {
        clock
    } else {
        format!("{days} d {clock}")
    }
}

/// The kernel's uptime, in whole seconds.
fn seconds_up() -> u64 {
    std::fs::read_to_string("/proc/uptime")
        .ok()
        .and_then(|t| t.split('.').next().and_then(|s| s.trim().parse().ok()))
        .unwrap_or(0)
}

/// An interface's role as the screen names it.
fn role(written: &str) -> &str {
    if written.is_empty() {
        release::INTERFACE_ROLES[0]
    } else {
        written
    }
}

/// The rows of the display.
pub fn rows(status: &HostStatus, interfaces: &[NetStatus], up: u64) -> Vec<String> {
    let row = |label: &str, value: &str| format!("{} {value}", kit::dotted(label, LABEL));
    let get = |key: &str| status.fact(key).unwrap_or("").to_string();
    let mut out = vec![
        row(
            "Release",
            &format!("{} build {}", get(fact::RELEASE), get(fact::BUILD)),
        ),
        row(
            "Slot",
            &format!("{} ({})", get(fact::SLOT), get(fact::RECORD)),
        ),
        row("Host name", &get(fact::HOST)),
    ];
    if interfaces.is_empty() {
        out.push(row("Open network", &get(fact::MANAGEMENT)));
    }
    for i in interfaces {
        out.push(row(&format!("Network {}", role(&i.role)), &i.line));
    }
    if !interfaces.iter().any(|i| role(&i.role) == "private") {
        out.push(row("Network private", "none configured"));
    }
    out.push(row("Load source", &get(fact::LOAD_SOURCE)));
    out.push(row("Data", &get(fact::DATA)));
    let names: Vec<&str> = processor::NEEDED.iter().map(|(_, n)| *n).collect();
    out.push(row("Processor", &get(fact::PROCESSOR)));
    out.push(row("Features needed", &names.join(", ")));
    out.push(row("Machine", &get(fact::MACHINE)));
    out.push(row("Uptime", &uptime(up)));
    out.push(row("Reference code", &get(fact::REFERENCE)));
    for s in &status.services {
        let failures = match s.failures {
            0 => String::new(),
            1 => ", 1 failure".into(),
            n => format!(", {n} failures"),
        };
        out.push(row(
            &format!("Service {}", s.name),
            &format!("{}{failures}", s.state),
        ));
    }
    out
}

impl Panel<Env, Outcome> for Status {
    fn name(&self) -> &'static str {
        "console-status"
    }

    fn render(&self, env: &mut Env) -> Screen {
        let status = env.status();
        let interfaces = env.interfaces();
        let all = rows(&status, &interfaces, seconds_up());
        let (shown, foot) = kit::window(&all, self.top, ROWS);
        let mut s = kit::page(
            "Display Console Status",
            &env.system(),
            &shown,
            "Enter=Continue   F1=Help   F3=Exit   F7=Back   F8=Forward   F12=Cancel",
        );
        s.put(
            panels::ui::BODY_ROW + ROWS,
            80 - foot.len(),
            &foot,
            console::Attr::WHITE,
        );
        s
    }

    fn enter(&mut self, _fields: &[String], _env: &mut Env) -> Step {
        Flow::Pop
    }

    fn function(&mut self, key: u8, _fields: &[String], env: &mut Env) -> Step {
        match key {
            7 | 8 => {
                let status = env.status();
                let interfaces = env.interfaces();
                let len = rows(&status, &interfaces, 0).len();
                self.top = kit::scroll(self.top, key, len, ROWS);
                Flow::Stay
            }
            3 => Flow::PopAll,
            12 => Flow::Pop,
            _ => Flow::Stay,
        }
    }

    fn help(&self) -> Help {
        Help {
            title: "Display Console Status",
            lines: &[
                "What the console is running and how, drawn again every second.",
                "",
                "Release and slot: the code running and which of the two slots",
                "it came from. Network: each interface by its role. open is the",
                "network operators reach the console on; private, when there is",
                "one, faces the systems it manages. Load source: its disks and",
                "their protection. Processor: whether it has what QSI and the",
                "web server need. Each service of the console with its state.",
                "",
                "F8 shows more rows, F7 goes back. Enter, F3 or F12 leaves.",
            ],
        }
    }

    fn live(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use supervise::control::ServiceStatus;

    #[test]
    fn uptime_reads_as_days_and_a_clock() {
        assert_eq!(uptime(59), "00:00:59");
        assert_eq!(uptime(3 * 86_400 + 4 * 3600 + 5 * 60 + 6), "3 d 04:05:06");
    }

    #[test]
    fn the_rows_name_each_interface_by_role_and_each_service() {
        let status = HostStatus {
            facts: vec![
                (fact::RELEASE.into(), "V1R0M0".into()),
                (fact::BUILD.into(), "abc1234".into()),
                (fact::SLOT.into(), "A".into()),
                (fact::RECORD.into(), "default A".into()),
            ],
            services: vec![ServiceStatus {
                name: "netd".into(),
                state: "ready".into(),
                pid: 9,
                failures: 2,
            }],
        };
        let open = NetStatus {
            line: "10.0.2.15/24 on eth0, gateway 10.0.2.2".into(),
            ..NetStatus::default()
        };
        let rows = rows(&status, &[open], 61);
        assert_eq!(rows[0], "Release  . . . . : V1R0M0 build abc1234");
        assert_eq!(rows[1], "Slot . . . . . . : A (default A)");
        assert!(rows.contains(&"Network open . . : 10.0.2.15/24 on eth0, gateway 10.0.2.2".into()));
        assert!(rows.contains(&"Network private  : none configured".into()));
        assert!(rows.contains(&"Uptime . . . . . : 00:01:01".into()));
        assert!(rows.last().unwrap().ends_with("ready, 2 failures"));
        assert!(rows.iter().all(|r| r.len() <= 78));
    }
}
