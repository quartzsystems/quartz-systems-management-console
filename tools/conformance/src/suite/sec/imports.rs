// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! Imported files as hostile: every file of the fuzzers' corpora for
//! the three files a page takes (`tools/fuzz/corpus`: `backup-file`,
//! `cert-import`, `system-plan`) sent through its page as a browser
//! sends it. Each is answered with a page and a message, never an error
//! of the inside or a dropped connection; no service restarts; a plan
//! refused is not kept.

use std::collections::BTreeMap;
use std::path::PathBuf;

use crate::suite::base::{Run, channel};
use crate::suite::web::{ADMIN, Console, PASSWORD, follow, served, shows, signed_on, start};
use crate::web::Answer;

/// The corpus of fuzz target `target`, its files in name order.
fn corpus(target: &str) -> Result<Vec<(String, Vec<u8>)>, String> {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../fuzz/corpus")
        .join(target);
    let mut files: Vec<(String, Vec<u8>)> = std::fs::read_dir(&dir)
        .map_err(|e| format!("{}: {e}", dir.display()))?
        .flatten()
        .filter(|e| e.path().is_file())
        .map(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            std::fs::read(e.path()).map(|b| (name, b))
        })
        .collect::<Result<_, _>>()
        .map_err(|e| e.to_string())?;
    files.sort();
    if files.is_empty() {
        return Err(format!("the {target} corpus is empty"));
    }
    Ok(files)
}

/// Whether a page answered: a page with a message.
fn said(t: &str) -> bool {
    t.contains("class=\"msgid\"")
}

/// The answer to an upload of `name`: a page, or a redirect to one,
/// and never an error of the inside.
fn answered(what: &str, name: &str, a: &Answer) -> Result<(), String> {
    match a.status {
        200 | 303 | 400 | 413 => Ok(()),
        s => Err(format!("{what} {name} answered {s}")),
    }
}

/// The services' restarts so far, by name.
fn failures(c: &mut Console<'_>) -> Result<BTreeMap<String, u32>, String> {
    Ok(channel(&mut c.guest)?
        .status()?
        .services
        .iter()
        .map(|s| (s.name.clone(), s.failures))
        .collect())
}

/// The plans kept: their files.
fn plans(c: &mut Console<'_>) -> Result<usize, String> {
    Ok(channel(&mut c.guest)?
        .ls("/data/plans")
        .unwrap_or_default()
        .iter()
        .filter(|n| {
            std::path::Path::new(n)
                .extension()
                .is_some_and(|e| e == plan::doc::EXTENSION)
        })
        .count())
}

fn backups(c: &mut Console<'_>) -> Result<(usize, usize), String> {
    let (mut opened, mut refused) = (0, 0);
    for (name, bytes) in corpus("backup-file")? {
        let csrf = c.browser.csrf("/qsmc/console/backup?tab=restore")?;
        let a = c.browser.upload(
            "/qsmc/console/backup?tab=restore",
            &[
                ("csrf", &csrf),
                ("pp", "sec-imports passphrase"),
                ("open", "Open"),
            ],
            &bytes,
        )?;
        answered("the backup", &name, &a)?;
        let t = follow(&mut c.browser, &a)?.text();
        if t.contains("The backup holds") {
            opened += 1;
        } else if said(&t) {
            refused += 1;
        } else {
            return Err(format!("the backup {name} was answered with no message"));
        }
    }
    Ok((opened, refused))
}

fn certificates(c: &mut Console<'_>) -> Result<(usize, usize), String> {
    let (mut taken, mut refused) = (0, 0);
    for (name, bytes) in corpus("cert-import")? {
        let csrf = c.browser.csrf("/qsmc/console/certs")?;
        let a = c.browser.upload(
            "/qsmc/console/certs",
            &[("csrf", &csrf), ("pw", "fuzz")],
            &bytes,
        )?;
        answered("the certificate", &name, &a)?;
        // One taken is served to the next connection.
        let sha = served(&mut c.guest)?;
        c.browser.pin(&sha)?;
        let t = follow(&mut c.browser, &a)?.text();
        if shows(&t, qmcmsg::CERT_IMPORTED.id) {
            taken += 1;
        } else if said(&t) {
            refused += 1;
        } else {
            return Err(format!(
                "the certificate {name} was answered with no message"
            ));
        }
    }
    Ok((taken, refused))
}

fn plan_files(c: &mut Console<'_>) -> Result<(usize, usize), String> {
    let before = plans(c)?;
    let (mut taken, mut refused) = (0, 0);
    for (name, bytes) in corpus("system-plan")? {
        let csrf = c.browser.csrf("/qsmc/plans")?;
        let a = c.browser.upload(
            "/qsmc/plans",
            &[("csrf", &csrf), ("import", "Import")],
            &bytes,
        )?;
        answered("the plan", &name, &a)?;
        let t = follow(&mut c.browser, &a)?.text();
        if shows(&t, qmcmsg::PLAN_IMPORTED.id) {
            taken += 1;
        } else if said(&t) {
            refused += 1;
        } else {
            return Err(format!("the plan {name} was answered with no message"));
        }
    }
    let after = plans(c)?;
    if after != before + taken {
        return Err(format!(
            "{taken} plans imported, but {before} listed before and {after} after"
        ));
    }
    Ok((taken, refused))
}

/// The suite.
pub fn imports(run: &Run<'_>) -> Result<String, String> {
    let mut c = start(run, "sec-imports")?;
    signed_on(&mut c.browser, ADMIN, PASSWORD)?;
    let idle = failures(&mut c)?;
    let (opened, b_refused) = backups(&mut c)?;
    let (c_taken, c_refused) = certificates(&mut c)?;
    let (p_taken, p_refused) = plan_files(&mut c)?;
    let now = failures(&mut c)?;
    if let Some((name, n)) = now.iter().find(|(k, n)| idle.get(*k) != Some(*n)) {
        return Err(format!(
            "{name} failed {n} times, {} before the files",
            idle.get(name).copied().unwrap_or(0)
        ));
    }
    let _ = crate::suite::base::power_off(&mut c.guest);
    Ok(format!(
        "the fuzzers' corpora through their pages: {} backups ({opened} opened, {b_refused} \
         refused), {} certificates ({c_taken} imported, {c_refused} refused), {} system plans \
         ({p_taken} imported, {p_refused} refused), each answered with a page and its message; \
         no service restarted; the plans kept are the ones imported",
        opened + b_refused,
        c_taken + c_refused,
        p_taken + p_refused
    ))
}
