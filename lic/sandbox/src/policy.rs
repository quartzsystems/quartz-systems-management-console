// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! What each service may reach and call once it is confined
//! (`docs/SECURITY.md` has the table). The paths are Landlock's; a
//! unix socket is reached by its path whatever they say, and the
//! peers check who connects.

use confine::bpf::Rule;

use crate::landlock::{Access, Rules};
pub use crate::service::{ALL, Service};

/// A service's whole policy.
#[derive(Clone, Debug)]
pub struct Policy {
    /// Its paths and TCP ports; `None`: not under Landlock.
    pub reach: Option<Rules>,
    /// Its system calls.
    pub calls: Vec<Rule>,
}

// Socket families.
const AF_UNIX: u32 = 1;
const AF_INET: u32 = 2;
const AF_INET6: u32 = 10;
const AF_NETLINK: u32 = 16;
const AF_PACKET: u32 = 17;

/// The flags of a thread's `clone`, as musl's `pthread_create` makes
/// it; a `fork` has others and is refused.
pub const THREAD_CLONE: u32 = 0x007D_0F00;

// `ioctl`s every service may make: a terminal's attributes (is it
// one?), and descriptors made non-blocking or close-on-exec.
const TCGETS: u32 = 0x5401;
const FIONBIO: u32 = 0x5421;
const FIOCLEX: u32 = 0x5451;

// `prctl`s: a thread's name.
const PR_SET_NAME: u32 = 15;
const PR_GET_NAME: u32 = 16;
// And ublk's server marking itself as one that frees memory.
const PR_SET_IO_FLUSHER: u32 = 57;

fn nr(n: libc::c_long) -> u32 {
    u32::try_from(n).expect("a system call's number")
}

fn any(calls: &[libc::c_long]) -> Vec<Rule> {
    calls.iter().map(|&n| Rule::Any(nr(n))).collect()
}

/// What every service calls with any arguments: memory, threads, time,
/// descriptors, files (where Landlock lets it), sockets, waiting.
const COMMON: &[libc::c_long] = &[
    libc::SYS_read,
    libc::SYS_write,
    libc::SYS_readv,
    libc::SYS_writev,
    libc::SYS_pread64,
    libc::SYS_pwrite64,
    libc::SYS_close,
    libc::SYS_lseek,
    libc::SYS_fstat,
    libc::SYS_newfstatat,
    libc::SYS_statx,
    libc::SYS_openat,
    libc::SYS_fcntl,
    libc::SYS_mmap,
    libc::SYS_munmap,
    libc::SYS_mprotect,
    libc::SYS_mremap,
    libc::SYS_madvise,
    libc::SYS_brk,
    libc::SYS_rt_sigaction,
    libc::SYS_rt_sigprocmask,
    libc::SYS_rt_sigreturn,
    libc::SYS_sigaltstack,
    libc::SYS_futex,
    libc::SYS_set_robust_list,
    libc::SYS_set_tid_address,
    libc::SYS_exit,
    libc::SYS_exit_group,
    libc::SYS_gettid,
    libc::SYS_getpid,
    libc::SYS_getuid,
    libc::SYS_geteuid,
    libc::SYS_getgid,
    libc::SYS_getegid,
    libc::SYS_clock_gettime,
    libc::SYS_clock_getres,
    libc::SYS_clock_nanosleep,
    libc::SYS_nanosleep,
    libc::SYS_gettimeofday,
    libc::SYS_getrandom,
    libc::SYS_sched_yield,
    libc::SYS_sched_getaffinity,
    libc::SYS_poll,
    libc::SYS_ppoll,
    libc::SYS_epoll_create1,
    libc::SYS_epoll_ctl,
    libc::SYS_epoll_wait,
    libc::SYS_epoll_pwait,
    libc::SYS_eventfd2,
    libc::SYS_pipe2,
    libc::SYS_dup,
    libc::SYS_dup3,
    libc::SYS_socketpair,
    libc::SYS_connect,
    libc::SYS_accept4,
    libc::SYS_accept,
    libc::SYS_bind,
    libc::SYS_listen,
    libc::SYS_sendto,
    libc::SYS_recvfrom,
    libc::SYS_sendmsg,
    libc::SYS_recvmsg,
    libc::SYS_shutdown,
    libc::SYS_getsockopt,
    libc::SYS_setsockopt,
    libc::SYS_getsockname,
    libc::SYS_getpeername,
    libc::SYS_mkdirat,
    libc::SYS_unlinkat,
    libc::SYS_renameat,
    libc::SYS_renameat2,
    libc::SYS_fsync,
    libc::SYS_fdatasync,
    libc::SYS_ftruncate,
    libc::SYS_fchmod,
    libc::SYS_fchmodat,
    libc::SYS_getdents64,
    libc::SYS_statfs,
    libc::SYS_fstatfs,
    libc::SYS_uname,
    libc::SYS_getcwd,
    libc::SYS_readlinkat,
    libc::SYS_faccessat,
    libc::SYS_faccessat2,
    libc::SYS_signalfd4,
    libc::SYS_restart_syscall,
    libc::SYS_membarrier,
    libc::SYS_sysinfo,
    libc::SYS_umask,
    libc::SYS_utimensat,
    libc::SYS_rseq,
    // musl's x86_64 makes the older calls where they exist, not their
    // `*at` forms: what it opens, stats, lists, makes, renames and
    // removes, and how it waits and copies (`std::io::copy`).
    libc::SYS_open,
    libc::SYS_creat,
    libc::SYS_stat,
    libc::SYS_lstat,
    libc::SYS_access,
    libc::SYS_readlink,
    libc::SYS_getdents,
    libc::SYS_mkdir,
    libc::SYS_rmdir,
    libc::SYS_rename,
    libc::SYS_unlink,
    libc::SYS_link,
    libc::SYS_symlink,
    libc::SYS_chmod,
    libc::SYS_chown,
    libc::SYS_fchown,
    libc::SYS_fchownat,
    libc::SYS_truncate,
    libc::SYS_flock,
    libc::SYS_fallocate,
    libc::SYS_pipe,
    libc::SYS_dup2,
    libc::SYS_select,
    libc::SYS_pselect6,
    libc::SYS_copy_file_range,
    libc::SYS_sendfile,
    libc::SYS_getrlimit,
    libc::SYS_prlimit64,
    libc::SYS_getrusage,
    libc::SYS_getppid,
    libc::SYS_time,
    libc::SYS_times,
    libc::SYS_sched_getparam,
    libc::SYS_sched_getscheduler,
];

