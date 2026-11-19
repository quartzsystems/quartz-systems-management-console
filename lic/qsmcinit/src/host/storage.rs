// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! The load source as the status screen shows it: the storage service's
//! status, asked for by a thread of its own every two seconds. PID 1's
//! loop only reads what the thread last found, so a storage service
//! that is slow or gone never holds it up.

use std::fmt::Write as _;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use stgd::{Calls, Client, Status};

/// How often the storage service is asked.
const EVERY: Duration = Duration::from_secs(2);

/// The line for `status`: the protection, each unit's state and disk,
/// and what the service is doing.
pub fn line(status: &Status) -> String {
    // What the service answers before it has found the disks.
    if status.protection.is_empty() && status.units.is_empty() {
        return "not known: the storage service is finding the load-source disks".into();
    }
    let units: Vec<String> = status
        .units
        .iter()
        .map(|u| {
            if u.disk.is_empty() {
                format!("unit {} {}", u.number, u.state)
            } else {
                format!("unit {} {} on {}", u.number, u.state, u.disk)
            }
        })
        .collect();
    let mut out = format!("{}: {}", status.protection, units.join(", "));
    if !status.work.is_empty() {
        let _ = write!(
            out,
            "; {} unit {}, {}%",
            status.work, status.work_unit, status.percent
        );
    }
    if status.choose {
        out.push_str("; the units ran apart: the unit to keep is to be chosen");
    }
    out
}

/// What the thread last found.
#[derive(Clone)]
pub struct Watched(Arc<Mutex<String>>);

impl Watched {
    /// The line as last found.
    pub fn get(&self) -> String {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }
}

/// Start the thread that asks the storage service of `appliance`.
pub fn watch(appliance: &appliance::Appliance) -> Watched {
    let shown = Watched(Arc::new(Mutex::new(
        "not known: the storage service has not answered".to_string(),
    )));
    let to = shown.clone();
    let mut client = Client::of(appliance);
    let _ = std::thread::Builder::new()
        .name("load-source".into())
        .spawn(move || {
            loop {
                let now = match client.status() {
                    Ok(status) => line(&status),
                    Err(e) => format!("not known: the storage service does not answer ({e})"),
                };
                *to.0.lock().unwrap_or_else(PoisonError::into_inner) = now;
                std::thread::sleep(EVERY);
            }
        });
    shown
}

#[cfg(test)]
mod tests {
    use super::*;
    use stgd::Unit;

    fn unit(number: u32, disk: &str, state: &str) -> Unit {
        Unit {
            number,
            disk: disk.into(),
            state: state.into(),
            ..Unit::default()
        }
    }

    #[test]
    fn the_line_names_each_unit_and_the_work() {
        let one = Status {
            protection: stgd::NONE.into(),
            units: vec![unit(1, "vda", "Unprotected")],
            ..Status::default()
        };
        assert_eq!(line(&one), "none: unit 1 Unprotected on vda");
        assert!(line(&Status::default()).starts_with("not known"));
        let pair = Status {
            protection: stgd::MIRRORED.into(),
            units: vec![unit(1, "vda", "Active"), unit(2, "", "Missing")],
            work: "resynchronizing".into(),
            work_unit: 2,
            percent: 40,
            ..Status::default()
        };
        assert_eq!(
            line(&pair),
            "mirrored: unit 1 Active on vda, unit 2 Missing; resynchronizing unit 2, 40%"
        );
    }
}
