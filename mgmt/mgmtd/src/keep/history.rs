// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! The reference code history of every managed system: read from each
//! system by its cursor and from its events, kept bounded on disk
//! (`release::MGMT_DIR`, a file a system), newest kept.

use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::io::Write as _;
use std::path::{Path, PathBuf};

use mgmtproto::calls::{Filter, Page, Row};
use qsi::base::Refcode;
use qsi::host::refcodes::{Posted, Source};

/// One system's codes, in the order they came, with what is kept of
/// each (partition, sequence) to drop one seen again.
#[derive(Debug, Default)]
struct Kept {
    rows: Vec<Row>,
    seen: HashSet<(u32, u64)>,
}

/// Every system's history.
#[derive(Debug, Default)]
pub struct History {
    dir: Option<PathBuf>,
    keep: usize,
    by: BTreeMap<String, Kept>,
}

fn clean(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c == '\t' || c == '\n' || c == '\r' {
                ' '
            } else {
                c
            }
        })
        .collect()
}

fn line(r: &Row) -> String {
    let p = &r.posted;
    format!(
        "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\n",
        p.refcode.seq,
        r.partition,
        if p.source == Source::Host {
            "host"
        } else {
            "system"
        },
        p.refcode.time_ms,
        clean(&p.refcode.code),
        clean(&p.refcode.words.join(" ")),
        clean(&r.partition_name),
        clean(&p.refcode.text)
    )
}

fn parse(system: &str, l: &str) -> Option<Row> {
    let f: Vec<&str> = l.splitn(8, '\t').collect();
    if f.len() != 8 {
        return None;
    }
    Some(Row {
        system: system.into(),
        partition: f[1].parse().ok()?,
        partition_name: f[6].into(),
        posted: Posted {
            refcode: Refcode {
                seq: f[0].parse().ok()?,
                code: f[4].into(),
                words: f[5].split_whitespace().map(String::from).collect(),
                text: f[7].into(),
                time_ms: f[3].parse().ok()?,
            },
            source: if f[2] == "host" {
                Source::Host
            } else {
                Source::System
            },
        },
    })
}

impl History {
    /// The history kept in `dir` (none: in memory only), `keep` codes a
    /// system.
    pub fn open(dir: Option<&Path>, keep: usize) -> Self {
        let mut h = Self {
            dir: dir.map(Path::to_path_buf),
            keep: keep.max(64),
            by: BTreeMap::new(),
        };
        let Some(d) = dir else {
            return h;
        };
        let _ = fs::create_dir_all(d);
        for e in fs::read_dir(d).into_iter().flatten().flatten() {
            let path = e.path();
            let Some(name) = path
                .file_name()
                .and_then(|n| n.to_str())
                .and_then(|n| n.strip_suffix(".codes"))
            else {
                continue;
            };
            let text = fs::read_to_string(&path).unwrap_or_default();
            let rows: Vec<Row> = text.lines().filter_map(|l| parse(name, l)).collect();
            let seen = rows
                .iter()
                .map(|r| (r.partition, r.posted.refcode.seq))
                .collect();
            h.by.insert(name.to_string(), Kept { rows, seen });
        }
        h
    }

    fn file(&self, system: &str) -> Option<PathBuf> {
        self.dir.as_ref().map(|d| d.join(format!("{system}.codes")))
    }

    /// The last code kept of `system`'s `partition` (0 the system
    /// itself): where its history is read on from.
    pub fn last_seq(&self, system: &str, partition: u32) -> u64 {
        self.by
            .get(system)
            .into_iter()
            .flat_map(|k| k.rows.iter())
            .filter(|r| r.partition == partition)
            .map(|r| r.posted.refcode.seq)
            .max()
            .unwrap_or(0)
    }

