// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! `updates.toml`, written only by `fetchd` in its own folder: the
//! update server's address, whether it checks and downloads by itself
//! (both off until an operator turns them on), and the highest sequence
//! seen of each stream, which never goes back.

use serde::{Deserialize, Serialize};

/// The update server QS/400's `docs/UPDATE_PROTOCOL.md` names; each
/// stream is a path under it.
pub const DEFAULT_SERVER: &str = "https://updates.quartzsystems.net";

/// When the streams are checked without being asked.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Check {
    /// Only when asked.
    #[default]
    Never,
    /// Every day at 04:00, the console's time.
    Daily,
    /// Every Sunday at 04:00.
    Weekly,
}
ipc::wire_enum!(Check {
    1 => Never,
    2 => Daily,
    3 => Weekly,
});

impl Check {
    /// The words of the Code updates page.
    pub fn words(self) -> &'static str {
        match self {
            Check::Never => "Never",
            Check::Daily => "Daily 04:00",
            Check::Weekly => "Weekly",
        }
    }

    /// The setting of `words`.
    pub fn of_words(words: &str) -> Option<Self> {
        [Check::Daily, Check::Weekly, Check::Never]
            .into_iter()
            .find(|c| c.words() == words)
    }
}

/// The highest sequence seen of each stream.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Seen {
    /// `qsmc`.
    #[serde(default)]
    pub qsmc: u64,
    /// `qsvs`.
    #[serde(default)]
    pub qsvs: u64,
    /// `qs400`.
    #[serde(default)]
    pub qs400: u64,
}

/// `updates.toml`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Settings {
    /// The update server.
    pub server: String,
    /// When it is checked without being asked.
    #[serde(default)]
    pub check: Check,
    /// New levels found are downloaded without being asked.
    #[serde(default)]
    pub download: bool,
    /// The highest sequence seen of each stream.
    #[serde(default)]
    pub seen: Seen,
    /// When the streams were last checked, Unix seconds; 0 for never.
    #[serde(default)]
    pub checked: u64,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            server: DEFAULT_SERVER.into(),
            check: Check::Never,
            download: false,
            seen: Seen::default(),
            checked: 0,
        }
    }
}

impl state::Document for Settings {
    const FILE: &'static str = "updates.toml";
    const VERSION: u32 = 1;
    const MIGRATIONS: &'static [state::Migration] = &[];

    fn validate(&self) -> Result<(), String> {
        if valid_server(&self.server) {
            Ok(())
        } else {
            Err(format!("{} is not an update server's address", self.server))
        }
    }
}

/// Whether `server` is an update server's address: `https://` or
/// `http://`, a host, no user, query or fragment.
pub fn valid_server(server: &str) -> bool {
    server.len() <= 256
        && !server.contains(['?', '#', ' '])
        && httpc::Url::parse(server).is_ok_and(|u| !u.host.is_empty() && u.userinfo.is_none())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nothing_is_called_until_an_operator_says() {
        let s = Settings::default();
        assert_eq!((s.check, s.download), (Check::Never, false));
        assert_eq!(s.server, "https://updates.quartzsystems.net");
        let text = state::store::render(&s).unwrap();
        assert_eq!(state::store::parse::<Settings>(&text).unwrap(), s);
    }

    #[test]
    fn a_server_is_an_address() {
        assert!(valid_server("https://updates.quartzsystems.net"));
        assert!(valid_server("http://10.0.2.2:8480/mirror"));
        assert!(!valid_server("ftp://updates.quartzsystems.net"));
        assert!(!valid_server("https://u:p@updates.quartzsystems.net"));
        assert!(!valid_server("https://updates.quartzsystems.net/?x=1"));
        assert!(!valid_server("updates.quartzsystems.net"));
        assert_eq!(Check::of_words("Daily 04:00"), Some(Check::Daily));
    }
}
