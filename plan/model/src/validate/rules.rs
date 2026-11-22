// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! The validation rules: every outcome an item can have, with its
//! status and its condition in words. `docs/PLANS.md` holds the same
//! table, which a test compares with [`table`].

use std::fmt::Write as _;

use ipc::wire_enum;
use serde::{Deserialize, Serialize};

/// How an item of a plan stands against a host.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    /// It is there as the plan has it, or it will be made so.
    #[default]
    Ok,
    /// It is deployed with the change its message names.
    Warning,
    /// It is skipped at deployment.
    Error,
}
wire_enum!(Status {
    0 => Ok,
    1 => Warning,
    2 => Error,
});

impl Status {
    /// As the pages say it.
    pub fn word(self) -> &'static str {
        match self {
            Self::Ok => "OK",
            Self::Warning => "Warning",
            Self::Error => "Error",
        }
    }
}

macro_rules! rules {
    ($($v:ident = $n:literal, $item:literal, $st:ident, $when:literal;)*) => {
        /// Why an item stands as it does: one rule of the table.
        #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
        pub enum Why {
            $(
                #[doc = $when]
                $v,
            )*
            /// Not yet decided.
            #[default]
            Unset,
        }
        wire_enum!(Why {
            $($n => $v,)*
            255 => Unset,
        });

        /// Every rule, in the table's order.
        pub const ALL: &[Why] = &[$(Why::$v),*];

        impl Why {
            /// The item it is of, as the table's first column says it.
            pub fn item(self) -> &'static str {
                match self { $(Self::$v => $item,)* Self::Unset => "" }
            }
            /// Its status.
            pub fn status(self) -> Status {
                match self { $(Self::$v => Status::$st,)* Self::Unset => Status::Error }
            }
            /// Its condition.
            pub fn when(self) -> &'static str {
                match self { $(Self::$v => $when,)* Self::Unset => "" }
            }
        }
    };
}

rules! {
    SystemOk = 0, "Managed system", Ok, "A QS/VS host answering the host family";
    SystemModel = 1, "Managed system", Warning, "A different type and model";
    SystemSmaller = 2, "Managed system", Warning, "fewer processors or less memory than the plan's host but enough for the chosen partitions' minimums";
    SystemNotHost = 3, "Managed system", Error, "Not a host";
    SystemNotConnected = 4, "Managed system", Error, "not connected";
    SystemTooSmall = 5, "Managed system", Error, "not enough for any partition's minimum";
    PoolExists = 10, "Storage pool", Ok, "Exists with room for the plan's disks";
    PoolMade = 11, "Storage pool", Ok, "or can be made on unlabelled disks at the plan's location codes";
    PoolProtection = 12, "Storage pool", Warning, "Exists with another protection";
    PoolSmaller = 13, "Storage pool", Warning, "will be made smaller than the plan's but large enough";
    PoolNoDisk = 14, "Storage pool", Error, "No disk at those location codes";
    PoolLabelled = 15, "Storage pool", Error, "a disk there carries a label";
    PoolNoRoom = 16, "Storage pool", Error, "not enough room";
    SwitchExists = 20, "Virtual switch", Ok, "Exists with the plan's VLANs";
    SwitchMade = 21, "Virtual switch", Ok, "or will be made";
    SwitchNoUplink = 22, "Virtual switch", Warning, "Will be made without its uplink because the interface is absent or in use";
    SwitchOtherVlans = 23, "Virtual switch", Error, "Exists under that name with other VLANs";
    SwitchManagement = 24, "Virtual switch", Error, "the uplink is the management interface";
    ProcPoolExists = 30, "Shared processor pool", Ok, "Exists the same";
    ProcPoolMade = 31, "Shared processor pool", Ok, "or will be made";
    ProcPoolFewer = 32, "Shared processor pool", Warning, "Made with fewer processors";
    ProcPoolCannot = 33, "Shared processor pool", Error, "Cannot be made";
    DiskExists = 40, "Virtual disk, tape volume", Ok, "Exists the same";
    DiskMade = 41, "Virtual disk, tape volume", Ok, "or will be made";
    DiskThin = 42, "Virtual disk, tape volume", Warning, "Thick asked where only thin fits";
    DiskPoolError = 43, "Virtual disk, tape volume", Error, "Its pool is in Error";
    DiskOther = 44, "Virtual disk, tape volume", Error, "a different one exists under the name";
    ImageThere = 50, "Optical device", Ok, "The image is in the target's library";
    ImageAbsent = 51, "Optical device", Warning, "The image is absent: the device is made empty";
    IoFree = 60, "Physical I/O", Ok, "The device at that location is released and free";
    IoSkipped = 61, "Physical I/O", Warning, "Absent, not released or held: the assignment is skipped";
    PartOk = 70, "Partition", Ok, "Its ID and name are free and everything it needs is OK";
    PartExists = 71, "Partition", Ok, "or it exists the same, or as an earlier deployment of the plan left it";
    PartNeeds = 72, "Partition", Warning, "Something it needs has a Warning";
    PartIdentity = 73, "Partition", Warning, "its MAC addresses and serial number will be new";
    PartTaken = 74, "Partition", Error, "Its ID or name is taken";
    PartRequires = 75, "Partition", Error, "something it requires is in Error";
    PartTooBig = 76, "Partition", Error, "more than the host has left";
}

/// The rules as `docs/PLANS.md` has them: a row an item, its OK,
/// Warning and Error conditions each joined by `; `.
pub fn table() -> String {
    let mut items: Vec<&str> = Vec::new();
    for w in ALL {
        if !items.contains(&w.item()) {
            items.push(w.item());
        }
    }
    let mut s = String::from("| Item | OK | Warning | Error |\n| --- | --- | --- | --- |\n");
    for item in items {
        let cell = |st: Status| {
            ALL.iter()
                .filter(|w| w.item() == item && w.status() == st)
                .map(|w| w.when())
                .collect::<Vec<_>>()
                .join("; ")
        };
        let _ = writeln!(
            s,
            "| {item} | {} | {} | {} |",
            cell(Status::Ok),
            cell(Status::Warning),
            cell(Status::Error)
        );
    }
    s
}

#[cfg(test)]
mod tests {
    #[test]
    fn plans_md_has_the_rules_table() {
        let doc =
            std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/PLANS.md"))
                .expect("docs/PLANS.md");
        let t = super::table();
        assert!(
            doc.contains(&t),
            "docs/PLANS.md lacks the rules table:\n{t}"
        );
    }
}
