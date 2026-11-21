// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! Suites 7 and 8: the mirrored load source. With either disk removed
//! the console IPLs and says which unit is missing; the disk returned,
//! only what changed is copied to it; the storage service killed under
//! write load is started again and nothing is lost. Disks that each ran
//! alone stop the IPL with the code until the unit to keep is chosen.

use std::time::{Duration, Instant};

use qsmchook::proto::{LOAD_DIR, command};
use release::fact;

use super::{Run, all_ready, base_ready, booted, booted_degraded, channel, posted, power_off};
use crate::qemu::channel::Channel;
use crate::qemu::session::{Disk, Machine};

/// Megabytes each file of the write load has.
const LOAD_MIB: u64 = 4;
/// How often the storage service is killed under the load.
const KILLS: u32 = 3;

fn machine(run: &Run<'_>, disks: &[Disk]) -> Result<Machine, String> {
    Ok(Machine {
        disks: disks.to_vec(),
        ..run.machine()?
    })
}

/// Write files under `/data` for `secs` seconds: the numbers of those
/// the load says are whole.
fn write_for(channel: &mut Channel, secs: u64) -> Result<Vec<u64>, String> {
    channel.ask(&format!("{} {LOAD_MIB}", command::LOAD))?;
    std::thread::sleep(Duration::from_secs(secs));
    channel.ask(&format!("{} 0", command::LOAD))?;
    // The file being written when the load stopped is finished first.
    std::thread::sleep(Duration::from_secs(2));
    whole(channel)
}

/// The numbers of the load's files that are whole, by its list.
fn whole(channel: &mut Channel) -> Result<Vec<u64>, String> {
    let done = channel.text(&format!("{LOAD_DIR}/done"))?;
    Ok(done
        .lines()
        .filter_map(|l| l.split(' ').next()?.parse().ok())
        .collect())
}

/// Each of the last three whole files reads back as it was written.
fn read_back(channel: &mut Channel, files: &[u64]) -> Result<usize, String> {
    let last: Vec<u64> = files.iter().rev().take(3).copied().collect();
    for n in &last {
        let (len, sum) = channel.sum(&format!("{LOAD_DIR}/file-{n}"))?;
        let bytes = LOAD_MIB << 20;
        if (len, sum) != (bytes, bytes * (n % 251)) {
            return Err(format!(
                "file-{n} is {len} bytes summing to {sum}: it was not kept whole"
            ));
        }
    }
    Ok(last.len())
}

/// The numbers in "unit 2, 950 of 956 MiB already in step".
fn in_step(detail: &str) -> Option<(u64, u64)> {
    let (_, rest) = detail.split_once(", ")?;
    let mut words = rest.split(' ');
    let done = words.next()?.parse().ok()?;
    let _of = words.next()?;
    let all = words.next()?.parse().ok()?;
    Some((done, all))
}

/// One unit removed: the console IPLs on the other and says which is
/// missing. `keep` is the place of the disk left.
fn one_missing(run: &Run<'_>, pair: &[Disk; 2], keep: usize) -> Result<String, String> {
    let gone = 1 - keep;
    let mut guest = run.boot(&machine(run, &pair[keep..=keep])?)?;
    let deadline = run.deadline_degraded();
    let missing = posted(
        &refcode::UNIT_FAILED,
        &format!("unit {}, serial {}: missing", gone + 1, pair[gone].serial),
    );
    guest.seen(&missing, deadline)?;
    base_ready(&mut guest, deadline)?;
    let storage = guest.channel(deadline)?.storage()?;
    let unit = storage
        .units
        .get(gone)
        .ok_or("the storage status has one unit")?;
    if unit.state != "Missing" {
        return Err(format!("unit {} is {}", gone + 1, unit.state));
    }
    // PID 1 asks the storage service every two seconds.
    let wanted = format!("unit {} Missing", gone + 1);
    let by = Instant::now() + Duration::from_secs(10);
    channel(&mut guest)?.until("the status screen's load source", by, |s| {
        s.fact(fact::LOAD_SOURCE)
            .is_some_and(|l| l.contains(&wanted))
    })?;
    Ok(missing)
}

