// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! QS/MC as the base crates see it (`appliance::Appliance`), with PID
//! 1's poster and its notes. PID 1 posts itself: an event's line goes
//! to the console. A note is a fact PID 1 keeps for its status screen,
//! which comes in Phase 1; Phase 0 keeps them in memory.

use std::sync::Mutex;

use appliance::{Appliance, Event};

use crate::console::Console;

/// The facts noted so far, in the order noted; a key noted again is
/// replaced.
static FACTS: Mutex<Vec<(String, String)>> = Mutex::new(Vec::new());

/// The key of the slot fact.
pub const SLOT: &str = "slot";
/// The key of the processor fact.
pub const PROCESSOR: &str = "processor";

/// Post an event a base crate reports.
fn post(event: &Event<'_>) {
    Console::open().line(&refcode::post::event_line(event));
}

/// Keep a fact.
fn note(key: &str, value: &str) {
    let mut facts = FACTS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    match facts.iter_mut().find(|(k, _)| k == key) {
        Some(fact) => fact.1 = value.to_string(),
        None => facts.push((key.to_string(), value.to_string())),
    }
}

/// QS/MC, with PID 1's poster and notes.
pub const QSMC: Appliance = release::appliance(post, note);

/// The fact noted under `key`.
#[cfg(test)]
pub fn fact(key: &str) -> Option<String> {
    let facts = FACTS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    facts.iter().find(|(k, _)| k == key).map(|(_, v)| v.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_description_is_qs_mc_s_and_notes_are_kept() {
        assert_eq!((QSMC.id, QSMC.name), ("qsmc", "QS/MC"));
        assert_eq!((QSMC.banner)(), release::banner());
        (QSMC.note)(SLOT, "A");
        (QSMC.note)(SLOT, "B");
        assert_eq!(fact(SLOT).as_deref(), Some("B"));
        assert_eq!(fact("nothing"), None);
    }
}
