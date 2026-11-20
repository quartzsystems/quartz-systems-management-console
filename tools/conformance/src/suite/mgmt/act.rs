// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! Suite 6: power off and restart through the confirmation only, a
//! refusal in the host's words. Suite 7: the reference code history and
//! its explanations. Suite 9: the control panel.

use std::time::Duration;

use super::{Fleet, keep, page, until};
use crate::suite::web::{ADMIN, PASSWORD, follow, message_of, shows, signed_on, start, submit};

/// How many power or panel requests the host has had.
fn asked(fleet: &Fleet, op: &str) -> usize {
    fleet.lines(0).matches(op).count()
}

/// Suite 6.
pub fn power(run: &crate::suite::base::Run<'_>) -> Result<String, String> {
    let fleet = Fleet::start(vec![sim::fixture::qshqvs1()])?;
    let mut c = start(run, "mgmt-power")?;
    let b = &mut c.browser;
    signed_on(b, ADMIN, PASSWORD)?;
    keep(b, &fleet, 0)?;
    let power = "request 0001/0003";
    let path = "/qsmc/server/QSHQVS1/confirm?op=poweroff";
    let t = page(b, path)?;
    for p in [
        "QSPROD01",
        "QSDEV01",
        "QSTEST01",
        "Shut down (delayed, 300 s) then power off",
    ] {
        if !t.contains(p) {
            return Err(format!("the confirmation does not list `{p}`"));
        }
    }
    // Without the token, or by GET: nothing.
    let r = b.post(path, &[("ms", "QSHQVS1"), ("ok", "OK")])?;
    if r.status != 403 || asked(&fleet, power) != 0 {
        return Err(format!("a POST without its token answered {}", r.status));
    }
    // Refused: the console's QMC2210 and the host's own message.
    fleet.tell(0, "refuse-power QVS1011 Two partitions are running.");
    let r = submit(
        b,
        path,
        &[("ms", "QSHQVS1"), ("how", "delayed"), ("ok", "OK")],
    )?;
    let t = follow(b, &r)?.text();
    if !shows(&t, "QMC2210") || !shows(&t, "QVS1011") || !t.contains("Two partitions are running.")
    {
        return Err(format!("the refusal: {}", message_of(&t)));
    }
    fleet.tell(0, "allow");
    // Restart: started, the system away, and its session back.
    let restart = "/qsmc/server/QSHQVS1/confirm?op=restart";
    let r = submit(
        b,
        restart,
        &[("ms", "QSHQVS1"), ("how", "delayed"), ("ok", "OK")],
    )?;
    let t = follow(b, &r)?.text();
    if !shows(&t, "QMC2203") {
        return Err(format!("the restart: {}", message_of(&t)));
    }
    until("the host back", Duration::from_secs(120), || {
        Ok(fleet.lines(0).matches("session from").count() >= 3
            && page(b, "/qsmc/connectivity")?.contains(">Connected<"))
    })?;
    if !fleet.lines(0).contains("request 0001/0003 from user admin") {
        return Err("the power request did not name the user".into());
    }
    // Power on answers that this release cannot.
    let r = submit(
        b,
        "/qsmc/server/QSHQVS1/confirm?op=poweron",
        &[("ms", "QSHQVS1"), ("ok", "OK")],
    )?;
    if !shows(&follow(b, &r)?.text(), "QMC2201") {
        return Err("Power on did not answer QMC2201".into());
    }
    c.guest.kill();
    Ok(
        "the power off confirmation lists QSPROD01, QSDEV01 and QSTEST01; a POST without its \
         token is 403 and nothing reached the host; the host's refusal shows QMC2210 then its \
         QVS1011; a restart named the user, the host went and its session came back; Power on \
         answers QMC2201"
            .into(),
    )
}