/// Both units of `pair` booted after unit 2 was away while `written`
/// were written: how much of it was already in step, of all, the
/// storage's status once whole, and the files read back.
fn returned(
    run: &Run<'_>,
    pair: &[Disk; 2],
    written: &[u64],
) -> Result<(u64, u64, stgd::Status, usize), String> {
    let mut guest = run.boot(&machine(run, pair)?)?;
    let deadline = run.deadline();
    base_ready(&mut guest, deadline)?;
    // The storage service starts first: the resynchronization may be
    // posted before "Base ready", so it is looked for in the whole log.
    let started = posted(&refcode::RESYNC_STARTED, "unit 2, ");
    guest.wait_for(&format!("`{started}`"), deadline, |log| {
        log.split_inclusive('\n')
            .any(|l| l.starts_with(started.as_str()) && l.ends_with('\n'))
    })?;
    let session: &crate::qemu::session::Session = &guest;
    let detail = session
        .log()
        .lines()
        .find_map(|l| l.strip_prefix(started.as_str()))
        .unwrap_or_default()
        .trim_end()
        .to_string();
    let (done, all) = in_step(&format!("unit 2, {detail}"))
        .ok_or_else(|| format!("`{detail}` does not say how much is in step"))?;
    if done == 0 || done >= all {
        return Err(format!(
            "the returned unit was resynchronized from {done} of {all} MiB in step: not only \
             what changed"
        ));
    }
    let finish = Instant::now() + Duration::from_secs(120);
    guest.seen(&posted(&refcode::RESYNC_FINISHED, "unit 2, "), finish)?;
    let channel = guest.channel(deadline)?;
    let storage = channel.storage_until("both units in step", deadline, stgd::Status::whole)?;
    let kept = read_back(channel, written)?;
    power_off(&mut guest)?;
    Ok((done, all, storage, kept))
}

/// Unit 2 absent at the IPL, written to without it, the console stopped
/// in order and started with it back: it comes back by the map its set
/// began for it when assembled without it (`docs/OPEN_QUESTIONS.md`
/// item 13), not copied whole.
fn absent_returns(run: &Run<'_>) -> Result<String, String> {
    let pair = run.mirrored("absent")?;
    let mut guest = run.boot(&machine(run, &pair[..1])?)?;
    base_ready(&mut guest, run.deadline_degraded())?;
    let written = write_for(channel(&mut guest)?, 5)?;
    power_off(&mut guest)?;
    drop(guest);
    let (done, all, _, kept) = returned(run, &pair, &written)?;
    Ok(format!(
        "unit 2 absent at the IPL and returned: {} of {all} MiB copied ({done} already in step \
         by its map), {kept} files read back",
        all - done
    ))
}

/// Suite 7.
pub fn mirror(run: &Run<'_>) -> Result<String, String> {
    // Either unit absent at the IPL.
    let said_two = one_missing(run, &run.mirrored("unit2")?, 0)?;
    let said_one = one_missing(run, &run.mirrored("unit1")?, 1)?;

    // Unit 2 lost while the console runs (every transfer to it fails
    // from then on), written to without it, then returned: it comes
    // back by what changed.
    let pair = run.mirrored("leave")?;
    let leave = Machine {
        monitor: Some(run.socket("monitor.sock")),
        ..machine(run, &pair)?
    };
    let mut guest = booted(run, &leave)?;
    guest.monitor("drive_del hd1")?;
    let deadline = run.deadline();
    // The set notices at the first transfer to the unit gone.
    let written = write_for(channel(&mut guest)?, 5)?;
    // The disk is there when it fails: the inventory names its place.
    guest.seen(&posted(&refcode::UNIT_FAILED, "unit 2 at P"), deadline)?;
    power_off(&mut guest)?;
    drop(guest);

    let (done, all, storage, kept) = returned(run, &pair, &written)?;
    let absent = absent_returns(run)?;

    let killed = storage_killed(run)?;
    Ok(format!(
        "absent at the IPL: `{said_two}`, `{said_one}`; unit 2 taken away while running and \
         returned: {} of {all} MiB copied ({done} already in step), {} units Active, {kept} \
         files read back; {absent}; {killed}",
        all - done,
        storage.units.len()
    ))
}

