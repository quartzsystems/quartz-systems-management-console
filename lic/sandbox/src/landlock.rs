// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! Landlock: the file system beneath the paths a service is given, and
//! TCP to the ports it is given, and nothing else, for the calling
//! thread and every thread it starts after. `unsafe` lives only here.

#![allow(unsafe_code)] // every block says why it is sound

use std::ffi::CString;
use std::io;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};

/// What a path is given.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Access {
    /// Its files read and its directories listed.
    Read,
    /// That, and files made, written, truncated, renamed and removed.
    Write,
    /// Read, and its devices' requests (`ioctl`).
    Device,
}

// The file system's access rights (`linux/landlock.h`).
const EXECUTE: u64 = 1 << 0;
const WRITE_FILE: u64 = 1 << 1;
const READ_FILE: u64 = 1 << 2;
const READ_DIR: u64 = 1 << 3;
const REMOVE_DIR: u64 = 1 << 4;
const REMOVE_FILE: u64 = 1 << 5;
#[cfg_attr(not(test), allow(dead_code))] // a device node: never given
const MAKE_CHAR: u64 = 1 << 6;
const MAKE_DIR: u64 = 1 << 7;
const MAKE_REG: u64 = 1 << 8;
const MAKE_SOCK: u64 = 1 << 9;
const MAKE_FIFO: u64 = 1 << 10;
#[cfg_attr(not(test), allow(dead_code))]
const MAKE_BLOCK: u64 = 1 << 11;
const MAKE_SYM: u64 = 1 << 12;
const REFER: u64 = 1 << 13;
const TRUNCATE: u64 = 1 << 14;
const IOCTL_DEV: u64 = 1 << 15;

/// The rights a regular file or a device can be given; the rest are a
/// directory's.
const FILE_RIGHTS: u64 = EXECUTE | WRITE_FILE | READ_FILE | TRUNCATE | IOCTL_DEV;

// TCP's.
const BIND_TCP: u64 = 1 << 0;
const CONNECT_TCP: u64 = 1 << 1;

// What a domain is scoped to.
const SCOPE_ABSTRACT_UNIX: u64 = 1 << 0;
const SCOPE_SIGNAL: u64 = 1 << 1;

const RULE_PATH_BENEATH: libc::c_int = 1;
const RULE_NET_PORT: libc::c_int = 2;
const CREATE_RULESET_VERSION: u32 = 1 << 0;

// The calls' numbers on x86_64.
const SYS_CREATE_RULESET: libc::c_long = 444;
const SYS_ADD_RULE: libc::c_long = 445;
const SYS_RESTRICT_SELF: libc::c_long = 446;

/// The least ABI taken: 6 (Linux 6.12) has TCP ports, the devices'
/// requests and the scopes. The console's kernel is 6.18.
pub const ABI_MIN: i64 = 6;

#[repr(C)]
struct RulesetAttr {
    handled_access_fs: u64,
    handled_access_net: u64,
    scoped: u64,
}

#[repr(C, packed)]
struct PathBeneath {
    allowed_access: u64,
    parent_fd: i32,
}

#[repr(C)]
struct NetPort {
    allowed_access: u64,
    port: u64,
}

/// Every right of the file system this ABI knows.
fn handled_fs(abi: i64) -> u64 {
    let mut all = (MAKE_SYM << 1) - 1;
    if abi >= 2 {
        all |= REFER;
    }
    if abi >= 3 {
        all |= TRUNCATE;
    }
    if abi >= 5 {
        all |= IOCTL_DEV;
    }
    all
}

/// The rights `access` gives a directory.
pub fn rights(access: Access) -> u64 {
    match access {
        Access::Read => READ_FILE | READ_DIR,
        Access::Write => {
            READ_FILE
                | READ_DIR
                | WRITE_FILE
                | TRUNCATE
                | REMOVE_DIR
                | REMOVE_FILE
                | MAKE_DIR
                | MAKE_REG
                | MAKE_SOCK
                | MAKE_FIFO
                | MAKE_SYM
                | REFER
        }
        Access::Device => READ_FILE | READ_DIR | WRITE_FILE | IOCTL_DEV,
    }
}

