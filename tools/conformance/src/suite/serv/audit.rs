// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! Suite 16's audit: every action of this phase leaves its record with
//! the user; no log holds a password, a hash or a key.

use std::time::Duration;

use super::pages::DUMP;
use super::{QSVS_IMAGE, STANDALONE, VS, download, lab, number_of};
use crate::suite::base::channel;
use crate::suite::mgmt::page;
use crate::suite::parts::{all_logs, expect, post_form, post_on};
use crate::suite::web::{ADMIN, PASSWORD};

/// Suite 16's audit.
#[allow(clippy::too_many_lines, reason = "one action a step")]
pub fn audit(run: &crate::suite::base::Run<'_>) -> Result<String, String> {
    let mut lab = lab(run, "serv-audit", &[])?;
    let mut wants: Vec<String> = vec![
        "update settings: changed".into(),
        "product streams: checked".into(),
    ];
    let b = &mut lab.c.browser;
    download(b, VS, "qsvs", QSVS_IMAGE)?;
    wants.push(format!("qsvs {QSVS_IMAGE}: download begun"));
    // An event closed, one noted and exported.
    let n = number_of(b, "B6000100")?;
    let path = format!("/qsmc/confirm/event-close?n={n}");
    let html = page(b, &path)?;
    let r = post_on(b, &html, &path, ("ok", "OK"), &[])?;
    expect(b, &r, qmcmsg::EVENT_CLOSED.id)?;
    wants.push(format!(
        "serviceable event {n} of {VS}: closed while it holds"
    ));
    lab.fleet
        .tell(0, "problem open 2 A2003010 An adapter lost its switch");
    let b = &mut lab.c.browser;
    let mut m = 0;
    crate::suite::mgmt::until("the new event listed", Duration::from_secs(60), || {
        m = number_of(b, "A2003010").unwrap_or(0);
        Ok(m > 0)
    })?;
    let r = post_form(
        b,
        &format!("/qsmc/event/{m}/note"),
        ("ok", "OK"),
        &[("text", "Looked at by the audit suite")],
    )?;
    expect(b, &r, qmcmsg::EVENT_NOTED.id)?;
    wants.push(format!("serviceable event {m} of {VS}: noted"));
    b.get(&format!("/qsmc/event/{m}/export"))?;
    wants.push(format!("serviceable event {m} of {VS}: exported"));
    // A dump downloaded, one deleted, the console's taken.
    let fw = format!("{STANDALONE}:0:FWDUMP.QSHQ400.20260930.120000");
    b.stream_get(&format!("/qsmc/dumps/download?d={fw}"), Some(4096), |_| {})?;
    wants.push(format!(
        "dump FWDUMP.QSHQ400.20260930.120000 of {STANDALONE}: downloaded"
    ));
    let confirm = format!("/qsmc/confirm/dump-delete?d={DUMP}");
    let html = page(b, &confirm)?;
    let r = post_on(b, &html, &confirm, ("ok", "OK"), &[])?;
    expect(b, &r, qmcmsg::DUMP_DELETED.id)?;
    wants.push(format!(
        "dump SYSDUMP.QSTEST01.20261029.221405 of {VS}: deleted"
    ));
    let token = b.csrf("/qsmc/dumps")?;
    let r = b.post("/qsmc/dumps/take", &[("csrf", &token)])?;
    expect(b, &r, qmcmsg::DUMP_TAKEN.id)?;
    wants.push("console dump: taken".into());
    // An install begun, one scheduled.
    let path = format!("/qsmc/confirm/update-install?t={VS}");
    let html = page(b, &path)?;
    let r = post_on(b, &html, &path, ("ok", "OK"), &[("timeout", "30")])?;
    expect(b, &r, qmcmsg::INSTALL_BEGUN.id)?;
    wants.push(format!(
        "code update {} on {VS}: install begun",
        super::QSVS_NEXT
    ));
    let form = page(b, &format!("/qsmc/schedule/add?op=update&ms={VS}"))?;
    let r = post_on(
        b,
        &form,
        "/qsmc/schedule/add",
        ("ok", "OK"),
        &[("r", "once"), ("delay", "30")],
    )?;
    expect(b, &r, "QMC3401")?;
    wants.push(format!("schedule Install code update {VS}: scheduled"));
    std::thread::sleep(Duration::from_secs(3));
    let logs = all_logs(&mut lab.c.guest)?;
    let by = format!("audit: user {ADMIN} from ");
    let lacking: Vec<&str> = wants
        .iter()
        .filter(|w| {
            !logs
                .lines()
                .any(|l| l.contains(&by) && l.contains(w.as_str()))
        })
        .map(String::as_str)
        .collect();
    if !lacking.is_empty() {
        return Err(format!(
            "{} of {} actions have no audit record: {}",
            lacking.len(),
            wants.len(),
            lacking.join("; ")
        ));
    }
    // No password, hash or key in any log.
    let ch = channel(&mut lab.c.guest)?;
    let mut secrets = vec![PASSWORD.to_string()];
    let users = ch.text("/data/state/webusers.toml").unwrap_or_default();
    secrets.extend(
        users
            .split('"')
            .filter(|s| s.starts_with("$argon2"))
            .map(str::to_string),
    );
    for file in ["/data/state/identity.key", "/data/web/key.pem"] {
        let text = ch.text(file).unwrap_or_default();
        secrets.extend(
            text.lines()
                .filter(|l| l.len() > 32 && !l.starts_with("-----"))
                .map(str::to_string),
        );
    }
    if secrets.len() < 3 {
        return Err("too few secrets were found to look for".into());
    }
    if let Some(s) = secrets.iter().find(|s| logs.contains(s.as_str())) {
        return Err(format!(
            "a log holds a secret ({}...)",
            &s[..s.len().min(10)]
        ));
    }
    let lines = logs.lines().filter(|l| l.contains("audit: user ")).count();
    lab.c.guest.kill();
    Ok(format!(
        "{} kinds of this phase's action each audited with user {ADMIN}; {lines} audit lines \
         in {} bytes of logs, none holding the password, a password hash or a private key ({} \
         looked for)",
        wants.len(),
        logs.len(),
        secrets.len()
    ))
}
