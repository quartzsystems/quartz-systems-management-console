// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! The pairing password, as a QS/VS host keeps one: good for one
//! pairing, a time and a number of attempts; the guard `qsitrust`
//! asks; and the pins TLS checks a console against.

use std::time::{Duration, Instant};

use qsitrust::pair::{Guard, Paired};
use qsitrust::{Fingerprint, Pins};

use super::{Shared, System, lock};

/// How long a password is good for, as a QS/VS host's.
pub const GOOD_FOR: Duration = Duration::from_mins(15);
/// How many attempts a password has, as a QS/VS host's.
pub const ATTEMPTS: u32 = 5;

/// A pairing password out.
#[derive(Clone, Debug)]
pub struct Password {
    /// The password as shown.
    pub text: String,
    /// When it stops being good.
    pub until: Instant,
    /// Attempts left.
    pub attempts: u32,
}

impl System {
    /// Make a password, shown once: what the operator reads at the
    /// managed system.
    pub fn start_pairing(&mut self) -> String {
        let text =
            qsitrust::pair::one_time_password().unwrap_or_else(|_| "0000-0000-0000-0000".into());
        self.password = Some(Password {
            text: text.clone(),
            until: Instant::now() + GOOD_FOR,
            attempts: ATTEMPTS,
        });
        self.note("pairing password made".into());
        text
    }

    /// Let the password out expire now.
    pub fn expire_pairing(&mut self) {
        if let Some(p) = &mut self.password {
            p.until = Instant::now();
        }
    }

    /// Forget a console's pin.
    pub fn unpin(&mut self, fp: &Fingerprint) {
        self.pins.retain(|(f, _)| f != fp);
    }

    fn password_out(&self) -> Option<&Password> {
        self.password
            .as_ref()
            .filter(|p| Instant::now() < p.until && p.attempts > 0)
    }
}

/// The guard over a system's password, for one pairing's exchange.
pub struct SystemGuard<'a>(pub &'a mut System);

impl Guard for SystemGuard<'_> {
    fn password(&mut self) -> Option<String> {
        let out = self.0.password_out()?.text.clone();
        if let Some(p) = &mut self.0.password {
            p.attempts -= 1;
        }
        Some(out)
    }

    fn failed(&mut self) {
        self.0.note("pairing attempt did not confirm".into());
    }

    fn paired(&mut self, console: &Paired) {
        self.0.password = None;
        if self.0.pins.len() < self.0.max_pins {
            self.0
                .pins
                .push((console.fingerprint, console.name.clone()));
        }
        self.0.note(format!("console {} paired", console.name));
    }
}

/// What TLS accepts of a console: a pinned one, or, while a password is
/// out, any.
#[derive(Clone)]
pub struct SystemPins(pub Shared);

impl Pins for SystemPins {
    fn pinned(&self, peer: &Fingerprint) -> bool {
        lock(&self.0).pins.iter().any(|(f, _)| f == peer)
    }

    fn pairing_open(&self) -> bool {
        let s = lock(&self.0);
        s.password_out().is_some() && s.pins.len() < s.max_pins
    }
}

impl std::fmt::Debug for SystemPins {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("SystemPins")
    }
}