/// What a rule set allows.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Rules {
    /// Each path and what it is given, beneath it included.
    pub paths: Vec<(String, Access)>,
    /// TCP ports it may connect to; none: no TCP connection.
    pub connect: Vec<u16>,
    /// Whether it may connect to any TCP port instead: its peers' ports
    /// are settings.
    pub any_connect: bool,
    /// TCP ports it may bind; none: no TCP listener of its own.
    pub bind: Vec<u16>,
}

/// The kernel's Landlock ABI; 0 when it has none.
pub fn abi() -> i64 {
    // SAFETY: with a null attribute, size 0 and the version flag the
    // call reads nothing and returns the ABI or -1.
    let v = unsafe {
        libc::syscall(
            SYS_CREATE_RULESET,
            std::ptr::null::<RulesetAttr>(),
            0usize,
            CREATE_RULESET_VERSION,
        )
    };
    v.max(0)
}

fn check(r: libc::c_long) -> io::Result<libc::c_long> {
    if r < 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(r)
    }
}

/// Restrict the calling thread, and the threads it starts after, to
/// `rules`. A folder given to write that does not exist is made; any
/// other path that does not exist is left out, and nothing beneath it
/// can be reached. Fails on a kernel older than [`ABI_MIN`].
pub fn restrict(rules: &Rules) -> io::Result<()> {
    let abi = abi();
    if abi < ABI_MIN {
        return Err(io::Error::other(format!(
            "the kernel's Landlock ABI is {abi}, {ABI_MIN} is needed"
        )));
    }
    let handled = handled_fs(abi);
    let attr = RulesetAttr {
        handled_access_fs: handled,
        handled_access_net: if rules.any_connect {
            BIND_TCP
        } else {
            BIND_TCP | CONNECT_TCP
        },
        scoped: SCOPE_ABSTRACT_UNIX | SCOPE_SIGNAL,
    };
    // SAFETY: `attr` is the kernel's layout for this size, read before
    // the call returns; the result is a new descriptor or -1.
    let fd = check(unsafe {
        libc::syscall(
            SYS_CREATE_RULESET,
            &raw const attr,
            std::mem::size_of::<RulesetAttr>(),
            0u32,
        )
    })?;
    // SAFETY: the descriptor was just made and is owned here alone.
    let ruleset = unsafe { OwnedFd::from_raw_fd(i32::try_from(fd).map_err(io::Error::other)?) };
    for (path, access) in &rules.paths {
        // A rule holds an inode: a folder the service will make is made
        // now, or nothing beneath it could be reached.
        if *access == Access::Write && !std::path::Path::new(path).exists() {
            std::fs::create_dir_all(path).map_err(|e| io::Error::other(format!("{path}: {e}")))?;
        }
        let Some(dir) = open_path(path)? else {
            continue;
        };
        let mut allowed = rights(*access) & handled;
        if !is_dir(&dir)? {
            allowed &= FILE_RIGHTS;
        }
        let rule = PathBeneath {
            allowed_access: allowed,
            parent_fd: dir.as_raw_fd(),
        };
        // SAFETY: `rule` is the kernel's packed layout, read before
        // the call returns; both descriptors are open.
        check(unsafe {
            libc::syscall(
                SYS_ADD_RULE,
                ruleset.as_raw_fd(),
                RULE_PATH_BENEATH,
                &raw const rule,
                0u32,
            )
        })
        .map_err(|e| io::Error::other(format!("{path}: {e}")))?;
    }
    for (ports, access) in [(&rules.connect, CONNECT_TCP), (&rules.bind, BIND_TCP)] {
        for port in ports {
            let rule = NetPort {
                allowed_access: access,
                port: u64::from(*port),
            };
            // SAFETY: as above, for a port's rule.
            check(unsafe {
                libc::syscall(
                    SYS_ADD_RULE,
                    ruleset.as_raw_fd(),
                    RULE_NET_PORT,
                    &raw const rule,
                    0u32,
                )
            })?;
        }
    }
    // SAFETY: prctl takes integers; restrict_self takes the rule set's
    // descriptor and no flags.
    unsafe {
        if libc::prctl(libc::PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0) != 0 {
            return Err(io::Error::last_os_error());
        }
        check(libc::syscall(SYS_RESTRICT_SELF, ruleset.as_raw_fd(), 0u32))?;
    }
    Ok(())
}

