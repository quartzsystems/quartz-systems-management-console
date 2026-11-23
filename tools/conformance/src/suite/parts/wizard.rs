// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! Suite 4: the create wizard, its checks, Back, Cancel and Finish.
//! Suite 8: a custom group is the Partitions list of its members, and
//! a partition deleted at the host leaves it.

use std::time::Duration;

use qsi::host::partition::{Delete, Request};

use super::{STANDALONE, VS, expect, field, host, kept, lab, line_of, post_on};
use crate::suite::mgmt::{page, until};
use crate::suite::web::message_of;
use crate::web::Browser;

/// A step's page.
fn step(n: usize) -> String {
    format!("/qsmc/partitions/create/step/{n}")
}

/// Post step `n`'s form with `set` and `button`: the answer's status
/// and text.
fn post_step(
    b: &mut Browser,
    n: usize,
    button: (&str, &str),
    set: &[(&str, &str)],
) -> Result<(u16, String, Option<String>), String> {
    let r = post_answer(b, n, button, set)?;
    Ok((r.status, r.text(), r.header("location").map(str::to_string)))
}

/// Post step `n`'s form with `set` and `button`: the answer.
fn post_answer(
    b: &mut Browser,
    n: usize,
    button: (&str, &str),
    set: &[(&str, &str)],
) -> Result<crate::web::Answer, String> {
    let html = page(b, &step(n))?;
    post_on(b, &html, &step(n), button, set)
}

/// A step's check: posting `set` is refused on the step's own page with
/// `QMC8011` and `words`.
fn refused(b: &mut Browser, n: usize, set: &[(&str, &str)], words: &str) -> Result<String, String> {
    let (status, t, _) = post_step(b, n, ("next", "Next >"), set)?;
    let m = super::plain(&message_of(&t));
    if status == 200 && m.contains("QMC8011") && m.contains(words) {
        Ok(m)
    } else {
        Err(format!("step {n} with {set:?}: {status} {m}"))
    }
}

/// Next on step `n`, which must lead to step `n + 1`.
fn next(b: &mut Browser, n: usize, set: &[(&str, &str)]) -> Result<(), String> {
    let (status, t, to) = post_step(b, n, ("next", "Next >"), set)?;
    if status == 303 && to.is_some_and(|t| t.ends_with(&step(n + 1))) {
        Ok(())
    } else {
        Err(format!("step {n}'s Next: {status} {}", message_of(&t)))
    }
}

