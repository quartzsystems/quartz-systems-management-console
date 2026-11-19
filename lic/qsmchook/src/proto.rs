// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! The channel's lines. The harness writes one command a line; the
//! channel answers with any number of lines and then [`END`] alone.
//! An answer's line is a word and its text; file contents travel as
//! hex, so nothing in them can look like a line of the protocol.

use stgd::{Status as Storage, Unit};
use supervise::control::{HostStatus, ServiceStatus};

/// The line that ends an answer.
pub const END: &str = ".";
/// The serial port the channel is on.
pub const PORT: &str = "/dev/ttyS1";

/// The commands.
pub mod command {
    /// `ping <text>`: answers `pong <text>`.
    pub const PING: &str = "ping";
    /// `status`: the control socket's status, a `fact` or `service` a line.
    pub const STATUS: &str = "status";
    /// `kill <service>`: `SIGKILL` to the service's process;
    /// `kill init`: `SIGTERM` to PID 1.
    pub const KILL: &str = "kill";
    /// The name PID 1 goes by in `kill`.
    pub const INIT: &str = "init";
    /// `cat <path>`: the file, as `hex`.
    pub const CAT: &str = "cat";
    /// `put <path> <hex>`: write a file atomically.
    pub const PUT: &str = "put";
    /// `rm <path>`: remove a file.
    pub const RM: &str = "rm";
    /// `ls <directory>`: a `name` a line.
    pub const LS: &str = "ls";
    /// `stat <path>`: `ok <mode in octal> <uid> <gid> <bytes>`.
    pub const STAT: &str = "stat";
    /// `kmsg <text>`: the kernel log's lines that contain the text.
    pub const KMSG: &str = "kmsg";
    /// `poweroff`: the control socket's power off.
    pub const POWER_OFF: &str = "poweroff";
    /// `restart`: the control socket's restart.
    pub const RESTART: &str = "restart";
    /// `sethost <name>`: the control socket's `set_host` with the name,
    /// the zone and the time servers kept.
    pub const SET_HOST: &str = "sethost";
    /// `netreload`: the network service's reload.
    pub const NET_RELOAD: &str = "netreload";
    /// `setclock <secs>`: the network service sets the clock and the RTC.
    pub const SET_CLOCK: &str = "setclock";
    /// `storage`: the storage service's status, a `fact` or `unit` a line.
    pub const STORAGE: &str = "storage";
    /// `keep <disk>`: of units that ran apart, keep the one on the disk.
    pub const KEEP: &str = "keep";
    /// `load <megabytes>`: write and sync a file of that size under
    /// `/data`, again and again, until `load 0`.
    pub const LOAD: &str = "load";
    /// `sum <path>`: a file's length and the sum of its bytes.
    pub const SUM: &str = "sum";
}

/// The first words of an answer's lines.
pub mod word {
    /// A fact of the status: `fact <key>=<value>`.
    pub const FACT: &str = "fact";
    /// A service of the status: `service <name> <state> <pid> <failures>`.
    pub const SERVICE: &str = "service";
    /// A load-source unit: `unit <number>|<disk>|<serial>|<location>|<bytes>|<state>|<written>`.
    pub const UNIT: &str = "unit";
    /// File contents in hex.
    pub const HEX: &str = "hex";
    /// A directory entry.
    pub const NAME: &str = "name";
    /// A line of the kernel log.
    pub const LINE: &str = "line";
    /// The command was done.
    pub const OK: &str = "ok";
    /// The command failed, and why.
    pub const ERROR: &str = "error";
}

/// Where the load writes, under `/data`.
pub const LOAD_DIR: &str = "/data/load";

/// A status as the lines of an answer.
pub fn status_lines(status: &HostStatus) -> Vec<String> {
    let facts = status
        .facts
        .iter()
        .map(|(k, v)| format!("{} {k}={v}", word::FACT));
    let services = status.services.iter().map(|s| {
        format!(
            "{} {} {} {} {}",
            word::SERVICE,
            s.name,
            s.state,
            s.pid,
            s.failures
        )
    });
    facts.chain(services).collect()
}

/// The status in the lines of an answer.
pub fn parse_status(lines: &[String]) -> Result<HostStatus, String> {
    let mut status = HostStatus::default();
    for line in lines {
        let bad = || format!("not a line of a status: {line}");
        match line.split_once(' ') {
            Some((word::FACT, rest)) => {
                let (key, value) = rest.split_once('=').ok_or_else(bad)?;
                status.facts.push((key.to_string(), value.to_string()));
            }
            Some((word::SERVICE, rest)) => {
                let fields: Vec<&str> = rest.split(' ').collect();
                let [name, state, pid, failures] = fields.as_slice() else {
                    return Err(bad());
                };
                status.services.push(ServiceStatus {
                    name: (*name).to_string(),
                    state: (*state).to_string(),
                    pid: pid.parse().map_err(|_| bad())?,
                    failures: failures.parse().map_err(|_| bad())?,
                });
            }
            Some((word::ERROR, why)) => return Err(why.to_string()),
            _ => return Err(bad()),
        }
    }
    Ok(status)
}

