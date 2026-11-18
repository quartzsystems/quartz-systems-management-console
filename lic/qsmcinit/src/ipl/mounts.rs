// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! What PID 1 mounts: proc, sysfs, devtmpfs, a tmpfs on the run
//! directory, then cgroup2.

use std::ffi::CStr;

use rustix::mount::{MountFlags, mount};

/// One mount: source, mount point, type, flags, options.
struct Mount<'a> {
    source: &'static str,
    at: &'a str,
    fs: &'static str,
    flags: MountFlags,
    data: Option<&'static CStr>,
}

fn plain() -> MountFlags {
    MountFlags::NOSUID.union(MountFlags::NOEXEC)
}

fn nodev() -> MountFlags {
    plain().union(MountFlags::NODEV)
}

/// The table, in order; `run` is the appliance's run directory.
fn table(run: &str) -> [Mount<'_>; 5] {
    [
        Mount {
            source: "proc",
            at: "/proc",
            fs: "proc",
            flags: nodev(),
            data: None,
        },
        Mount {
            source: "sysfs",
            at: "/sys",
            fs: "sysfs",
            flags: nodev(),
            data: None,
        },
        Mount {
            source: "devtmpfs",
            at: "/dev",
            fs: "devtmpfs",
            flags: plain(),
            data: Some(c"mode=0755"),
        },
        Mount {
            source: "tmpfs",
            at: run,
            fs: "tmpfs",
            flags: nodev(),
            data: Some(c"mode=0755"),
        },
        // Supervision puts each service in a group of its own (Phase 1).
        Mount {
            source: "cgroup2",
            at: "/sys/fs/cgroup",
            fs: "cgroup2",
            flags: nodev(),
            data: None,
        },
    ]
}

/// Mount everything, in order. Returns what failed and why: the console
/// is not open yet, so the caller reports.
pub fn kernel(run: &str) -> Vec<(String, String)> {
    let mut failed = Vec::new();
    for m in table(run) {
        let _ = std::fs::create_dir_all(m.at);
        if let Err(e) = mount(m.source, m.at, m.fs, m.flags, m.data) {
            failed.push((m.at.to_string(), e.to_string()));
        }
    }
    failed
}
