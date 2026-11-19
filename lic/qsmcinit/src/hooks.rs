// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! The test hooks of PID 1, compiled under `cfg(test_hooks)` only: a
//! test image has them, a release image does not. A hook is switched
//! on for one IPL by an SMBIOS OEM string, `qsmc.hook=<name>`, which
//! the harness gives QEMU; the image itself is the same every IPL.
//!
//! The channel is first in a test image's table, before the storage
//! service: the harness reaches the console through it while the IPL
//! waits for the unit to keep of two that ran apart, and it answers
//! that question there.

use std::time::Duration;

use supervise::Service;

/// What PID 1 of a test image prints, and no other PID 1 contains.
pub const MARKER: &str = "TEST IMAGE: the test hooks are in this image; it is not a release";

/// The hook that keeps a slot on trial from confirming.
pub const REFUSE_CONFIRM: &str = "refuse-confirm";

/// The test hooks' binary.
const BINARY: &str = "/qsmc/bin/qsmchook";

/// The harness's way to the control socket: the second serial port.
pub const CHANNEL: Service = Service {
    name: "hook",
    binary: BINARY,
    args: &["channel"],
    order: 1,
    needs: &[],
    ready_within: Duration::from_secs(10),
    caps: supervise::table::Caps::All,
};

/// A service that fails on demand.
pub const FLAKY: Service = Service {
    name: "flaky",
    binary: BINARY,
    args: &["flaky"],
    order: 80,
    needs: &[],
    ready_within: Duration::from_secs(10),
    caps: supervise::table::Caps::Only(&[]),
};

/// Whether the hook `name` is on for this IPL.
pub fn has(name: &str) -> bool {
    let wanted = format!("{}{}={name}", release::KEY_PREFIX, release::HOOK_KEY);
    std::fs::read("/sys/firmware/dmi/tables/DMI")
        .is_ok_and(|table| oem_strings_of(&table).contains(&wanted))
}

/// The OEM strings (type 11) of an SMBIOS table: each structure is its
/// formatted part (its second byte is that part's length), then its
/// strings, each ending with a zero, and a zero more after the last.
/// The test hooks' binary reads them the same way (`qsmchook`).
fn oem_strings_of(table: &[u8]) -> Vec<String> {
    let mut out = Vec::new();
    let mut at = 0;
    while at + 4 <= table.len() {
        let (kind, len) = (table[at], usize::from(table[at + 1]));
        if len < 4 {
            break;
        }
        let strings = at + len;
        let mut end = strings;
        while end + 1 < table.len() && (table[end], table[end + 1]) != (0, 0) {
            end += 1;
        }
        if kind == 11 {
            let area = table.get(strings..end).unwrap_or_default();
            out.extend(
                area.split(|b| *b == 0)
                    .filter(|s| !s.is_empty())
                    .map(|s| String::from_utf8_lossy(s).into_owned()),
            );
        }
        if kind == 127 {
            break;
        }
        at = end + 2;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_oem_strings_are_read_from_the_table() {
        let mut table = b"\x01\x04\x01\x00Vendor\x00\x00".to_vec();
        table.extend_from_slice(b"\x0b\x05\x02\x00\x02qsmc.hook=churn\x00second\x00\x00");
        table.extend_from_slice(b"\x7f\x04\x04\x00\x00\x00");
        assert_eq!(oem_strings_of(&table), ["qsmc.hook=churn", "second"]);
    }
}
