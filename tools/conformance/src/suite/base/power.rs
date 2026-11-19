// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! Suites 6 and 10: the power pulled at random moments while the state
//! files churn, and the orderly shutdown by the power button (and by the
//! control socket and `SIGTERM`).

use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use qsmchook::churn::{check_host, check_network};
use qsmchook::proto::command;
use state::{Host, Identity, Network};

use super::{Run, STATE_DIR, booted, channel, posted};
use crate::qemu::channel::Channel;
use crate::qemu::session::Machine;

/// How often the power is pulled.
const CUTS: u32 = 50;
/// The longest the console runs on after it was checked, in
/// milliseconds.
const RUN_ON_MS: u64 = 1500;
/// What the shutdown is given, from the button to QEMU's end.
const SHUTDOWN_WITHIN: Duration = Duration::from_secs(30);

/// A small generator: the moments need to vary, not to be secret.
struct Moments(u64);

impl Moments {
    fn next_ms(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0 % RUN_ON_MS
    }
}

/// A state file's text; `None` when the file is not there.
fn state_file(channel: &mut Channel, name: &str) -> Result<Option<String>, String> {
    match channel.text(&format!("{STATE_DIR}/{name}")) {
        Ok(text) => Ok(Some(text)),
        Err(e) if e.contains("No such file") => Ok(None),
        Err(e) => Err(e),
    }
}

