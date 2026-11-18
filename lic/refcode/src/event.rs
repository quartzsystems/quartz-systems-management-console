// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! The reference code QS/MC posts for each event the base crates
//! report. Phase 0 runs no base service, so no event has a code yet:
//! Phase 1 gives each one its code with the services that report it.

use appliance::Event;

use crate::RefCode;

/// The code of `event` and what the posting adds; `None` while the
/// event has no code.
pub fn code_of(event: &Event<'_>) -> Option<(&'static RefCode, String)> {
    let _ = event;
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn phase_0_has_no_event_codes() {
        assert_eq!(code_of(&Event::SeedApplied), None);
    }
}
