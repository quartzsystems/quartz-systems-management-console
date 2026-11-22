// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! Every managed system's problems as the console has seen them, each
//! with a number of the console's own (`docs/SERVICE.md`): one list
//! across the fleet, its numbers ascending from 1 and never used again,
//! kept on disk (`problems.bin` in `release::MGMT_DIR`) so a number is
//! the same across a restart and a reconnection. The system keeps the
//! problem; this is what the console knows of it, and nothing is made of
//! it but the row.

use std::fs;
use std::path::{Path, PathBuf};

use ipc::wire_struct;
use mgmtproto::calls::{ProblemFilter, ProblemPage, ProblemRow};
use qsi::service::{ProblemStatus, Severity, Summary};

/// The file, under the service's folder.
pub const FILE: &str = "problems.bin";

/// What the file holds.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Book {
    /// The number the next problem seen takes.
    next: u64,
    /// Every row, oldest number first.
    rows: Vec<ProblemRow>,
}
wire_struct!(Book { next, rows });

/// The problems of every system.
#[derive(Debug)]
pub struct Problems {
    path: Option<PathBuf>,
    most: usize,
    book: Book,
}

impl Problems {
    /// The problems kept in `dir`, at most `most` of them; in memory
    /// when there is no folder. A file that does not read is set aside.
    pub fn open(dir: Option<&Path>, most: usize) -> Self {
        let path = dir.map(|d| d.join(FILE));
        let book = path
            .as_ref()
            .and_then(|p| fs::read(p).ok().map(|b| (p, b)))
            .and_then(|(p, b)| {
                ipc::decode::<Book>(&b).ok().or_else(|| {
                    let _ = fs::rename(p, p.with_extension("bad"));
                    None
                })
            })
            .unwrap_or(Book {
                next: 1,
                rows: Vec::new(),
            });
        Self {
            path,
            most: most.max(1),
            book,
        }
    }

    fn save(&self) {
        let Some(p) = &self.path else {
            return;
        };
        let tmp = p.with_extension("new");
        if fs::write(&tmp, ipc::encode(&self.book)).is_ok() {
            let _ = fs::rename(&tmp, p);
        }
    }

    /// Keep `summary` of `system`: the row it had, or a new number. Its
    /// severity is kept unless `severity` says one. The console's number.
    pub fn keep(&mut self, system: &str, severity: Option<Severity>, summary: Summary) -> u64 {
        let theirs = summary.number;
        let at = self
            .book
            .rows
            .iter()
            .position(|r| r.system.eq_ignore_ascii_case(system) && r.summary.number == theirs);
        let number = if let Some(i) = at {
            let r = &mut self.book.rows[i];
            if severity.is_some() {
                r.severity = severity;
            }
            if r.summary != summary {
                r.summary = summary;
            } else if severity.is_none() {
                return r.number;
            }
            r.number
        } else {
            let number = self.book.next;
            self.book.next += 1;
            self.book.rows.push(ProblemRow {
                number,
                system: system.to_string(),
                severity,
                summary,
            });
            self.bound();
            number
        };
        self.save();
        number
    }

    /// Past the most kept, the oldest closed go first, then the oldest.
    fn bound(&mut self) {
        while self.book.rows.len() > self.most {
            let i = self
                .book
                .rows
                .iter()
                .position(|r| r.summary.status == ProblemStatus::Closed)
                .unwrap_or(0);
            self.book.rows.remove(i);
        }
    }

    /// The row of the console's `number`.
    pub fn get(&self, number: u64) -> Option<ProblemRow> {
        self.book.rows.iter().find(|r| r.number == number).cloned()
    }

    /// The console's number of `system`'s problem `theirs`.
    pub fn number_of(&self, system: &str, theirs: u64) -> Option<u64> {
        self.book
            .rows
            .iter()
            .find(|r| r.system.eq_ignore_ascii_case(system) && r.summary.number == theirs)
            .map(|r| r.number)
    }

