// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! Suite 9: a host's install saves its plan first, the step done and
//! the plan linked; the eleventh automatic plan removes the oldest
//! automatic one and no operator's. Suite 11: a backup carries the
//! plans, and its restore brings them back.

use std::time::Duration;

use super::{VS1, create, export, lab, table};
use crate::suite::mgmt::{page, until};
use crate::suite::parts::{expect, post_on};
use crate::suite::web::{ADMIN, PASSWORD, boot, message_of, signed_on, submit};
use crate::web::Browser;

/// What every automatic plan of the update is described as.
const BEFORE: &str = "Before QS/VS update 26300";

/// Install the host's update through its confirmation, its partitions
/// given 30 s; the progress page once it says `done`.
fn install(b: &mut Browser, done: &str) -> Result<String, String> {
    let path = format!("/qsmc/confirm/update-install?t={VS1}");
    let html = page(b, &path)?;
    if !html.contains("Saved first") {
        return Err("the confirmation does not say a plan is saved first".into());
    }
    let set: &[(&str, &str)] = if html.contains("name=\"timeout\"") {
        &[("timeout", "30")]
    } else {
        &[]
    };
    let r = post_on(b, &html, &path, ("ok", "OK"), set)?;
    expect(b, &r, qmcmsg::INSTALL_BEGUN.id)?;
    let progress = format!("/qsmc/updates/code/{VS1}/progress");
    let mut last = String::new();
    until(
        &format!("the progress page says `{done}`"),
        Duration::from_secs(420),
        || {
            last = page(b, &progress)?;
            Ok(last.contains(done))
        },
    )
    .map_err(|e| format!("{e}: {}", message_of(&last)))?;
    Ok(last)
}

/// The automatic plans of the host on System plans, by name.
fn automatic(b: &mut Browser) -> Result<Vec<String>, String> {
    let t = page(b, "/qsmc/plans?sort=n")?;
    Ok(table(&t, "Actions")
        .into_iter()
        .filter(|r| r.get(2).is_some_and(|d| d == BEFORE))
        .filter_map(|r| r.get(1).cloned())
        .collect())
}

/// Suite 9.
pub fn before_update(run: &crate::suite::base::Run<'_>) -> Result<String, String> {
    let mut lab = crate::suite::serv::lab(run, "plans-update", &[])?;
    let b = &mut lab.c.browser;
    crate::suite::serv::download(b, VS1, "qsvs", crate::suite::serv::QSVS_IMAGE)?;
    let mine = create(b, VS1)?;
    let reason = "Partition QSPROD01 did not start on the new level";
    let mut first = String::new();
    for n in 1..=11 {
        lab.fleet
            .tell(0, &format!("outcome rollback A2001035 {reason}"));
        let t = install(&mut lab.c.browser, &format!("rolled back: {reason}"))?;
        let b = &mut lab.c.browser;
        let steps = table(&t, "Step");
        let saved = steps
            .iter()
            .find(|r| r[0].starts_with("Save system plan"))
            .ok_or("the progress page has no Save system plan step")?;
        if saved[1] != "Complete" || !t.contains("/qsmc/plan/qshqvs1-") {
            return Err(format!("install {n}: the plan's step is {saved:?}"));
        }
        let auto = automatic(b)?;
        if n == 1 {
            first = auto
                .first()
                .cloned()
                .ok_or("no automatic plan after the install")?;
        }
        let want = n.min(10);
        if auto.len() != want {
            return Err(format!(
                "after install {n}: {} automatic plans: {auto:?}",
                auto.len()
            ));
        }
    }
    let b = &mut lab.c.browser;
    let auto = automatic(b)?;
    let all = page(b, "/qsmc/plans")?;
    lab.c.guest.kill();
    if auto.contains(&first) {
        return Err(format!(
            "the oldest automatic plan {first} is kept: {auto:?}"
        ));
    }
    if !all.contains(&mine) {
        return Err(format!("the operator's plan {mine} was removed"));
    }
    Ok(format!(
        "eleven installs of {VS1}, each rolled back as told: each saved its plan first, the step \
         Complete with the plan linked; after the eleventh, 10 automatic plans described `{BEFORE}`, \
         the oldest ({first}) removed, the operator's {mine} kept"
    ))
}

const PASS: &str = "suite backup passphrase";

/// Suite 11.
pub fn backup(run: &crate::suite::base::Run<'_>) -> Result<String, String> {
    let mut lab = lab(run, "plans-backup")?;
    let b = &mut lab.c.browser;
    let name = create(b, VS1)?;
    let bytes = export(b, &name)?;
    let r = submit(
        b,
        "/qsmc/console/backup",
        &[
            ("d", "on"),
            ("w", "file"),
            ("pp", PASS),
            ("back_up_now", "x"),
        ],
    )?;
    if r.status != 200 {
        return Err(format!("no backup came: {}", message_of(&r.text())));
    }
    let file = r.body;
    let opened = qmcbackup::seal::open(&file, PASS).map_err(|e| e.to_string())?;
    let held = opened
        .entries
        .iter()
        .any(|e| e.path == format!("plans/{name}.qmcplan"));
    if !held {
        return Err("the backup does not hold the plan".into());
    }
    // The plan deleted, then the backup restored.
    let path = format!("/qsmc/confirm/plan-delete?p={name}");
    let html = page(b, &path)?;
    let r = post_on(b, &html, &path, ("ok", "OK"), &[])?;
    expect(b, &r, qmcmsg::PLAN_DELETED.id)?;
    let token = b.csrf("/qsmc/console/backup?tab=restore")?;
    let t = b
        .upload(
            "/qsmc/console/backup?tab=restore",
            &[("csrf", &token), ("pp", PASS), ("open", "Open")],
            &file,
        )?
        .text();
    if !t.contains("The backup holds") {
        return Err(format!("the backup did not open: {}", message_of(&t)));
    }
    let _ = submit(b, "/qsmc/confirm/restore", &[("ok", "OK")]);
    lab.c
        .guest
        .wait_exit(std::time::Instant::now() + Duration::from_secs(60))
        .map_err(|e| format!("the console did not restart for the restore: {e}"))?;
    let m = lab.c.machine.clone();
    let mut r = boot(run, m)?;
    signed_on(&mut r.browser, ADMIN, PASSWORD)?;
    let listed = page(&mut r.browser, "/qsmc/plans")?;
    let back = export(&mut r.browser, &name)?;
    let _ = crate::suite::base::power_off(&mut r.guest);
    if !listed.contains(&name) || back != bytes {
        return Err("the restore did not bring the plan back as it was".into());
    }
    Ok(format!(
        "the backup held plans/{name}.qmcplan; the plan deleted, the backup restored at the \
         restart: {name} listed again and exported byte for byte ({} bytes)",
        bytes.len()
    ))
}
