// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! The prototype's serviceability: its serviceable events (1042 open,
//! `B6000100` on virtual disk VD04 of QSTEST01; 1039 to 1041 closed),
//! its dumps, the host's code slots and the fix levels the Updates page
//! shows.

use qsi::base::{DumpKind, Level, Refcode, SystemDump};
use qsi::host::dumps::DumpInfo;
use qsi::service::{
    Line, Problem, ProblemStatus, ResourceKind, Severity, Source, SourceKind, Summary,
};

use crate::system::System;
use crate::system::servicing::{resource, slot};

/// 2026-10-29 22:14:05 UTC, the design's time of event 1042.
const T1042: u64 = 1_793_312_045;

#[allow(
    clippy::too_many_arguments,
    reason = "a problem's fields, as the design shows them"
)]
fn problem(
    number: u64,
    open: bool,
    time: u64,
    code: &str,
    partition: Option<(u32, &str)>,
    description: &str,
    msgid: &str,
    severity: Severity,
) -> (Severity, Problem) {
    let source = partition.map_or(
        Source {
            kind: SourceKind::System,
            id: 0,
            name: String::new(),
        },
        |(id, name)| Source {
            kind: SourceKind::Partition,
            id,
            name: name.into(),
        },
    );
    (
        severity,
        Problem {
            summary: Summary {
                number,
                status: if open {
                    ProblemStatus::Open
                } else {
                    ProblemStatus::Closed
                },
                first_seen: time,
                last_seen: time,
                count: 1,
                refcode: Refcode {
                    seq: 0,
                    code: code.into(),
                    words: if code == "B6000100" {
                        vec!["00000002".into(), "4B4F4E53".into()]
                    } else {
                        Vec::new()
                    },
                    text: description.into(),
                    time_ms: time * 1000,
                },
                source,
                description: description.into(),
                msgid: msgid.into(),
                holds: open,
                closed_by: if open {
                    String::new()
                } else {
                    "qsadmin".into()
                },
                closed_at: if open { 0 } else { time + 600 },
            },
            resources: Vec::new(),
            log: vec![Line {
                time,
                text: format!("Opened by {code}: {description}"),
            }],
        },
    )
}

/// QSHQVS1's: events 1039, 1040 and 1042, its dumps and slots.
pub fn host(s: &mut System) {
    s.servicing.slots = vec![slot("A", "V1R0M0", "26280", true), slot("B", "", "", false)];
    let mut e1042 = problem(
        1042,
        true,
        T1042,
        "B6000100",
        Some((2, "QSTEST01")),
        "Virtual disk VD04 backing file read error; partition IPL stopped",
        "QVS5119",
        Severity::Terminal,
    );
    e1042.1.resources = vec![resource(
        ResourceKind::VirtualDisk,
        "virtual disk VD04 in pool POOL1",
        "QSHQVS1 POOL1 /pool1/VD04.img",
        "VD04",
    )];
    e1042.1.log.extend([
        Line {
            time: T1042,
            text: "read error /pool1/VD04.img offset 0x34FE0000: I/O error".into(),
        },
        Line {
            time: T1042 + 2,
            text: "Serviceable event 1042 opened; partition left Not activated".into(),
        },
    ]);
    s.servicing.problems = vec![
        problem(
            1039,
            false,
            T1042 - 27 * 86_400,
            "A6005008",
            Some((2, "QSTEST01")),
            "Console not connected within 10 minutes of activation (informational)",
            "QVS5142",
            Severity::Information,
        ),
        problem(
            1040,
            false,
            T1042 - 11 * 86_400,
            "11007611",
            None,
            "Power supply 2 lost input power (restored 08:06)",
            "QVS5107",
            Severity::Attention,
        ),
        e1042,
    ];
    s.servicing.next = 1043;
    s.servicing.dumps = vec![SystemDump {
        name: "SPDUMP.QSHQVS1.20261018.080450".into(),
        kind: DumpKind::ServiceData,
        kind_name: "service data".into(),
        bytes: 42 << 20,
        time: T1042 - 11 * 86_400,
        by: "PID 1".into(),
    }];
    let dump = |name: &str, mib: u64, time: u64, reason: &str| DumpInfo {
        name: name.into(),
        time,
        bytes: mib << 20,
        allocated: (mib << 20) / 2,
        code: "B6000100".into(),
        reason: reason.into(),
    };
    for p in &mut s.parts {
        match p.definition.id {
            1 => {
                p.dumps = vec![dump(
                    "SYSDUMP.QSDEV01.20261014.184012",
                    4100,
                    T1042 - 15 * 86_400,
                    "function22",
                )];
                p.status.channel.fix_level = "QS400-CUM-26270".into();
                p.status.channel.release = "V1R2M0".into();
            }
            2 => {
                p.dumps = vec![dump(
                    "SYSDUMP.QSTEST01.20261029.221405",
                    8210,
                    T1042 + 60,
                    "request",
                )];
            }
            3 => {
                p.status.channel.fix_level = "QS400-CUM-26290".into();
                p.status.channel.release = "V1R2M0".into();
            }
            _ => {}
        }
    }
}

/// QSHQ400's: event 1041, its firmware dump, its fix level.
pub fn standalone(s: &mut System) {
    s.servicing.problems = vec![problem(
        1041,
        false,
        T1042 - 4 * 86_400,
        "B1811001",
        None,
        "Scheduled power off failed: service interface password rejected",
        "CPF1E99",
        Severity::Attention,
    )];
    s.servicing.next = 1042;
    s.servicing.dumps = vec![SystemDump {
        name: "FWDUMP.QSHQ400.20260930.120000".into(),
        kind: DumpKind::ServiceData,
        kind_name: "firmware".into(),
        bytes: 120 << 20,
        time: T1042 - 29 * 86_400,
        by: "QSYSOPR".into(),
    }];
    s.servicing.fix_level = Some(Level {
        level: "QS400-CUM-26290".into(),
        date: T1042,
    });
}
