// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! The web certificate's end, watched (`docs/SERVICE.md`): within 30 days
//! of it the console posts `A3006050`, which opens a problem of its own;
//! a certificate with time left again posts `C3006050`, which clears it.

use std::sync::Arc;
use std::time::Duration;

use crate::App;

/// How long before its end a certificate is said to be ending.
pub const WARN: u64 = 30 * 86_400;
/// How often it is looked at.
const LOOK: Duration = Duration::from_hours(1);

/// What changed: `Some(true)` it is ending now, `Some(false)` it has
/// time left again, `None` nothing to say.
pub fn change(was: Option<bool>, not_after: u64, now: u64) -> Option<bool> {
    let ending = not_after < now.saturating_add(WARN);
    match was {
        Some(w) if w == ending => None,
        // At the start, only an ending certificate is said.
        None if !ending => None,
        _ => Some(ending),
    }
}

/// Look at the certificate in use every hour, for as long as the web
/// server runs, and post what changed through PID 1.
pub fn watch(app: Arc<App>) {
    std::thread::spawn(move || {
        let mut was = None;
        // A problem opened before this start, its certificate since
        // replaced, is cleared by the first look.
        let open = app
            .console
            .problems()
            .list(problem::Filter::default(), None, 500)
            .is_ok_and(|p| {
                p.problems.iter().any(|s| {
                    s.refcode.code == refcode::CERT_ENDING.code && s.status == problem::Status::Open
                })
            });
        if open {
            was = Some(true);
        }
        loop {
            if let Some(c) = app.certs.get() {
                let info = c.shown().info;
                let now = app.console.now();
                if let Some(ending) = change(was, info.not_after, now) {
                    let when = crate::fmt::date(
                        i64::try_from(info.not_after).unwrap_or(i64::MAX),
                        app.host().zone(),
                        &app.settings().date_format,
                    );
                    #[cfg(unix)]
                    if ending {
                        refcode::post::remote::code(
                            &refcode::CERT_ENDING,
                            &format!("it ends {when}"),
                        );
                    } else {
                        refcode::post::remote::code(
                            &refcode::CERT_REPLACED,
                            &format!("it ends {when}"),
                        );
                    }
                    let _ = &when;
                }
                was = Some(info.not_after < now.saturating_add(WARN));
            }
            std::thread::sleep(LOOK);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_certificate_is_said_ending_once_and_replaced_once() {
        let now = 1_800_000_000;
        assert_eq!(change(None, now + 365 * 86_400, now), None);
        assert_eq!(change(None, now + 86_400, now), Some(true));
        assert_eq!(change(Some(true), now + 86_400, now), None);
        assert_eq!(change(Some(true), now + 365 * 86_400, now), Some(false));
        assert_eq!(change(Some(false), now + 365 * 86_400, now), None);
        assert_eq!(change(Some(false), now + 29 * 86_400, now), Some(true));
    }
}