/// Wait for the churn's first `host.toml`.
fn await_churn(channel: &mut Channel, deadline: Instant) -> Result<(), String> {
    loop {
        let host = state_file(channel, "host.toml")?;
        let churned = host
            .as_deref()
            .map(state::store::parse::<Host>)
            .and_then(Result::ok)
            .is_some_and(|h| check_host(&h).is_ok_and(|n| n.is_some()));
        if churned {
            return Ok(());
        }
        if Instant::now() >= deadline {
            return Err("the churn never wrote host.toml".into());
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// Every state file parses and is a whole old or new version. The
/// generation of `host.toml`, if the churn has written it.
fn verify(channel: &mut Channel) -> Result<Option<u64>, String> {
    let parsed = |e: state::StateError| e.to_string();
    let Some(host) = state_file(channel, "host.toml")? else {
        return Ok(None);
    };
    let host: Host = state::store::parse(&host).map_err(parsed)?;
    let generation = check_host(&host)?;
    if let Some(network) = state_file(channel, "network.toml")? {
        let network: Network = state::store::parse(&network).map_err(parsed)?;
        check_network(&network)?;
    }
    if let Some(identity) = state_file(channel, "identity.toml")? {
        let _: Identity = state::store::parse(&identity).map_err(parsed)?;
    }
    Ok(generation)
}

/// Suite 6: power pull. With the state churn running, QEMU is killed at
/// 50 random moments; after each the console IPLs, `/data` mounts, and
/// every state file parses and is a whole old or new version.
pub fn power_pull(run: &Run<'_>) -> Result<String, String> {
    let machine = Machine {
        disks: vec![run.disk_copy("disk")?],
        oem: vec![qsmchook::oem_string(qsmchook::CHURN)],
        ..run.machine()?
    };
    let seed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(1, |d| u64::from(d.subsec_nanos()) | 1);
    let mut moments = Moments(seed);
    let mut last: Option<u64> = None;
    let mut first: Option<u64> = None;
    for cut in 0..=CUTS {
        let mut guest = run.boot(&machine)?;
        let deadline = run.deadline();
        let at = |e: String| format!("after {cut} power cuts: {e}");
        guest
            .expect(&refcode::DATA_MOUNTED.tag(), deadline)
            .map_err(at)?;
        guest
            .expect(&refcode::SERVICES_STARTED.tag(), deadline)
            .map_err(at)?;
        if cut == 0 {
            await_churn(guest.channel(deadline).map_err(at)?, deadline).map_err(at)?;
        }
        let generation = verify(guest.channel(deadline).map_err(at)?).map_err(at)?;
        for code in [&refcode::STATE_INVALID, &refcode::DATA_NOT_MOUNTED] {
            if guest.said(&code.tag()) {
                return Err(at(format!("the IPL posted {}", code.line())));
            }
        }
        // What was read may not have reached the disk yet: its rename is
        // seen before it is durable. One generation back is whole.
        if let (Some(now), Some(before)) = (generation, last)
            && now + 1 < before
        {
            return Err(at(format!(
                "host.toml went back from generation {before} to {now}"
            )));
        }
        first = first.or(generation);
        last = generation.or(last);
        if cut < CUTS {
            std::thread::sleep(Duration::from_millis(moments.next_ms()));
            guest.kill();
        }
    }
    let (Some(first), Some(last)) = (first, last) else {
        return Err("the churn never wrote host.toml".into());
    };
    if last <= first {
        return Err(format!("the churn did not advance: generation {last}"));
    }
    Ok(format!(
        "{CUTS} power cuts (seed {seed}), {} IPLs, /data mounted and every state file whole each \
         time; host.toml went from generation {first} to {last}",
        CUTS + 1
    ))
}

/// Suite 10: orderly shutdown. Ctrl-Alt-Del is ignored. QEMU's power
/// button gives the `D3` codes, a clean unmount (no xfs log recovery at
/// the next mount) and power off within 30 s. Then the control socket's
/// restart and `SIGTERM` to PID 1.
pub fn shutdown(run: &Run<'_>) -> Result<String, String> {
    let machine = Machine {
        disks: vec![run.disk_copy("disk")?],
        monitor: Some(run.socket("monitor.sock")),
        ..run.machine()?
    };
    let mut guest = booted(run, &machine)?;
    guest.monitor("sendkey ctrl-alt-delete")?;
    guest.seen("Ctrl-Alt-Del ignored", run.deadline())?;
    if guest.said(&refcode::SHUTDOWN_STARTED.tag()) {
        return Err("Ctrl-Alt-Del started a shutdown".into());
    }
    guest.monitor("system_powerdown")?;
    let pressed = Instant::now();
    let limit = pressed + SHUTDOWN_WITHIN;
    let started = posted(&refcode::SHUTDOWN_STARTED, "power off, by the power button");
    guest.expect(&started, limit)?;
    guest.expect(&posted(&refcode::POWER_OFF, ""), limit)?;
    guest.wait_exit(limit)?;
    let took = pressed.elapsed();
    if guest.said("not unmounted") {
        return Err("/data was not unmounted".into());
    }
    if guest.said("was killed") {
        return Err("a service did not end when asked to and was killed".into());
    }
    drop(guest);

    // The next IPL mounts it clean, and the history has the shutdown.
    let mut guest = booted(run, &machine)?;
    let channel = channel(&mut guest)?;
    let xfs = channel.kmsg("XFS")?;
    if let Some(recovery) = xfs.iter().find(|l| l.contains("recovery")) {
        return Err(format!("the next mount recovered the xfs log: {recovery}"));
    }
    if !xfs.iter().any(|l| l.contains("Ending clean mount")) {
        return Err(format!("the next mount was not seen to be clean: {xfs:?}"));
    }
    let history = channel.text("/data/log/refcodes.ndjson")?;
    if !history.contains(refcode::SHUTDOWN_STARTED.code) {
        return Err("the reference code history lacks the shutdown".into());
    }

    channel.tell(command::RESTART)?;
    let limit = Instant::now() + SHUTDOWN_WITHIN;
    guest.expect(
        &posted(&refcode::SHUTDOWN_STARTED, "restart, by the control socket"),
        limit,
    )?;
    guest.expect(&posted(&refcode::RESTART, ""), limit)?;
    // QEMU was told not to restart the machine, so the reset ends it.
    guest.wait_exit(limit)?;
    drop(guest);

    let mut guest = booted(run, &machine)?;
    let channel = super::channel(&mut guest)?;
    let logs = channel.ls("/data/log")?;
    for file in ["system.ndjson", "refcodes.ndjson", "kernel"] {
        if !logs.iter().any(|name| name == file) {
            return Err(format!("no {file} among the logs {logs:?}"));
        }
    }
    channel.tell(&format!("{} {}", command::KILL, command::INIT))?;
    let limit = Instant::now() + SHUTDOWN_WITHIN;
    guest.expect(
        &posted(&refcode::SHUTDOWN_STARTED, "power off, by SIGTERM"),
        limit,
    )?;
    guest.expect(&posted(&refcode::POWER_OFF, ""), limit)?;
    guest.wait_exit(limit)?;
    if guest.said("was killed") || guest.said("not unmounted") {
        return Err("a later shutdown was not clean".into());
    }
    Ok(format!(
        "`{started}`, `{}`, powered off {:.1} s after the button; the next mount is clean; the \
         history had {} codes; then a restart by the control socket (`{}`) and a power off by \
         SIGTERM",
        refcode::POWER_OFF.tag(),
        took.as_secs_f64(),
        history.lines().count(),
        refcode::RESTART.tag()
    ))
}
