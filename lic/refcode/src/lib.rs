// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! Console reference codes. One table, no scattered strings.
//!
//! Every code is eight characters, a family and six digits. A console's
//! families end in `3` (docs/REFCODES.md), which no family of QS/400 or
//! of QS/VS does: `C3` IPL and progress, `A3` attention, `B3` fatal,
//! `D3` power down. Every `A3` and `B3` code says what an operator
//! should do.
//!
//! - [`event`]: the code of each event the base crates report.
//! - [`post`]: the poster: the console line and the history record,
//!   and how a service posts through PID 1.

#![forbid(unsafe_code)]

pub mod event;
pub mod post;

/// What a code's family says of it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Family {
    /// `C3`: the console's IPL and its progress.
    Progress,
    /// `A3`: attention, the console continues.
    Attention,
    /// `B3`: fatal, the console parks.
    Fatal,
    /// `D3`: power down.
    PowerDown,
}

impl Family {
    /// The two characters a code of the family starts with.
    pub const fn prefix(self) -> &'static str {
        match self {
            Family::Progress => "C3",
            Family::Attention => "A3",
            Family::Fatal => "B3",
            Family::PowerDown => "D3",
        }
    }
}

/// One reference code.
#[derive(Debug, PartialEq, Eq)]
pub struct RefCode {
    /// Eight-character code.
    pub code: &'static str,
    /// Short text.
    pub text: &'static str,
    /// Family.
    pub family: Family,
    /// What an operator should do: every `A3` and `B3` code has it,
    /// no other code does.
    pub action: &'static str,
}

const fn progress(code: &'static str, text: &'static str) -> RefCode {
    RefCode {
        code,
        text,
        family: Family::Progress,
        action: "",
    }
}

const fn attention(code: &'static str, text: &'static str, action: &'static str) -> RefCode {
    RefCode {
        code,
        text,
        family: Family::Attention,
        action,
    }
}

const fn fatal(code: &'static str, text: &'static str, action: &'static str) -> RefCode {
    RefCode {
        code,
        text,
        family: Family::Fatal,
        action,
    }
}

const fn power_down(code: &'static str, text: &'static str) -> RefCode {
    RefCode {
        code,
        text,
        family: Family::PowerDown,
        action: "",
    }
}

// C3001xxx: PID 1's own IPL, in the order it is posted.

/// PID 1 started the console's IPL.
pub const IPL_STARTED: RefCode = progress("C3001000", "Console IPL started");
/// The heartbeat, on the serial port only.
pub const HEARTBEAT: RefCode = progress("C3001005", "Console running");
/// proc, sysfs, devtmpfs, `/run`, efivarfs and cgroup2 are mounted.
pub const MOUNTED: RefCode = progress("C3001010", "Kernel file systems mounted");
/// The load-source disks were found and the A/B record read.
pub const RECORD_READ: RefCode = progress("C3001030", "A/B record read");
/// `QSMCDATA` is mounted at `/data`.
pub const DATA_MOUNTED: RefCode = progress("C3001040", "Data file system mounted");
/// A restore waiting for this IPL was put in place.
pub const RESTORE_APPLIED: RefCode = progress("C3001045", "Restore applied");
/// The system clock was set from the RTC.
pub const CLOCK_SET: RefCode = progress("C3001050", "Clock set from the RTC");
/// The seed step ran and applied nothing.
pub const SEED_CHECKED: RefCode = progress("C3001060", "Seed checked");
/// The state files were written from a seed volume.
pub const SEED_APPLIED: RefCode = progress("C3001061", "Seed applied");
/// The state files were read and the host name set.
pub const STATE_LOADED: RefCode = progress("C3001070", "State loaded");
/// The console's identity was made: its first IPL with a `/data`.
pub const IDENTITY_GENERATED: RefCode = progress("C3001075", "Console identity generated");
/// The service table was started.
pub const SERVICES_STARTED: RefCode = progress("C3001080", "Services started");
/// Every base service is ready.
pub const BASE_READY: RefCode = progress("C3001090", "Base ready");
/// The slot this IPL came from is confirmed as the default.
pub const SLOT_CONFIRMED: RefCode = progress("C3001095", "Slot confirmed");
/// A restore on trial was kept: the state before it is gone.
pub const RESTORE_CONFIRMED: RefCode = progress("C3001096", "Restore confirmed");

