// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! What PID 1 mounts: proc, sysfs, devtmpfs, a tmpfs on the run
//! directory, efivarfs and cgroup2 first; then `QSMCDATA` at `/data`.

use std::ffi::CStr;
use std::path::Path;

use loadsource::boot::EFIVARS;
use rustix::mount::{MountFlags, UnmountFlags, mount, mount_bind, mount_remount, unmount};

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
fn table(run: &str) -> [Mount<'_>; 7] {
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
        // What the kernel kept of a panic, in the firmware's variables
        // (`logd::panics`, taken at the next IPL).
        Mount {
            source: "pstore",
            at: logd::panics::PSTORE,
            fs: "pstore",
            flags: nodev(),
            data: None,
        },
        // The firmware's boot entries, by which the slot is chosen.
        Mount {
            source: "efivarfs",
            at: EFIVARS,
            fs: "efivarfs",
            flags: nodev(),
            data: None,
        },
        // As Phase 0 mounted it: `base/supervise` puts no service in a
        // group of its own, and nothing else does yet.
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

/// Mount the xfs on `device` at `at`.
pub fn data(device: &Path, at: &str) -> Result<(), String> {
    let _ = std::fs::create_dir_all(at);
    mount(device, at, "xfs", nodev(), None).map_err(|e| e.to_string())
}

/// The root, where every program is, read-only from here on, so nothing
/// written is ever run (`docs/SECURITY.md`): each of `writable`, a folder
/// of the root a service or PID 1 still writes, is first bound onto
/// itself with no devices, set-user-ID or execution, then the root's own
/// mount alone is made read-only, its binds and what they hold kept.
/// Called before the services that waited for `/data`.
pub fn root_read_only(writable: &[&str]) -> Result<(), String> {
    for at in writable {
        let _ = std::fs::create_dir_all(at);
        mount_bind(*at, *at).map_err(|e| format!("{at}: {e}"))?;
        mount_remount(*at, MountFlags::BIND.union(nodev()), c"")
            .map_err(|e| format!("{at}: {e}"))?;
    }
    mount_remount(
        "/",
        MountFlags::BIND.union(MountFlags::RDONLY).union(nodev()),
        c"",
    )
    .map_err(|e| format!("/: {e}"))
}

/// Unmount `at`. When something still holds it, it is at least made
/// read-only, which also leaves its log clean.
pub fn unmount_data(at: &str) -> Result<(), String> {
    match unmount(at, UnmountFlags::empty()) {
        Ok(()) => Ok(()),
        Err(busy) => match mount_remount(at, nodev().union(MountFlags::RDONLY), c"") {
            Ok(()) => Err(format!("{busy}; made read-only instead")),
            Err(e) => Err(format!("{busy}; not made read-only either: {e}")),
        },
    }
}
