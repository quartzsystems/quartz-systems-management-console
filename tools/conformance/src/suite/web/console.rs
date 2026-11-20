// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! Suite 3: the first start after an installation: the installation's
//! user signs on as the first Administrator, and the certificate names
//! the host and its address. Suite 10: a slot on trial whose web server
//! cannot start is not confirmed, and the next IPL is on the old slot.
//! Suite 12: the local console's option 4 resets a password the web
//! interface then signs on with. Suite 15: a processor without what the
//! cryptography needs runs no web server, says why, and its local
//! console works.

use std::time::{Duration, Instant};

use release::fact;
use x509kit::x509::CertInfo;

use super::{ADMIN, CERT, PASSWORD, machine, signed_on, start};
use crate::qemu::term::key;
use crate::suite::base::network::listening;
use crate::suite::base::{base_ready, channel, fact_of, posted, power_off};
use crate::suite::install::{self, MAIN_MENU, USER, option, sign_on, titled};
use crate::suite::steps::LACKING_CPU;

/// The suite of the first start.
pub fn first(run: &super::Run<'_>) -> Result<String, String> {
    let mut c = start(run, "web-first")?;
    signed_on(&mut c.browser, ADMIN, PASSWORD)?;
    let pem = channel(&mut c.guest)?.text(CERT)?;
    let b = x509kit::bundle::read_pem(&pem, None).map_err(|e| e.to_string())?;
    let info: &CertInfo = &b.chain.first().ok_or("no certificate")?.info;
    for name in [install::HOST, "10.0.2.15"] {
        if !info.san.iter().any(|s| s.eq_ignore_ascii_case(name)) {
            return Err(format!(
                "the certificate does not name {name}: {:?}",
                info.san
            ));
        }
    }
    let users = channel(&mut c.guest)?.text("/data/state/webusers.toml")?;
    if !users.contains("First Administrator, from the installation") {
        return Err("the first Administrator is not the installation's".into());
    }
    power_off(&mut c.guest)?;
    Ok(format!(
        "{USER} signed on with the installation's password as {ADMIN}, the first Administrator; \
         the certificate is {} and names {}",
        info.subject,
        info.san.join(", ")
    ))
}

/// The suite of a slot whose web server cannot start.
pub fn slot(run: &super::Run<'_>) -> Result<String, String> {
    let mut m = crate::suite::base::slot::on_trial(run, "web-slot")?;
    m.oem = vec![qsmchook::oem_string(qsmchook::WEBD_FAILS)];
    let mut guest = run.boot(&m)?;
    let deadline = Instant::now() + Duration::from_secs(300);
    guest.expect("IPL slot B (qsmc.slot=B)", deadline)?;
    guest.seen(&posted(&refcode::SERVICE_STOPPED, ""), deadline)?;
    let status = channel(&mut guest)?.status()?;
    if fact_of(&status, fact::RECORD)? != "default A, pending B (0 attempts left)" {
        return Err(format!(
            "the record is `{}`",
            fact_of(&status, fact::RECORD)?
        ));
    }
    if guest.said(&refcode::SLOT_CONFIRMED.tag()) {
        return Err("the slot confirmed though its web server never started".into());
    }
    power_off(&mut guest)?;
    drop(guest);
    m.oem.clear();
    let mut guest = run.boot(&m)?;
    let deadline = run.deadline();
    guest.expect("IPL slot A (qsmc.slot=A)", deadline)?;
    let fell = posted(&refcode::SLOT_FELL_BACK, "slot B; this IPL is on slot A");
    guest.expect(&fell, deadline)?;
    base_ready(&mut guest, deadline)?;
    power_off(&mut guest)?;
    Ok(format!(
        "slot B on trial with a web server that fails: stopped after its failures, not \
         confirmed; the next IPL was on slot A with `{fell}`"
    ))
}

/// The suite of the local console's option 4.
pub fn option4(run: &super::Run<'_>) -> Result<String, String> {
    let mut c = start(run, "web-option4")?;
    let deadline = run.deadline();
    sign_on(&mut c.guest, USER, PASSWORD, deadline)?;
    let shown = option(&mut c.guest, 4, "Work with the Web Interface", deadline)?;
    if !shown.contains("https://10.0.2.15/qsmc/") {
        return Err(format!(
            "option 4 does not say where the web interface is:\n{shown}"
        ));
    }
    option(&mut c.guest, 1, "Reset or Make an Administrator", deadline)?;
    let pw = "Fresh-start-pass-8";
    let keys = install::fill(&[Some("rescue"), Some(pw), Some(pw)], &[32, 32, 32]);
    c.guest.type_keys(&format!("{keys}{}", key::ENTER))?;
    let back = install::titled_with(
        &mut c.guest,
        "Work with the Web Interface",
        "rescue can sign on",
        deadline,
    )?;
    if !back.contains("rescue can sign on to the web interface as an Administrator.") {
        return Err(format!("the reset said:\n{back}"));
    }
    signed_on(&mut c.browser, "rescue", pw)?;
    c.guest.type_keys(key::F12)?;
    titled(&mut c.guest, MAIN_MENU, deadline)?;
    c.guest.type_keys(key::F3)?;
    power_off(&mut c.guest)?;
    Ok(
        "option 4 showed https://10.0.2.15/qsmc/; option 1 made `rescue` an Administrator, \
         and the web interface signed it on with the password typed at the serial port"
            .into(),
    )
}

/// The suite of a processor that lacks what the cryptography needs.
pub fn processor(run: &super::Run<'_>) -> Result<String, String> {
    let mut m = machine(run, "web-processor")?;
    m.cpu = Some(LACKING_CPU.into());
    let mut guest = run.boot(&m)?;
    let deadline = run.deadline();
    guest.expect(&refcode::PROCESSOR_LACKS.tag(), deadline)?;
    base_ready(&mut guest, deadline)?;
    let status = channel(&mut guest)?.status()?;
    if status.services.iter().any(|s| s.name == "webd") {
        return Err("a web server is in the table".into());
    }
    let tcp = channel(&mut guest)?.text("/proc/net/tcp")?;
    if let Some((a, p)) = listening(&tcp, false).first() {
        return Err(format!("{a}:{p} listens with no web server"));
    }
    sign_on(&mut guest, USER, PASSWORD, deadline)?;
    let shown = option(&mut guest, 4, "Work with the Web Interface", deadline)?;
    if !shown.contains("not running") {
        return Err(format!(
            "option 4 does not say the web server is not running:\n{shown}"
        ));
    }
    guest.type_keys(key::F12)?;
    titled(&mut guest, MAIN_MENU, deadline)?;
    guest.type_keys(key::F3)?;
    power_off(&mut guest)?;
    Ok(format!(
        "on `{LACKING_CPU}`: `{}` posted, no web server in the table and nothing listening; the \
         local console signed {USER} on and option 4 said the web server is not running",
        refcode::PROCESSOR_LACKS.code
    ))
}
