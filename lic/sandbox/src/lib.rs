// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! QS/MC's services confined (`docs/SECURITY.md`). PID 1 starts each
//! with only the capabilities it keeps and no new privileges; then,
//! before it starts a thread, the service confines itself here:
//! Landlock gives it only its paths and no TCP of its own but what its
//! job is, and `base/confine`'s seccomp filter only its calls. Any
//! other call ends it with `SIGSYS`, which PID 1 posts as `A3001085`
//! and supervision restarts it.
//!
//! In a test image, PID 1 writes [`HOOKS`] from the boot's hooks:
//! `learn` has a refused call reported and failed with `ENOSYS`
//! instead, so a run lists what a policy lacks; `trip <service>` has
//! that service make a forbidden call once it is confined.
//!
//! - [`landlock`]: the paths and ports.
//! - [`policy`]: each service's.
//! - [`service`]: the services, by name.

#[cfg(target_os = "linux")]
pub mod landlock;
#[cfg(target_os = "linux")]
pub mod policy;
pub mod service;

pub use service::{ALL, Service};

/// A test image's confinement hooks, one per line.
pub const HOOKS: &str = "/run/qsmc-confine";

/// The code a service that cannot be confined exits with.
pub const CANNOT: i32 = 40;

/// What [`HOOKS`] says for `service`: learning, and a forbidden call.
#[cfg(all(test_hooks, target_os = "linux"))]
fn hooks(service: Service) -> (bool, bool) {
    let text = std::fs::read_to_string(HOOKS).unwrap_or_default();
    // A test image built to learn learns at every IPL.
    let learn = cfg!(confine_learn) || text.lines().any(|l| l.trim() == "learn");
    let trip = text
        .lines()
        .any(|l| l.trim().strip_prefix("trip ") == Some(service.name()));
    (learn, trip)
}

#[cfg(all(not(test_hooks), target_os = "linux"))]
fn hooks(_: Service) -> (bool, bool) {
    (false, false)
}

/// Confine the calling process as `service`. Called while it has one
/// thread: Landlock and the filter hold the caller and the threads it
/// starts after.
#[cfg(target_os = "linux")]
pub fn confine(service: Service) -> std::io::Result<()> {
    use confine::bpf;
    let (learn, trip) = hooks(service);
    let policy = policy::of(service, std::process::id());
    if let Some(reach) = &policy.reach {
        landlock::restrict(reach)?;
    }
    if learn {
        confine::sys::report_refused_calls(service.name())?;
    }
    let default = if learn { bpf::TRAP } else { bpf::KILL_PROCESS };
    confine::filter(service.name(), &policy.calls, default)?;
    if trip {
        eprintln!("TEST IMAGE: {} makes a forbidden call", service.name());
        // SAFETY: ptrace(PTRACE_TRACEME) takes integers; the filter
        // ends the process before the kernel sees it.
        unsafe { libc::ptrace(libc::PTRACE_TRACEME, 0, 0, 0) };
    }
    Ok(())
}

/// [`confine`], or the process ends: a service is never run
/// unconfined.
#[cfg(target_os = "linux")]
pub fn confine_or_end(service: Service) {
    if let Err(e) = confine(service) {
        eprintln!("{}: not confined: {e}", service.name());
        std::process::exit(CANNOT);
    }
}
