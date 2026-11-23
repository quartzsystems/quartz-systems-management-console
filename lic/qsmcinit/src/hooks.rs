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

/// The hook that runs a web server that fails at once.
pub const WEBD_FAILS: &str = "webd-fails";

/// A slot on trial is given 90 seconds to confirm, not 15 minutes.
pub const SHORT_TRIAL: &str = "short-trial";

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

/// The web server of [`WEBD_FAILS`]: the test hooks' binary, failing.
pub const FAILING_WEBD: Service = Service {
    binary: BINARY,
    args: &["fail"],
    ..crate::host::table::WEBD
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

/// Every service's refused calls reported and failed, not fatal: the
/// learning mode its policy is derived in (`docs/SECURITY.md`).
pub const CONFINE_LEARN: &str = "confine-learn";

/// `confine-trip-<service>`: that service makes a forbidden call once
/// confined, once.
pub const CONFINE_TRIP: &str = "confine-trip-";

/// Write the services' confinement hooks (`sandbox::HOOKS`) from this
/// IPL's, before any service starts.
pub fn write_confine() {
    let prefix = format!("{}{}=", release::KEY_PREFIX, release::HOOK_KEY);
    let mut lines = Vec::new();
    for s in inventory::oem_strings(&inventory::Roots::default()) {
        let Some(hook) = s.strip_prefix(&prefix) else {
            continue;
        };
        if hook == CONFINE_LEARN {
            lines.push("learn".to_string());
        } else if let Some(service) = hook.strip_prefix(CONFINE_TRIP) {
            lines.push(format!("trip {service}"));
        }
    }
    let _ = std::fs::write(sandbox::HOOKS, joined(&lines));
}

/// `service` made its forbidden call: started again, it does not.
pub fn tripped(service: &str) {
    let Ok(text) = std::fs::read_to_string(sandbox::HOOKS) else {
        return;
    };
    let gone = format!("trip {service}");
    let kept: Vec<String> = text
        .lines()
        .filter(|l| l.trim() != gone)
        .map(str::to_string)
        .collect();
    let _ = std::fs::write(sandbox::HOOKS, joined(&kept));
}

/// Lines, each ended.
fn joined(lines: &[String]) -> String {
    lines.iter().map(|l| l.clone() + "\n").collect()
}
