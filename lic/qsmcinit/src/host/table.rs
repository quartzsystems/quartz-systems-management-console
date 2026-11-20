// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! The service table: fixed, compiled in. Starting follows `order`;
//! stopping goes the other way. The storage service is first: `/data`
//! is mounted from what it names, and it is stopped after `/data` is
//! unmounted.
//!
//! The local console is at 25, after the network service it asks; the
//! web server at 30 and the managed-systems service at 40, both left
//! out on a processor that lacks what their cryptography needs. The
//! place a later phase takes: the scheduler at 50 (Phase 7).

use std::time::Duration;

use supervise::Service;
use supervise::table::Caps;
use supervise::table::caps::{
    DAC_OVERRIDE, FOWNER, IPC_LOCK, NET_ADMIN, NET_BIND_SERVICE, NET_RAW, SETGID, SETUID,
    SYS_ADMIN, SYS_RAWIO, SYS_RESOURCE, SYS_TIME, SYS_TTY_CONFIG, SYSLOG,
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

/// The web server: no capabilities, handed its listeners by PID 1
/// (`host::web`). It is ready once it has answered a request over TLS,
/// which may wait for the `open` address.
pub const WEBD: Service = Service {
    name: "webd",
    binary: release::WEBD,
    args: &[],
    order: 30,
    needs: &[],
    ready_within: Duration::from_secs(120),
    caps: Caps::Only(&[]),
};

/// The managed-systems service: it binds its sockets as root, then
/// becomes its own user for good (`release::MGMT_ID`), which alone
/// reads the console's QSI key. Nothing else is left it.
pub const MGMTD: Service = Service {
    name: "mgmtd",
    binary: release::MGMTD,
    args: &[],
    order: 40,
    needs: &[],
    ready_within: READY_WITHIN,
    caps: Caps::Only(&[SETUID, SETGID]),
};

/// The base services, the local console, the web server and the
/// managed-systems service.
#[cfg(not(test_hooks))]
pub static TABLE: &[Service] = &[STGD, LOGD, NETD, CONSOLE, WEBD, MGMTD];

/// The same and, in a test image, the test hooks' two.
#[cfg(test_hooks)]
pub static TABLE: &[Service] = &[
    crate::hooks::CHANNEL,
    STGD,
    LOGD,
    NETD,
    CONSOLE,
    WEBD,
    MGMTD,
    crate::hooks::FLAKY,
];

/// The table of a processor without what the cryptography needs: no
/// web server and no managed-systems service.
#[cfg(not(test_hooks))]
pub static WITHOUT_WEB: &[Service] = &[STGD, LOGD, NETD, CONSOLE];

/// The same in a test image.
#[cfg(test_hooks)]
pub static WITHOUT_WEB: &[Service] = &[
    crate::hooks::CHANNEL,
    STGD,
    LOGD,
    NETD,
    CONSOLE,
    crate::hooks::FLAKY,
];

/// A test image's table with the `webd-fails` hook: the web server
/// fails at once.
#[cfg(test_hooks)]
pub static FAILING_WEB: &[Service] = &[
    crate::hooks::CHANNEL,
    STGD,
    LOGD,
    NETD,
    CONSOLE,
    crate::hooks::FAILING_WEBD,
    MGMTD,
    crate::hooks::FLAKY,
];

/// The table for this processor.
pub fn chosen() -> &'static [Service] {
    #[cfg(test_hooks)]
    if crate::hooks::has(crate::hooks::WEBD_FAILS) {
        return FAILING_WEB;
    }
    if processor::lacking().is_empty() {
        TABLE
    } else {
        WITHOUT_WEB
    }
}

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
        // The web server and the managed-systems service took their
        // places; the later phase's is free.
        assert!(TABLE.iter().any(|s| s.name == WEBD.name && s.order == 30));
        assert!(TABLE.iter().any(|s| s.name == MGMTD.name && s.order == 40));
        assert!(TABLE.iter().all(|s| s.order != 50));
        // Without them the table is the same less both.
        assert_eq!(supervise::table::check(WITHOUT_WEB), Ok(()));
        let less: Vec<&str> = TABLE
            .iter()
            .map(|s| s.name)
            .filter(|n| *n != WEBD.name && *n != MGMTD.name)
            .collect();
        let without: Vec<&str> = WITHOUT_WEB.iter().map(|s| s.name).collect();
        assert_eq!(less, without);
    }
}
