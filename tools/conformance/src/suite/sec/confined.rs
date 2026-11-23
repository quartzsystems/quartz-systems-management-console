// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! Every service confined (`docs/SECURITY.md`): the kernel sees each
//! under its seccomp filter with no new privileges; then, one IPL a
//! service, that service makes a forbidden call, is ended, its code is
//! posted, supervision starts it again, and no other service notices.

use sandbox::ALL;

use super::super::base::{Run, all_ready, booted, channel, posted};
use crate::qemu::session::Machine;

/// A field of `/proc/<pid>/status`.
fn field<'t>(status: &'t str, key: &str) -> Option<&'t str> {
    status
        .lines()
        .find_map(|l| l.strip_prefix(key)?.strip_prefix(':'))
        .map(str::trim)
}

/// Each service as the kernel sees it.
fn seen(run: &Run<'_>) -> Result<String, String> {
    let mut guest = booted(run, &run.machine()?)?;
    let channel = channel(&mut guest)?;
    let status = channel.status()?;
    all_ready(&status)?;
    let mut said = Vec::new();
    for service in ALL {
        let name = service.name();
        let pid = status
            .service(name)
            .map(|s| s.pid)
            .filter(|p| *p != 0)
            .ok_or_else(|| format!("{name} is not running"))?;
        let text = channel.text(&format!("/proc/{pid}/status"))?;
        let (mode, nnp) = (field(&text, "Seccomp"), field(&text, "NoNewPrivs"));
        if (mode, nnp) != (Some("2"), Some("1")) {
            return Err(format!(
                "{name}: seccomp {mode:?}, no new privileges {nnp:?}"
            ));
        }
        let uid = field(&text, "Uid").and_then(|u| u.split_whitespace().next());
        said.push(format!("{name} (user {})", uid.unwrap_or("?")));
    }
    Ok(format!("under their filters: {}", said.join(", ")))
}

/// `name` makes its forbidden call.
fn tripped(run: &Run<'_>, name: &str) -> Result<(), String> {
    let machine = Machine {
        oem: vec![qsmchook::oem_string(&qsmchook::confine_trip(name))],
        ..run.machine()?
    };
    let mut guest = booted(run, &machine)?;
    let deadline = run.deadline();
    guest.seen(&posted(&refcode::SERVICE_CONFINED, name), deadline)?;
    let status = channel(&mut guest)?.until(&format!("{name} ready again"), deadline, |s| {
        s.service(name)
            .is_some_and(|n| n.state == "ready" && n.failures == 1)
    })?;
    all_ready(&status)?;
    for other in status.services.iter().filter(|s| s.name != name) {
        if other.state != "ready" || other.failures != 0 {
            return Err(format!("{other:?} beside {name}'s forbidden call"));
        }
    }
    Ok(())
}

/// The suite.
pub fn confined(run: &Run<'_>) -> Result<String, String> {
    let seen = seen(run)?;
    for service in ALL {
        tripped(run, service.name())?;
    }
    Ok(format!(
        "{seen}; each of the {} ended by a forbidden call with `{}`, started again, and alone",
        ALL.len(),
        refcode::SERVICE_CONFINED.code
    ))
}
