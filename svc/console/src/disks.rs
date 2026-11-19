// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! `Work with disk units`: the load-source units with their location
//! codes, serial numbers, sizes and states, the protection, a second
//! disk added to an unprotected load source, a failed unit replaced and
//! its rebuild watched. The storage service does the work; these panels
//! ask it and show what it says.
//!
//! BLOCKED(phase2): these panels are a load source's, the same for every
//! appliance, but QS/VS's are in its `svc/tools` and not in
//! `base/panels` (`docs/OPEN_QUESTIONS.md` item 16). They are QS/MC's
//! own until they move.

use console::Screen;
use panels::tools::Confirm;
use panels::{Flow, Help, Named, Panel, kit, ui};
use state::doc::inventory::Disk;
use stgd::{MIRRORED, Status, Unit};

use crate::env::Env;
use crate::menu::{Outcome, Step};

const TITLE: &str = "Work with Disk Units";

/// The unit a replacement takes the place of: one that is missing,
/// failed or suspended.
pub fn failed_unit(status: &Status) -> Option<u32> {
    status
        .units
        .iter()
        .find(|u| matches!(u.state.as_str(), "Missing" | "Failed" | "Suspended"))
        .map(|u| u.number)
}

/// One unit's row: number, location, serial, size, state and any work.
pub fn unit_row(unit: &Unit, status: &Status) -> String {
    let mut state = unit.state.clone();
    if status.work_unit == unit.number && !status.work.is_empty() {
        state = format!("{state} {}%", status.percent);
    }
    let size = if unit.bytes == 0 {
        String::new()
    } else {
        kit::size(unit.bytes)
    };
    format!(
        "{:>4}  {} {} {size:>9}  {state}",
        unit.number,
        kit::fit(&unit.location, LOCATION),
        kit::fit(&unit.serial, SERIAL),
    )
}

/// The width of a location code's column, and of a serial number's.
const LOCATION: usize = 21;
const SERIAL: usize = 14;

/// The units' headings, over [`unit_row`]'s columns.
fn headings() -> String {
    format!(
        "Unit  {} {} {:>9}  State",
        kit::fit("Location", LOCATION),
        kit::fit("Serial", SERIAL),
        "Size"
    )
}

/// What the protection is, in a line.
pub fn protection(status: &Status) -> &'static str {
    if status.protection == MIRRORED {
        if status.whole() {
            "Mirrored; both units are in step"
        } else {
            "Mirrored; NOT protected now: a unit is not in step"
        }
    } else if status.restart_needed {
        "Mirrored protection starts at the next IPL"
    } else {
        "None: the load source is one disk"
    }
}

/// The lines of the display.
pub fn lines(status: &Status) -> Vec<String> {
    let mut out = vec![
        format!("{} {}", kit::dotted("Protection", 15), protection(status)),
        String::new(),
        headings(),
    ];
    out.extend(status.units.iter().map(|u| unit_row(u, status)));
    if !status.failure.is_empty() {
        out.push(kit::fit(&format!("Last request: {}", status.failure), 76));
    }
    out
}

/// The disks that could become a unit: no unit already, no optical
/// drive or the kernel's own, and large enough.
pub fn candidates(status: &Status, disks: Vec<Disk>) -> Vec<Disk> {
    disks
        .into_iter()
        .filter(|d| !status.units.iter().any(|u| u.disk == d.name))
        .filter(|d| {
            !["sr", "ublkb", "loop"]
                .iter()
                .any(|p| d.name.starts_with(p))
        })
        .filter(|d| d.bytes >= release::MIN_DISK_BYTES)
        .collect()
}

/// `Work with disk units`.
pub struct DiskUnits;

