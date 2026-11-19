// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! Dates, times and numbers as Language and locale says, in the
//! console's display zone (`host.toml`'s offset from UTC).

use appliance::time::{Zone, civil};

/// `secs` as a date in `format` (`MM/DD/YY`, `DD/MM/YY`, `YYYY-MM-DD`).
pub fn date(secs: i64, zone: Zone, format: &str) -> String {
    let c = civil(secs + i64::from(zone.minutes) * 60);
    let yy = c.year.rem_euclid(100);
    match format {
        "DD/MM/YY" => format!("{:02}/{:02}/{yy:02}", c.day, c.month),
        "YYYY-MM-DD" => format!("{:04}-{:02}-{:02}", c.year, c.month, c.day),
        _ => format!("{:02}/{:02}/{yy:02}", c.month, c.day),
    }
}

/// `secs` as a time in `format` (`24-hour`, `12-hour AM/PM`), seconds
/// shown when `seconds`.
pub fn time(secs: i64, zone: Zone, format: &str, seconds: bool) -> String {
    let c = civil(secs + i64::from(zone.minutes) * 60);
    let s = if seconds {
        format!(":{:02}", c.second)
    } else {
        String::new()
    };
    if format == "12-hour AM/PM" {
        let h = match c.hour % 12 {
            0 => 12,
            h => h,
        };
        let am = if c.hour < 12 { "AM" } else { "PM" };
        format!("{h}:{:02}{s} {am}", c.minute)
    } else {
        format!("{:02}:{:02}{s}", c.hour, c.minute)
    }
}

/// `n` with the thousands mark of `format`.
pub fn number(n: u64, format: &str) -> String {
    let mark = if format == "1.234,56" { '.' } else { ',' };
    let digits = n.to_string();
    let mut out = String::new();
    for (i, d) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(mark);
        }
        out.push(d);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_prototype_s_forms() {
        // 2026-10-30 11:14:02 UTC, shown at UTC-04:00.
        let t = appliance::time::from_civil(2026, 10, 30, 11, 14, 2);
        let z: Zone = "UTC-04:00".parse().unwrap();
        assert_eq!(date(t, z, "MM/DD/YY"), "10/30/26");
        assert_eq!(date(t, z, "YYYY-MM-DD"), "2026-10-30");
        assert_eq!(time(t, z, "24-hour", true), "07:14:02");
        assert_eq!(time(t, z, "12-hour AM/PM", false), "7:14 AM");
        assert_eq!(number(131_072, "1,234.56"), "131,072");
        assert_eq!(number(4096, "1.234,56"), "4.096");
    }
}