/// The storage service killed under write load: started again, the
/// device taken over, the writes go on, nothing is lost and nothing is
/// resynchronized.
fn storage_killed(run: &Run<'_>) -> Result<String, String> {
    let pair = run.mirrored("kill")?;
    let mut guest = booted(run, &machine(run, &pair)?)?;
    let channel = channel(&mut guest)?;
    all_ready(&channel.status()?)?;
    channel.ask(&format!("{} {LOAD_MIB}", command::LOAD))?;
    let mut pids = Vec::new();
    for _ in 0..KILLS {
        std::thread::sleep(Duration::from_secs(2));
        let pid = channel.status()?.service(stgd::NAME).map_or(0, |s| s.pid);
        pids.push(pid);
        channel.ask(&format!("{} {}", command::KILL, stgd::NAME))?;
        let deadline = Instant::now() + Duration::from_secs(30);
        channel.until(
            "the storage service started again and ready",
            deadline,
            |s| {
                s.service(stgd::NAME)
                    .is_some_and(|n| n.state == "ready" && n.pid != pid)
            },
        )?;
    }
    std::thread::sleep(Duration::from_secs(2));
    channel.ask(&format!("{} 0", command::LOAD))?;
    std::thread::sleep(Duration::from_secs(2));
    let files = whole(channel)?;
    let kept = read_back(channel, &files)?;
    // A kill under writes leaves the regions then in flight to be
    // copied again, and no more: the write-intent map bounds it.
    let by = Instant::now() + Duration::from_secs(120);
    channel.storage_until("both units in step, nothing to copy", by, |s| {
        s.whole() && s.work.is_empty()
    })?;
    if !guest.said("taken over as /dev/ublkb") {
        return Err("no restarted storage service said it took the device over".into());
    }
    let prefix = posted(&refcode::RESYNC_STARTED, "units in step again, ");
    let session: &crate::qemu::session::Session = &guest;
    let copied: Vec<String> = session
        .log()
        .lines()
        .filter_map(|l| l.strip_prefix(prefix.as_str()))
        .map(|rest| rest.split(" MiB").next().unwrap_or(rest).to_string())
        .collect();
    if let Some(whole_set) = copied.iter().find(|c| {
        let mut w = c.split(" of ");
        w.next() == w.next()
    }) {
        return Err(format!(
            "a kill had the whole set copied again: {whole_set} MiB"
        ));
    }
    power_off(&mut guest)?;
    Ok(format!(
        "the storage service killed {KILLS} times (processes {pids:?}) under {LOAD_MIB} MiB \
         writes, each time taken over: {} files written, the last {kept} read back whole; only \
         the regions in flight copied again ({} MiB)",
        files.len(),
        copied.join(", ")
    ))
}

/// Suite 8: each disk run alone, then both: the IPL stops with the code
/// until the unit to keep is chosen (here through the test hooks'
/// channel, while the console's panel shows the question; Phase 2's
/// `apart-panel` answers at the panel).
pub fn apart(run: &Run<'_>) -> Result<String, String> {
    let pair = run.mirrored("apart")?;
    for alone in [0, 1] {
        let mut guest = booted_degraded(run, &machine(run, &pair[alone..=alone])?)?;
        power_off(&mut guest)?;
    }
    let mut guest = run.boot(&machine(run, &pair)?)?;
    let deadline = run.deadline();
    guest.seen(&refcode::UNITS_APART.tag(), deadline)?;
    guest.seen("the IPL waits until the unit to keep is chosen", deadline)?;
    // It waits, for as long as it takes.
    std::thread::sleep(Duration::from_secs(15));
    if guest.said(&refcode::DATA_MOUNTED.tag()) || guest.said(&refcode::BASE_READY.tag()) {
        return Err("the IPL went on though no unit was chosen".into());
    }
    let channel = guest.channel(deadline)?;
    let storage = channel.storage()?;
    if !storage.choose || storage.units.iter().any(|u| u.state != stgd::APART) {
        return Err(format!("the storage status while waiting is {storage:?}"));
    }
    channel.ask(&format!("{} vda", command::KEEP))?;
    let deadline = run.deadline();
    let secs = base_ready(&mut guest, deadline)?;
    // The console's panel had the serial port while the IPL waited: a
    // code posted then is on the display and in the history, which
    // takes what PID 1 queued before the log service once it is up.
    let by = Instant::now() + Duration::from_secs(20);
    loop {
        let history = super::channel(&mut guest)?.text("/data/log/refcodes.ndjson")?;
        let lacking = [&refcode::UNIT_KEPT, &refcode::DATA_MOUNTED]
            .into_iter()
            .find(|code| !history.contains(code.code));
        match lacking {
            None => break,
            Some(code) if Instant::now() > by => {
                return Err(format!("the history lacks `{}`", code.line()));
            }
            Some(_) => std::thread::sleep(Duration::from_millis(500)),
        }
    }
    power_off(&mut guest)?;
    Ok(format!(
        "both disks ran alone: `{}` and the IPL waited 15 s and more; unit 1 kept through the \
         channel, then /data mounted and base ready ({secs} s after the kernel started)",
        refcode::UNITS_APART.tag()
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn what_is_in_step_is_read_from_the_detail() {
        assert_eq!(
            in_step("unit 2, 950 of 956 MiB already in step"),
            Some((950, 956))
        );
        assert_eq!(in_step("unit 2"), None);
    }
}