impl Panel<Env, Outcome> for DiskUnits {
    fn name(&self) -> &'static str {
        "console-disks"
    }

    fn render(&self, env: &mut Env) -> Screen {
        let Some(status) = env.storage() else {
            return kit::page(
                TITLE,
                &env.system(),
                &kit::lines(&["The storage service does not answer."]),
                "F3=Exit   F12=Cancel",
            );
        };
        ui::menu_after(
            TITLE,
            &env.system(),
            &lines(&status),
            &[
                (1, "Start mirrored protection"),
                (2, "Replace a failed unit"),
                (5, "Display the units again"),
            ],
            "F1=Help   F3=Exit   F12=Cancel",
        )
    }

    fn enter(&mut self, fields: &[String], env: &mut Env) -> Step {
        let Some(status) = env.storage() else {
            return Flow::Error("The storage service does not answer.".into());
        };
        match kit::number(fields.first()) {
            Some(1) if status.protection == MIRRORED || status.restart_needed => {
                Flow::Error("The load source is mirrored already.".into())
            }
            Some(1) => Flow::Push(Box::new(Select {
                purpose: Purpose::Protect,
            })),
            Some(2) => match failed_unit(&status) {
                Some(number) => Flow::Push(Box::new(Select {
                    purpose: Purpose::Replace(number),
                })),
                None => Flow::Error("No unit is missing or failed.".into()),
            },
            Some(5) => Flow::Stay,
            Some(_) => Flow::Error("Selection not valid.".into()),
            None => Flow::Error("Type the number of an option.".into()),
        }
    }

    fn help(&self) -> Help {
        Help {
            title: TITLE,
            lines: &[
                "The load source is the disk the console starts from and keeps",
                "its own data on, or a mirrored pair of disks. Each unit is",
                "shown by its location code, serial number, size and state.",
                "",
                "1 adds a second disk to a load source of one: it becomes a",
                "mirrored pair at the next IPL, and the new disk is written",
                "from the first. 2 puts a new disk in the place of a unit",
                "that is missing or failed; it is rebuilt while the console",
                "runs. The disk chosen is erased. 5 shows the units again.",
                "",
                "While a unit is not in step the console runs, but a second",
                "failure would stop it.",
            ],
        }
    }
}

/// Why a disk is chosen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Purpose {
    /// As the second unit of a load source of one.
    Protect,
    /// In the place of this unit.
    Replace(u32),
}

/// The choice of a disk.
struct Select {
    purpose: Purpose,
}

impl Select {
    fn disks(env: &mut Env) -> Vec<Disk> {
        let status = env.storage().unwrap_or_default();
        candidates(&status, crate::env::inventory().disks)
    }

    fn title(&self) -> &'static str {
        match self.purpose {
            Purpose::Protect => "Select Disk to Mirror With",
            Purpose::Replace(_) => "Select Replacement Disk",
        }
    }
}

impl Panel<Env, Outcome> for Select {
    fn name(&self) -> &'static str {
        "console-disk-select"
    }

    fn render(&self, env: &mut Env) -> Screen {
        let disks = Self::disks(env);
        let rows: Vec<String> = disks
            .iter()
            .map(|d| {
                format!(
                    "{} {} {} {:>9}",
                    kit::fit(&d.location, 23),
                    kit::fit(&d.model, 15),
                    kit::fit(&d.serial, 18),
                    kit::size(d.bytes)
                )
            })
            .collect();
        let mut s = ui::list(
            self.title(),
            &env.system(),
            "Type 1 by the disk to use. It is erased.",
            "Opt Location                Model           Serial                  Size",
            &rows,
            "Enter=Continue   F1=Help   F3=Exit   F12=Cancel",
        );
        if disks.is_empty() {
            kit::put(
                &mut s,
                ui::BODY_ROW + 3,
                &format!(
                    "(No disk is free and at least {})",
                    kit::size(release::MIN_DISK_BYTES)
                ),
            );
        }
        if let Purpose::Replace(n) = self.purpose {
            kit::put(
                &mut s,
                ui::BODY_ROW + 1,
                &format!("It takes the place of unit {n}."),
            );
        }
        s
    }

    fn enter(&mut self, fields: &[String], env: &mut Env) -> Step {
        let disks = Self::disks(env);
        let chosen: Vec<&Disk> = disks
            .iter()
            .zip(fields)
            .filter(|(_, f)| !f.trim().is_empty())
            .map(|(d, _)| d)
            .collect();
        let [disk] = chosen.as_slice() else {
            return Flow::Error("Type 1 by one disk.".into());
        };
        let (name, purpose) = (disk.name.clone(), self.purpose);
        let what = match purpose {
            Purpose::Protect => {
                "It becomes the second unit of a mirrored load source at the next IPL.".to_string()
            }
            Purpose::Replace(n) => format!("It takes the place of unit {n} and is rebuilt now."),
        };
        let text = vec![
            "Everything on this disk will be erased:".to_string(),
            String::new(),
            format!(
                "  {}  {}  serial {}  {}",
                disk.location,
                disk.model,
                disk.serial,
                kit::size(disk.bytes)
            ),
            String::new(),
            what,
        ];
        Flow::Push(Box::new(Confirm::new(
            "console-disk-confirm",
            "Confirm Erase of Disk",
            text,
            Help {
                title: "Confirm Erase of Disk",
                lines: &[
                    "F10 erases the disk named and gives it to the storage",
                    "service, which writes it from the unit the console runs on.",
                    "Nothing on the disk can be had back afterwards.",
                    "",
                    "F12 goes back to the disks.",
                ],
            },
            move |env: &mut Env| {
                let done = match purpose {
                    Purpose::Protect => env.start_protection(&name),
                    Purpose::Replace(n) => env.replace(n, &name),
                };
                match (done, purpose) {
                    (Ok(()), Purpose::Protect) => Flow::Unwind(
                        2,
                        Some(("Mirrored protection starts at the next IPL.".into(), false)),
                    ),
                    (Ok(()), Purpose::Replace(_)) => Flow::Replace(Box::new(Watch)),
                    (Err(e), _) => Flow::Unwind(1, Some((e, true))),
                }
            },
        )))
    }

    fn help(&self) -> Help {
        Help {
            title: "Select a disk",
            lines: &[
                "The disks of the machine that are not a load-source unit and",
                "are large enough, each by its location code, model, serial",
                "number and size. Type 1 by the one to use and press Enter;",
                "the next screen asks you to confirm. The disk is erased.",
                "",
                "A disk as large as the unit it mirrors or replaces is best:",
                "a mirrored pair has the size of its smaller disk.",
            ],
        }
    }
}