/// `path` opened to name it in a rule, or `None` when it does not exist.
fn open_path(path: &str) -> io::Result<Option<OwnedFd>> {
    let c = CString::new(path).map_err(io::Error::other)?;
    // SAFETY: `c` is a valid C string for the call; the result is a
    // new descriptor or -1.
    let fd = unsafe { libc::open(c.as_ptr(), libc::O_PATH | libc::O_CLOEXEC) };
    if fd < 0 {
        let e = io::Error::last_os_error();
        return if e.kind() == io::ErrorKind::NotFound {
            Ok(None)
        } else {
            Err(io::Error::other(format!("{path}: {e}")))
        };
    }
    // SAFETY: the descriptor was just opened and is owned here alone.
    Ok(Some(unsafe { OwnedFd::from_raw_fd(fd) }))
}

fn is_dir(fd: &OwnedFd) -> io::Result<bool> {
    // SAFETY: `st` is written by the kernel before it is read; the
    // descriptor is open.
    let mut st = unsafe { std::mem::zeroed::<libc::stat>() };
    if unsafe { libc::fstat(fd.as_raw_fd(), &raw mut st) } != 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(st.st_mode & libc::S_IFMT == libc::S_IFDIR)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rights_are_the_kernels() {
        // ABI 1's thirteen rights, then each ABI's own.
        assert_eq!(handled_fs(1), 0x1FFF);
        assert_eq!(handled_fs(3), 0x7FFF);
        assert_eq!(handled_fs(6), 0xFFFF);
        assert_eq!(rights(Access::Read), READ_FILE | READ_DIR);
        assert_eq!(
            rights(Access::Write) & (EXECUTE | MAKE_CHAR | MAKE_BLOCK | IOCTL_DEV),
            0
        );
        assert_eq!(
            rights(Access::Device) & FILE_RIGHTS,
            READ_FILE | WRITE_FILE | IOCTL_DEV
        );
        assert_eq!(std::mem::size_of::<PathBeneath>(), 12);
        assert_eq!(std::mem::size_of::<NetPort>(), 16);
    }

    #[test]
    fn a_confined_child_reaches_only_its_paths() {
        if abi() < ABI_MIN {
            return; // a build machine's kernel without it
        }
        let dir = std::env::temp_dir().join(format!("qsmc-landlock-{}", std::process::id()));
        let (inside, outside) = (dir.join("in"), dir.join("out"));
        std::fs::create_dir_all(&inside).unwrap();
        std::fs::create_dir_all(&outside).unwrap();
        std::fs::write(outside.join("f"), b"x").unwrap();
        let rules = Rules {
            paths: vec![(inside.display().to_string(), Access::Write)],
            ..Rules::default()
        };
        // A thread of its own, so the test's other threads stay free.
        let (i, o) = (inside.clone(), outside.clone());
        let seen = std::thread::spawn(move || {
            restrict(&rules).unwrap();
            let wrote = std::fs::write(i.join("g"), b"y").is_ok();
            let read = std::fs::read(o.join("f")).is_ok();
            let tcp = std::net::TcpStream::connect(("127.0.0.1", 9)).map_err(|e| e.kind());
            (wrote, read, tcp)
        })
        .join()
        .unwrap();
        let _ = std::fs::remove_dir_all(&dir);
        assert!(seen.0, "the path it was given");
        assert!(!seen.1, "a path it was not");
        assert_eq!(seen.2.err(), Some(io::ErrorKind::PermissionDenied));
    }
}