/// Suite 4.
#[allow(clippy::too_many_lines, reason = "one wizard, step by step")]
pub fn wizard(run: &crate::suite::base::Run<'_>) -> Result<String, String> {
    let mut lab = lab(run, "parts-wizard")?;
    let (fleet, b) = (&lab.fleet, &mut lab.c.browser);
    let parts = |f: &crate::suite::mgmt::Fleet| host(f).parts.len();
    let first = page(b, &format!("{}?ms={VS}", step(1)))?;
    let free = field(&first, &step(1), "id");
    // Step 1: an ID in use, one not a number, a name another has.
    let (status, t, _) = post_step(b, 1, ("next", "Next >"), &[("id", "2"), ("n", "QSNEW")])?;
    if status != 200 || !message_of(&t).contains("QMC3131") || !t.contains("QSTEST01") {
        return Err(format!("an ID in use: {}", message_of(&t)));
    }
    let mut said = vec![format!("ID 2: {}", message_of(&t))];
    said.push(refused(b, 1, &[("id", "x"), ("n", "QSNEW")], "1 to 999")?);
    said.push(refused(
        b,
        1,
        &[("id", "7"), ("n", "qsdev01")],
        "has that name",
    )?);
    // Cancel at step 3: nothing at the host, the entries gone.
    let before = (parts(fleet), host(fleet).lines.len());
    next(b, 1, &[("id", "8"), ("n", "QSCANCEL")])?;
    next(b, 2, &[("p", "gone")])?;
    let (status, _, to) = post_step(b, 3, ("cancel", "Cancel"), &[])?;
    let made = host(fleet).lines[before.1..].iter().any(|l| {
        (l.contains("request 0012/") || l.contains("request 0013/"))
            && l.ends_with("from user admin")
    });
    if status != 303
        || !to.is_some_and(|t| t.ends_with("/partitions"))
        || made
        || parts(fleet) != before.0
    {
        return Err("Cancel made something at the host or did not leave the wizard".into());
    }
    let again = page(b, &step(1))?;
    if field(&again, &step(1), "n") == "QSCANCEL" {
        return Err("Cancel kept the wizard's entries".into());
    }
    // The whole wizard, each step's check on the way.
    next(
        b,
        1,
        &[("id", "7"), ("n", "QSNEW07"), ("d", "Made by the harness")],
    )?;
    said.push(refused(b, 2, &[("p", "")], "profile's name")?);
    next(b, 2, &[("p", "wprof")])?;
    said.push(refused(b, 3, &[("vmin", "3"), ("vdes", "1")], "in order")?);
    said.push(refused(
        b,
        3,
        &[("vmin", "1"), ("vdes", "1"), ("min", "x")],
        "hundredths",
    )?);
    next(
        b,
        3,
        &[
            ("min", "0.25"),
            ("des", "0.75"),
            ("max", "2.00"),
            ("vmin", "1"),
            ("vdes", "1"),
            ("vmax", "2"),
        ],
    )?;
    said.push(refused(b, 4, &[("min", "x")], "whole megabytes")?);
    said.push(refused(
        b,
        4,
        &[("min", "9000"), ("des", "8192")],
        "in order",
    )?);
    // Back from step 4 keeps both steps' entries.
    let (status, _, to) = post_step(
        b,
        4,
        ("back", "< Back"),
        &[("min", "4096"), ("des", "12288"), ("max", "16384")],
    )?;
    if status != 303 || !to.is_some_and(|t| t.ends_with(&step(3))) {
        return Err("Back from step 4 did not lead to step 3".into());
    }
    let s3 = page(b, &step(3))?;
    let s4 = page(b, &step(4))?;
    if field(&s3, &step(3), "des") != "0.75" || field(&s4, &step(4), "des") != "12288" {
        return Err(format!(
            "Back lost entries: step 3 desired `{}`, step 4 desired `{}`",
            field(&s3, &step(3), "des"),
            field(&s4, &step(4), "des")
        ));
    }
    next(b, 4, &[])?;
    next(b, 5, &[])?;
    // Step 6: a SCSI client adapter with VD06, the load source.
    let form = post_answer(
        b,
        6,
        ("add", "Add..."),
        &[("t", "SCSI client (virtual disk)")],
    )?;
    if form.status != 200 || !form.text().contains("name=\"disk\"") {
        return Err(format!("step 6's Add: {}", form.status));
    }
    let r = post_on(
        b,
        &form.text(),
        &step(6),
        ("keep", "OK"),
        &[("disk", "POOL1/VD06"), ("req", "on")],
    )?;
    if r.status != 303 {
        return Err(format!("the adapter's OK: {}", message_of(&r.text())));
    }
    for n in 6..=8 {
        next(b, n, &[])?;
    }
    let summary = page(b, &step(9))?;
    for want in [
        "7 / QSNEW07",
        "wprof (default)",
        "units 0.25 / 0.75 / 2.00",
        "4,096 / 12,288 / 16,384",
        "SCSI client -> VD06",
    ] {
        if !super::plain(&summary).contains(want) {
            return Err(format!("the summary lacks `{want}`"));
        }
    }
    let done = post_answer(b, 9, ("finish", "Finish"), &[])?;
    let finished = expect(b, &done, "QMC3130")?;
    let profs = kept(fleet, "profiles 7");
    let said_finish = message_of(&finished);
    let (desired, mib, name) = {
        let sys = host(fleet);
        let made = sys.part_ref(7).ok_or("no partition 7 at the host")?;
        let prof = made.kept("wprof").ok_or_else(|| {
            format!("partition 7 has no profile wprof (`{profs}`): {said_finish}")
        })?;
        (
            prof.profile.processors.desired_units,
            prof.profile.memory.desired_mib,
            made.definition.name.clone(),
        )
    };
    if !line_of(&profs, "wprof").is_some_and(|l| l.ends_with(" default"))
        || desired != Some(75)
        || mib != 12_288
        || name != "QSNEW07"
    {
        return Err(format!(
            "partition 7 at the host: {name}, {desired:?} units, {mib} MB, profiles `{profs}`"
        ));
    }
    lab.c.guest.kill();
    Ok(format!(
        "step 1 offered ID {free} and refused ID 2 (QMC3131 naming QSTEST01); each step's check: \
         {}; Cancel at step 3 sent nothing and cleared the entries; Back kept steps 3 and 4; the \
         summary was the profile, and Finish made exactly it: QSNEW07 (ID 7) with `{}`, 0.75 \
         units and 12,288 MB",
        said.join("; "),
        profs.trim()
    ))
}

/// The editor's box of the member named `name`: `m<n>`.
fn member(editor: &str, name: &str) -> Option<String> {
    editor.split("<tr").find_map(|row| {
        if !row.contains(&format!(">{name}<")) {
            return None;
        }
        let at = row.find("name=\"m")? + 6;
        let end = row[at..].find('"')? + at;
        Some(row[at..end].to_string())
    })
}

