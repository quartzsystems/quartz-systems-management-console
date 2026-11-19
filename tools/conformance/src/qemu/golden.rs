// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! Golden screens: every installer and local console screen as 24 rows
//! of text under `tools/conformance/golden/`. What changes from run to
//! run is masked on both sides before they are compared: the build ID,
//! dates and times, counts of seconds, and the values of the rows that
//! say what the machine is doing at that moment. `QSMC_GOLDEN=update`
//! writes the files instead; what it wrote is read and committed by a
//! person.

use std::fs;
use std::path::PathBuf;

/// Set to `update` to write the golden files instead of comparing.
pub const ENV: &str = "QSMC_GOLDEN";

/// The words after which a number changes from run to run.
const COUNTED: [&str; 4] = ["process ", "restored in ", " for ", "Uptime . . . . . : "];

/// Labels whose value is what the console is doing at that moment: the
/// suites assert those facts by themselves.
const OF_THE_MOMENT: [&str; 8] = [
    "Reference",
    "Date and time",
    "Open, now",
    "Private, now",
    "Network open",
    "Uptime",
    "Open ",
    "Last written",
];

/// The directory of the golden files.
pub fn dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("golden")
}

fn digits(bytes: &[u8], lens: &[usize], seps: &[u8]) -> bool {
    let mut at = 0;
    for (i, len) in lens.iter().enumerate() {
        if bytes.len() < at + len || !bytes[at..at + len].iter().all(u8::is_ascii_digit) {
            return false;
        }
        at += len;
        if let Some(sep) = seps.get(i) {
            if bytes.get(at) != Some(sep) {
                return false;
            }
            at += 1;
        }
    }
    true
}

/// The length of `12.34 s` at the start of `bytes`.
fn seconds(bytes: &[u8]) -> Option<usize> {
    let whole = bytes.iter().take_while(|b| b.is_ascii_digit()).count();
    if whole == 0 || bytes.get(whole) != Some(&b'.') {
        return None;
    }
    let part = bytes[whole + 1..]
        .iter()
        .take_while(|b| b.is_ascii_digit())
        .count();
    let end = whole + 1 + part;
    (part > 0 && bytes[end..].starts_with(b" s")).then_some(end + 2)
}

/// One line with its dates, times and counts masked.
fn mask_line(line: &str) -> String {
    let bytes = line.as_bytes();
    let mut out = String::with_capacity(line.len());
    let mut i = 0;
    while i < bytes.len() {
        let rest = &bytes[i..];
        let joined = |b: Option<&u8>| b.is_some_and(|b| b.is_ascii_alphanumeric() || *b == b':');
        let before = i > 0 && joined(bytes.get(i - 1));
        let apart = |len: usize| !before && !joined(bytes.get(i + len));
        if digits(rest, &[4, 2, 2], b"--") && apart(10) {
            out.push_str("yyyy-mm-dd");
            i += 10;
        } else if digits(rest, &[2, 2, 2], b"::") && apart(8) {
            out.push_str("hh:mm:ss");
            i += 8;
        } else if digits(rest, &[2, 2], b":") && apart(5) {
            out.push_str("hh:mm");
            i += 5;
        } else if let Some(len) = seconds(rest).filter(|_| !before) {
            out.push_str("#.## s");
            i += len;
        } else if let Some(word) = COUNTED.iter().find(|w| rest.starts_with(w.as_bytes())) {
            out.push_str(word);
            i += word.len();
            let run = bytes[i..].iter().take_while(|b| b.is_ascii_digit()).count();
            if run > 0 {
                out.push('#');
                i += run;
            }
        } else {
            let c = line[i..].chars().next().unwrap_or(' ');
            out.push(c);
            i += c.len_utf8();
        }
    }
    out
}

/// `line` with the build ID after the release masked.
fn mask_build(line: &str, release: &str) -> String {
    let mark = format!("{release} ");
    let Some(at) = line.find(&mark) else {
        return line.to_string();
    };
    let mut after = at + mark.len();
    if line[after..].starts_with("build ") {
        after += 6;
    }
    let end = line[after..]
        .find(|c: char| c.is_whitespace() || c == ')')
        .map_or(line.len(), |n| after + n);
    format!("{}<build>{}", &line[..after], &line[end..])
}

/// `line` with the value after a label of [`OF_THE_MOMENT`] masked.
fn mask_moment(line: &str) -> String {
    let trimmed = line.trim_start();
    let lead = line.len() - trimmed.len();
    for label in OF_THE_MOMENT {
        if !trimmed.starts_with(label) {
            continue;
        }
        // A dotted label: its value is after the colon.
        if let Some(colon) = trimmed.find(" : ").or_else(|| trimmed.find("  :")) {
            let at = lead + colon + 3;
            return format!("{}<now>", &line[..at.min(line.len())]);
        }
        // A status screen row: the label in its column of 15.
        if trimmed.len() > 15 && trimmed.as_bytes().get(14) == Some(&b' ') {
            return format!("{}<now>", &line[..lead + 15]);
        }
    }
    line.to_string()
}

/// A screen with everything that changes from run to run masked.
pub fn mask(screen: &str, release: &str) -> String {
    screen
        .lines()
        .map(|l| {
            mask_moment(&mask_line(&mask_build(l, release)))
                .trim_end()
                .to_string()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Hold `screen` against the golden file `name`, or write it there when
/// `QSMC_GOLDEN=update`.
pub fn check(name: &str, screen: &str, release: &str) -> Result<(), String> {
    let path = dir().join(format!("{name}.txt"));
    let masked = mask(screen, release);
    if std::env::var(ENV).is_ok_and(|v| v == "update") {
        fs::create_dir_all(dir()).map_err(|e| e.to_string())?;
        return fs::write(&path, format!("{masked}\n")).map_err(|e| e.to_string());
    }
    let want = fs::read_to_string(&path).map_err(|e| {
        format!(
            "{}: {e}; `{ENV}=update` writes it from this run",
            path.display()
        )
    })?;
    let want = mask(&want, release);
    if want == masked {
        return Ok(());
    }
    let first = want
        .lines()
        .zip(masked.lines())
        .position(|(a, b)| a != b)
        .unwrap_or(0);
    Err(format!(
        "the screen differs from {} at row {}:\n--- golden\n{want}\n--- seen\n{masked}",
        path.display(),
        first + 1
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn what_changes_from_run_to_run_is_masked() {
        assert_eq!(
            mask("QS/MC V1R0M0 8cd7186-dirty (Quartz)", "V1R0M0"),
            "QS/MC V1R0M0 <build> (Quartz)"
        );
        assert_eq!(
            mask("  2026-11-18 22:05:03 UTC  12.5 s  in 30 s", "V1R0M0"),
            "  yyyy-mm-dd hh:mm:ss UTC  #.## s  in 30 s"
        );
        assert_eq!(
            mask(
                "If it has not, the configuration of before is restored in 27 s.",
                "V1R0M0"
            ),
            "If it has not, the configuration of before is restored in # s."
        );
        assert_eq!(
            mask(" Reference      C3001090 Base ready: 0.81 s", "V1R0M0"),
            " Reference      <now>"
        );
        assert_eq!(
            mask(" Uptime . . . . . : 00:01:02", "V1R0M0"),
            " Uptime . . . . . : <now>"
        );
        // A hardware address is not a time.
        assert_eq!(mask("52:54:00:12:34:56", "V1R0M0"), "52:54:00:12:34:56");
    }
}
