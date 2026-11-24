// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! Suite 12: every action of this phase leaves an audit record with its
//! user; a scheduled run names its owner; no log holds a password or the
//! console's key.

use super::{VS, all_logs, expect, lab, post_form, post_on};
use crate::suite::base::channel;
use crate::suite::mgmt::page;
use crate::suite::web::{ADMIN, PASSWORD, submit};
use crate::web::stream::Upload;

/// Suite 12.
#[allow(clippy::too_many_lines, reason = "one action a step")]
pub fn audit(run: &crate::suite::base::Run<'_>) -> Result<String, String> {
    let mut lab = lab(run, "parts-audit")?;
    let done = super::requests::scenario(&mut lab)?;
    let mut wants: Vec<String> = done.audits.iter().map(|s| (*s).to_string()).collect();
    let b = &mut lab.c.browser;
    // A destructive action, a partition made, a group, a schedule run
    // now, an upload.
    let r = submit(
        b,
        "/qsmc/confirm/vdisk-delete?ms=QSHQVS1&n=VD06&p=POOL1",
        &[("ms", VS), ("n", "VD06"), ("p", "POOL1"), ("ok", "OK")],
    )?;
    expect(b, &r, "QMC2414")?;
    wants.push("delete virtual disk POOL1/VD06 on QSHQVS1".into());
    // The new partition's load source: a disk of its own.
    let r = post_form(
        b,
        "/qsmc/server/QSHQVS1/virtual/storage/create",
        ("ok", "OK"),
        &[("n", "VD08"), ("s", "10"), ("p", "POOL1"), ("t", "thin")],
    )?;
    expect(b, &r, "QMC2410")?;
    let mut html = page(b, "/qsmc/partitions/create/step/1?ms=QSHQVS1")?;
    let mut here = "/qsmc/partitions/create/step/1?ms=QSHQVS1".to_string();
    for n in 1..=8 {
        let set: &[(&str, &str)] = if n == 1 {
            &[("id", "7"), ("n", "QSAUD07")]
        } else {
            &[]
        };
        if n == 6 {
            let add = post_on(
                b,
                &html,
                &here,
                ("add", "Add..."),
                &[("t", "SCSI client (virtual disk)")],
            )?;
            let r = post_on(
                b,
                &add.text(),
                &here,
                ("keep", "OK"),
                &[("disk", "POOL1/VD08"), ("req", "on")],
            )?;
            if r.status != 303 {
                return Err(format!("step 6's adapter: {}", r.status));
            }
            html = page(b, &here)?;
        }
        post_on(b, &html, &here, ("next", "Next >"), set)?;
        here = format!("/qsmc/partitions/create/step/{}", n + 1);
        html = page(b, &here)?;
    }
    let r = post_on(b, &html, &here, ("finish", "Finish"), &[])?;
    expect(b, &r, "QMC3130")?;
    wants.push("create partition QSAUD07 (ID 7) on QSHQVS1".into());
    let r = post_form(
        b,
        "/qsmc/groups/create",
        ("ok", "OK"),
        &[("g", "AUDIT"), ("m1", "on")],
    )?;
    expect(b, &r, "QMC3301")?;
    wants.push("custom group AUDIT".into());
    let r = post_form(
        b,
        "/qsmc/schedule/add",
        ("ok", "OK"),
        &[
            ("r", "daily"),
            ("op", "Restart partition"),
            ("tg", "QSDEV01 on QSHQVS1"),
        ],
    )?;
    expect(b, &r, "QMC3401")?;
    wants.push("schedule Restart partition QSDEV01 on QSHQVS1".into());
    let confirm = page(b, "/qsmc/schedule?run=1")?;
    let r = post_on(b, &confirm, "/qsmc/schedule?run=1", ("ok", "OK"), &[])?;
    expect(b, &r, "QMC3404")?;
    let path = format!("/qsmc/server/{VS}/virtual/optical");
    let csrf = b.csrf(&path)?;
    let a = b
        .stream_upload(
            &path,
            &Upload {
                csrf,
                name: "AUDIT.iso".into(),
                bytes: 512 * 2048,
                announce: None,
                cut: None,
                head_only: false,
                label: "AUDIT".into(),
            },
        )?
        .ok_or("the upload had no answer")?;
    expect(b, &a, "QMC2401")?;
    wants.push(format!(
        "upload image AUDIT ({} bytes) to QSHQVS1",
        512 * 2048
    ));
    let r = submit(
        b,
        "/qsmc/confirm/schedule-remove?id=1",
        &[("id", "1"), ("ok", "OK")],
    )?;
    expect(b, &r, "QMC3403")?;
    wants.push("remove scheduled operation 1".into());
    std::thread::sleep(std::time::Duration::from_secs(3));
    let logs = all_logs(&mut lab.c.guest)?;
    let by = format!("audit: user {ADMIN} from ");
    let mut lacking: Vec<&str> = wants
        .iter()
        .filter(|w| {
            !logs
                .lines()
                .any(|l| l.contains(&by) && l.contains(w.as_str()))
        })
        .map(String::as_str)
        .collect();
    // The run now, in its owner's name, by the scheduler.
    let run = "audit: user admin from schedule 1: Restart partition QSDEV01 on QSHQVS1";
    if !logs.contains(run) {
        lacking.push(run);
    }
    if !lacking.is_empty() {
        return Err(format!(
            "{} of {} actions have no audit record: {}",
            lacking.len(),
            wants.len() + 1,
            lacking.join("; ")
        ));
    }
    if logs.contains(PASSWORD) {
        return Err("a log holds the Administrator's password".into());
    }
    let key = channel(&mut lab.c.guest)?
        .cat("/data/state/identity.key")
        .unwrap_or_default();
    let key = String::from_utf8_lossy(&key);
    if let Some(line) = key.lines().nth(1).filter(|l| l.len() > 16)
        && logs.contains(line)
    {
        return Err("a log holds the console's QSI key".into());
    }
    let lines = logs.lines().filter(|l| l.contains("audit: user ")).count();
    lab.c.guest.kill();
    Ok(format!(
        "{} kinds of this phase's action each audited with user admin, the scheduled run with its \
         owner; {lines} audit lines in {} bytes of logs, none with a password or the key",
        wants.len() + 1,
        logs.len()
    ))
}
