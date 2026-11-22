// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! Suite 13: a real QS/VS host managed through the pages alone. The
//! host (`QSMC_QSVS_DISK`) has a blank further disk; it is paired from
//! the Add page, a storage pool made on that disk and made the media
//! repository, a virtual disk made in it, QS/400's installation ISO
//! (`QSMC_QS400_ISO`) uploaded through Virtual Optical, and a partition
//! made by the wizard, shared, with the disk, an Ethernet adapter and an
//! optical device holding the image, shown before it activates, then
//! activated from Activate with IPL source D. Its codes come into the
//! history and the terminal window shows the installed system's Sign On
//! (the ISO installs by itself), where QSECOFR is signed on. Then: its
//! entitlement changed, a shut down through its operating system, its
//! activation from its disk scheduled, its configuration saved, its
//! end, and its delete with its disk.

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
pub(crate) const POOL: &str = "MCPOOL";
const VDISK: &str = "MCLS01";
const SWITCH: &str = "MCSW1";
const SAVED: &str = "SAVED1";
const IPL_COMPLETE: &str = "C6001090";
/// The image's name in the media library: a store name has no dot.
const IMAGE: &str = "QS400";
/// The blank further disk.
pub(crate) const BLANK: u64 = 16 << 30;
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
pub(crate) fn free_disk(html: &str) -> Option<String> {
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
    // A switch for its Ethernet adapter: the host's first, or one made.
    let eth = page(b, &v("ethernet"))?;
    let (sw, sw_how) = if let Some(n) = eth
        .split("virtual/ethernet/switch?n=")
        .nth(1)
        .and_then(|s| s.split(['"', '&']).next())
        .filter(|n| !n.is_empty())
    {
        (n.to_string(), "the host's")
    } else {
        let r = post_form(
            b,
            &v("ethernet/switch"),
            ("ok", "OK"),
            &[("to", SWITCH), ("v", "1"), ("mtu", "1500")],
        )?;
        expect(b, &r, "QMC2425")?;
        until(
            "the switch on Virtual Ethernet",
            Duration::from_mins(1),
            || Ok(page(b, &v("ethernet"))?.contains(SWITCH)),
        )?;
        (SWITCH.to_string(), "made from Virtual Ethernet")
    };
    let first = page(b, &format!("{}?ms={name}", step(1)))?;
    let id = field(&first, &step(1), "id");
    next(b, 1, &[("id", &id), ("n", PART), ("d", "Made by suite 13")])?;
    next(b, 2, &[("p", PROFILE), ("dft", "on")])?;
    next(
        b,
        3,
        &[
            ("m", "shared"),
            ("min", "0.50"),
            ("des", "1.00"),
            ("max", "2.00"),
            ("vmin", "1"),
            ("vdes", "1"),
            ("vmax", "2"),
        ],
    )?;
    next(b, 4, &[("min", "2048"), ("des", "2048"), ("max", "2048")])?;
    next(b, 5, &[])?;
    let s6 = page(b, &step(6))?;
    let add = post_on(b, &s6, &step(6), ("add", "Add..."), &[("t", "Ethernet")])?;
    let r = post_on(
        b,
        &add.text(),
        &step(6),
        ("keep", "OK"),
        &[("sw", &sw), ("vlan", "1")],
    )?;
    if r.status != 303 {
        return Err(format!(
            "the Ethernet adapter's OK: {}",
            message_of(&r.text())
        ));
    }
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
    // The drive and its image on Virtual Optical before any activation.
    let mut drives = String::new();
    until(
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
            Ok(drives.contains(&format!(">{PART}<"))
                && drives.contains(">OPT01<")
                && drives.contains(&format!(">{IMAGE}<")))
        },
    )
    .map_err(|e| format!("{e}: Virtual optical drives shows `{}`", text_of(&drives)))?;
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
    // The table's last column, its one row after the heads.
    let row = confirm
        .split("Virtual optical</th>")
        .nth(1)
        .and_then(|s| s.split("</table>").next())
        .and_then(|s| s.rsplit("<td").next())
        .unwrap_or("");
    if !row.contains("OPT01") || !row.contains(IMAGE) {
        return Err(format!(
            "the confirmation's Virtual optical row is `{}`: {}",
            text_of(row),
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
    if seen.is_empty() {
        return Err("no code of the IPL in the history".into());
    }
    let done = format!(
        "QS/VS host {name} paired from the Add page; pool {POOL} made on its blank disk {d} \
         ({made_pool}) and made the media repository ({designated}); virtual disk {VDISK} made in \
         it; switch {sw} {sw_how}; QS/400's ISO streamed through Virtual Optical in {took} s and \
         in the library as {IMAGE}; partition {PART} (ID {id}) made by the wizard, shared, with \
         {backing}, an Ethernet adapter on {sw} and OPT01 holding {IMAGE} ({made}), shown on \
         Virtual Optical and on Activate's confirmation before its first activation, activated \
         with IPL source D; {} codes in its history ({}); the terminal window showed \
         `{shown_first}`",
        seen.len(),
        seen.join(" ")
    );
    // The rest needs the installed system: an ISO that installs by itself
    // comes to its Sign On.
    if shown_first != "Sign On" {
        c.guest.kill();
        host.kill();
        return Err(format!(
            "{done}; the installer waits for an operator, so the rest of the suite cannot run"
        ));
    }
    let after = super::host::signed_on_with(b, &window, &SECOFR)
        .map_err(|e| format!("{e}:\n{}", shown(b, &window).unwrap_or_default()))?;
    if !after.contains("===>") {
        return Err("no command line after the sign-on".into());
    }
    let p = Part {
        host: name.clone(),
        id: id.clone(),
    };
    let rest = life(b, &p, &window, &history)?;
    c.guest.kill();
    host.kill();
    Ok(format!(
        "{done}; QSECOFR signed on from the window to a command line; {}",
        rest.join("; ")
    ))
}

/// The partition under test, at its host.
struct Part {
    host: String,
    id: String,
}

impl Part {
    fn at(&self, tab: &str) -> String {
        format!("/qsmc/partition/{}/{}/{tab}", self.host, self.id)
    }

    /// Its state as General shows it.
    fn state(&self, b: &mut Browser) -> Result<String, String> {
        let g = page(b, &self.at("general"))?;
        Ok(g.split("<th>State</th>")
            .nth(1)
            .and_then(|s| s.split("</tr>").next())
            .map(text_of)
            .unwrap_or_default())
    }

    /// Wait until its state is `want`.
    fn reaches(&self, b: &mut Browser, want: &str, within: Duration) -> Result<(), String> {
        let mut last = String::new();
        until(&format!("{PART} {want}"), within, || {
            last = self.state(b)?;
            if last != want {
                std::thread::sleep(Duration::from_secs(3));
            }
            Ok(last == want)
        })
        .map_err(|e| format!("{e}: it is `{last}`"))
    }
}

/// A form with a confirmation (shut down): the form's OK, then the
/// confirmation's; message `id` shown.
fn confirmed(
    b: &mut Browser,
    path: &str,
    set: &[(&str, &str)],
    id: &str,
) -> Result<String, String> {
    let r = post_form(b, path, ("ok", "OK"), set)?;
    let to = r
        .header("location")
        .ok_or_else(|| {
            format!(
                "{path} did not lead to its confirmation: {}",
                message_of(&r.text())
            )
        })?
        .to_string();
    let html = page(b, &to)?;
    let r = post_on(b, &html, &to, ("ok", "OK"), &[])?;
    expect(b, &r, id).map(|t| message_of(&t))
}

/// A date as the console writes them, its format taken from `sample`.
fn date_as(y: i64, m: u32, d: u32, sample: &str) -> String {
    if sample.contains('.') {
        format!("{d:02}.{m:02}.{:02}", y % 100)
    } else if sample.contains('-') {
        format!("{y:04}-{m:02}-{d:02}")
    } else {
        format!("{m:02}/{d:02}/{:02}", y % 100)
    }
}

/// The console's display offset in minutes, from the add form's words.
fn offset(add: &str) -> i64 {
    let Some(z) = add
        .split("console time zone, ")
        .nth(1)
        .and_then(|r| r.split(')').next())
    else {
        return 0;
    };
    let rest = z.trim().trim_start_matches("UTC");
    let sign = if rest.starts_with('-') { -1 } else { 1 };
    let hm = rest.trim_start_matches(['+', '-']);
    let (h, m) = hm.split_once(':').unwrap_or((hm, "0"));
    sign * (h.parse::<i64>().unwrap_or(0) * 60 + m.parse::<i64>().unwrap_or(0))
}

/// The installed partition's life from the pages: its entitlement, an
/// operating system's shut down, a scheduled activation from its disk,
/// its configuration saved, its end and its delete with its disk.
#[allow(
    clippy::too_many_lines,
    clippy::many_single_char_names,
    clippy::similar_names
)] // one partition's life, a page at a time
fn life(b: &mut Browser, p: &Part, window: &str, history: &str) -> Result<Vec<String>, String> {
    let mut said = Vec::new();
    // Its entitlement changed now from the Processors tab.
    let r = post_form(
        b,
        &p.at("processors"),
        ("apply", "Apply"),
        &[("des", "0.75")],
    )?;
    let t = follow(b, &r)?.text();
    if !shows(&t, "QMC3122") {
        return Err(format!(
            "the running partition's entitlement: {}",
            message_of(&t)
        ));
    }
    said.push(format!(
        "its entitlement changed to 0.75 units from Processors ({})",
        message_of(&t)
    ));
    // Through its operating system, to Not activated.
    let m = confirmed(
        b,
        &p.at("shutdown"),
        &[("how", "os"), ("delay", "60")],
        "QMC3102",
    )?;
    p.reaches(b, "Not activated", Duration::from_mins(15))?;
    said.push(format!(
        "shut down through its operating system ({m}) to Not activated"
    ));
    // From its disk next time, unattended.
    let r = post_form(
        b,
        &p.at("settings"),
        ("ok", "OK"),
        &[("src", "A"), ("bm", "Normal"), ("key", "Normal")],
    )?;
    let t = follow(b, &r)?.text();
    let s = page(b, &p.at("settings"))?;
    let src = field(&s, &p.at("settings"), "src");
    if src != "A" {
        return Err(format!(
            "the IPL source is `{src}` after Settings: {}",
            message_of(&t)
        ));
    }
    // Its activation scheduled a minute or so ahead, nothing moved.
    let add = "/qsmc/schedule/add";
    let form = page(b, add)?;
    let off = offset(&form);
    let sample = field(&form, add, "d");
    let now = appliance::time::now_secs();
    let due = (now + 90 + 59) / 60 * 60;
    let c = appliance::time::civil(due + off * 60);
    let (d, t) = (
        date_as(c.year, c.month, c.day, &sample),
        format!("{:02}:{:02}", c.hour, c.minute),
    );
    let target = format!("{PART} on {}", p.host);
    let r = post_on(
        b,
        &form,
        add,
        ("ok", "OK"),
        &[
            ("d", &d),
            ("t", &t),
            ("r", "once"),
            ("op", "Activate partition"),
            ("tg", &target),
            ("p", PROFILE),
        ],
    )?;
    expect(b, &r, "QMC3401")?;
    let before = codes(&page(b, history)?);
    let wait = u64::try_from(due - now).unwrap_or(0) + 120;
    let mut list = text_of(&page(b, "/qsmc/schedule")?);
    let had = list.matches("Completed").count();
    until(
        "the scheduled run Completed",
        Duration::from_secs(wait),
        || {
            list = text_of(&page(b, "/qsmc/schedule")?);
            let done = list.matches("Completed").count() > had;
            if !done {
                std::thread::sleep(Duration::from_secs(5));
            }
            Ok(done)
        },
    )
    .map_err(|e| format!("{e}: the schedule shows `{list}`"))?;
    // New codes, the newest IPL complete.
    let mut newest = Vec::new();
    until("the IPL complete from its disk", IPL, || {
        newest = codes(&page(b, history)?);
        let done = newest != before && newest.first().is_some_and(|c| c == IPL_COMPLETE);
        if !done {
            std::thread::sleep(Duration::from_secs(5));
        }
        Ok(done)
    })
    .map_err(|e| {
        format!(
            "{e}: the newest codes are {:?}",
            &newest[..newest.len().min(8)]
        )
    })?;
    let mut last = String::new();
    until("Sign On in the window", Duration::from_mins(10), || {
        last = shown(b, window).unwrap_or_default();
        if !last.contains("Sign On") {
            std::thread::sleep(Duration::from_secs(5));
        }
        Ok(last.contains("Sign On"))
    })
    .map_err(|e| format!("{e}:\n{last}"))?;
    let after = super::host::signed_on_with(b, window, &SECOFR)?;
    if !after.contains("===>") {
        return Err("no command line after the second sign-on".into());
    }
    said.push(format!(
        "its activation scheduled at {d} {t} on Schedule operations: Completed, IPL source A, \
         to {IPL_COMPLETE} and Sign On, QSECOFR signed on again"
    ));
    // Its configuration saved as a profile.
    let r = post_form(b, &p.at("profiles?save=1"), ("ok", "OK"), &[("n", SAVED)])?;
    expect(b, &r, "QMC3202")?;
    until("the saved profile listed", Duration::from_mins(1), || {
        Ok(page(b, &p.at("profiles"))?.contains(SAVED))
    })?;
    said.push(format!(
        "its configuration saved as {SAVED} (QMC3202) and in Manage profiles"
    ));
    // Ended, then deleted with its disk.
    confirmed(b, &p.at("shutdown"), &[("how", "imm")], "QMC3102")?;
    p.reaches(b, "Not activated", Duration::from_mins(5))?;
    let del = format!("/qsmc/confirm/partition-delete?ms={}&p={}", p.host, p.id);
    let html = page(b, &del)?;
    let r = post_on(b, &html, &del, ("ok", "OK"), &[("disks", "on")])?;
    expect(b, &r, "QMC3133")?;
    let storage = format!("/qsmc/server/{}/virtual/storage", p.host);
    until(
        "the partition and its disk gone",
        Duration::from_mins(2),
        || {
            let list = page(b, "/qsmc/partitions")?;
            let st = page(b, &storage)?;
            Ok(!list.contains(&format!(">{PART}<")) && !st.contains(VDISK))
        },
    )?;
    said.push(format!(
        "ended immediately and deleted with its disk (QMC3133): {PART} and {VDISK} gone at the host"
    ));
    Ok(said)
}