// C3002xxx: the management network and time.

/// The management interface is up and has carrier.
pub const INTERFACE_UP: RefCode = progress("C3002010", "Management interface up");
/// The management interface has its address.
pub const ADDRESS_ACQUIRED: RefCode = progress("C3002020", "Management address acquired");
/// The clock is in step with a time server.
pub const TIME_SYNCHRONIZED: RefCode = progress("C3002030", "Time synchronized");

// C3003xxx: the load source and its protection.

/// The mirrored load source was assembled and is served.
pub const SET_ASSEMBLED: RefCode = progress("C3003010", "Mirrored load source assembled");
/// A unit is being brought back in step.
pub const RESYNC_STARTED: RefCode = progress("C3003020", "Resynchronization started");
/// The unit is in step again.
pub const RESYNC_FINISHED: RefCode = progress("C3003025", "Resynchronization finished");
/// A replacement unit is being rebuilt.
pub const REBUILD_STARTED: RefCode = progress("C3003030", "Rebuild started");
/// The replacement unit is in step.
pub const REBUILD_FINISHED: RefCode = progress("C3003035", "Rebuild finished");
/// Mirrored protection was started; it takes effect at the next IPL.
pub const PROTECTION_STARTED: RefCode = progress("C3003040", "Mirrored protection started");
/// The unit to keep of two that ran apart was chosen.
pub const UNIT_KEPT: RefCode = progress("C3003050", "Load-source unit kept");

// C3004xxx: the installer, in the order of its steps.

/// The installation's first write.
pub const INSTALL_STARTED: RefCode = progress("C3004000", "Installation started");
/// The media's manifest is signed by a trusted key and its files match.
pub const INSTALL_VERIFIED: RefCode = progress("C3004010", "Installation media verified");
/// Each load-source disk has its partition table.
pub const INSTALL_PARTITIONED: RefCode = progress("C3004020", "Disks partitioned");
/// The mirrored set's label is written, or there is no protection.
pub const INSTALL_PROTECTED: RefCode = progress("C3004030", "Protection set up");
/// `QSMCDATA` is made.
pub const INSTALL_FORMATTED: RefCode = progress("C3004040", "Data file system made");
/// Slot A's UKI is on every disk's ESP.
pub const INSTALL_SLOT_A: RefCode = progress("C3004050", "Slot A copied");
/// Slot B's UKI is on every disk's ESP.
pub const INSTALL_SLOT_B: RefCode = progress("C3004055", "Slot B copied");
/// The A/B record is on every disk.
pub const INSTALL_RECORD: RefCode = progress("C3004060", "A/B record written");
/// The firmware has an entry for each disk.
pub const INSTALL_BOOT: RefCode = progress("C3004070", "Boot entries written");
/// The state files and the first service tools user are written.
pub const INSTALL_STATE: RefCode = progress("C3004080", "State written");
/// Every step is done.
pub const INSTALL_COMPLETE: RefCode = progress("C3004090", "Installation complete");

// A3001xxx: attention from PID 1, numbered after the IPL step.