/// [`COMMON`], and the calls every service makes with some arguments.
fn common(families: &[u32], ioctl_any: bool) -> Vec<Rule> {
    let mut rules = any(COMMON);
    rules.push(Rule::When(nr(libc::SYS_socket), 0, families.to_vec()));
    rules.push(Rule::When(nr(libc::SYS_clone), 0, vec![THREAD_CLONE]));
    rules.push(Rule::When(
        nr(libc::SYS_prctl),
        0,
        vec![PR_SET_NAME, PR_GET_NAME],
    ));
    if ioctl_any {
        rules.push(Rule::Any(nr(libc::SYS_ioctl)));
    } else {
        rules.push(Rule::When(
            nr(libc::SYS_ioctl),
            1,
            vec![TCGETS, FIONBIO, FIOCLEX],
        ));
    }
    rules
}

/// What a signal to itself needs: `abort`'s `tgkill` to its own process.
fn own_signals(process: u32) -> Rule {
    Rule::When(nr(libc::SYS_tgkill), 0, vec![process])
}

fn paths(list: &[(&str, Access)]) -> Vec<(String, Access)> {
    list.iter().map(|(p, a)| ((*p).to_string(), *a)).collect()
}

/// Name resolution (musl's): its files, and DNS over UDP or TCP.
const RESOLVER: [(&str, Access); 3] = [
    ("/etc/resolv.conf", Access::Read),
    ("/etc/hosts", Access::Read),
    ("/etc/services", Access::Read),
];

