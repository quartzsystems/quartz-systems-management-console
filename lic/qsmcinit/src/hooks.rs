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

/// Whether the hook `name` is on for this IPL: an SMBIOS OEM string,
/// read by `base/inventory`, switches it on.
pub fn has(name: &str) -> bool {
    let wanted = format!("{}{}={name}", release::KEY_PREFIX, release::HOOK_KEY);
    inventory::oem_strings(&inventory::Roots::default()).contains(&wanted)
}
