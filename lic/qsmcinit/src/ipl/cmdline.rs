// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! What the baked kernel command line says of this IPL: `qsmc.slot=A`
//! or `qsmc.slot=B` on a disk, `qsmc.install=1` on the media UKI.

use release::KEY_PREFIX;

/// Where the console's IPL came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    /// ESP slot A.
    SlotA,
    /// ESP slot B.
    SlotB,
    /// The installation media.
    Install,
    /// The command line names neither.
    Unknown,
}

/// The value of `qsmc.<name>` on `line`, the last one given.
fn value<'a>(line: &'a str, name: &str) -> Option<&'a str> {
    line.split_ascii_whitespace()
        .filter_map(|word| word.strip_prefix(KEY_PREFIX))
        .filter_map(|word| word.split_once('='))
        .filter(|(key, _)| *key == name)
        .map(|(_, value)| value)
        .next_back()
}

/// Read the command line. The media word wins: that UKI carries no slot.
pub fn parse(line: &str) -> Source {
    if value(line, "install") == Some("1") {
        return Source::Install;
    }
    match value(line, "slot") {
        Some("A") => Source::SlotA,
        Some("B") => Source::SlotB,
        _ => Source::Unknown,
    }
}

impl Source {
    /// The slot as a fact names it: `A`, `B`, `media` or `unknown`.
    pub fn name(self) -> &'static str {
        match self {
            Source::SlotA => "A",
            Source::SlotB => "B",
            Source::Install => "media",
            Source::Unknown => "unknown",
        }
    }

    /// The line PID 1 prints.
    pub fn line(self) -> String {
        match self {
            Source::SlotA => format!("IPL slot A ({KEY_PREFIX}slot=A)"),
            Source::SlotB => format!("IPL slot B ({KEY_PREFIX}slot=B)"),
            Source::Install => format!("IPL from installation media ({KEY_PREFIX}install=1)"),
            Source::Unknown => format!("IPL slot unknown: no {KEY_PREFIX}slot on the command line"),
        }
    }
}

/// What the media UKI says after its slot line.
pub const MEDIA_LINE: &str = "The installer arrives in Phase 2; this IPL starts no services";

#[cfg(test)]
mod tests {
    use super::*;

    const BASE: &str = "console=ttyS0,115200 console=tty0 panic=-1 quiet";

    #[test]
    fn the_slot_is_read() {
        assert_eq!(parse(&format!("{BASE} qsmc.slot=A")), Source::SlotA);
        assert_eq!(parse(&format!("{BASE} qsmc.slot=B\n")), Source::SlotB);
        assert_eq!(parse(BASE), Source::Unknown);
        assert_eq!(parse("qsmc.slot=C"), Source::Unknown);
        // Another product's key is not ours.
        assert_eq!(parse("qsvs.slot=B"), Source::Unknown);
        assert_eq!(parse("qs400.slot=B"), Source::Unknown);
    }

    #[test]
    fn the_media_word_wins() {
        assert_eq!(parse(&format!("{BASE} qsmc.install=1")), Source::Install);
        assert_eq!(parse("qsmc.slot=A qsmc.install=1"), Source::Install);
        assert_eq!(parse("qsmc.install=0 qsmc.slot=A"), Source::SlotA);
    }

    #[test]
    fn the_lines_name_the_key() {
        assert_eq!(Source::SlotA.line(), "IPL slot A (qsmc.slot=A)");
        assert_eq!(Source::SlotB.line(), "IPL slot B (qsmc.slot=B)");
        assert_eq!(
            Source::Install.line(),
            "IPL from installation media (qsmc.install=1)"
        );
        assert_eq!(Source::Install.name(), "media");
    }
}
