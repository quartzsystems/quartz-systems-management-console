// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! Suite 13: a real QS/VS host managed through the pages alone. The
//! host (`QSMC_QSVS_DISK`) has a blank further disk; it is paired from
//! the Add page, a storage pool made on that disk and made the media
//! repository, a virtual disk made in it, QS/400's installation ISO
//! (`QSMC_QS400_ISO`) uploaded through Virtual Optical, and a partition
//! made by the wizard with the disk and an optical device holding the
//! image, then activated from Activate with IPL source D. Its codes
//! come into the history and the terminal window shows QS/400's
//! installer's first screen, or, from an ISO that installs by itself,
//! the installed system's Sign On, where QSECOFR is signed on.

use std::time::{Duration, Instant};

use super::host::{
    DISK, MAC, MAIN_MENU, PAIRING, SIGN_ON, USER, USER_PASSWORD, codes, env, password_of, shown,
};
use super::{page, until};
use crate::qemu::session::{Disk, Machine, Size};
use crate::qemu::term::key;
use crate::suite::install::{option, titled};
use crate::suite::parts::{expect, field, post_form, post_on};
use crate::suite::web::{ADMIN, PASSWORD, follow, message_of, shows, signed_on, start, submit};
use crate::web::form::OFF;
use crate::web::{Browser, free_port};

/// QS/400's installation ISO, built at the commit QS/VS pins.
pub const ISO: &str = "QSMC_QS400_ISO";

const PART: &str = "MCINST";
const PROFILE: &str = "INSTALL";
const POOL: &str = "MCPOOL";
const VDISK: &str = "MCLS01";
/// The image's name in the media library: a store name has no dot.
const IMAGE: &str = "QS400";
/// The blank further disk.
const BLANK: u64 = 16 << 30;
/// What QS/400's installer shows first, as QS/VS's own suite takes it.
const FIRST: [&str; 5] = [
    "Install Licensed Internal Code - Status",
    "Install Licensed Internal Code",
    "IPL or Install the System",
    "Select a Language",
    "Sign On",
];
/// How long the IPL from D may take to its first screen.
const IPL: Duration = Duration::from_mins(40);
/// QSECOFR's password after this ISO's install, as QS/VS's suite has it,
/// then the one it ships with.
const SECOFR: [&str; 2] = ["Quartz1", "QSECOFR"];

fn step(n: usize) -> String {
    format!("/qsmc/partitions/create/step/{n}")
}

/// Next on wizard step `n` with `set`: it must lead to step `n + 1`.
fn next(b: &mut Browser, n: usize, set: &[(&str, &str)]) -> Result<(), String> {
    let r = post_form(b, &step(n), ("next", "Next >"), set)?;
    let to = r.header("location").unwrap_or("");
    if r.status == 303 && to.ends_with(&step(n + 1)) {
        Ok(())
    } else {
        Err(format!(
            "step {n}'s Next: {} {}",
            r.status,
            message_of(&r.text())
        ))
    }
}

/// The first free disk the pool form offers: its name.
fn free_disk(html: &str) -> Option<String> {
    html.split("name=\"d_")
        .nth(1)
        .and_then(|s| s.split('"').next())
        .map(str::to_string)
}

