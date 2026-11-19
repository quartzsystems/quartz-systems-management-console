// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! The service table: fixed, compiled in. Starting follows `order`;
//! stopping goes the other way. The storage service is first: `/data`
//! is mounted from what it names, and it is stopped after `/data` is
//! unmounted.
//!
//! The local console is at 25, after the network service it asks. The
//! places later phases take: the web server at 30 (Phase 3), the
//! managed-systems service at 40 (Phase 4), the scheduler at 50
//! (Phase 7).

use std::time::Duration;

use supervise::Service;
use supervise::table::Caps;
use supervise::table::caps::{
    DAC_OVERRIDE, FOWNER, IPC_LOCK, NET_ADMIN, NET_BIND_SERVICE, NET_RAW, SYS_ADMIN, SYS_RAWIO,
    SYS_RESOURCE, SYS_TIME, SYS_TTY_CONFIG, SYSLOG,
};

/// What a base service is given to say it is ready.
const READY_WITHIN: Duration = Duration::from_secs(10);

/// The storage service: the load source, served before `/data` is
/// mounted and until it is unmounted.
pub const STGD: Service = Service {
    name: stgd::NAME,
    binary: "/qsmc/bin/qsmcstgd",
    args: &[],
    order: 5,
    needs: &[],
    ready_within: Duration::from_secs(30),
    caps: Caps::Only(&[
        SYS_ADMIN,
        SYS_RAWIO,
        IPC_LOCK,
        SYS_RESOURCE,
        DAC_OVERRIDE,
        FOWNER,
    ]),
};

/// The log service.
pub const LOGD: Service = Service {
    name: "logd",
    binary: "/qsmc/bin/qsmclogd",
    args: &[],
    order: 10,
    needs: &[],
    ready_within: READY_WITHIN,
    caps: Caps::Only(&[SYSLOG]),
};

/// The management network and time.
pub const NETD: Service = Service {
    name: "netd",
    binary: "/qsmc/bin/qsmcnetd",
    args: &[],
    order: 20,
    needs: &[],
    ready_within: READY_WITHIN,
    caps: Caps::Only(&[NET_ADMIN, NET_RAW, NET_BIND_SERVICE, SYS_TIME]),
};

/// The local console on the display and the serial port. It takes the
/// display from the kernel's console, which needs `SYS_TTY_CONFIG`.
pub const CONSOLE: Service = Service {
    name: "console",
    binary: release::CONSOLE,
    args: &[],
    order: 25,
    needs: &[],
    ready_within: READY_WITHIN,
    caps: Caps::Only(&[SYS_TTY_CONFIG]),
};

/// The base services and the local console.
#[cfg(not(test_hooks))]
pub static TABLE: &[Service] = &[STGD, LOGD, NETD, CONSOLE];

/// The same and, in a test image, the test hooks' two.
#[cfg(test_hooks)]
pub static TABLE: &[Service] = &[
    crate::hooks::CHANNEL,
    STGD,
    LOGD,
    NETD,
    CONSOLE,
    crate::hooks::FLAKY,
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_table_is_one_and_storage_then_the_log_service_start_first() {
        assert_eq!(supervise::table::check(TABLE), Ok(()));
        let names: Vec<&str> = supervise::table::start_order(TABLE)
            .into_iter()
            .map(|i| TABLE[i].name)
            .filter(|n| ["stgd", "logd", "netd", "console"].contains(n))
            .collect();
        assert_eq!(names, ["stgd", "logd", "netd", "console"]);
        for service in TABLE {
            assert!(service.binary.starts_with(release::BIN_DIR));
        }
        // The places later phases take are free.
        assert!(TABLE.iter().all(|s| ![30, 40, 50].contains(&s.order)));
    }
}
