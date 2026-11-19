// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! `web.toml`: the web interface's own settings, written only by the
//! web server through `base/state`: the sign-on notice, the session
//! timeouts, the formats of Language and locale, and whether the web
//! interface also listens on the `private` interface.

use serde::{Deserialize, Serialize};

fn idle() -> u32 {
    30
}
fn max() -> u32 {
    12
}
fn date() -> String {
    "MM/DD/YY".into()
}
fn time() -> String {
    "24-hour".into()
}
fn number() -> String {
    "1,234.56".into()
}

/// `web.toml`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WebSettings {
    /// The notice on the sign-on page; none when empty. No page sets
    /// it (`docs/OPEN_QUESTIONS.md`).
    #[serde(default)]
    pub notice: String,
    /// Minutes a session may be idle.
    #[serde(default = "idle")]
    pub idle_minutes: u32,
    /// Hours a session may last.
    #[serde(default = "max")]
    pub max_hours: u32,
    /// `MM/DD/YY`, `DD/MM/YY` or `YYYY-MM-DD`.
    #[serde(default = "date")]
    pub date_format: String,
    /// `24-hour` or `12-hour AM/PM`.
    #[serde(default = "time")]
    pub time_format: String,
    /// `1,234.56` or `1.234,56`.
    #[serde(default = "number")]
    pub number_format: String,
    /// Whether the web interface also answers on `private`.
    #[serde(default)]
    pub listen_private: bool,
}

impl Default for WebSettings {
    fn default() -> Self {
        Self {
            notice: String::new(),
            idle_minutes: idle(),
            max_hours: max(),
            date_format: date(),
            time_format: time(),
            number_format: number(),
            listen_private: false,
        }
    }
}

/// The date formats offered.
pub const DATE_FORMATS: [&str; 3] = ["MM/DD/YY", "DD/MM/YY", "YYYY-MM-DD"];
/// The time formats offered.
pub const TIME_FORMATS: [&str; 2] = ["24-hour", "12-hour AM/PM"];
/// The number formats offered.
pub const NUMBER_FORMATS: [&str; 2] = ["1,234.56", "1.234,56"];

impl state::Document for WebSettings {
    const FILE: &'static str = "web.toml";
    const VERSION: u32 = 1;
    const MIGRATIONS: &'static [state::Migration] = &[];

    fn validate(&self) -> Result<(), String> {
        if !(1..=480).contains(&self.idle_minutes) {
            return Err("The idle timeout is 1 to 480 minutes.".into());
        }
        if !(1..=72).contains(&self.max_hours) {
            return Err("The maximum session length is 1 to 72 hours.".into());
        }
        if !DATE_FORMATS.contains(&self.date_format.as_str())
            || !TIME_FORMATS.contains(&self.time_format.as_str())
            || !NUMBER_FORMATS.contains(&self.number_format.as_str())
        {
            return Err("A format is not one offered.".into());
        }
        if self.notice.len() > 500 || self.notice.contains(['\r', '\n']) {
            return Err("The notice is one line of 500 characters at most.".into());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use state::Document as _;

    #[test]
    fn the_defaults_are_the_prototype_s() {
        let s = WebSettings::default();
        assert_eq!((s.idle_minutes, s.max_hours), (30, 12));
        assert_eq!(s.validate(), Ok(()));
        assert!(!s.listen_private);
    }
}
