// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! Becoming the service's own user for good: no supplementary group,
//! the user and group IDs all its own, no capability left in any set,
//! and no new privileges. The only unsafe code of the service.

use std::io;

fn last_error(r: libc::c_int) -> io::Result<()> {
    if r == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

/// Become user and group `id`, before any thread is started.
///
/// # Errors
/// A call failed, or root could be taken back after it.
pub fn become_user(id: u32) -> io::Result<()> {
    // SAFETY: plain system calls with no pointers but `setgroups`'s empty
    // list; made while the process has one thread, so musl's set*id
    // apply to all of it.
    unsafe {
        last_error(libc::setgroups(0, std::ptr::null()))?;
        last_error(libc::setresgid(id, id, id))?;
        last_error(libc::setresuid(id, id, id))?;
        last_error(libc::prctl(libc::PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0))?;
        if libc::setresuid(0, 0, 0) == 0 || libc::getuid() != id || libc::geteuid() != id {
            return Err(io::Error::other("root could be taken back"));
        }
    }
    Ok(())
}