/// `service`'s paths; none for the local console.
fn reach(service: Service) -> &'static [(&'static str, Access)] {
    use Access::{Device, Read, Write};
    match service {
        Service::Webd => &[
            ("/data/state", Write),
            ("/data/web", Write),
            ("/data/restore", Write),
            ("/data/dumps/console", Write),
            ("/data", Read),
            ("/qsmc/catalogue.json", Read),
            ("/proc/uptime", Read),
            ("/proc/sys/kernel/osrelease", Read),
            ("/proc/cpuinfo", Read),
            ("/dev/urandom", Read),
            ("/tmp", Write),
            ("/run", Write),
        ],
        Service::Fetchd => &[
            ("/data/fetch", Write),
            ("/data/updates", Write),
            ("/data/state", Read),
            ("/qsmc/keys", Read),
            ("/dev/urandom", Read),
        ],
        Service::Mgmtd => &[
            ("/data/mgmt", Write),
            ("/data/state", Read),
            ("/data/updates", Read),
            ("/proc/sys/kernel/hostname", Read),
            ("/dev/urandom", Read),
        ],
        Service::Schedd => &[
            ("/data/state", Write),
            ("/data/plans", Write),
            ("/proc/sys/kernel/hostname", Read),
            ("/dev/urandom", Read),
        ],
        Service::Probd => &[
            ("/data/problems", Write),
            ("/data/log", Read),
            ("/run", Write),
        ],
        Service::Updd => &[
            ("/data/update", Write),
            ("/data/state", Read),
            ("/data/updates", Read),
            ("/qsmc/keys", Read),
            ("/sys/firmware/efi/efivars", Write),
            ("/sys", Read),
            ("/dev", Device),
            ("/run", Write),
        ],
        Service::Logd => &[
            ("/data/log", Write),
            ("/proc/sys/kernel/random/boot_id", Read),
            ("/dev/kmsg", Read),
            ("/run", Write),
        ],
        Service::Netd => &[
            ("/data/state", Write),
            ("/etc", Write),
            ("/proc/sys/net", Write),
            ("/sys", Read),
            ("/dev/rtc0", Device),
            ("/dev/urandom", Read),
            ("/run", Write),
        ],
        Service::Stgd => &[
            ("/dev", Device),
            ("/sys/firmware/efi/efivars", Write),
            ("/sys", Read),
            ("/proc", Read),
            ("/run", Write),
        ],
        Service::Console => &[],
    }
}

/// `service`'s calls; `process` is its own process ID.
fn calls(service: Service, process: u32) -> Vec<Rule> {
    let unix = [AF_UNIX];
    let inet = [AF_UNIX, AF_INET, AF_INET6];
    let mut calls = match service {
        Service::Webd | Service::Schedd | Service::Probd | Service::Logd => common(&unix, false),
        Service::Fetchd | Service::Mgmtd => common(&inet, false),
        Service::Updd => common(&unix, true),
        Service::Netd => {
            let mut c = common(&[AF_UNIX, AF_INET, AF_INET6, AF_NETLINK, AF_PACKET], true);
            c.extend(any(&[
                libc::SYS_clock_settime,
                libc::SYS_adjtimex,
                libc::SYS_clock_adjtime,
            ]));
            c
        }
        Service::Stgd => {
            let mut c = common(&unix, true);
            c.extend(any(&[
                libc::SYS_io_uring_setup,
                libc::SYS_io_uring_enter,
                libc::SYS_io_uring_register,
            ]));
            // `PR_SET_IO_FLUSHER`, which ublk's server takes.
            c.retain(|r| !matches!(r, Rule::When(n, 0, _) if *n == nr(libc::SYS_prctl)));
            c.push(Rule::When(
                nr(libc::SYS_prctl),
                0,
                vec![PR_SET_NAME, PR_GET_NAME, PR_SET_IO_FLUSHER],
            ));
            c
        }
        Service::Console => {
            let mut c = common(&unix, true);
            c.extend(any(&[libc::SYS_mount, libc::SYS_umount2, libc::SYS_sync]));
            c
        }
    };
    calls.push(own_signals(process));
    calls
}

