// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! The console's own problem table (`docs/SERVICE.md`): for every
//! attention and fatal code of the console (`docs/REFCODES.md`), whether
//! it opens a problem in the console's log, and when it does, the
//! message that describes it, the kinds of resource it names and the
//! progress codes that clear it. Only the console's own conditions are
//! here: a managed system keeps its own log.
//!
//! A message's values are the names of the resources the posting is
//! about, one for each kind the row names and in that order, then the
//! text the code was posted with.

#![forbid(unsafe_code)]

use msg::Message;
use problem::table::{Condition, Messages, Opening, Rules};
use qmcmsg::base as m;
use supervise::control::about::{CODE_SLOT, DISK_UNIT, NETWORK_INTERFACE, SERVICE, SYSTEM};

pub use problem::Severity;

/// The severity of a problem the code `code` opened: an attention code
/// (`A3`) the console carries on past, a fatal one (`B3`) it parks for.
#[must_use]
pub fn severity(code: &str) -> Severity {
    if code.starts_with("B3") {
        Severity::Terminal
    } else {
        Severity::Attention
    }
}

const fn opens(
    code: &'static str,
    message: &'static Message,
    resources: &'static [&'static str],
    cleared_by: &'static [&'static str],
) -> Condition {
    Condition {
        code,
        opens: Some(Opening {
            message,
            resources,
            cleared_by,
            system_code: false,
        }),
        note: "",
    }
}

const fn none(code: &'static str, note: &'static str) -> Condition {
    Condition {
        code,
        opens: None,
        note,
    }
}

/// Every attention and fatal code of the console, in the code's order.
pub const CONDITIONS: &[Condition] = &[
    opens("A3001005", &m::PRB_PANICKED, &[], &[]),
    opens("A3001010", &m::PRB_NOT_MOUNTED, &[], &[]),
    opens("A3001015", &m::PRB_PROCESSOR, &[], &[]),
    opens("A3001030", &m::PRB_RECORD, &[], &[]),
    opens("A3001035", &m::PRB_SLOT_FELL_BACK, &[CODE_SLOT], &[]),
    opens("A3001040", &m::PRB_DATA_NOT_MOUNTED, &[], &["C3001040"]),
    opens("A3001045", &m::PRB_RESTORE_TAKEN_BACK, &[], &[]),
    none(
        "A3001046",
        "A restore refused changed nothing: Backup and restore and the history say why.",
    ),
    opens("A3001050", &m::PRB_CLOCK, &[], &["C3001050", "C3002030"]),
    none(
        "A3001060",
        "A seed is the operator's: the console says why it was refused, and nothing was written.",
    ),
    opens("A3001070", &m::PRB_STATE, &[], &[]),
    opens("A3001075", &m::PRB_IDENTITY, &[], &["C3001075"]),
    opens("A3001076", &m::PRB_MGMT_NOT_SHARED, &[], &[]),
    opens("A3001080", &m::PRB_SERVICE_STOPPED, &[SERVICE], &[]),
    opens("A3001095", &m::PRB_BOOT_ORDER, &[CODE_SLOT], &[]),
    opens("A3002005", &m::PRB_NO_INTERFACE, &[], &["C3002010"]),
    opens(
        "A3002010",
        &m::PRB_NO_CARRIER,
        &[NETWORK_INTERFACE],
        &["C3002010"],
    ),
    opens(
        "A3002020",
        &m::PRB_NO_ADDRESS,
        &[NETWORK_INTERFACE],
        &["C3002020"],
    ),
    opens("A3003010", &m::PRB_SET, &[], &["C3003010"]),
    opens(
        "A3003020",
        &m::PRB_UNIT_FAILED,
        &[DISK_UNIT],
        &["C3003025", "C3003035"],
    ),
    opens(
        "A3003030",
        &m::PRB_REBUILD,
        &[DISK_UNIT],
        &["C3003025", "C3003035"],
    ),
    opens("A3003040", &m::PRB_APART, &[], &["C3003050"]),
    opens("A3003070", &m::PRB_BOOT_ENTRIES, &[], &[]),
    none(
        "A3004010",
        "The installer has no problem log: its screens and its log say what failed.",
    ),
    none(
        "A3004015",
        "The installer has no problem log: its screens and its log say what failed.",
    ),
    none(
        "A3004070",
        "The installer has no problem log: its screens and its log say what failed.",
    ),
    none(
        "A3004090",
        "The installer has no problem log: its screens and its log say what failed.",
    ),
    opens("A3005010", &m::PRB_MGMT_STATE, &[], &[]),
    opens("A3005020", &m::PRB_CERT_CHANGED, &[SYSTEM], &[]),
    opens("A3005030", &m::PRB_NOT_REACHED, &[SYSTEM], &["C3005040"]),
    opens("A3006010", &m::PRB_DATA_FULL, &[], &["C3006010"]),
    opens("A3006040", &m::PRB_DUMP_FAILED, &[], &[]),
    opens("A3006050", &m::PRB_CERT_ENDING, &[], &["C3006050"]),
    none(
        "A3007010",
        "An image refused is answered to whoever staged it, with why, and is not kept.",
    ),
    opens(
        "A3007030",
        &m::PRB_INSTALL_FAILED,
        &[CODE_SLOT],
        &["C3007040"],
    ),
    none(
        "A3007100",
        "Code updates says why a stream was refused, and nothing was taken from it.",
    ),
    none(
        "A3007105",
        "Code updates says the server was not reached; nothing changed.",
    ),
    opens("A3007110", &m::PRB_DOWNLOAD_BAD, &[], &["C3007110"]),
    opens("B3001000", &m::PRB_PID1, &[], &[]),
    opens("B3003000", &m::PRB_STORAGE_LOST, &[], &[]),
];