    /// A page of what `filter` matches, newest number first, after
    /// `skip`, at most `rows`.
    pub fn page(&self, filter: &ProblemFilter, skip: u32, rows: u32) -> ProblemPage {
        let wanted: Vec<&ProblemRow> = self
            .book
            .rows
            .iter()
            .rev()
            .filter(|r| filter.status.is_none_or(|s| r.summary.status == s))
            .filter(|r| filter.system.is_empty() || r.system.eq_ignore_ascii_case(&filter.system))
            .filter(|r| filter.from == 0 || r.summary.last_seen >= filter.from)
            .filter(|r| filter.to == 0 || r.summary.last_seen < filter.to)
            .collect();
        ProblemPage {
            total: u32::try_from(wanted.len()).unwrap_or(u32::MAX),
            rows: wanted
                .into_iter()
                .skip(skip as usize)
                .take(rows as usize)
                .cloned()
                .collect(),
        }
    }

    /// How many are open, every system's.
    pub fn open_count(&self) -> usize {
        self.book
            .rows
            .iter()
            .filter(|r| r.summary.status == ProblemStatus::Open)
            .count()
    }

    /// A system removed: its rows go; their numbers are not used again.
    pub fn forget(&mut self, system: &str) {
        let before = self.book.rows.len();
        self.book
            .rows
            .retain(|r| !r.system.eq_ignore_ascii_case(system));
        if self.book.rows.len() != before {
            self.save();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use qsi::base::Refcode;
    use qsi::service::{Source, SourceKind};

    fn summary(number: u64, status: ProblemStatus, last: u64) -> Summary {
        Summary {
            number,
            status,
            first_seen: last,
            last_seen: last,
            count: 1,
            refcode: Refcode {
                seq: 1,
                code: "A2007020".into(),
                words: Vec::new(),
                text: "Disk unit failed".into(),
                time_ms: last * 1000,
            },
            source: Source {
                kind: SourceKind::System,
                id: 0,
                name: String::new(),
            },
            description: "Disk unit 2 of storage pool POOL1 failed or is missing.".into(),
            msgid: "QVS5130".into(),
            holds: true,
            closed_by: String::new(),
            closed_at: 0,
        }
    }

    #[test]
    fn a_number_is_the_console_s_and_stays_across_a_restart() {
        let d = std::env::temp_dir().join(format!("mgmtd-problems-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        let mut p = Problems::open(Some(&d), 100);
        let a = p.keep(
            "QSHQVS1",
            Some(Severity::Attention),
            summary(7, ProblemStatus::Open, 10),
        );
        let b = p.keep("QSHQ400", None, summary(7, ProblemStatus::Open, 20));
        assert_eq!((a, b), (1, 2));
        // Seen again: the same number, its summary as said now.
        let mut seen = summary(7, ProblemStatus::Open, 30);
        seen.count = 2;
        assert_eq!(p.keep("QSHQVS1", None, seen), 1);
        drop(p);
        let p = Problems::open(Some(&d), 100);
        assert_eq!(p.number_of("qshqvs1", 7), Some(1));
        let row = p.get(1).unwrap();
        assert_eq!(
            (row.summary.count, row.severity),
            (2, Some(Severity::Attention))
        );
        let page = p.page(&ProblemFilter::default(), 0, 25);
        assert_eq!(page.total, 2);
        assert_eq!(page.rows[0].number, 2);
        let only = ProblemFilter {
            system: "QSHQVS1".into(),
            ..ProblemFilter::default()
        };
        assert_eq!(p.page(&only, 0, 25).total, 1);
        let mut p = p;
        p.keep("QSHQ400", None, summary(8, ProblemStatus::Open, 40));
        assert_eq!(p.number_of("QSHQ400", 8), Some(3));
        p.forget("QSHQ400");
        assert_eq!(p.page(&ProblemFilter::default(), 0, 25).total, 1);
        // A number is never used again.
        assert_eq!(
            p.keep("QSHQ400", None, summary(9, ProblemStatus::Open, 50)),
            4
        );
        fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn past_the_most_the_oldest_closed_go_first() {
        let mut p = Problems::open(None, 2);
        p.keep("A", None, summary(1, ProblemStatus::Open, 1));
        p.keep("A", None, summary(2, ProblemStatus::Closed, 2));
        p.keep("A", None, summary(3, ProblemStatus::Open, 3));
        assert!(p.get(2).is_none() && p.get(1).is_some() && p.get(3).is_some());
        assert_eq!(p.open_count(), 2);
    }
}
