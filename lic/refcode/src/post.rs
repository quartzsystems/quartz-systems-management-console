// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! Posting a code: one line on the console, `REF <code> <text>`. PID 1
//! posts. The history record for the log service comes with that
//! service in Phase 1.

use appliance::Event;

use crate::{RefCode, WORD};

/// The line that posts `code`, with `detail` added to its text when
/// there is some.
pub fn line(code: &RefCode, detail: &str) -> String {
    format!("{WORD} {} {}", code.code, code.text_with(detail))
}

/// The line that posts `event`: its code's, or, while it has none, the
/// event as it is, so nothing a base crate says is lost.
pub fn event_line(event: &Event<'_>) -> String {
    match crate::event::code_of(event) {
        Some((code, detail)) => line(code, &detail),
        None => format!("{WORD} (no code yet) {event:?}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_posting_is_the_word_the_code_and_the_text() {
        assert_eq!(
            line(&crate::PROCESSOR_LACKS, "AVX2, ADX"),
            "REF A3001015 Processor lacks what the cryptography needs: AVX2, ADX"
        );
        assert_eq!(line(&crate::PANIC, ""), "REF B3001000 Console PID 1 failed");
        assert_eq!(
            event_line(&Event::SeedApplied),
            "REF (no code yet) SeedApplied"
        );
    }
}