/// The text of `html`, its tags gone.
fn text_of(html: &str) -> String {
    qsweb::golden::tokens(html)
        .into_iter()
        .filter_map(|t| match t {
            qsweb::golden::Tok::Text(x) => Some(x.trim().to_string()),
            _ => None,
        })
        .filter(|x| !x.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

/// Suite 13.
#[allow(clippy::too_many_lines, clippy::many_single_char_names)] // a host and a partition, a page at a time
pub fn host_pages(run: &crate::suite::base::Run<'_>) -> Result<String, String> {
    let how = "build both at the pinned commits (`build doctor` says how), install the QS/VS disk \
               with its suites, then name them";
    let disk = env(DISK, how)?;
    let iso = env(ISO, how)?;
    // The library takes the file's name: one it accepts.
    // A directory of its own: the run's files carry the suite's name.
    let dir = run.file("upload");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let upload = dir.join(IMAGE);
    let _ = std::fs::remove_file(&upload);
    if std::fs::hard_link(&iso, &upload).is_err() {
        std::fs::copy(&iso, &upload).map_err(|e| format!("{}: {e}", iso.display()))?;
    }
    let port = free_port()?;
    let scratch = run.file("qsvs.img");
    driver::disk::sparse_copy(&disk, &scratch)?;
    let blank = run.file("pool.img");
    std::fs::File::create(&blank)
        .and_then(|f| f.set_len(BLANK))
        .map_err(|e| format!("the blank disk: {e}"))?;
    let vs = Machine {
        disks: vec![
            Disk {
                path: scratch,
                serial: "QSVSHD0".into(),
                snapshot: false,
            },
            Disk {
                path: blank,
                serial: "QSVSPOOL0".into(),
                snapshot: false,
            },
        ],
        vars: run.vars("qsvs")?,
        nic: Some(MAC.into()),
        forwards: vec![(port, 2300)],
        keys: true,
        hook: None,
        size: Some(Size {
            memory_mb: 5120,
            cpus: 6,
            prealloc: true,
        }),
        ..run.machine()?
    };
    let mut host = run.boot(&vs)?;
    let deadline = Instant::now() + Duration::from_mins(10);
    // 1. Paired from the Add page with the password Service Tools shows.
    host.expect("QSI server started", deadline)?;
    host.expect("Press Enter to sign on", deadline)?;
    host.type_keys(key::ENTER)?;
    titled(&mut host, SIGN_ON, deadline)?;
    host.type_keys(&format!("{USER}{}{USER_PASSWORD}{}", key::TAB, key::ENTER))?;
    titled(&mut host, MAIN_MENU, deadline)?;
    option(&mut host, 4, PAIRING, deadline)?;
    let screen = option(&mut host, 2, "Start Pairing", deadline)?;
    let pw = password_of(&screen).ok_or_else(|| format!("no password on:\n{screen}"))?;
    let mut c = start(run, "mgmt-host-pages")?;
    let b = &mut c.browser;
    signed_on(b, ADMIN, PASSWORD)?;
    let r = submit(
        b,
        "/qsmc/servers/add",
        &[
            ("host", super::HOST),
            ("port", &port.to_string()),
            ("pw", &pw),
            ("connect", "Connect"),
        ],
    )?;
    let t = r.text();
    if !shows(&t, "QMC1001") || !t.contains("QS/VS host") {
        return Err(format!("the host was not added: {}", message_of(&t)));
    }
    let name = t
        .split("<th>Name</th><td><span class=\"mono\">")
        .nth(1)
        .and_then(|s| s.split('<').next())
        .unwrap_or("")
        .to_string();
    let r = submit(b, "/qsmc/servers/add", &[("ms", &name), ("add", "Add")])?;
    follow(b, &r)?;
    let v = |p: &str| format!("/qsmc/server/{name}/virtual/{p}");
    until("the host's storage tab", Duration::from_mins(2), || {
        Ok(b.get(&v("storage"))?.status == 200)
    })?;
    // 2. A storage pool on the blank disk, the media repository, and a
    // virtual disk in it.
    let form = page(b, &v("storage/pool"))?;
    let d = free_disk(&form)
        .ok_or_else(|| format!("the pool form offers no free disk: {}", message_of(&form)))?;
    let use_d = format!("d_{d}");
    let r = post_on(
        b,
        &form,
        &v("storage/pool"),
        ("ok", "Create"),
        &[("to", POOL), ("pr", "None"), (&use_d, "on")],
    )?;
    let made_pool = message_of(&expect(b, &r, "QMC2420")?);
    until(
        "the pool on Virtual Storage",
        Duration::from_mins(2),
        || Ok(page(b, &v("storage"))?.contains(POOL)),
    )?;
    let mut designated = String::new();
    until("the pool designated", Duration::from_mins(3), || {
        let r = post_form(
            b,
            &v(&format!("storage/pool?n={POOL}")),
            ("ok", "OK"),
            &[("repo", "on")],
        )?;
        let t = follow(b, &r)?.text();
        designated = message_of(&t);
        if !shows(&t, "QMC2421") {
            std::thread::sleep(Duration::from_secs(5));
        }
        Ok(shows(&t, "QMC2421"))
    })
    .map_err(|e| format!("{e}: {designated}"))?;
    let r = post_form(
        b,
        &v("storage/create"),
        ("ok", "OK"),
        &[
            ("n", VDISK),
            ("s", "9"),
            ("p", POOL),
            ("t", "thin"),
            ("a", "(none)"),
            ("c", ""),
        ],
    )?;
    expect(b, &r, "QMC2410")?;
    until(
        "the virtual disk on Virtual Storage",
        Duration::from_mins(1),
        || Ok(page(b, &v("storage"))?.contains(VDISK)),
    )?;
    // 3. QS/400's ISO uploaded through Virtual Optical, streamed.
    let csrf = b.csrf(&v("optical"))?;
    let t0 = Instant::now();
    let a = b
        .stream_file(&v("optical"), &csrf, &upload)?
        .ok_or("the upload had no answer")?;
    expect(b, &a, "QMC2401")?;
    let took = t0.elapsed().as_secs();
    let mut library = String::new();
    until(
        "the image in the media library",
        Duration::from_mins(5),
        || {
            library = page(b, &v("optical"))?;
            Ok(library.contains(&format!(">{IMAGE}<")))
        },
    )
    .map_err(|e| {
        let text = library
            .split("<h2>Media library</h2>")
            .nth(1)
            .and_then(|s| s.split("<h2>").next())
            .unwrap_or("");
        let l = super::logs(&mut c.guest, &["mgmtd", "upload", "image"]);
        format!(
            "{e}; the library: {}\n{l}",
            super::super::parts::plain(text)
        )
    })?;
    // 4. The partition by the wizard: the virtual disk, an optical
    // device holding the image; activated with IPL source D.
    let first = page(b, &format!("{}?ms={name}", step(1)))?;
    let id = field(&first, &step(1), "id");
    next(b, 1, &[("id", &id), ("n", PART), ("d", "Made by suite 13")])?;
    next(b, 2, &[("p", PROFILE), ("dft", "on")])?;
    next(
        b,
        3,
        &[("m", "ded"), ("vmin", "1"), ("vdes", "1"), ("vmax", "1")],
    )?;
    next(b, 4, &[("min", "2048"), ("des", "2048"), ("max", "2048")])?;
    next(b, 5, &[])?;
    let s6 = page(b, &step(6))?;
    let add = post_on(
        b,
        &s6,
        &step(6),
        ("add", "Add..."),
        &[("t", "SCSI client (virtual disk)")],
    )?;
    let backing = format!("{POOL}/{VDISK}");
    if add.status != 200 || !add.text().contains(&backing) {
        return Err(format!(
            "step 6's Add does not offer {backing}: {}",
            add.status
        ));
    }
    let r = post_on(
        b,
        &add.text(),
        &step(6),
        ("keep", "OK"),
        &[("disk", &backing), ("req", "on")],
    )?;
    if r.status != 303 {
        return Err(format!("the adapter's OK: {}", message_of(&r.text())));
    }
    next(b, 6, &[])?;
    next(b, 7, &[("opt", "on"), ("iso", IMAGE)])?;
    next(
        b,
        8,
        &[
            (
                "src",
                "D (optical/install) - first install from mounted ISO",
            ),
            ("key", "Manual"),
            ("bm", "Service - manual IPL"),
        ],
    )?;
    let r = post_form(b, &step(9), ("finish", "Finish"), &[])?;
    let made = message_of(&expect(b, &r, "QMC3130")?);
    // What the console shows of the drive: a finding when it lacks it.
    let mut findings: Vec<String> = Vec::new();
    let mut drives = String::new();
    let shows_drive = until(
        "the partition's drive on Virtual Optical",
        Duration::from_mins(2),
        || {
            let o = page(b, &v("optical"))?;
            drives = o
                .split("<h2>Virtual optical drives</h2>")
                .nth(1)
                .and_then(|s| s.split("<form").next())
                .unwrap_or("")
                .to_string();
            Ok(drives.contains(&format!(">{PART}<")) && drives.contains(&format!(">{IMAGE}<")))
        },
    );
    if let Err(e) = shows_drive {
        findings.push(format!(
            "{e}: Virtual optical drives shows `{}`",
            text_of(&drives)
        ));
    }
    let activate = format!("/qsmc/partition/{name}/{id}/activate");
    until(
        "the partition at the console",
        Duration::from_mins(1),
        || Ok(b.get(&activate)?.status == 200),
    )?;
    let r = post_form(
        b,
        &activate,
        ("ok", "OK"),
        &[
            ("key", "Manual"),
            ("src", "D"),
            ("type", "Attended"),
            ("tw", OFF),
        ],
    )?;
    let to = r
        .header("location")
        .ok_or_else(|| {
            format!(
                "Activate did not lead to its confirmation: {} {}",
                r.status,
                message_of(&r.text())
            )
        })?
        .to_string();
    let confirm = page(b, &to)?;
    if !confirm.contains(IMAGE) {
        findings.push(format!(
            "the activation's confirmation does not list {IMAGE}: `{}`",
            text_of(&confirm)
        ));
    }
    let r = post_on(b, &confirm, &to, ("ok", "OK"), &[])?;
    expect(b, &r, "QMC3101")?;
    // 5. Its codes in the history; the installer's first screen in the
    // terminal window.
    let history = format!("/qsmc/refcodes?ms={name}&p={PART}");
    let window = format!("/qsmc/terminal/{name}/{id}");
    let end = Instant::now() + IPL;
    let mut seen: Vec<String> = Vec::new();
    let mut last = String::new();
    let shown_first = loop {
        for code in codes(&page(b, &history)?) {
            if !seen.contains(&code) {
                seen.push(code);
            }
        }
        if let Ok(s) = shown(b, &window) {
            last = s;
        }
        if let Some(w) = FIRST.iter().find(|w| last.contains(*w)) {
            break *w;
        }
        if Instant::now() > end {
            let l = super::logs(&mut c.guest, &["mgmtd"]);
            return Err(format!(
                "no installer screen in the window; codes {seen:?}; the window:\n{last}\n{l}"
            ));
        }
        std::thread::sleep(Duration::from_secs(5));
    };
    for code in codes(&page(b, &history)?) {
        if !seen.contains(&code) {
            seen.push(code);
        }
    }
    // An ISO that installs by itself comes to the installed system's
    // Sign On: QSECOFR signed on from the window.
    let signed = if shown_first == "Sign On" {
        let after = super::host::signed_on_with(b, &window, &SECOFR)
            .map_err(|e| format!("{e}:\n{}", shown(b, &window).unwrap_or_default()))?;
        if !after.contains("===>") {
            return Err("no command line after the sign-on".into());
        }
        "; QSECOFR signed on from the window to a command line"
    } else {
        ""
    };
    c.guest.kill();
    host.kill();
    if seen.is_empty() {
        return Err("no code of the IPL in the history".into());
    }
    Ok(format!(
        "QS/VS host {name} paired from the Add page; pool {POOL} made on its blank disk {d} \
         ({made_pool}) and made the media repository ({designated}); virtual disk {VDISK} made in \
         it; QS/400's ISO streamed through Virtual Optical in {took} s and in the library as \
         {IMAGE}; partition {PART} (ID {id}) made by the wizard with {backing} and OPT01 holding \
         {IMAGE} ({made}), activated from Activate with IPL source D; {} codes in its history \
         ({}); the terminal window showed `{shown_first}`{signed}; findings: {}",
        seen.len(),
        seen.join(" "),
        if findings.is_empty() {
            "none".to_string()
        } else {
            findings.join("; ")
        }
    ))
}