/// The rebuild of a replacement, watched.
struct Watch;

impl Panel<Env, Outcome> for Watch {
    fn name(&self) -> &'static str {
        "console-disk-watch"
    }

    fn render(&self, env: &mut Env) -> Screen {
        let Some(status) = env.storage() else {
            return kit::page(
                "Disk Unit Rebuild",
                &env.system(),
                &kit::lines(&["The storage service does not answer."]),
                "Enter=Continue",
            );
        };
        let mut text = lines(&status);
        text.push(String::new());
        text.push(if status.work.is_empty() {
            "No work is under way: every unit is as shown.".into()
        } else {
            format!(
                "Unit {}: {} {}%",
                status.work_unit, status.work, status.percent
            )
        });
        let percent =
            (!status.work.is_empty()).then(|| u8::try_from(status.percent.min(100)).unwrap_or(100));
        kit::enterable(ui::text(
            "Disk Unit Rebuild",
            &env.system(),
            &text,
            percent,
            "Enter=Continue   F1=Help",
        ))
    }

    // Back past the disk's selection, to the units.
    fn enter(&mut self, _fields: &[String], _env: &mut Env) -> Step {
        Flow::Unwind(2, None)
    }

    fn function(&mut self, key: u8, _fields: &[String], _env: &mut Env) -> Step {
        match key {
            3 | 12 => Flow::Unwind(2, None),
            _ => Flow::Stay,
        }
    }

    fn help(&self) -> Help {
        Help {
            title: "Disk unit rebuild",
            lines: &[
                "The new unit is written from the one the console runs on,",
                "while the console runs. The display is drawn again every",
                "second. When the work is done both units are Active and the",
                "load source is protected again.",
                "",
                "Leaving this display does not stop the rebuild.",
            ],
        }
    }

    fn live(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unit(number: u32, state: &str, disk: &str) -> Unit {
        Unit {
            number,
            disk: disk.into(),
            serial: format!("QSMCHD{number}"),
            location: format!("P0000-B00-D0{}-F0-U0", number + 4),
            bytes: 64 << 30,
            state: state.into(),
            written: 0,
        }
    }

    #[test]
    fn the_units_are_shown_by_location_with_state_and_work() {
        let status = Status {
            protection: MIRRORED.into(),
            units: vec![unit(1, "Active", "vda"), unit(2, "Rebuilding", "vdc")],
            work: "rebuilding".into(),
            work_unit: 2,
            percent: 40,
            ..Status::default()
        };
        let shown = lines(&status);
        assert!(shown[0].contains("NOT protected now"));
        assert!(shown[3].contains("P0000-B00-D05-F0-U0"));
        assert!(shown[4].ends_with("Rebuilding 40%"));
        assert_eq!(shown[2].find("State"), shown[3].find("Active"));
        assert!(shown.iter().all(|l| l.len() <= 78));
    }

    #[test]
    fn a_missing_unit_is_the_one_replaced_and_units_are_no_candidates() {
        let status = Status {
            protection: MIRRORED.into(),
            units: vec![unit(1, "Active", "vda"), unit(2, "Missing", "")],
            ..Status::default()
        };
        assert_eq!(failed_unit(&status), Some(2));
        let disk = |name: &str, bytes: u64| Disk {
            name: name.into(),
            bytes,
            ..Disk::default()
        };
        let found = candidates(
            &status,
            vec![
                disk("vda", 64 << 30),
                disk("vdb", 64 << 30),
                disk("vdc", 8 << 30),
                disk("sr0", 64 << 30),
                disk("ublkb0", 64 << 30),
            ],
        );
        let names: Vec<&str> = found.iter().map(|d| d.name.as_str()).collect();
        assert_eq!(names, ["vdb"]);
        let single = Status {
            protection: stgd::NONE.into(),
            units: vec![unit(1, "Active", "vda")],
            ..Status::default()
        };
        assert_eq!(failed_unit(&single), None);
        assert_eq!(protection(&single), "None: the load source is one disk");
    }
}