/// A kernel file system did not mount.
pub const NOT_MOUNTED: RefCode = attention(
    "A3001010",
    "Kernel file systems not mounted",
    "Note the file systems named and report them with this code; the console runs without them",
);
/// The processor lacks what the cryptography of QSI and the web server
/// needs.
pub const PROCESSOR_LACKS: RefCode = attention(
    "A3001015",
    "Processor lacks what the cryptography needs",
    "Run the console on a processor with AVX2, ADX, AES and PCLMULQDQ, or give a virtual machine the host's processor model",
);
/// The A/B record could not be read or written.
pub const RECORD_INVALID: RefCode = attention(
    "A3001030",
    "A/B record not valid",
    "The console runs from the slot the firmware started; if this is posted at every IPL, the load-source disk may be failing: replace it",
);
/// The pending slot did not confirm: this IPL is on the other one.
pub const SLOT_FELL_BACK: RefCode = attention(
    "A3001035",
    "Pending slot not confirmed",
    "The console went back to the slot it ran before; the update in the other slot did not start its services and is not used",
);
/// `/data` could not be mounted: state is in memory only.
pub const DATA_NOT_MOUNTED: RefCode = attention(
    "A3001040",
    "Data file system not mounted",
    "Nothing is kept across a restart: check the load-source disks and their connections; the data file system is never reformatted by the console",
);
/// A restore was not confirmed: the state before it is back.
pub const RESTORE_TAKEN_BACK: RefCode = attention(
    "A3001045",
    "Restore taken back",
    "The console runs on the state it had before the restore; restore the backup again, and if it is taken back again, report this code with the system log",
);
/// A restore waiting for this IPL could not be used.
pub const RESTORE_REFUSED: RefCode = attention(
    "A3001046",
    "Restore refused",
    "Nothing was changed; restore a backup made by a console of this release or an earlier one",
);
/// The RTC could not be read.
pub const CLOCK_NOT_SET: RefCode = attention(
    "A3001050",
    "Clock not set from the RTC",
    "Configure a time server; if the machine has a real-time clock, check its battery",
);
/// A seed volume was found and refused.
pub const SEED_INVALID: RefCode = attention(
    "A3001060",
    "Seed not valid",
    "Correct seed.toml on the QSMCSEED volume as the text says and restart; the console runs on its defaults meanwhile",
);
/// A state file was refused: its defaults are in use.
pub const STATE_INVALID: RefCode = attention(
    "A3001070",
    "State file not valid",
    "The file named is not used and its defaults are; set those settings again",
);
/// The identity could not be made.
pub const IDENTITY_NOT_GENERATED: RefCode = attention(
    "A3001075",
    "Console identity not generated",
    "It is made again at the next IPL; if this is posted again, check that the data file system has room",
);
/// A service failed too often and is no longer restarted.
pub const SERVICE_STOPPED: RefCode = attention(
    "A3001080",
    "Service stopped",
    "Restart the console; if the service stops again, report this code with the system log",
);
/// The firmware's boot order could not be written at confirmation.
pub const BOOT_ORDER_NOT_WRITTEN: RefCode = attention(
    "A3001095",
    "Boot order not written",
    "The record holds the slot; in the firmware's setup, put the confirmed slot's entry first",
);

// A3002xxx: attention from the management network.

/// No interface to manage the console through.
pub const NO_INTERFACE: RefCode = attention(
    "A3002005",
    "No management interface",
    "Connect a cable to a supported Ethernet port, or correct the device named in the network settings",
);
/// The management interface has had no carrier for too long.
pub const NO_CARRIER: RefCode = attention(
    "A3002010",
    "No carrier on the management interface",
    "Check the cable and the switch port of the interface named",
);
/// The management interface has had no address for too long.
pub const NO_ADDRESS: RefCode = attention(
    "A3002020",
    "No management address",
    "Check that a DHCP server answers on the interface's network, or set a static address",
);

// A3003xxx: attention from the load source.

/// The mirrored set could not be assembled.
pub const SET_NOT_ASSEMBLED: RefCode = attention(
    "A3003010",
    "Mirrored load source not assembled",
    "Check that both load-source disks are present and connected, then restart",
);
/// A unit is missing or failed.
pub const UNIT_FAILED: RefCode = attention(
    "A3003020",
    "Load-source unit failed",
    "The console runs on the other unit without protection: return or replace the disk of the unit named",
);
/// A resynchronization or rebuild ended before the unit was in step.
pub const REBUILD_FAILED: RefCode = attention(
    "A3003030",
    "Rebuild not finished",
    "The unit is still out of step; it is tried again at the next IPL. If it fails again, replace its disk",
);
/// The units of the pair each ran without the other.
pub const UNITS_APART: RefCode = attention(
    "A3003040",
    "Load-source units ran apart",
    "Each disk ran without the other: choose the unit to keep; the other is written again from it",
);
/// The boot entries for a new unit could not be written.
pub const BOOT_ENTRIES_NOT_WRITTEN: RefCode = attention(
    "A3003070",
    "Boot entries not written",
    "The new unit holds both slots but the firmware has no entry for it: add one in the firmware's setup",
);

// A3004xxx: attention from the installer.

