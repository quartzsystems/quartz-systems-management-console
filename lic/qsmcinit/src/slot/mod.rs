// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! The slot rule. The firmware chooses the slot by boot entry; the A/B
//! record (`loadsource::record`) holds the default, the slot on trial
//! and what was last confirmed in each. A slot booted on trial becomes
//! the default, in the record on every load-source disk and in
//! `BootOrder`, when [`rule`] says the console is up. One that never
//! gets there was only `BootNext`: the next IPL is on the old default,
//! clears the pending field and says so. Nothing here writes a UKI.

/// When a slot on trial is confirmed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rule {
    /// When every base service has said it is ready.
    BaseReady,
    /// When the service named has said it is ready.
    ServiceReady(&'static str),
}

/// The rule. From Phase 3 it is the web server's first answer over
/// TLS, which is when it says it is ready. A processor that cannot run
/// the web server has none: there it is the base's readiness, or no
/// slot could ever confirm.
pub fn rule(table: &[supervise::Service], web: &'static str) -> Rule {
    if table.iter().any(|s| s.name == web) {
        Rule::ServiceReady(web)
    } else {
        Rule::BaseReady
    }
}

impl Rule {
    /// Whether the slot is confirmed now that `service` is ready, or,
    /// with `None`, now that every service is.
    pub fn due(self, service: Option<&str>) -> bool {
        match (self, service) {
            (Rule::BaseReady, None) => true,
            (Rule::ServiceReady(name), Some(ready)) => name == ready,
            _ => false,
        }
    }
}

#[cfg(target_os = "linux")]
pub use on_console::{Slots, at_ipl};

#[cfg(target_os = "linux")]
mod on_console {
    use std::fs::File;
    use std::path::Path;

    use abrec::Slot;
    use diskimg::efi::EspLocation;
    use loadsource::boot::{self, EFIVARS};
    use loadsource::disks::{self, LoadSource};
    use loadsource::multi::{At, Copies};
    use loadsource::record::{self, HostRecord, Ipl, letter};
    use refcode::RefCode;

    /// The record and what this IPL is by it.
    pub struct Slots {
        /// The slot this IPL came from.
        pub booted: Slot,
        /// The record as it stands, when it could be read or made.
        pub record: Option<HostRecord>,
        /// What this IPL is.
        pub ipl: Ipl,
        /// The record on every load-source disk.
        part: Option<Copies<At<File>>>,
        /// The ESP of every load-source disk.
        esps: Vec<EspLocation>,
    }

