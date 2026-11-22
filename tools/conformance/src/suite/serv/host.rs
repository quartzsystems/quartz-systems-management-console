// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! Suite 7: a host's update against the simulator: refused while its
//! partitions run without leave, one rolled back with the host's
//! reason, then one confirmed. Suite 13: a scheduled install runs at
//! its time with the page's checks, for a host and for the console.

use std::time::Duration;

use appliance::time::civil;

use super::{
    CONSOLE_ROW, Lab, QSVS_IMAGE, QSVS_NEXT, VS, download, lab, qsmc_release, row, set_clock,
};
use crate::suite::mgmt::{page, until};
use crate::suite::parts::sched::{date, offset, rows, runs_of};
use crate::suite::parts::{expect, field, post_on};
use crate::web::Browser;

/// The partitions the install confirmation says it shuts down.
fn to_shut(html: &str) -> Vec<String> {
    html.split("<tr><th>Shut down</th><td>")
        .skip(1)
        .filter_map(|r| r.split(" (ID").next())
        .map(|n| {
            n.replace("<span class=\"mono\">", "")
                .replace("</span>", "")
        })
        .collect()
}

/// Install the host's update through its confirmation, its partitions
/// given `timeout` seconds; the progress page once it says `done`.
fn install(b: &mut Browser, timeout: &str, done: &str) -> Result<String, String> {
    let path = format!("/qsmc/confirm/update-install?t={VS}");
    let html = page(b, &path)?;
    let set: &[(&str, &str)] = if html.contains("name=\"timeout\"") {
        &[("timeout", timeout)]
    } else {
        &[]
    };
    let r = post_on(b, &html, &path, ("ok", "OK"), set)?;
    expect(b, &r, qmcmsg::INSTALL_BEGUN.id)?;
    let progress = format!("/qsmc/updates/code/{VS}/progress");
    let mut last = String::new();
    until(
        &format!("the progress page says `{done}`"),
        Duration::from_secs(420),
        || {
            last = page(b, &progress)?;
            Ok(last.contains(done))
        },
    )?;
    Ok(last)
}

/// What the simulated host runs.
fn host_runs(lab: &Lab<'_>) -> String {
    sim::lock(&lab.fleet.systems[0]).hello.release.clone()
}

/// Suite 7.
#[allow(
    clippy::too_many_lines,
    reason = "the refusal, the roll back, the confirm"
)]
pub fn host_update(run: &crate::suite::base::Run<'_>) -> Result<String, String> {
    let mut lab = lab(run, "serv-host", &[])?;
    let b = &mut lab.c.browser;
    download(b, VS, "qsvs", QSVS_IMAGE)?;
    let path = format!("/qsmc/confirm/update-install?t={VS}");
    let html = page(b, &path)?;
    let listed = to_shut(&html);
    if listed.is_empty() {
        return Err("the confirmation lists no partition to shut down".into());
    }
    // Without leave to shut them down: refused, nothing sent.
    let token = b.csrf(&path)?;
    let r = b.post(&path, &[("csrf", &token), ("t", VS), ("ok", "OK")])?;
    expect(b, &r, qmcmsg::PARTITIONS_RUN.id)?;
    std::thread::sleep(Duration::from_secs(3));
    if sim::lock(&lab.fleet.systems[0]).servicing.staged.is_some() {
        return Err("an install refused still sent the host its image".into());
    }
    // Rolled back, as the host is told to.
    let reason = "Partition QSPROD01 did not start on the new level";
    lab.fleet
        .tell(0, &format!("outcome rollback A2001035 {reason}"));
    let t = install(&mut lab.c.browser, "30", &format!("rolled back: {reason}"))?;
    if host_runs(&lab) != "V1R0M0" {
        return Err(format!(
            "after the roll back the host runs {}",
            host_runs(&lab)
        ));
    }
    if !t.contains("Verify signature") || !t.contains("Save system plan") {
        return Err("the progress page lacks its steps".into());
    }
    // Then confirmed.
    lab.fleet.tell(0, "outcome confirm");
    install(
        &mut lab.c.browser,
        "30",
        &format!("{QSVS_NEXT} build 26300 confirmed"),
    )?;
    let b = &mut lab.c.browser;
    until("the new level shown", Duration::from_secs(120), || {
        let code = page(b, "/qsmc/updates/code")?;
        let fleet = page(b, "/qsmc/updates")?;
        Ok(row(&code, VS).contains(QSVS_NEXT)
            && row(&code, VS).contains("Current")
            && row(&fleet, VS).contains(QSVS_NEXT))
    })?;
    if host_runs(&lab) != QSVS_NEXT {
        return Err(format!("the host runs {}", host_runs(&lab)));
    }
    let log = page(&mut lab.c.browser, &format!("/qsmc/updates/code/{VS}/log"))?;
    lab.c.guest.kill();
    Ok(format!(
        "the confirmation listed {} to shut down; without leave refused (QMC7004) and nothing \
         sent; with it, the host told to roll back reported `rolled back: {reason}` and still ran \
         V1R0M0; then confirmed, {QSVS_NEXT} build 26300 shown on Code updates and Updates; the \
         log has {} lines",
        listed.join(", "),
        log.matches("<tr").count()
    ))
}

/// The console's local time, seconds, and its display offset, minutes.
fn local_now(lab: &mut Lab<'_>) -> Result<(i64, i64), String> {
    let form = page(&mut lab.c.browser, "/qsmc/schedule/add")?;
    Ok((super::clock(&mut lab.c.guest)?, offset(&form)))
}

