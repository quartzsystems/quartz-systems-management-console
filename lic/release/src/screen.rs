// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! The status screen: what the control socket's status says, a row a
//! fact. PID 1 draws it as plain text on the serial port while nobody
//! has a session there; the local console shows the same rows as its
//! idle screen.

use supervise::control::HostStatus;

use crate::fact;

/// The line before a status screen on the serial port.
pub const BEGIN: &str = "---- status ----";
/// The line after it.
pub const END: &str = "---- end ----";
/// The slot row's label.
pub const SLOT: &str = "Slot";
/// The host name row's label.
pub const HOST: &str = "Host name";
/// The `open` interface's row label.
pub const MANAGEMENT: &str = "Open";
/// The `private` interface's row label.
pub const PRIVATE: &str = "Private";
/// The date and time row's label.
pub const DATE: &str = "Date and time";
/// The load source row's label.
pub const LOAD_SOURCE: &str = "Load source";
/// The reference code row's label.
pub const REFERENCE: &str = "Reference";
/// The processor row's label.
pub const PROCESSOR: &str = "Processor";
/// The machine row's label.
pub const MACHINE: &str = "Machine";
/// The signature row's label.
pub const SIGNATURE: &str = "Signature";
/// The label of a service's row.
pub const SERVICE: &str = "Service";

/// One row: the label in its column, then the value.
pub fn row(label: &str, value: &str) -> String {
    format!("{label:<14} {value}")
}

/// A service's row: its name in its column, then its state.
pub fn service(name: &str, state: &str) -> String {
    row(SERVICE, &format!("{name:<10} {state}"))
}

/// The rows of the screen: the label and the fact it shows.
const ROWS: [(&str, &str); 10] = [
    (SLOT, fact::SLOT),
    (HOST, fact::HOST),
    (MANAGEMENT, fact::MANAGEMENT),
    (PRIVATE, fact::PRIVATE),
    (DATE, fact::DATE),
    (LOAD_SOURCE, fact::LOAD_SOURCE),
    (REFERENCE, fact::REFERENCE),
    (PROCESSOR, fact::PROCESSOR),
    (MACHINE, fact::MACHINE),
    (SIGNATURE, fact::SIGNATURE),
];

/// The screen for `status`, under `banner`: the facts, then a line a
/// service.
pub fn lines(banner: &str, status: &HostStatus) -> Vec<String> {
    let mut out = vec![banner.to_string()];
    for (label, key) in ROWS {
        let mut value = status.fact(key).unwrap_or("").to_string();
        if key == fact::SLOT
            && let Some(record) = status.fact(fact::RECORD)
        {
            value = format!("{value} ({record})");
        }
        out.push(row(label, &value));
    }
    for one in &status.services {
        let failures = match one.failures {
            0 => String::new(),
            1 => ", 1 failure".to_string(),
            n => format!(", {n} failures"),
        };
        out.push(service(&one.name, &format!("{}{failures}", one.state)));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use supervise::control::ServiceStatus;

    #[test]
    fn the_screen_is_the_banner_ten_facts_and_a_line_a_service() {
        let fact = |k: &str, v: &str| (k.to_string(), v.to_string());
        let status = HostStatus {
            facts: vec![
                fact(fact::SLOT, "A"),
                fact(fact::RECORD, "default A, nothing pending"),
                fact(fact::HOST, "QSMC123456"),
                fact(fact::MANAGEMENT, "10.0.2.15/24 on eth0, gateway 10.0.2.2"),
                fact(fact::PRIVATE, "192.0.2.1/24 on eth1"),
                fact(fact::DATE, "2026-11-18 10:22 UTC"),
                fact(fact::LOAD_SOURCE, "mirrored: unit 1 Active, unit 2 Active"),
                fact(fact::REFERENCE, "C3001090 Base ready"),
                fact(fact::PROCESSOR, "has what the cryptography needs"),
                fact(fact::MACHINE, "serial M525400123456 (derived), 2 threads"),
                fact(fact::SIGNATURE, "release key, Secure Boot on"),
                fact(fact::TIME, "1794998523"),
            ],
            services: vec![
                ServiceStatus {
                    name: "logd".into(),
                    state: "ready".into(),
                    pid: 2,
                    failures: 0,
                },
                ServiceStatus {
                    name: "netd".into(),
                    state: "restarting".into(),
                    pid: 0,
                    failures: 2,
                },
            ],
        };
        assert_eq!(
            lines("QS/MC V1R0M0 abc1234", &status),
            [
                "QS/MC V1R0M0 abc1234",
                "Slot           A (default A, nothing pending)",
                "Host name      QSMC123456",
                "Open           10.0.2.15/24 on eth0, gateway 10.0.2.2",
                "Private        192.0.2.1/24 on eth1",
                "Date and time  2026-11-18 10:22 UTC",
                "Load source    mirrored: unit 1 Active, unit 2 Active",
                "Reference      C3001090 Base ready",
                "Processor      has what the cryptography needs",
                "Machine        serial M525400123456 (derived), 2 threads",
                "Signature      release key, Secure Boot on",
                "Service        logd       ready",
                "Service        netd       restarting, 2 failures",
            ]
        );
    }
}