/// Suite 7.
pub fn refcodes(run: &crate::suite::base::Run<'_>) -> Result<String, String> {
    let fleet = Fleet::start(vec![sim::fixture::qshqvs1()])?;
    let mut c = start(run, "mgmt-refcodes")?;
    let b = &mut c.browser;
    signed_on(b, ADMIN, PASSWORD)?;
    keep(b, &fleet, 0)?;
    for i in 0..30 {
        fleet.tell(0, &format!("pcode 1 C6004031 Partition IPL step {i}"));
    }
    fleet.tell(0, "code C2005090 Partition started: QSDEV01, side A");
    fleet.tell(0, "code A2999999 A code no table has");
    until("the codes kept", Duration::from_secs(30), || {
        Ok(page(b, "/qsmc/refcodes?ms=QSHQVS1")?.contains(">A2999999</a>"))
    })?;
    // Filters: the partition, the host, the dates; paging.
    let only = page(b, "/qsmc/refcodes?ms=QSHQVS1&p=QSTEST01")?;
    if !only.contains(">A6005008</a>") || only.contains(">C6004031</a>") {
        return Err("the partition filter".into());
    }
    let host = page(b, "/qsmc/refcodes?ms=QSHQVS1&p=(host)")?;
    if !host.contains(">C2005090</a>") || host.contains(">A6005008</a>") {
        return Err("the host filter".into());
    }
    let later = page(b, "/qsmc/refcodes?ms=QSHQVS1&f=12/31/99")?;
    if later.contains(">C6004031</a>") {
        return Err("the date filter".into());
    }
    let paged = page(b, "/qsmc/refcodes?ms=QSHQVS1&rows=10")?;
    if !paged.contains("Page 1 of 4") {
        return Err("paging".into());
    }
    // Explanations: a partition's code from QS/400's table, a host's
    // from QS/VS's catalogue, and one neither knows.
    let system = page(b, "/qsmc/refcode/A6005008?ms=QSHQVS1&p=2")?;
    let src = srcdata::lookup("A6005008").ok_or("srcdata lacks A6005008")?;
    if !system.contains(src.meaning) || !system.contains("Open terminal window") {
        return Err("the partition's explanation".into());
    }
    let hosts = page(b, "/qsmc/refcode/C2005090?ms=QSHQVS1&p=0")?;
    if shows(&hosts, "QMC1201") || !hosts.contains("Partition started") {
        return Err("the host's explanation is not the catalogue's".into());
    }
    let unknown = page(b, "/qsmc/refcode/A2999999?ms=QSHQVS1&p=0")?;
    if !shows(&unknown, "QMC1201") || !unknown.contains("A code no table has") {
        return Err("the unknown code".into());
    }
    c.guest.kill();
    Ok(
        "the history filtered by partition, by the host and by date, and paged 10 a page over \
         4 pages; A6005008 explained from QS/400's table, C2005090 from QS/VS's catalogue, \
         A2999999 shown with its text and QMC1201"
            .into(),
    )
}

/// Suite 9.
pub fn panel(run: &crate::suite::base::Run<'_>) -> Result<String, String> {
    let fleet = Fleet::start(vec![sim::fixture::qshqvs1()])?;
    let mut c = start(run, "mgmt-panel")?;
    let b = &mut c.browser;
    signed_on(b, ADMIN, PASSWORD)?;
    keep(b, &fleet, 0)?;
    let panel = "/qsmc/partition/QSHQVS1/1/panel";
    let mut answered = Vec::new();
    for f in ["01", "11", "12", "20"] {
        let r = submit(
            b,
            panel,
            &[("func", &format!("{f} - x")), ("select", "Select")],
        )?;
        let t = follow(b, &r)?.text();
        if !shows(&t, "QMC5003") || !t.contains("Reference code before:") {
            return Err(format!("function {f}: {}", message_of(&t)));
        }
        answered.push(f);
    }
    // Refused by the host, in its words.
    let r = submit(b, panel, &[("f", "03 IPL")])?;
    let t = follow(b, &r)?.text();
    if !shows(&t, "QMC5010") || !shows(&t, "QVS2307") {
        return Err(format!("function 03 at normal: {}", message_of(&t)));
    }
    // 08 and 22: through the confirmation only.
    fleet.tell(0, "keylock 1 manual");
    let functions = "request 0014/0001";
    let before = asked(&fleet, functions);
    let r = submit(b, panel, &[("f", "08 Power off")])?;
    let to = r.header("location").unwrap_or("").to_string();
    if !to.starts_with("/qsmc/confirm/panel") || asked(&fleet, functions) != before {
        return Err(format!("08 went to {to}"));
    }
    page(b, &to)?;
    let bare = b.post(
        &to,
        &[("ms", "QSHQVS1"), ("p", "1"), ("f", "08"), ("ok", "OK")],
    )?;
    if bare.status != 403 || asked(&fleet, functions) != before {
        return Err("08 without its token".into());
    }
    let r = submit(
        b,
        &to,
        &[("ms", "QSHQVS1"), ("p", "1"), ("f", "08"), ("ok", "OK")],
    )?;
    let t = follow(b, &r)?.text();
    if !shows(&t, "QMC5003") || asked(&fleet, functions) != before + 1 {
        return Err(format!("08 confirmed: {}", message_of(&t)));
    }
    let r = submit(b, panel, &[("f", "34 Retry")])?;
    let t = follow(b, &r)?.text();
    if !shows(&t, "QVS2309") && !shows(&t, "QVS2003") {
        return Err(format!("34: {}", message_of(&t)));
    }
    c.guest.kill();
    Ok(format!(
        "functions {} answered with the code before and after; 03 at a normal keylock refused \
         with the host's QVS2307; 08 only through its confirmation and a POST with its token; 34 \
         refused in the host's words",
        answered.join(", ")
    ))
}
