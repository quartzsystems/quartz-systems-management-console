// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! Reference codes as the system posts them: its own, and a partition's
//! (the host's `C2005` and `C2009` codes, then the system's own from
//! QS/400's table), each in its history and as an event; and the
//! sequences of an IPL from each source and of each kind of shut down.

use qsi::any::AnyEvent;
use qsi::base::{self, Refcode};
use qsi::host::model::{IplSource, PartitionState};
use qsi::host::partition::ShutdownKind;
use qsi::host::refcodes::{self, Posted, Source};

use super::{System, now_ms};

/// The host's codes of a partition's IPL from a side, in order.
const IPL: &[(&str, &str)] = &[
    ("C2005000", "Partition activation started"),
    ("C2005010", "Partition disk units opened"),
    ("C2005020", "Partition load-source table read"),
    ("C2005030", "Partition LIC record read"),
    ("C2005040", "Partition IPL image read"),
    ("C2005050", "Partition kernel loaded"),
    ("C2005060", "Partition machine built"),
    ("C2005090", "Partition started"),
];

/// The code an installation waits at: the installer found no console
/// and draws on the serial port (QS/VS's `docs/QS400_GUEST.md`).
pub const INSTALL_WAIT: &str = "A6005008";

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

/// A host code of partition `id`, its text after the partition's name.
fn host_code(s: &mut System, id: u32, code: &str, text: &str) {
    let name = s.part_name(id);
    post_part(s, id, code, &format!("{text}: {name}"), Source::Host, false);
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

/// What an IPL came to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ipl {
    /// It ran through: Running.
    Ran,
    /// From source D: the installer waits at [`INSTALL_WAIT`], Starting.
    Installing,
    /// From source D with no medium: it did not IPL.
    NoMedia,
}

/// Partition `id`'s IPL from `source`: its codes and states. `media` is
/// the image in its first optical device and its volume identifier,
/// for source D.
pub fn ipl(s: &mut System, id: u32, source: IplSource, media: Option<(&str, &str)>) -> Ipl {
    s.set_state(id, PartitionState::Starting);
    if source != IplSource::D {
        for (code, text) in IPL {
            host_code(s, id, code, text);
        }
        for (code, text) in system_ipl() {
            post_part(s, id, code, text, Source::System, false);
        }
        s.set_state(id, PartitionState::Running);
        return Ipl::Ran;
    }
    host_code(s, id, "C2005000", "Partition activation started");
    host_code(s, id, "C2005010", "Partition disk units opened");
    let Some((image, label)) = media else {
        host_code(s, id, "A2005032", "Partition has no installation media");
        host_code(s, id, "A2005100", "Partition did not IPL");
        return Ipl::NoMedia;
    };
    let name = s.part_name(id);
    post_part(
        s,
        id,
        "C2005045",
        &format!("Partition installation media read: {name}, {image}, volume {label}"),
        Source::Host,
        false,
    );
    for (code, text) in &IPL[5..] {
        host_code(s, id, code, text);
    }
    let wait = srcdata::TABLE
        .iter()
        .find(|c| c.code == INSTALL_WAIT)
        .map_or("Console not found, continuing on serial", |c| c.text);
    post_part(s, id, INSTALL_WAIT, wait, Source::System, false);
    Ipl::Installing
}

/// The codes and states of shutting partition `id` down `kind`; it is
/// Not activated after, holding nothing.
pub fn shut_down(s: &mut System, id: u32, kind: ShutdownKind, delay: u32) {
    let name = s.part_name(id);
    match kind {
        ShutdownKind::Delayed => {
            post_part(
                s,
                id,
                "C2009040",
                &format!("Partition delayed shut down requested: {name}, delay {delay} s"),
                Source::Host,
                true,
            );
            s.set_state(id, PartitionState::ShuttingDown);
            end(s, id);
        }
        ShutdownKind::System => {
            post_part(
                s,
                id,
                "C2009041",
                &format!("Partition operating system shut down requested: {name}"),
                Source::Host,
                true,
            );
            s.set_state(id, PartitionState::ShuttingDown);
            end(s, id);
        }
        _ => {
            s.set_state(id, PartitionState::ShuttingDown);
            host_code(s, id, "A2005140", "Partition ended by the host");
            s.release(id);
            s.set_state(id, PartitionState::NotActivated);
        }
    }
}

/// End partition `id` at once (function 08, a power off): it gives back
/// what it held.
pub fn end(s: &mut System, id: u32) {
    host_code(s, id, "C2005100", "Partition powered off");
    s.release(id);
    s.set_state(id, PartitionState::NotActivated);
}

/// Restart partition `id`'s IPL (function 03, a restart): it keeps what
/// it holds and IPLs from its profile's source.
pub fn restart(s: &mut System, id: u32) {
    host_code(s, id, "C2005110", "Partition ended for a restart");
    let (source, media) = s.boot_of(id);
    let media = media.as_ref().map(|(i, l)| (i.as_str(), l.as_str()));
    ipl(s, id, source, media);
}

/// Dump partition `id`, then restart its IPL (function 22).
pub fn dump(s: &mut System, id: u32) {
    host_code(s, id, "C2005180", "Partition dump started");
    host_code(s, id, "C2005185", "Partition dump written");
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

    #[test]
    fn the_install_wait_is_in_qs400_s_table() {
        assert!(srcdata::TABLE.iter().any(|c| c.code == INSTALL_WAIT));
    }
}