/// The media failed its check; nothing was written.
pub const INSTALL_MEDIA_REFUSED: RefCode = attention(
    "A3004010",
    "Installation media refused",
    "Nothing was written: install from media of a QS/MC build, written whole, and start again",
);
/// The answers of a seed volume cannot be used.
pub const INSTALL_ANSWERS_INVALID: RefCode = attention(
    "A3004015",
    "Installation answers not valid",
    "Nothing was written: correct seed.toml as the screen says, or install by hand",
);
/// The firmware took no boot entry; the disks start by their default path.
pub const INSTALL_BOOT_NOT_WRITTEN: RefCode = attention(
    "A3004070",
    "Installation boot entries not written",
    "The disks start by the firmware's default path; if the console does not start, add an entry for each disk in the firmware's setup",
);
/// A step of the installation failed.
pub const INSTALL_FAILED: RefCode = attention(
    "A3004090",
    "Installation failed",
    "The disks are partly written: start the installation over; if the same step fails, check or replace the disk named",
);

// B3xxxxxx: fatal; the console parks.

/// PID 1 panicked.
pub const PANIC: RefCode = fatal(
    "B3001000",
    "Console PID 1 failed",
    "Restart the console; report this code with the text after it",
);
/// The storage service is gone for good under a mounted `/data`.
pub const STORAGE_LOST: RefCode = fatal(
    "B3003000",
    "Storage service lost",
    "The data file system has no server and nothing more is written: restart the console",
);

// D3001xxx: power down.

/// An orderly shutdown began.
pub const SHUTDOWN_STARTED: RefCode = power_down("D3001000", "Console shutdown started");
/// The console powers off.
pub const POWER_OFF: RefCode = power_down("D3001090", "Console powering off");
/// The console restarts.
pub const RESTART: RefCode = power_down("D3001091", "Console restarting");

/// Every code, in the order docs/REFCODES.md lists them.
pub const ALL: &[&RefCode] = &[
    &IPL_STARTED,
    &HEARTBEAT,
    &MOUNTED,
    &RECORD_READ,
    &DATA_MOUNTED,
    &RESTORE_APPLIED,
    &CLOCK_SET,
    &SEED_CHECKED,
    &SEED_APPLIED,
    &STATE_LOADED,
    &IDENTITY_GENERATED,
    &SERVICES_STARTED,
    &BASE_READY,
    &SLOT_CONFIRMED,
    &RESTORE_CONFIRMED,
    &INTERFACE_UP,
    &ADDRESS_ACQUIRED,
    &TIME_SYNCHRONIZED,
    &SET_ASSEMBLED,
    &RESYNC_STARTED,
    &RESYNC_FINISHED,
    &REBUILD_STARTED,
    &REBUILD_FINISHED,
    &PROTECTION_STARTED,
    &UNIT_KEPT,
    &INSTALL_STARTED,
    &INSTALL_VERIFIED,
    &INSTALL_PARTITIONED,
    &INSTALL_PROTECTED,
    &INSTALL_FORMATTED,
    &INSTALL_SLOT_A,
    &INSTALL_SLOT_B,
    &INSTALL_RECORD,
    &INSTALL_BOOT,
    &INSTALL_STATE,
    &INSTALL_COMPLETE,
    &NOT_MOUNTED,
    &PROCESSOR_LACKS,
    &RECORD_INVALID,
    &SLOT_FELL_BACK,
    &DATA_NOT_MOUNTED,
    &RESTORE_TAKEN_BACK,
    &RESTORE_REFUSED,
    &CLOCK_NOT_SET,
    &SEED_INVALID,
    &STATE_INVALID,
    &IDENTITY_NOT_GENERATED,
    &SERVICE_STOPPED,
    &BOOT_ORDER_NOT_WRITTEN,
    &NO_INTERFACE,
    &NO_CARRIER,
    &NO_ADDRESS,
    &SET_NOT_ASSEMBLED,
    &UNIT_FAILED,
    &REBUILD_FAILED,
    &UNITS_APART,
    &BOOT_ENTRIES_NOT_WRITTEN,
    &INSTALL_MEDIA_REFUSED,
    &INSTALL_ANSWERS_INVALID,
    &INSTALL_BOOT_NOT_WRITTEN,
    &INSTALL_FAILED,
    &PANIC,
    &STORAGE_LOST,
    &SHUTDOWN_STARTED,
    &POWER_OFF,
    &RESTART,
];

