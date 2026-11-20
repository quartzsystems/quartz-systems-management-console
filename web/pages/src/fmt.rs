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

/// A date typed in `format`: year, month, day. Two-digit years are
/// 2000 to 2099.
pub fn parse_date(text: &str, format: &str) -> Option<(i64, i64, i64)> {
    let parts: Vec<i64> = text
        .trim()
        .split(['/', '-'])
        .map(|p| p.parse().ok())
        .collect::<Option<_>>()?;
    let [one, two, three] = parts[..] else {
        return None;
    };
    let (y, m, d) = match format {
        "DD/MM/YY" => (three, two, one),
        "YYYY-MM-DD" => (one, two, three),
        _ => (three, one, two),
    };
    let y = if y < 100 { 2000 + y } else { y };
    let days = match m {
        2 if y % 4 == 0 && (y % 100 != 0 || y % 400 == 0) => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        1..=12 => 31,
        _ => return None,
    };
    ((1970..=2099).contains(&y) && (1..=days).contains(&d)).then_some((y, m, d))
}

/// A time typed as `HH:MM`, or `h:mm AM` / `h:mm PM`: hour and minute.
pub fn parse_time(text: &str) -> Option<(i64, i64)> {
    let t = text.trim().to_ascii_uppercase();
    let (t, half) = if let Some(t) = t.strip_suffix("AM") {
        (t.trim().to_string(), Some(0))
    } else if let Some(t) = t.strip_suffix("PM") {
        (t.trim().to_string(), Some(12))
    } else {
        (t, None)
    };
    let (h, m) = t.split_once(':')?;
    let (h, m): (i64, i64) = (h.parse().ok()?, m.parse().ok()?);
    let h = match half {
        Some(add) if (1..=12).contains(&h) => h % 12 + add,
        Some(_) => return None,
        None => h,
    };
    ((0..24).contains(&h) && (0..60).contains(&m)).then_some((h, m))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn typed_dates_and_times() {
        assert_eq!(parse_date("10/30/26", "MM/DD/YY"), Some((2026, 10, 30)));
        assert_eq!(parse_date("30/10/26", "DD/MM/YY"), Some((2026, 10, 30)));
        assert_eq!(parse_date("2028-02-29", "YYYY-MM-DD"), Some((2028, 2, 29)));
        assert_eq!(parse_date("02/29/27", "MM/DD/YY"), None);
        assert_eq!(parse_date("13/01/26", "MM/DD/YY"), None);
        assert_eq!(parse_time("07:14"), Some((7, 14)));
        assert_eq!(parse_time("12:05 am"), Some((0, 5)));
        assert_eq!(parse_time("1:30 PM"), Some((13, 30)));
        assert_eq!(parse_time("24:00"), None);
    }

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
