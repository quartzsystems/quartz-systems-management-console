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
//! harness gives QEMU, [`oem_string`], read by `base/inventory`.

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
/// The hook that runs, for the web server, a binary that fails at once:
/// a slot whose web server cannot start.
pub const WEBD_FAILS: &str = "webd-fails";

/// The SMBIOS OEM string that switches the hook `name` on:
/// `qsmc.hook=<name>`.
pub fn oem_string(name: &str) -> String {
    format!("{}{}={name}", release::KEY_PREFIX, release::HOOK_KEY)
}

/// Whether the hook `name` is on for this IPL: one of the machine's
/// SMBIOS OEM strings switches it on.
pub fn on(name: &str) -> bool {
    inventory::oem_strings(&inventory::Roots::default()).contains(&oem_string(name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_hook_is_named_under_the_product_s_key_prefix() {
        assert_eq!(oem_string("churn"), "qsmc.hook=churn");
    }
}
