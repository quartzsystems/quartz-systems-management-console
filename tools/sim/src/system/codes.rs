// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! Reference codes as the system posts them: its own, and a partition's
//! (the host's `C2005` codes, then the system's own from QS/400's
//! table), each in its history and as an event.

use qsi::any::AnyEvent;
use qsi::base::{self, Refcode};
use qsi::host::model::PartitionState;
use qsi::host::refcodes::{self, Posted, Source};

use super::{System, now_ms};

/// The host's codes of a partition's IPL, in order.
const IPL: &[(&str, &str)] = &[
    ("C2005000", "Partition activation started"),
    ("C2005010", "Partition disk units opened"),
    ("C2005030", "Partition LIC record read"),
    ("C2005040", "Partition IPL image read"),
    ("C2005060", "Partition machine built"),
    ("C2005090", "Partition started"),
];

/// Post one of the system's own codes.
pub fn post(s: &mut System, code: &str, text: &str) {
    let seq = s.codes.last().map_or(1, |c| c.seq + 1);
    let r = Refcode {
        seq,
        code: code.into(),
        words: Vec::new(),
        text: text.into(),
        time_ms: now_ms(),
    };
    s.codes.push(r.clone());
    s.push(&AnyEvent::Base(base::Event::Refcode(r)));
}

/// Post a code of partition `id`; it becomes its current code unless
/// `note`.
pub fn post_part(s: &mut System, id: u32, code: &str, text: &str, source: Source, note: bool) {
    let Some(p) = s.part(id) else {
        return;
    };
    let seq = p.history.last().map_or(1, |c| c.refcode.seq + 1);
    let posted = Posted {
        refcode: Refcode {
            seq,
            code: code.into(),
            words: vec!["00000000".into(); 8],
            text: text.into(),
            time_ms: now_ms(),
        },
        source,
    };
    p.history.push(posted.clone());
    if p.history.len() > 256 {
        p.history.remove(0);
    }
    if !note {
        p.status.code = code.into();
        p.status.text = text.into();
    }
    let name = p.status.name.clone();
    // A QS/VS host's event has no place in the history: seq 0.
    let mut posted = posted;
    posted.refcode.seq = 0;
    s.push(&AnyEvent::Refcodes(refcodes::Event::Code(refcodes::Code {
        id,
        name,
        posted,
        current: !note,
    })));
}

/// The system's own codes of an IPL, from QS/400's table: the licensed
/// internal code's first, then the operating system's.
fn system_ipl() -> Vec<(&'static str, &'static str)> {
    let pick = |prefix: &str| {
        srcdata::TABLE
            .iter()
            .filter(|c| c.code.starts_with(prefix))
            .take(3)
            .map(|c| (c.code, c.text))
            .collect::<Vec<_>>()
    };
    let mut v = pick("C1");
    v.extend(pick("C6"));
    v
}

/// Activate partition `id`: its IPL's codes, then Running.
pub fn activate(s: &mut System, id: u32) {
    s.set_state(id, PartitionState::Starting);
    let name = s
        .part(id)
        .map(|p| p.status.name.clone())
        .unwrap_or_default();
    for (code, text) in IPL {
        post_part(s, id, code, &format!("{text}: {name}"), Source::Host, false);
    }
    for (code, text) in system_ipl() {
        post_part(s, id, code, text, Source::System, false);
    }
    s.set_state(id, PartitionState::Running);
}

/// End partition `id` at once (function 08, a power off).
pub fn end(s: &mut System, id: u32) {
    let name = s
        .part(id)
        .map(|p| p.status.name.clone())
        .unwrap_or_default();
    post_part(
        s,
        id,
        "C2005100",
        &format!("Partition powered off: {name}"),
        Source::Host,
        false,
    );
    s.set_state(id, PartitionState::NotActivated);
}

/// Restart partition `id`'s IPL (function 03).
pub fn restart(s: &mut System, id: u32) {
    end(s, id);
    activate(s, id);
}

/// Dump partition `id`, then restart its IPL (function 22).
pub fn dump(s: &mut System, id: u32) {
    let name = s
        .part(id)
        .map(|p| p.status.name.clone())
        .unwrap_or_default();
    post_part(
        s,
        id,
        "C2005180",
        &format!("Partition dump started: {name}"),
        Source::Host,
        false,
    );
    post_part(
        s,
        id,
        "C2005185",
        &format!("Partition dump written: {name}"),
        Source::Host,
        false,
    );
    restart(s, id);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_system_ipl_has_codes_of_both_families() {
        let v = system_ipl();
        assert!(v.iter().any(|(c, _)| c.starts_with("C1")));
        assert!(v.iter().any(|(c, _)| c.starts_with("C6")));
    }
}
