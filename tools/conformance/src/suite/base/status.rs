// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! Suite 1: every IPL step's code in order, "Base ready" within 10 s of
//! the kernel starting under KVM, the status screen and the control
//! socket's status; on a single-disk load source and on a mirrored one.
//! Suite 13: on a processor without what the cryptography needs the
//! console reaches "Base ready" and its status screen says so.

use release::{fact, screen};

use super::{Run, all_ready, base_ready, channel, fact_of, posted, screen_with};
use crate::qemu::session::Machine;
use crate::suite::steps::LACKING_CPU;

/// The longest "Base ready" may take after the kernel started, under
/// KVM.
const READY_WITHIN: f64 = 10.0;

/// The IPL's codes, in the order PID 1 posts them.
fn ipl_codes() -> Vec<String> {
    [
        &refcode::IPL_STARTED,
        &refcode::MOUNTED,
        &refcode::RECORD_READ,
        &refcode::DATA_MOUNTED,
        &refcode::CLOCK_SET,
        &refcode::SEED_CHECKED,
        &refcode::STATE_LOADED,
        &refcode::IDENTITY_GENERATED,
        &refcode::SERVICES_STARTED,
    ]
    .iter()
    .map(|c| c.tag())
    .collect()
}

/// Boot `machine` and check the IPL, "Base ready" and the screen's
/// load-source row: the seconds "Base ready" took.
fn ipl(
    run: &Run<'_>,
    machine: &Machine,
    load_source: &str,
    also: &[String],
) -> Result<(f64, String), String> {
    let mut guest = run.boot(machine)?;
    let deadline = run.deadline();
    for code in ipl_codes() {
        guest.expect(&code, deadline)?;
    }
    for line in also {
        guest.seen(line, deadline)?;
    }
    let secs = base_ready(&mut guest, deadline)?;
    if run.launch.kvm && secs >= READY_WITHIN {
        return Err(format!(
            "base ready {secs} s after the kernel started; at most {READY_WITHIN}"
        ));
    }
    screen_with(
        &mut guest,
        &[
            screen::row(screen::SLOT, "A (default A, nothing pending)"),
            screen::row(screen::LOAD_SOURCE, load_source),
            screen::row(screen::REFERENCE, refcode::BASE_READY.code),
            screen::service("stgd", "ready"),
            screen::service("logd", "ready"),
            screen::service("netd", "ready"),
        ],
        deadline,
    )?;
    let status = guest.channel(deadline)?.status()?;
    all_ready(&status)?;
    if fact_of(&status, fact::IMAGE)? != "test" {
        return Err("the status does not say this is a test image".into());
    }
    let data = fact_of(&status, fact::DATA)?.to_string();
    let ready = status.services.len();
    Ok((secs, format!("{ready} services ready, /data on {data}")))
}

/// The suite.
pub fn status(run: &Run<'_>) -> Result<String, String> {
    let (one, said_one) = ipl(run, &run.machine()?, "none: unit 1 Unprotected on vda", &[])?;

    let pair = run.mirrored("pair")?;
    let machine = Machine {
        disks: pair.to_vec(),
        ..run.machine()?
    };
    let (two, said_two) = ipl(
        run,
        &machine,
        "mirrored: unit 1 Active on vda, unit 2 Active on vdb",
        &[posted(&refcode::SET_ASSEMBLED, "2 units, served as ")],
    )?;
    if !said_two.contains("/dev/ublkb") {
        return Err(format!("the mirrored load source's /data: {said_two}"));
    }
    let kvm = if run.launch.kvm {
        "under KVM"
    } else {
        "without KVM"
    };
    Ok(format!(
        "one disk: base ready {one} s after the kernel started {kvm}, {said_one}; \
         mirrored: {two} s, {said_two}"
    ))
}

/// Suite 13.
pub fn processor(run: &Run<'_>) -> Result<String, String> {
    let machine = Machine {
        cpu: Some(LACKING_CPU.into()),
        ..run.machine()?
    };
    let mut guest = run.boot(&machine)?;
    let deadline = run.deadline();
    guest.expect(&refcode::PROCESSOR_LACKS.tag(), deadline)?;
    let secs = base_ready(&mut guest, deadline)?;
    let lacks = screen::row(screen::PROCESSOR, "lacks ");
    screen_with(&mut guest, std::slice::from_ref(&lacks), deadline)?;
    let status = channel(&mut guest)?.status()?;
    all_ready(&status)?;
    Ok(format!(
        "on `{LACKING_CPU}`: base ready {secs} s after the kernel started; the screen says \
         `Processor      {}`",
        fact_of(&status, fact::PROCESSOR)?
    ))
}
