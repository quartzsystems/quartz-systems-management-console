// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! Suite 11: the identity is made once. The same certificate after a
//! restart, a host name change and a clock change; its key readable by
//! root only.

use std::time::{Duration, Instant};

use qsmchook::proto::command;
use release::fact;

use super::{Run, booted, channel, fact_of, posted};
use crate::qemu::session::{Machine, Session};

/// The identity's files in the state directory.
const CERT: &str = "/data/state/identity.crt";
const KEY: &str = "/data/state/identity.key";
/// How far the clock is moved: a year and more.
const AHEAD: i64 = 400 * 86_400;

fn restart(guest: &mut Session) -> Result<(), String> {
    channel(guest)?.tell(command::RESTART)?;
    guest
        .wait_exit(Instant::now() + Duration::from_secs(30))
        .map(|_| ())
}

/// The suite.
pub fn identity(run: &Run<'_>) -> Result<String, String> {
    let machine = Machine {
        disks: vec![run.disk_copy("disk")?],
        ..run.machine()?
    };
    let mut guest = run.boot(&machine)?;
    let deadline = run.deadline();
    guest.expect(
        &posted(&refcode::IDENTITY_GENERATED, identity::CERT_FILE),
        deadline,
    )?;
    super::base_ready(&mut guest, deadline)?;
    let channel_now = guest.channel(deadline)?;
    let certificate = channel_now.cat(CERT)?;
    let (mode, owner, _) = channel_now.stat(KEY)?;
    // From Phase 4 the key is mgmtd's user's alone (`docs/MGMT.md`).
    if mode != "600" || owner != release::MGMT_ID {
        return Err(format!("the key is mode {mode}, owner {owner}"));
    }
    channel_now.ask(&format!("{} mc-renamed", command::SET_HOST))?;
    restart(&mut guest)?;
    drop(guest);

    let same = |guest: &mut Session, after: &str| -> Result<(), String> {
        if guest.said(&refcode::IDENTITY_GENERATED.tag()) {
            return Err(format!("the identity was made again after {after}"));
        }
        if channel(guest)?.cat(CERT)? != certificate {
            return Err(format!("the certificate changed after {after}"));
        }
        Ok(())
    };
    let mut guest = booted(run, &machine)?;
    same(&mut guest, "a restart and a host name change")?;
    let status = channel(&mut guest)?.status()?;
    if fact_of(&status, fact::HOST)? != "mc-renamed" {
        return Err(format!(
            "the host name is `{}` after the change",
            fact_of(&status, fact::HOST)?
        ));
    }
    let now: i64 = fact_of(&status, fact::TIME)?
        .parse()
        .map_err(|_| "the status's time is not a number")?;
    channel(&mut guest)?.ask(&format!("{} {}", command::SET_CLOCK, now + AHEAD))?;
    // Each boot is a QEMU of its own, whose RTC starts from the host's
    // clock again: the clock is seen to move in this boot, and the
    // identity looked at after the restart that follows.
    let later: i64 = fact_of(&channel(&mut guest)?.status()?, fact::TIME)?
        .parse()
        .map_err(|_| "the status's time is not a number")?;
    if later < now + AHEAD - 600 {
        return Err(format!("the clock did not move: {now} then {later}"));
    }
    if channel(&mut guest)?.cat(CERT)? != certificate {
        return Err("the certificate changed with the clock".into());
    }
    restart(&mut guest)?;
    drop(guest);

    let mut guest = booted(run, &machine)?;
    same(&mut guest, "a clock change")?;
    Ok(format!(
        "made once ({} bytes of certificate), the key mode {mode} and root's; the same after a \
         restart, the host name `mc-renamed` and the clock {AHEAD} s ahead",
        certificate.len()
    ))
}
