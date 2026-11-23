// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! The machine as the running console has it (`docs/SECURITY.md`): every
//! file system that can be written mounted with no device nodes,
//! set-user-ID or execution, the root read-only, the kernel's log and
//! ptrace kept to their owners, the baked command line, Landlock and
//! seccomp in the kernel.

use crate::suite::base::{Run, all_ready, booted, channel};

/// `/proc/mounts`: each mount point and its options.
fn mounts(text: &str) -> Vec<(String, Vec<String>)> {
    text.lines()
        .filter_map(|l| {
            let mut f = l.split(' ');
            let (_, at, _, opts) = (f.next()?, f.next()?, f.next()?, f.next()?);
            Some((
                at.to_string(),
                opts.split(',').map(str::to_string).collect(),
            ))
        })
        .collect()
}

/// The suite.
pub fn machine(run: &Run<'_>) -> Result<String, String> {
    let mut guest = booted(run, &run.machine()?)?;
    let ch = channel(&mut guest)?;
    all_ready(&ch.status()?)?;
    let table = mounts(&ch.text("/proc/self/mounts")?);
    let opts = |at: &str| {
        table
            .iter()
            .rev()
            .find(|(m, _)| m == at)
            .map(|(_, o)| o.clone())
            .ok_or_else(|| format!("{at} is not mounted"))
    };
    let has = |o: &[String], w: &str| o.iter().any(|x| x == w);
    let root = opts("/")?;
    if !has(&root, "ro") {
        return Err(format!("the root is not read-only: {root:?}"));
    }
    let mut checked = vec!["/ ro".to_string()];
    for (at, needs) in [
        ("/data", &["nodev", "nosuid", "noexec"][..]),
        ("/run", &["nodev", "nosuid", "noexec"]),
        ("/proc", &["nodev", "nosuid", "noexec"]),
        ("/sys", &["nodev", "nosuid", "noexec"]),
        ("/dev", &["nosuid", "noexec"]),
        ("/etc", &["nodev", "nosuid", "noexec"]),
        ("/tmp", &["nodev", "nosuid", "noexec"]),
    ] {
        let o = opts(at)?;
        if let Some(missing) = needs.iter().find(|w| !has(&o, w)) {
            return Err(format!("{at} is mounted without {missing}: {o:?}"));
        }
        if has(&o, "rw") || at == "/dev" {
            checked.push(format!("{at} {}", needs.join(",")));
        }
    }
    for (file, want) in [
        ("/proc/sys/kernel/dmesg_restrict", "1"),
        ("/proc/sys/kernel/yama/ptrace_scope", "1"),
    ] {
        let v = ch
            .text(file)
            .map(|t| t.trim().to_string())
            .unwrap_or_default();
        if v.as_str() < want {
            return Err(format!("{file} is {v:?}, not at least {want}"));
        }
    }
    let line = ch.text("/proc/cmdline")?;
    let want = "console=ttyS0,115200 console=tty0 panic=-1 quiet qsmc.slot=";
    if !line.trim().starts_with(want) {
        return Err(format!("the command line is {line:?}"));
    }
    // The seccomp filter and Landlock: a service under both is running.
    let status = ch.status()?;
    let pid = status
        .service("webd")
        .map(|s| s.pid)
        .ok_or("no web server")?;
    let st = ch.text(&format!("/proc/{pid}/status"))?;
    if !st
        .lines()
        .any(|l| l.starts_with("Seccomp:") && l.ends_with('2'))
    {
        return Err("the web server runs without its filter".into());
    }
    guest.kill();
    Ok(format!(
        "{}; dmesg and ptrace restricted; the command line `{}`; the web \
         server under its filter",
        checked.join("; "),
        line.trim()
    ))
}
