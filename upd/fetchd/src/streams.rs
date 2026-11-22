// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! The three product streams under the one update server: `qsmc`, this
//! console; `qsvs`, its hosts; `qs400`, the level partitions and
//! standalone systems are compared with. The first two are `base/stream`
//! indexes, the third QS/400's signed catalog (`net/updcat`). Each is
//! trusted by its own keys, built into the image.

use std::path::{Path, PathBuf};

/// A product stream.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Stream {
    /// QS/MC, this console.
    #[default]
    Qsmc,
    /// QS/VS, its hosts.
    Qsvs,
    /// QS/400's fixes.
    Qs400,
}
ipc::wire_enum!(Stream {
    1 => Qsmc,
    2 => Qsvs,
    3 => Qs400,
});

/// Where the image keeps each stream's trusted keys: a file a stream.
pub const KEYS_DIR: &str = "/qsmc/keys/streams";

impl Stream {
    /// Every stream, in the pages' order.
    pub const ALL: [Stream; 3] = [Stream::Qsmc, Stream::Qsvs, Stream::Qs400];

    /// Its name: the path under the server, its folder, its keys' file.
    pub const fn name(self) -> &'static str {
        match self {
            Stream::Qsmc => "qsmc",
            Stream::Qsvs => "qsvs",
            Stream::Qs400 => "qs400",
        }
    }

    /// The stream named `name`.
    pub fn of(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|s| s.name() == name)
    }

    /// The product it is of, in a person's words.
    pub const fn product(self) -> &'static str {
        match self {
            Stream::Qsmc => "QS/MC",
            Stream::Qsvs => "QS/VS",
            Stream::Qs400 => "QS/400",
        }
    }

    /// Whether it is QS/400's catalog rather than an index.
    pub const fn is_catalog(self) -> bool {
        matches!(self, Stream::Qs400)
    }

    /// The index's or catalog's URL under `server`.
    pub fn index_url(self, server: &str) -> String {
        let leaf = if self.is_catalog() {
            "catalog"
        } else {
            "index"
        };
        format!("{}/{}/v1/{leaf}", server.trim_end_matches('/'), self.name())
    }

    /// A file's URL under `server`: an update image, an ISO or notes.
    pub fn file_url(self, server: &str, file: &str) -> String {
        format!(
            "{}/{}/v1/file/{file}",
            server.trim_end_matches('/'),
            self.name()
        )
    }

    /// The file of its trusted keys under `dir`.
    pub fn keys_file(self, dir: &Path) -> PathBuf {
        dir.join(format!("{}.pub", self.name()))
    }

    /// The sequence of it in `seen`.
    pub fn seen(self, seen: &crate::settings::Seen) -> u64 {
        match self {
            Stream::Qsmc => seen.qsmc,
            Stream::Qsvs => seen.qsvs,
            Stream::Qs400 => seen.qs400,
        }
    }

    /// `seen` with this stream's sequence `n`, never lower than it was.
    pub fn see(self, seen: &mut crate::settings::Seen, n: u64) {
        let slot = match self {
            Stream::Qsmc => &mut seen.qsmc,
            Stream::Qsvs => &mut seen.qsvs,
            Stream::Qs400 => &mut seen.qs400,
        };
        *slot = (*slot).max(n);
    }
}

/// Keys from a file of 64-hexadecimal-character lines; a line that is
/// not one is passed over, `#` begins a comment.
pub fn parse_keys(text: &str) -> Vec<[u8; 32]> {
    text.lines()
        .map(|l| l.split('#').next().unwrap_or("").trim())
        .filter_map(|l| hex::decode(l).ok()?.try_into().ok())
        .collect()
}

/// A stream's trusted keys from the image's folder `dir`; none when its
/// file is missing, and then nothing of the stream verifies.
pub fn keys(stream: Stream, dir: &Path) -> Vec<[u8; 32]> {
    std::fs::read_to_string(stream.keys_file(dir))
        .map(|t| parse_keys(&t))
        .unwrap_or_default()
}

/// Whether `name` is a file name a stream may name: no path in it.
pub fn file_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 128
        && name != "."
        && name != ".."
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-_.+".contains(&b))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_stream_is_a_path_under_the_one_server() {
        let s = "https://updates.quartz.systems/";
        assert_eq!(
            Stream::Qsmc.index_url(s),
            "https://updates.quartz.systems/qsmc/v1/index"
        );
        assert_eq!(
            Stream::Qs400.index_url(s),
            "https://updates.quartz.systems/qs400/v1/catalog"
        );
        assert_eq!(
            Stream::Qsvs.file_url(s, "QSVS-V1R0M1-update-ab.upd"),
            "https://updates.quartz.systems/qsvs/v1/file/QSVS-V1R0M1-update-ab.upd"
        );
        assert_eq!(Stream::of("qs400"), Some(Stream::Qs400));
    }

    #[test]
    fn keys_and_names_are_read_as_hostile() {
        let k = parse_keys(&format!(
            "{}\n# c\nzz\n{}  # two\n",
            "ab".repeat(32),
            "cd".repeat(32)
        ));
        assert_eq!(k, vec![[0xab; 32], [0xcd; 32]]);
        assert!(file_name("QSMC-V1R0M1-update-0123.upd"));
        for bad in ["", "..", "a/b", "a\\b", "a b", "../x"] {
            assert!(!file_name(bad), "{bad}");
        }
    }

    #[test]
    fn a_sequence_seen_never_goes_back() {
        let mut seen = crate::settings::Seen::default();
        Stream::Qsvs.see(&mut seen, 5);
        Stream::Qsvs.see(&mut seen, 3);
        assert_eq!(Stream::Qsvs.seen(&seen), 5);
        assert_eq!(Stream::Qsmc.seen(&seen), 0);
    }
}
