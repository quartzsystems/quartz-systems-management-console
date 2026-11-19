// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! The test hooks of a QS/MC test image. A release image has none of
//! this: the build puts the `qsmchook` binary and the hooks of PID 1
//! into a test image only, and the manifest says which kind an image
//! is.
//!
//! - [`proto`]: the lines the harness and the channel exchange on the
//!   second serial port.
//! - [`churn`]: the state churn of the power-pull suite, and the check
//!   of what it left.
//! - [`serve`]: the channel and the service that fails on demand
//!   (Linux).
//!
//! A hook is switched on for one IPL by an SMBIOS OEM string the
//! harness gives QEMU, [`oem_string`].

#![forbid(unsafe_code)]

pub mod churn;
pub mod proto;
#[cfg(target_os = "linux")]
pub mod serve;

/// The hook that keeps a slot on trial from confirming (PID 1's).
pub const REFUSE_CONFIRM: &str = "refuse-confirm";
/// The hook that makes the `flaky` service fail every time it starts.
pub const FLAKY_FAILS: &str = "flaky-fails";
/// The hook that churns the state files once `/data` is mounted.
pub const CHURN: &str = "churn";

/// The SMBIOS OEM string that switches the hook `name` on:
/// `qsmc.hook=<name>`.
pub fn oem_string(name: &str) -> String {
    format!("{}{}={name}", release::KEY_PREFIX, release::HOOK_KEY)
}

/// The OEM strings (type 11) of an SMBIOS table: each structure is its
/// formatted part (its second byte is that part's length), then its
/// strings, each ending with a zero, and a zero more after the last.
pub fn oem_strings_of(table: &[u8]) -> Vec<String> {
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
        // The end-of-table structure.
        if kind == 127 {
            break;
        }
        at = end + 2;
    }
    out
}

/// Whether the hook `name` is on for this IPL: one of the machine's
/// SMBIOS OEM strings switches it on.
pub fn on(name: &str) -> bool {
    std::fs::read("/sys/firmware/dmi/tables/DMI")
        .is_ok_and(|table| oem_strings_of(&table).contains(&oem_string(name)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_hook_is_named_under_the_product_s_key_prefix() {
        assert_eq!(oem_string("churn"), "qsmc.hook=churn");
    }

    #[test]
    fn the_oem_strings_are_read_from_the_table() {
        let mut table = b"\x01\x04\x01\x00Vendor\x00\x00".to_vec();
        table.extend_from_slice(b"\x0b\x05\x02\x00\x02qsmc.hook=churn\x00second\x00\x00");
        table.extend_from_slice(b"\x0b\x05\x03\x00\x00\x00\x00");
        table.extend_from_slice(b"\x7f\x04\x04\x00\x00\x00");
        assert_eq!(oem_strings_of(&table), [oem_string(CHURN), "second".into()]);
        assert!(oem_strings_of(&[]).is_empty());
    }
}