/// The word a posted line starts with.
pub const WORD: &str = "REF";

/// The code spelled `code`, when the table has it.
pub fn find(code: &str) -> Option<&'static RefCode> {
    ALL.iter().copied().find(|c| c.code == code)
}

impl RefCode {
    /// `REF C3001000`, what a reader of the console looks for.
    pub fn tag(&self) -> String {
        format!("{WORD} {}", self.code)
    }

    /// The line as posted: `REF C3001000 Console IPL started`.
    pub fn line(&self) -> String {
        format!("{WORD} {} {}", self.code, self.text)
    }

    /// The text with what this posting adds: `...: /run`.
    pub fn text_with(&self, detail: &str) -> String {
        if detail.is_empty() {
            self.text.to_string()
        } else {
            format!("{}: {detail}", self.text)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// QS/400's families (`lic/slic/src/src.rs` there, at the pinned
    /// commit) and QS/VS's (`lic/refcode/src/lib.rs` there).
    const QS400_FAMILIES: &[&str] = &["C1", "C6", "A6", "B1", "D1"];
    const QSVS_FAMILIES: &[&str] = &["C2", "A2", "B2", "D2"];

    #[test]
    fn codes_are_well_formed_and_unique() {
        for (i, c) in ALL.iter().enumerate() {
            assert_eq!(c.code.len(), 8, "{}", c.code);
            assert!(c.code.starts_with(c.family.prefix()), "{}", c.code);
            assert!(
                c.code[2..].bytes().all(|b| b.is_ascii_digit()),
                "{}",
                c.code
            );
            assert!(ALL[..i].iter().all(|o| o.code != c.code), "{}", c.code);
            assert!(!c.text.is_empty() && !c.text.contains(':'), "{}", c.code);
        }
    }

    #[test]
    fn every_attention_and_fatal_code_says_what_to_do_and_no_other() {
        for c in ALL {
            let needs = matches!(c.family, Family::Attention | Family::Fatal);
            assert_eq!(!c.action.is_empty(), needs, "{}", c.code);
            assert!(!c.action.contains('|'), "{}", c.code);
        }
    }

    #[test]
    fn no_code_is_in_a_qs400_or_qsvs_family() {
        for c in ALL {
            let family = &c.code[..2];
            assert!(!QS400_FAMILIES.contains(&family), "{}", c.code);
            assert!(!QSVS_FAMILIES.contains(&family), "{}", c.code);
            assert_eq!(&family[1..], "3", "{}", c.code);
        }
        // And no family of either ends in `3`.
        for f in QS400_FAMILIES.iter().chain(QSVS_FAMILIES) {
            assert!(!f.ends_with('3'), "{f}");
        }
    }

    #[test]
    fn a_line_is_the_word_the_code_and_the_text() {
        assert_eq!(IPL_STARTED.line(), "REF C3001000 Console IPL started");
        assert_eq!(HEARTBEAT.tag(), "REF C3001005");
        assert_eq!(
            NOT_MOUNTED.text_with("/run"),
            "Kernel file systems not mounted: /run"
        );
        assert_eq!(MOUNTED.text_with(""), "Kernel file systems mounted");
        assert_eq!(find("B3001000"), Some(&PANIC));
        assert_eq!(find("C2001000"), None);
    }

    /// docs/REFCODES.md lists every code with its text, and what to do
    /// for each `A3` and `B3` code, and no other code.
    #[test]
    fn the_document_lists_the_table() {
        let doc = include_str!("../../../docs/REFCODES.md");
        for c in ALL {
            let row = format!("| `{}` | {} |", c.code, c.text);
            let line = doc
                .lines()
                .find(|l| l.starts_with(&row))
                .unwrap_or_else(|| panic!("docs/REFCODES.md lacks: {row}"));
            if !c.action.is_empty() {
                assert!(
                    line.trim_end().ends_with(&format!("| {} |", c.action)),
                    "docs/REFCODES.md does not say what to do for {}",
                    c.code
                );
            }
        }
        let is_code_row = |l: &&str| {
            l.starts_with("| `")
                && l.get(11..12) == Some("`")
                && l[5..11].bytes().all(|b| b.is_ascii_digit())
        };
        assert_eq!(doc.lines().filter(is_code_row).count(), ALL.len());
    }
}