/// The storage service's status as the lines of an answer.
pub fn storage_lines(status: &Storage) -> Vec<String> {
    let fact = |key: &str, value: &str| format!("{} {key}={value}", word::FACT);
    let mut lines = vec![
        fact("protection", &status.protection),
        fact("device", &status.device),
        fact("work", &status.work),
        fact("work_unit", &status.work_unit.to_string()),
        fact("percent", &status.percent.to_string()),
        fact("restart_needed", &status.restart_needed.to_string()),
        fact("failure", &status.failure),
        fact("choose", &status.choose.to_string()),
    ];
    lines.extend(status.units.iter().map(|u| {
        format!(
            "{} {}|{}|{}|{}|{}|{}|{}",
            word::UNIT,
            u.number,
            u.disk,
            u.serial,
            u.location,
            u.bytes,
            u.state,
            u.written
        )
    }));
    lines
}

/// The storage service's status in the lines of an answer.
pub fn parse_storage(lines: &[String]) -> Result<Storage, String> {
    let mut status = Storage::default();
    for line in lines {
        let bad = || format!("not a line of the storage status: {line}");
        match line.split_once(' ') {
            Some((word::FACT, rest)) => {
                let (key, value) = rest.split_once('=').ok_or_else(bad)?;
                match key {
                    "protection" => status.protection = value.into(),
                    "device" => status.device = value.into(),
                    "work" => status.work = value.into(),
                    "work_unit" => status.work_unit = value.parse().map_err(|_| bad())?,
                    "percent" => status.percent = value.parse().map_err(|_| bad())?,
                    "restart_needed" => status.restart_needed = value == "true",
                    "failure" => status.failure = value.into(),
                    "choose" => status.choose = value == "true",
                    _ => return Err(bad()),
                }
            }
            Some((word::UNIT, rest)) => {
                let fields: Vec<&str> = rest.split('|').collect();
                let [number, disk, serial, location, bytes, state, written] = fields.as_slice()
                else {
                    return Err(bad());
                };
                status.units.push(Unit {
                    number: number.parse().map_err(|_| bad())?,
                    disk: (*disk).to_string(),
                    serial: (*serial).to_string(),
                    location: (*location).to_string(),
                    bytes: bytes.parse().map_err(|_| bad())?,
                    state: (*state).to_string(),
                    written: written.parse().map_err(|_| bad())?,
                });
            }
            Some((word::ERROR, why)) => return Err(why.to_string()),
            _ => return Err(bad()),
        }
    }
    Ok(status)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_status_crosses_the_channel_whole() {
        let status = HostStatus {
            facts: vec![
                ("slot".into(), "A".into()),
                (
                    "management".into(),
                    "10.0.2.15/24 on eth0, gateway 10.0.2.2".into(),
                ),
                (
                    "reference".into(),
                    "C3001090 Base ready: 0.8 s = fast".into(),
                ),
            ],
            services: vec![ServiceStatus {
                name: "netd".into(),
                state: "restarting".into(),
                pid: 0,
                failures: 3,
            }],
        };
        let lines = status_lines(&status);
        assert_eq!(lines[0], "fact slot=A");
        assert_eq!(lines[3], "service netd restarting 0 3");
        assert_eq!(parse_status(&lines), Ok(status));
        assert_eq!(
            parse_status(&["error no control socket".into()]),
            Err("no control socket".into())
        );
        assert!(parse_status(&["service netd".into()]).is_err());
    }

    #[test]
    fn the_storage_status_crosses_the_channel_whole() {
        let status = Storage {
            protection: "mirrored".into(),
            device: "/dev/ublkb0".into(),
            units: vec![Unit {
                number: 2,
                disk: String::new(),
                serial: "QSMCHD1".into(),
                location: String::new(),
                bytes: 0,
                state: "Missing".into(),
                written: 1_794_998_523,
            }],
            work: String::new(),
            work_unit: 0,
            percent: 0,
            restart_needed: false,
            failure: String::new(),
            choose: false,
        };
        let lines = storage_lines(&status);
        assert_eq!(lines[8], "unit 2||QSMCHD1||0|Missing|1794998523");
        assert_eq!(parse_storage(&lines), Ok(status));
        assert!(parse_storage(&["unit 2".into()]).is_err());
    }
}