/// Whether the list `page` has a row for `name`.
fn listed(page: &str, name: &str) -> bool {
    page.contains(&format!(">{name}</a>"))
}

/// Suite 8.
pub fn groups(run: &crate::suite::base::Run<'_>) -> Result<String, String> {
    let mut lab = lab(run, "parts-groups")?;
    let (fleet, b) = (&lab.fleet, &mut lab.c.browser);
    let editor = page(b, "/qsmc/groups/create")?;
    let mut set = vec![
        ("g".to_string(), "LAB".to_string()),
        ("d".to_string(), "The harness's".to_string()),
    ];
    for n in ["QSDEV01", "QSTEST01", STANDALONE] {
        let m = member(&editor, n).ok_or_else(|| format!("the editor has no box for {n}"))?;
        set.push((m, "on".into()));
    }
    let pairs: Vec<(&str, &str)> = set.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
    let r = post_on(b, &editor, "/qsmc/groups/create", ("ok", "OK"), &pairs)?;
    expect(b, &r, "QMC3301")?;
    let group = page(b, "/qsmc/group/LAB")?;
    let all = page(b, "/qsmc/partitions")?;
    for n in ["QSDEV01", "QSTEST01", STANDALONE] {
        if !listed(&group, n) {
            return Err(format!("the group's page lacks {n}"));
        }
    }
    if listed(&group, "QSPROD01") || !group.contains("Proc. units") || !all.contains("Proc. units")
    {
        return Err("the group's page is not the Partitions list of its members".into());
    }
    // QSTEST01 deleted at the host.
    {
        let mut h = host(fleet);
        sim::system::partition::answer(
            &mut h,
            &Request::Delete(Delete {
                id: 2,
                disks: false,
            }),
            "host",
        )
        .map_err(|e| format!("the host did not delete QSTEST01: {e:?}"))?;
    }
    until(
        "QSTEST01 gone from the lists",
        Duration::from_secs(60),
        || Ok(!listed(&page(b, "/qsmc/partitions")?, "QSTEST01")),
    )?;
    let group = page(b, "/qsmc/group/LAB")?;
    if listed(&group, "QSTEST01") || !listed(&group, "QSDEV01") {
        return Err("QSTEST01 is still in the group, or QSDEV01 is not".into());
    }
    let editor = page(b, "/qsmc/groups/create?g=LAB")?;
    if editor.contains(">QSTEST01<") {
        return Err("the group's editor still offers QSTEST01".into());
    }
    lab.c.guest.kill();
    Ok(
        "group LAB of QSDEV01, QSTEST01 and the standalone QSHQ400: its page was the Partitions \
         list of those three and not QSPROD01; QSTEST01 deleted at the host left the Partitions \
         list, the group's page and its editor, QSDEV01 and QSHQ400 staying"
            .into(),
    )
}

/// Make partition `id` named `name` on `host` through the wizard, every
/// step on its defaults but the first two and the load source, `VD06`
/// (Phase 8's soak).
pub(crate) fn create(b: &mut Browser, host: &str, id: u32, name: &str) -> Result<(), String> {
    page(b, &format!("{}?ms={host}", step(1)))?;
    next(b, 1, &[("id", &id.to_string()), ("n", name)])?;
    next(b, 2, &[("p", "default")])?;
    for n in 3..=5 {
        next(b, n, &[])?;
    }
    let form = post_answer(
        b,
        6,
        ("add", "Add..."),
        &[("t", "SCSI client (virtual disk)")],
    )?;
    let r = post_on(
        b,
        &form.text(),
        &step(6),
        ("keep", "OK"),
        &[("disk", "POOL1/VD06"), ("req", "on")],
    )?;
    if r.status != 303 {
        return Err(format!(
            "the load source's adapter: {}",
            message_of(&r.text())
        ));
    }
    for n in 6..=8 {
        next(b, n, &[])?;
    }
    let done = post_answer(b, 9, ("finish", "Finish"), &[])?;
    expect(b, &done, "QMC3130").map(|_| ())
}

/// Delete partition `id` of `host` through its confirmation.
pub(crate) fn delete(b: &mut Browser, host: &str, id: u32) -> Result<(), String> {
    let path = format!("/qsmc/confirm/partition-delete?ms={host}&p={id}");
    let html = page(b, &path)?;
    let p = id.to_string();
    let r = post_on(b, &html, &path, ("ok", "OK"), &[("ms", host), ("p", &p)])?;
    expect(b, &r, "QMC3133").map(|_| ())
}