    /// The record step of an IPL from `booted` on the load-source
    /// disks `sources`: read the record (the newest on any disk), note
    /// what this IPL is, and write the record back to every disk when
    /// that changes it. `post` is given each code with its detail.
    pub fn at_ipl(
        booted: Slot,
        sources: &[LoadSource],
        mut post: impl FnMut(&'static RefCode, String),
    ) -> Slots {
        let mut slots = Slots {
            booted,
            record: None,
            ipl: Ipl::Normal,
            part: None,
            esps: sources.iter().map(|s| s.esp_at).collect(),
        };
        if sources.is_empty() {
            post(
                &refcode::RECORD_INVALID,
                "no load-source disk was found".into(),
            );
            return slots;
        }
        let mut part = match disks::records(&release::LOAD_SOURCE, sources) {
            Ok(part) => part,
            Err(e) => {
                post(&refcode::RECORD_INVALID, format!("not read: {e}"));
                return slots;
            }
        };
        let read = record::read_all(&mut part);
        let (ipl, now) = match &read {
            Ok((found, _)) => found.at_ipl(booted),
            // No copy is a record: start one from the slot that booted.
            Err(_) => (Ipl::Normal, HostRecord::new(booted)),
        };
        // A disk whose copy is behind (it was away) is brought to the
        // record's sequence too.
        let behind = part
            .each()
            .iter()
            .any(|c| c.as_ref().map(|c| c.sequence) != read.as_ref().ok().map(|r| r.1));
        let written = match &read {
            Ok((found, _)) if *found == now && !behind => Ok(()),
            _ => record::write_all(&mut part, now).map(|_| ()),
        };
        match (&read, &written) {
            (Err(e), Ok(())) => post(
                &refcode::RECORD_INVALID,
                format!("{e}; a new one is written, {}", now.summary()),
            ),
            (Err(e), Err(w)) => post(&refcode::RECORD_INVALID, format!("{e}; not written: {w}")),
            (Ok(_), Err(w)) => post(&refcode::RECORD_INVALID, format!("not written: {w}")),
            (Ok(_), Ok(())) => {}
        }
        let disks = match sources.len() {
            1 => "1 load-source disk".to_string(),
            n => format!("{n} load-source disks"),
        };
        post(&refcode::RECORD_READ, format!("{}; {disks}", now.summary()));
        if let Ipl::FellBack { pending } = ipl {
            post(
                &refcode::SLOT_FELL_BACK,
                format!(
                    "slot {}; this IPL is on slot {}",
                    letter(pending),
                    letter(booted)
                ),
            );
        }
        slots.record = Some(now);
        slots.ipl = ipl;
        slots.part = Some(part);
        slots
    }

    impl Slots {
        /// The rule says the console is up. A slot on trial is
        /// confirmed: the record first, then the firmware's
        /// `BootOrder`. Any other IPL only notes its release in the
        /// record.
        pub fn confirm(&mut self, mut post: impl FnMut(&'static RefCode, String)) {
            let (Some(current), Some(part)) = (self.record, self.part.as_mut()) else {
                return;
            };
            let trial = self.ipl == Ipl::Trial;
            let next = if trial {
                current.confirm(self.booted, release::RELEASE)
            } else {
                current.with_release(self.booted, release::RELEASE)
            };
            if next != current {
                if let Err(e) = record::write_all(part, next) {
                    post(&refcode::RECORD_INVALID, format!("not written: {e}"));
                    return;
                }
                self.record = Some(next);
            }
            if !trial {
                return;
            }
            self.ipl = Ipl::Normal;
            let booted = letter(self.booted);
            match boot::make_default(
                Path::new(EFIVARS),
                &release::LOAD_SOURCE,
                release::NAME,
                &self.esps,
                self.booted,
            ) {
                Ok(order) => {
                    let list: Vec<String> = order.iter().map(|n| format!("{n:04X}")).collect();
                    post(
                        &refcode::SLOT_CONFIRMED,
                        format!("slot {booted} is the default; BootOrder {}", list.join(",")),
                    );
                }
                Err(e) => {
                    post(
                        &refcode::SLOT_CONFIRMED,
                        format!("slot {booted} is the default in the record"),
                    );
                    post(&refcode::BOOT_ORDER_NOT_WRITTEN, e);
                }
            }
        }

        /// The record as the control socket gives it.
        pub fn state(&self) -> supervise::control::SlotState {
            let slot = |s: Slot| letter(s).to_string();
            let current = self.record.unwrap_or(HostRecord::new(self.booted));
            supervise::control::SlotState {
                booted: slot(self.booted),
                default: slot(current.default_slot),
                pending: current.pending.map(slot).unwrap_or_default(),
                activated: current.pending.is_some() && current.attempts == 0,
                confirmed: current
                    .confirmed
                    .iter()
                    .map(|r| r.map(|r| r.to_string()).unwrap_or_default())
                    .collect(),
                fell_back: match self.ipl {
                    Ipl::FellBack { pending } => slot(pending),
                    _ => String::new(),
                },
            }
        }

        /// The record in words, or that there is none.
        pub fn summary(&self) -> String {
            self.record
                .map_or_else(|| "no A/B record".to_string(), |r| r.summary())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(target_os = "linux")]
    fn the_rule_is_the_web_server_s_answer_where_there_is_one() {
        let with = crate::host::table::TABLE;
        let without = crate::host::table::WITHOUT_WEB;
        assert_eq!(rule(without, "webd"), Rule::BaseReady);
        assert!(rule(without, "webd").due(None));
        assert!(!rule(without, "webd").due(Some("netd")));
        let later = rule(with, "webd");
        assert_eq!(later, Rule::ServiceReady("webd"));
        assert!(later.due(Some("webd")));
        assert!(!later.due(Some("netd")));
        assert!(!later.due(None));
    }
}