/// QS/MC's rules for its own problem log (`base/problem`).
pub const RULES: Rules = Rules {
    conditions: CONDITIONS,
    severity,
    messages: Messages {
        no_problem: &m::PRB_NO_PROBLEM,
        closed: &m::PRB_CLOSED,
        holds: &m::PRB_HOLDS,
        not_kept: &m::PRB_NOT_KEPT,
        note_invalid: &m::PRB_NOTE_INVALID,
    },
};

#[cfg(test)]
mod tests {
    use super::*;
    use refcode::Family;

    /// Every attention and fatal code has its row, once, and every code a
    /// row names is one of the table's: what clears is a progress code.
    #[test]
    fn the_table_holds_every_code_of_the_console() {
        for c in refcode::ALL {
            if matches!(c.family, Family::Attention | Family::Fatal) {
                assert!(RULES.condition(c.code).is_some(), "{} has no row", c.code);
            }
        }
        for (i, row) in CONDITIONS.iter().enumerate() {
            assert!(CONDITIONS[..i].iter().all(|o| o.code != row.code));
            let code = refcode::find(row.code).unwrap_or_else(|| panic!("{}", row.code));
            assert!(matches!(code.family, Family::Attention | Family::Fatal));
            match &row.opens {
                Some(o) => {
                    assert!(o.message.id.starts_with("QMC73"), "{}", row.code);
                    for clear in o.cleared_by {
                        let c = refcode::find(clear).unwrap_or_else(|| panic!("{clear}"));
                        assert_eq!(c.family, Family::Progress, "{clear}");
                    }
                    // A message names each resource, then the posted text.
                    assert!(o.message.values() <= o.resources.len() + 1, "{}", row.code);
                }
                None => assert!(!row.note.is_empty(), "{}", row.code),
            }
        }
    }

    /// The conditions the phase names each open a problem, with the
    /// resource a person finds it by.
    #[test]
    fn the_conditions_of_the_console_open_their_problems() {
        let names = |code: &str| -> &'static [&'static str] {
            RULES
                .condition(code)
                .and_then(|c| c.opens.as_ref())
                .unwrap_or_else(|| panic!("{code} opens nothing"))
                .resources
        };
        assert_eq!(names(refcode::UNIT_FAILED.code), [DISK_UNIT]);
        assert_eq!(names(refcode::UNITS_APART.code), [] as [&str; 0]);
        assert_eq!(names(refcode::SERVICE_STOPPED.code), [SERVICE]);
        assert_eq!(names(refcode::SLOT_FELL_BACK.code), [CODE_SLOT]);
        assert_eq!(names(refcode::DATA_NEARLY_FULL.code), [] as [&str; 0]);
        assert_eq!(names(refcode::PROCESSOR_LACKS.code), [] as [&str; 0]);
        assert_eq!(names(refcode::CERT_ENDING.code), [] as [&str; 0]);
        assert_eq!(names(refcode::SYSTEM_NOT_REACHED.code), [SYSTEM]);
        assert_eq!(names(refcode::UPDATE_INSTALL_FAILED.code), [CODE_SLOT]);
        assert_eq!(names(refcode::DOWNLOAD_BAD.code), [] as [&str; 0]);
        assert_eq!(names(refcode::KERNEL_PANICKED.code), [] as [&str; 0]);
    }

    #[test]
    fn a_fatal_code_is_terminal() {
        assert_eq!((RULES.severity)("B3003000"), Severity::Terminal);
        assert_eq!((RULES.severity)("A3003020"), Severity::Attention);
    }

    /// The table opens, counts, clears and closes by the codes posted,
    /// as `base/problem` takes them.
    #[test]
    fn a_unit_pulled_opens_is_seen_again_and_clears() {
        use problem::Posted;
        use problem::book::{Book, Posting};
        use supervise::control::About;
        let dir = std::env::temp_dir().join(format!("qsmcprobd-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut book = Book::open(&dir);
        let unit = About {
            kind: DISK_UNIT.into(),
            name: "2".into(),
            serial: "S2".into(),
            ..About::default()
        };
        let post = |seq: u64, code: &str, text: &str, about: Vec<About>| Posting {
            posted: Posted {
                seq,
                code: code.into(),
                words: Vec::new(),
                text: text.into(),
                time_ms: (1_800_000_000 + seq) * 1000,
            },
            about,
        };
        let opened = book.take(
            &RULES,
            &post(
                1,
                "A3003020",
                "Load-source unit failed: unit 2",
                vec![unit.clone()],
            ),
            1,
        );
        assert_eq!(opened.len(), 1);
        book.take(
            &RULES,
            &post(
                2,
                "A3003020",
                "Load-source unit failed: unit 2",
                vec![unit.clone()],
            ),
            2,
        );
        book.take(
            &RULES,
            &post(3, "C3003035", "Rebuild finished", vec![unit]),
            3,
        );
        let page = book.list(&problem::Filter::default(), None, 10);
        assert_eq!(page.problems.len(), 1);
        let p = &page.problems[0];
        assert_eq!((p.count, p.holds), (2, false));
        assert_eq!(p.msgid, "QMC7328");
        assert_eq!(p.description, "Load-source unit 2 is missing or failed.");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
