// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! Console reference codes. One table, no scattered strings.
//!
//! Every code is eight characters, a family and six digits. A console's
//! families end in `3` (docs/REFCODES.md), which no family of QS/400 or
//! of QS/VS does: `C3` IPL and progress, `A3` attention, `B3` fatal,
//! `D3` power down.
//!
//! - [`event`]: the code of each event the base crates report.
//! - [`post`]: the poster: the console line.

#![forbid(unsafe_code)]

pub mod event;
pub mod post;

/// What a code's family says of it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Family {
    /// `C3`: the console's IPL and its progress.
    Progress,
    /// `A3`: attention, the console continues.
    Attention,
    /// `B3`: fatal, the console parks.
    Fatal,
    /// `D3`: power down.
    PowerDown,
}

impl Family {
    /// The two characters a code of the family starts with.
    pub const fn prefix(self) -> &'static str {
        match self {
            Family::Progress => "C3",
            Family::Attention => "A3",
            Family::Fatal => "B3",
            Family::PowerDown => "D3",
        }
    }
}

/// One reference code.
#[derive(Debug, PartialEq, Eq)]
pub struct RefCode {
    /// Eight-character code.
    pub code: &'static str,
    /// Short text.
    pub text: &'static str,
    /// Family.
    pub family: Family,
}

const fn progress(code: &'static str, text: &'static str) -> RefCode {
    RefCode {
        code,
        text,
        family: Family::Progress,
    }
}

const fn attention(code: &'static str, text: &'static str) -> RefCode {
    RefCode {
        code,
        text,
        family: Family::Attention,
    }
}

const fn fatal(code: &'static str, text: &'static str) -> RefCode {
    RefCode {
        code,
        text,
        family: Family::Fatal,
    }
}

// C3001xxx: PID 1's own IPL, in the order it is posted.

/// PID 1 started the console's IPL.
pub const IPL_STARTED: RefCode = progress("C3001000", "Console IPL started");
/// The heartbeat, on the serial port only.
pub const HEARTBEAT: RefCode = progress("C3001005", "Console running");
/// proc, sysfs, devtmpfs, `/run` and cgroup2 are mounted.
pub const MOUNTED: RefCode = progress("C3001010", "Kernel file systems mounted");

// A3001xxx: attention; the console continues. Numbered after the step.

/// A kernel file system did not mount.
pub const NOT_MOUNTED: RefCode = attention("A3001010", "Kernel file systems not mounted");
/// The processor lacks what the cryptography of QSI and the web server
/// needs.
pub const PROCESSOR_LACKS: RefCode =
    attention("A3001015", "Processor lacks what the cryptography needs");

// B3001xxx: fatal; the console parks.

/// PID 1 panicked.
pub const PANIC: RefCode = fatal("B3001000", "Console PID 1 failed");

/// Every code, in the order docs/REFCODES.md lists them.
pub const ALL: &[&RefCode] = &[
    &IPL_STARTED,
    &HEARTBEAT,
    &MOUNTED,
    &NOT_MOUNTED,
    &PROCESSOR_LACKS,
    &PANIC,
];

/// The word a posted line starts with.
pub const WORD: &str = "REF";

/// The code spelled `code`, when the table has it.
pub fn find(code: &str) -> Option<&'static RefCode> {
    ALL.iter().copied().find(|c| c.code == code)
}

impl RefCode {
    /// `REF C3001000`, what a reader of the console looks for.
    pub fn tag(&self) -> String {
        format!("{WORD} {}", self.code)
    }

    /// The line as posted: `REF C3001000 Console IPL started`.
    pub fn line(&self) -> String {
        format!("{WORD} {} {}", self.code, self.text)
    }

    /// The text with what this posting adds: `...: /run`.
    pub fn text_with(&self, detail: &str) -> String {
        if detail.is_empty() {
            self.text.to_string()
        } else {
            format!("{}: {detail}", self.text)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// QS/400's families (`lic/slic/src/src.rs` there, at the pinned
    /// commit) and QS/VS's (`lic/refcode/src/lib.rs` there).
    const QS400_FAMILIES: &[&str] = &["C1", "C6", "A6", "B1", "D1"];
    const QSVS_FAMILIES: &[&str] = &["C2", "A2", "B2", "D2"];

    #[test]
    fn codes_are_well_formed_and_unique() {
        for (i, c) in ALL.iter().enumerate() {
            assert_eq!(c.code.len(), 8, "{}", c.code);
            assert!(c.code.starts_with(c.family.prefix()), "{}", c.code);
            assert!(
                c.code[2..].bytes().all(|b| b.is_ascii_digit()),
                "{}",
                c.code
            );
            assert!(ALL[..i].iter().all(|o| o.code != c.code), "{}", c.code);
            assert!(!c.text.is_empty() && !c.text.contains(':'), "{}", c.code);
        }
    }

    #[test]
    fn no_code_is_in_a_qs400_or_qsvs_family() {
        for c in ALL {
            let family = &c.code[..2];
            assert!(!QS400_FAMILIES.contains(&family), "{}", c.code);
            assert!(!QSVS_FAMILIES.contains(&family), "{}", c.code);
            assert_eq!(&family[1..], "3", "{}", c.code);
        }
        // And no family of either ends in `3`.
        for f in QS400_FAMILIES.iter().chain(QSVS_FAMILIES) {
            assert!(!f.ends_with('3'), "{f}");
        }
    }

    #[test]
    fn a_line_is_the_word_the_code_and_the_text() {
        assert_eq!(IPL_STARTED.line(), "REF C3001000 Console IPL started");
        assert_eq!(HEARTBEAT.tag(), "REF C3001005");
        assert_eq!(
            NOT_MOUNTED.text_with("/run"),
            "Kernel file systems not mounted: /run"
        );
        assert_eq!(MOUNTED.text_with(""), "Kernel file systems mounted");
        assert_eq!(find("B3001000"), Some(&PANIC));
        assert_eq!(find("C2001000"), None);
    }

    /// docs/REFCODES.md lists every code with its text, and no other.
    #[test]
    fn the_document_lists_the_table() {
        let doc = include_str!("../../../docs/REFCODES.md");
        for c in ALL {
            let row = format!("| `{}` | {} |", c.code, c.text);
            assert!(doc.contains(&row), "docs/REFCODES.md lacks: {row}");
        }
        let is_code_row = |l: &&str| {
            l.starts_with("| `")
                && l.get(11..12) == Some("`")
                && l[5..11].bytes().all(|b| b.is_ascii_digit())
        };
        assert_eq!(doc.lines().filter(is_code_row).count(), ALL.len());
    }
}
