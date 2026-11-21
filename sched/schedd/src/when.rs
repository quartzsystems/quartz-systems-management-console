// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! When a schedule is due: its first time, then each day, each chosen
//! weekday or each month's day at the same time of day in the console's
//! display offset (`host.toml`'s zone, no summer time), until its end.

use appliance::time::{Zone, civil, from_civil};

use crate::model::Schedule;

/// The display offset of a zone as `host.toml` keeps it: `UTC`,
/// `UTC+02:00`, `UTC-05:30`.
pub fn zone(text: &str) -> Zone {
    let rest = text.trim().trim_start_matches("UTC");
    let (sign, hm) = match rest.chars().next() {
        Some('+') => (1, &rest[1..]),
        Some('-') => (-1, &rest[1..]),
        _ => return Zone::UTC,
    };
    let (h, m) = hm.split_once(':').unwrap_or((hm, "0"));
    let (Ok(h), Ok(m)) = (h.parse::<i32>(), m.parse::<i32>()) else {
        return Zone::UTC;
    };
    Zone {
        minutes: sign * (h * 60 + m),
    }
}

/// The weekday of local day `days` since the epoch: 0 for Sunday.
fn weekday(days: i64) -> u8 {
    // 1970-01-01 was a Thursday.
    u8::try_from((days + 4).rem_euclid(7)).unwrap_or(0)
}

/// The first time after `after` that `s` is due; `None` for never.
#[allow(clippy::many_single_char_names, reason = "a date's parts")]
pub fn next(s: &Schedule, zone: Zone, after: i64) -> Option<i64> {
    let off = i64::from(zone.minutes) * 60;
    let local = |t: i64| t + off;
    let first = s.first;
    let tod = local(first).rem_euclid(86_400);
    let ok = |t: i64| t > after && (s.until == 0 || t <= s.until);
    let due = match s.repeat.every.as_str() {
        "once" => Some(first).filter(|t| *t > after),
        "daily" => {
            if first > after {
                Some(first)
            } else {
                let n = (after - first) / 86_400 + 1;
                Some(first + n * 86_400)
            }
        }
        "weekly" => {
            if s.repeat.days.is_empty() {
                return None;
            }
            let start = local(first.max(after)).div_euclid(86_400);
            (start..start + 8)
                .map(|d| d * 86_400 + tod - off)
                .find(|t| {
                    *t >= first
                        && *t > after
                        && s.repeat
                            .days
                            .contains(&weekday((t + off).div_euclid(86_400)))
                })
        }
        "monthly" => {
            let day = i64::from(s.repeat.day.clamp(1, 31));
            let c = civil(local(first.max(after)));
            let (mut y, mut m) = (c.year, i64::from(c.month));
            let mut found = None;
            for _ in 0..48 {
                let t = from_civil(y, m, day, 0, 0, 0) + tod - off;
                // A month without the day is passed over.
                if i64::from(civil(t + off).day) == day && t >= first && t > after {
                    found = Some(t);
                    break;
                }
                m += 1;
                if m > 12 {
                    m = 1;
                    y += 1;
                }
            }
            found
        }
        _ => None,
    };
    due.filter(|t| ok(*t))
}

/// `date` (`MM/DD/YY`, `DD.MM.YY` or `YYYY-MM-DD` as `fmt` read it) at
/// `hour`:`minute` local, in Unix seconds.
pub fn at(date: (i64, i64, i64), hour: i64, minute: i64, zone: Zone) -> i64 {
    let (y, m, d) = date;
    from_civil(y, m, d, hour, minute, 0) - i64::from(zone.minutes) * 60
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Repeat;

    fn s(every: &str, first: i64) -> Schedule {
        Schedule {
            first,
            repeat: Repeat {
                every: every.into(),
                days: vec![0, 3],
                day: 31,
            },
            ..Schedule::default()
        }
    }

    #[test]
    fn once_daily_weekly_and_monthly_fall_on_their_days() {
        let z = Zone::UTC;
        // Saturday 2026-10-31 06:00.
        let first = at((2026, 10, 31), 6, 0, z);
        assert_eq!(next(&s("once", first), z, first - 1), Some(first));
        assert_eq!(next(&s("once", first), z, first), None);
        assert_eq!(next(&s("daily", first), z, first), Some(first + 86_400));
        assert_eq!(
            next(&s("daily", first), z, first + 3 * 86_400 + 5),
            Some(first + 4 * 86_400)
        );
        // Sundays and Wednesdays: Sunday 11-01, then Wednesday 11-04.
        assert_eq!(
            next(&s("weekly", first), z, first),
            Some(at((2026, 11, 1), 6, 0, z))
        );
        assert_eq!(
            next(&s("weekly", first), z, at((2026, 11, 1), 6, 0, z)),
            Some(at((2026, 11, 4), 6, 0, z))
        );
        // The 31st: October's, then December's (November has none).
        assert_eq!(
            next(&s("monthly", first), z, first - 1),
            Some(at((2026, 10, 31), 6, 0, z))
        );
        assert_eq!(
            next(&s("monthly", first), z, first),
            Some(at((2026, 12, 31), 6, 0, z))
        );
        let mut until = s("daily", first);
        until.until = first + 86_400;
        assert_eq!(next(&until, z, first + 86_400), None);
    }

    #[test]
    fn the_display_offset_moves_the_time_of_day() {
        let z = zone("UTC-05:00");
        assert_eq!(z.minutes, -300);
        assert_eq!(zone("UTC+05:30").minutes, 330);
        assert_eq!(zone("UTC").minutes, 0);
        let first = at((2026, 10, 31), 23, 0, z);
        assert_eq!(civil(first).hour, 4);
        // Weekly at 23:00 local is the next day in UTC; Saturday is local.
        let mut w = s("weekly", first);
        w.repeat.days = vec![6];
        assert_eq!(next(&w, z, first), Some(first + 7 * 86_400));
    }
}