/// `service`'s policy; `process` is its own process ID.
pub fn of(service: Service, process: u32) -> Policy {
    let mut list = paths(reach(service));
    if matches!(service, Service::Fetchd | Service::Mgmtd) {
        list.extend(paths(&RESOLVER));
    }
    let reach = (service != Service::Console).then(|| Rules {
        paths: list,
        connect: Vec::new(),
        // Their peers' ports are settings.
        any_connect: matches!(service, Service::Fetchd | Service::Mgmtd),
        bind: Vec::new(),
    });
    Policy {
        reach,
        calls: calls(service, process),
    }
}
/// The folders of `/data` a service only reads, which another makes:
/// PID 1 makes them before any starts, since a Landlock rule holds a
/// folder that exists when the service confines itself.
pub fn read_folders() -> Vec<&'static str> {
    let mut v: Vec<&str> = ALL
        .iter()
        .flat_map(|s| reach(*s).iter())
        .filter(|(p, a)| *a == Access::Read && p.starts_with("/data/"))
        .map(|(p, _)| *p)
        .collect();
    v.sort_unstable();
    v.dedup();
    v
}

/// Calls no service ever makes: a policy that allows one is wrong.
pub const NEVER: [libc::c_long; 16] = [
    libc::SYS_execve,
    libc::SYS_execveat,
    libc::SYS_fork,
    libc::SYS_vfork,
    libc::SYS_ptrace,
    libc::SYS_setuid,
    libc::SYS_setgid,
    libc::SYS_capset,
    libc::SYS_pivot_root,
    libc::SYS_unshare,
    libc::SYS_setns,
    libc::SYS_kill,
    libc::SYS_bpf,
    libc::SYS_perf_event_open,
    libc::SYS_process_vm_readv,
    libc::SYS_kexec_load,
];

#[cfg(test)]
mod tests {
    use super::*;
    use confine::bpf::{self, ALLOW, ARCH_X86_64, KILL_PROCESS};

    fn action(service: Service, nr: libc::c_long, args: [u64; 6]) -> u32 {
        let p = of(service, 77);
        let program = bpf::program(&p.calls, KILL_PROCESS).unwrap();
        bpf::run(&program, ARCH_X86_64, u32::try_from(nr).unwrap(), args)
    }

    #[test]
    fn no_service_is_allowed_what_none_needs() {
        for service in ALL {
            for nr in NEVER {
                assert_eq!(
                    action(service, nr, [0; 6]),
                    KILL_PROCESS,
                    "{service:?} {nr}"
                );
            }
            // A fork is a clone with other flags.
            assert_eq!(
                action(service, libc::SYS_clone, [0x11, 0, 0, 0, 0, 0]),
                KILL_PROCESS
            );
            assert_eq!(
                action(
                    service,
                    libc::SYS_clone,
                    [u64::from(THREAD_CLONE), 0, 0, 0, 0, 0]
                ),
                ALLOW
            );
            // A signal to another process.
            assert_eq!(
                action(service, libc::SYS_tgkill, [1, 1, 9, 0, 0, 0]),
                KILL_PROCESS
            );
            assert_eq!(
                action(service, libc::SYS_tgkill, [77, 78, 6, 0, 0, 0]),
                ALLOW
            );
            // Programs of every size the kernel takes.
            let p = of(service, 77);
            assert!(bpf::program(&p.calls, KILL_PROCESS).is_ok());
        }
    }

    #[test]
    fn only_the_network_services_open_network_sockets() {
        let inet = [u64::from(AF_INET), 1, 0, 0, 0, 0];
        for service in ALL {
            let allowed = action(service, libc::SYS_socket, inet) == ALLOW;
            let network = matches!(service, Service::Fetchd | Service::Mgmtd | Service::Netd);
            assert_eq!(allowed, network, "{service:?}");
            assert_eq!(action(service, libc::SYS_socket, [1, 1, 0, 0, 0, 0]), ALLOW);
        }
        // The web server answers on the listeners it was handed.
        assert!(!of(Service::Webd, 1).reach.unwrap().any_connect);
        assert!(of(Service::Mgmtd, 1).reach.unwrap().any_connect);
    }

    #[test]
    fn the_folders_read_but_made_by_another_are_listed() {
        let f = read_folders();
        assert!(f.contains(&"/data/updates"), "{f:?}");
        assert!(f.contains(&"/data/log"), "{f:?}");
        assert!(!f.contains(&"/data"));
    }

    #[test]
    fn mounting_is_the_local_consoles_alone() {
        for service in ALL {
            let mounts = action(service, libc::SYS_mount, [0; 6]) == ALLOW;
            assert_eq!(mounts, service == Service::Console, "{service:?}");
            assert_eq!(of(service, 1).reach.is_none(), service == Service::Console);
        }
    }
}