/// Add a once operation of Install code update for `target`, due three
/// minutes on by the console's clock, its partitions given `delay`
/// seconds: its number and when it is due.
fn schedule(lab: &mut Lab<'_>, target: &str, delay: &str) -> Result<(u32, i64), String> {
    let (now, off) = local_now(lab)?;
    let local = now + off * 60;
    let due_local = (local / 60 + 3) * 60;
    let c = civil(due_local);
    let b = &mut lab.c.browser;
    let form = page(b, "/qsmc/schedule/add?op=update")?;
    let sample = field(&form, "/qsmc/schedule/add", "d");
    let d = date((c.year, i64::from(c.month), i64::from(c.day)), &sample);
    let t = format!("{:02}:{:02}", c.hour, c.minute);
    let r = post_on(
        b,
        &form,
        "/qsmc/schedule/add",
        ("ok", "OK"),
        &[
            ("d", &d),
            ("t", &t),
            ("r", "once"),
            ("op", "Install code update"),
            ("tg", target),
            ("delay", delay),
        ],
    )?;
    expect(b, &r, "QMC3401")?;
    let id = rows(&page(b, "/qsmc/schedule")?)
        .into_iter()
        .map(|(i, _)| i)
        .max()
        .ok_or("no operation listed")?;
    Ok((id, due_local - off * 60))
}

/// Move the clock to just before `at` and wait for `id`'s run: its
/// status.
fn run_at(lab: &mut Lab<'_>, id: u32, at: i64, within: Duration) -> Result<String, String> {
    set_clock(&mut lab.c.guest, at - 10)?;
    let mut last = String::new();
    until(&format!("operation {id}'s run"), within, || {
        let (runs, _) = runs_of(&mut lab.c.browser, id)?;
        if let Some(r) = runs.first() {
            last.clone_from(r);
            return Ok(true);
        }
        Ok(false)
    })?;
    Ok(last)
}

/// Suite 13.
#[allow(
    clippy::too_many_lines,
    reason = "the host's three, then the console's"
)]
pub fn sched(run: &crate::suite::base::Run<'_>) -> Result<String, String> {
    let next = run
        .image
        .next
        .as_ref()
        .ok_or("the next variant is not built (`build --test-image --variant next all`)")?;
    let mut lab = lab(run, "serv-sched", &[qsmc_release(next)?])?;
    let mut said = Vec::new();
    // The host's: not downloaded, partitions running without leave,
    // then one that installs.
    let (id, at) = schedule(&mut lab, VS, "0")?;
    let s = run_at(&mut lab, id, at, Duration::from_secs(120))?;
    if !s.contains(qmcmsg::DOWNLOAD_FIRST.id) {
        return Err(format!("operation {id} with nothing downloaded: {s}"));
    }
    said.push(format!("{id} before the download: {s}"));
    download(&mut lab.c.browser, VS, "qsvs", QSVS_IMAGE)?;
    let (id, at) = schedule(&mut lab, VS, "0")?;
    let s = run_at(&mut lab, id, at, Duration::from_secs(120))?;
    if !s.contains(qmcmsg::PARTITIONS_RUN.id) {
        return Err(format!(
            "operation {id} with partitions running and no delay: {s}"
        ));
    }
    said.push(format!(
        "{id} with partitions running and no time to shut them: {s}"
    ));
    let (id, at) = schedule(&mut lab, VS, "30")?;
    let s = run_at(&mut lab, id, at, Duration::from_secs(420))?;
    if s != "Completed" {
        return Err(format!("operation {id}: {s}"));
    }
    until(
        "the host on its new level",
        Duration::from_secs(300),
        || Ok(host_runs(&lab) == QSVS_NEXT),
    )?;
    said.push(format!(
        "{id} installed {QSVS_NEXT} on {VS} at its time: Completed"
    ));
    // The console's: downloaded, then due; it restarts into V1R0M1.
    let name = next
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    download(&mut lab.c.browser, CONSOLE_ROW, "qsmc", &name)?;
    let (id, at) = schedule(&mut lab, webpages::schedule::CONSOLE_TARGET, "0")?;
    set_clock(&mut lab.c.guest, at - 10)?;
    lab.c
        .guest
        .wait_exit(std::time::Instant::now() + Duration::from_secs(600))?;
    let Lab { fleet, c, streams } = lab;
    let crate::suite::web::Console { guest, machine, .. } = c;
    drop(guest);
    let mut c = crate::suite::web::boot(run, machine)?;
    crate::suite::web::signed_on(
        &mut c.browser,
        crate::suite::web::ADMIN,
        crate::suite::web::PASSWORD,
    )?;
    let mut lab = Lab { fleet, c, streams };
    until(
        "the console confirmed on V1R0M1",
        Duration::from_secs(300),
        || {
            let code = page(&mut lab.c.browser, "/qsmc/updates/code")?;
            Ok(row(&code, CONSOLE_ROW).contains("V1R0M1 build"))
        },
    )?;
    let (runs, _) = runs_of(&mut lab.c.browser, id)?;
    if runs.first().map(String::as_str) != Some("Completed") {
        return Err(format!("the console's operation {id}: {runs:?}"));
    }
    said.push(format!(
        "{id} for the console at its time: restarted into V1R0M1, which confirmed; Completed"
    ));
    lab.c.guest.kill();
    Ok(said.join("; "))
}
