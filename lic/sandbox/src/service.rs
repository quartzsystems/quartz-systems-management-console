// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! The services confined, by name.

/// A service of the console, most exposed first.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Service {
    /// The web server: browsers.
    Webd,
    /// The download service: the stream server.
    Fetchd,
    /// The managed-systems service: managed systems.
    Mgmtd,
    /// The scheduler.
    Schedd,
    /// The console's problem log.
    Probd,
    /// The console's update service.
    Updd,
    /// The log service.
    Logd,
    /// The network service.
    Netd,
    /// The storage service.
    Stgd,
    /// The local console: its calls alone, since Landlock would take
    /// option 8's mount of a removable volume.
    Console,
}

/// Every service, most exposed first.
pub const ALL: [Service; 10] = [
    Service::Webd,
    Service::Fetchd,
    Service::Mgmtd,
    Service::Schedd,
    Service::Probd,
    Service::Updd,
    Service::Logd,
    Service::Netd,
    Service::Stgd,
    Service::Console,
];

impl Service {
    /// Its name in the service table.
    pub fn name(self) -> &'static str {
        match self {
            Service::Webd => "webd",
            Service::Fetchd => "fetchd",
            Service::Mgmtd => "mgmtd",
            Service::Schedd => "schedd",
            Service::Probd => "probd",
            Service::Updd => "updd",
            Service::Logd => "logd",
            Service::Netd => "netd",
            Service::Stgd => "stgd",
            Service::Console => "console",
        }
    }
}