    /// Add `rows` of `system`, those not kept yet, after what came
    /// before them: codes are kept in the order they came, as the
    /// sources number them each on their own.
    pub fn add(&mut self, system: &str, rows: Vec<Row>) {
        let keep = self.keep;
        let k = self.by.entry(system.to_string()).or_default();
        let mut new: Vec<Row> = rows
            .into_iter()
            .filter(|r| r.posted.refcode.seq > 0)
            .filter(|r| k.seen.insert((r.partition, r.posted.refcode.seq)))
            .collect();
        if new.is_empty() {
            return;
        }
        // A page read newest first comes in order of its own source.
        new.sort_by_key(|r| (r.posted.refcode.time_ms, r.posted.refcode.seq));
        k.rows.extend(new.iter().cloned());
        let over = k.rows.len() > keep + keep / 4;
        if over {
            let cut = k.rows.len() - keep;
            for r in k.rows.drain(..cut) {
                k.seen.remove(&(r.partition, r.posted.refcode.seq));
            }
        }
        let Some(path) = self.file(system) else {
            return;
        };
        if over {
            let all: String = self.by[system].rows.iter().map(line).collect();
            let tmp = path.with_extension("codes.new");
            if fs::write(&tmp, all).is_ok() {
                let _ = fs::rename(&tmp, &path);
            }
        } else if let Ok(mut f) = fs::OpenOptions::new().create(true).append(true).open(&path) {
            let text: String = new.iter().map(line).collect();
            let _ = f.write_all(text.as_bytes());
        }
    }

    /// Forget `system`'s history.
    pub fn remove(&mut self, system: &str) {
        self.by.remove(system);
        if let Some(p) = self.file(system) {
            let _ = fs::remove_file(p);
        }
    }

    /// The rows `filter` matches, newest first, `rows` of them after the
    /// first `skip`.
    pub fn query(&self, filter: &Filter, skip: u32, rows: u32) -> Page {
        let matching: Vec<(usize, &Row)> = self
            .by
            .iter()
            .filter(|(name, _)| filter.system.is_empty() || **name == filter.system)
            .flat_map(|(_, k)| k.rows.iter().enumerate())
            .filter(|(_, r)| filter.partition.is_none_or(|p| r.partition == p))
            .filter(|(_, r)| filter.from_ms == 0 || r.posted.refcode.time_ms >= filter.from_ms)
            .filter(|(_, r)| filter.to_ms == 0 || r.posted.refcode.time_ms < filter.to_ms)
            .filter(|(_, r)| filter.code.is_empty() || r.posted.refcode.code == filter.code)
            .collect();
        // Newest first; at the same time, the later to come first.
        let mut sorted = matching;
        sorted.sort_by(|(ia, a), (ib, b)| {
            (b.posted.refcode.time_ms, ib).cmp(&(a.posted.refcode.time_ms, ia))
        });
        Page {
            total: u32::try_from(sorted.len()).unwrap_or(u32::MAX),
            rows: sorted
                .into_iter()
                .skip(skip as usize)
                .take(rows as usize)
                .map(|(_, r)| r.clone())
                .collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(system: &str, partition: u32, seq: u64, code: &str) -> Row {
        Row {
            system: system.into(),
            partition,
            partition_name: if partition == 0 {
                String::new()
            } else {
                "QSDEV01".into()
            },
            posted: Posted {
                refcode: Refcode {
                    seq,
                    code: code.into(),
                    words: vec!["00000000".into(); 8],
                    text: "a code\twith a tab".into(),
                    time_ms: 1_000 * seq,
                },
                source: Source::System,
            },
        }
    }

    #[test]
    fn kept_bounded_on_disk_and_read_back() {
        let dir = std::env::temp_dir().join(format!("mgmtd-history-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let mut h = History::open(Some(&dir), 64);
        h.add(
            "QSHQVS1",
            (1..=100)
                .map(|s| row("QSHQVS1", 1, s, "C6004031"))
                .collect(),
        );
        h.add("QSHQVS1", vec![row("QSHQVS1", 1, 100, "C6004031")]);
        h.add("QSHQVS1", vec![row("QSHQVS1", 0, 1, "C2001000")]);
        assert_eq!(h.last_seq("QSHQVS1", 1), 100);
        assert_eq!(h.last_seq("QSHQVS1", 0), 1);
        let again = History::open(Some(&dir), 64);
        let all = again.query(&Filter::default(), 0, 1000);
        assert!(all.total <= 80 && all.total >= 64, "{}", all.total);
        assert_eq!(all.rows[0].posted.refcode.seq, 100);
        assert_eq!(all.rows[0].posted.refcode.text, "a code with a tab");
        let host = again.query(
            &Filter {
                partition: Some(0),
                ..Filter::default()
            },
            0,
            25,
        );
        assert_eq!(host.total, 1);
        let paged = again.query(&Filter::default(), 25, 25);
        assert_eq!(paged.rows.len(), 25);
        let _ = fs::remove_dir_all(&dir);
    }
}
