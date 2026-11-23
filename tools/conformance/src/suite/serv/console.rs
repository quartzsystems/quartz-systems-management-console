// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! Suite 8: the console's own update from its stream, an image whose web
//! server cannot start rolled back with state as it was, and V1R0M1
//! confirmed with its state migrated after and not before. Suite 9: the
//! images the console's update service refuses, from a removable volume.
//! Suite 10: power pulled during an install and a pending slot's first
//! IPL, on a mirrored load source.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use super::{CONSOLE_ROW, Lab, VS, download, lab, publish, qsmc_release, row};
use crate::qemu::session::{Disk, Machine, Session};
use crate::qemu::term::key;
use crate::suite::base::{Guest, Run, channel};
use crate::suite::install::{self, Kind, USER, option, sign_on, titled};
use crate::suite::mgmt::{page, until};
use crate::suite::parts::{expect, post_on, user};
use crate::suite::web::{ADMIN, Console, PASSWORD, boot, message_of, signed_on};
use crate::web::Browser;

/// The folders whose files are the console's state.
const STATE: [&str; 4] = ["/data/state", "/data/mgmt", "/data/web", "/data/fetch"];

/// Each state file: its length and the sum of its bytes.
pub fn state_sums(guest: &mut Session) -> Result<BTreeMap<String, (u64, u64)>, String> {
    let ch = channel(guest)?;
    let mut out = BTreeMap::new();
    for dir in STATE {
        for name in ch.ls(dir).unwrap_or_default() {
            let path = format!("{dir}/{name}");
            if let Ok(s) = ch.sum(&path) {
                out.insert(path, s);
            }
        }
    }
    Ok(out)
}

/// What PID 1 takes again from the machine at every IPL, not state an
/// operator or a service made.
const RETAKEN: [&str; 1] = ["/data/state/inventory.toml"];

/// The files of `a` and `b` that differ, or that one has alone, but
/// what every IPL takes again.
fn differ(a: &BTreeMap<String, (u64, u64)>, b: &BTreeMap<String, (u64, u64)>) -> Vec<String> {
    a.keys()
        .chain(b.keys())
        .filter(|k| !RETAKEN.contains(&k.as_str()))
        .filter(|k| a.get(*k) != b.get(*k))
        .cloned()
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect()
}

/// Begin the console's own install from its confirmation.
fn install_console(b: &mut Browser) -> Result<(), String> {
    let path = "/qsmc/confirm/update-install?t=console";
    let a = b.get(path)?;
    if a.status != 200 {
        let t = crate::suite::web::follow(b, &a)?.text();
        return Err(format!(
            "the console's install confirmation answered {}: {}",
            a.status,
            message_of(&t)
        ));
    }
    let html = a.text();
    if !html.contains("other slot on every load-source disk") {
        return Err("the console's confirmation does not say what it does".into());
    }
    let r = post_on(b, &html, path, ("ok", "OK"), &[])?;
    expect(b, &r, qmcmsg::INSTALL_BEGUN.id).map(|_| ())
}

/// Wait for the console to restart into its update, or say why the
/// install ended without it, as its progress page says.
fn restarts(guest: &mut Guest<'_>, b: &mut Browser) -> Result<(), String> {
    let end = Instant::now() + Duration::from_secs(600);
    loop {
        if guest
            .wait_exit(Instant::now() + Duration::from_secs(5))
            .is_ok()
        {
            return Ok(());
        }
        if let Ok(a) = b.get("/qsmc/updates/code/console/progress") {
            let t = a.text();
            if t.contains("<td class=\"error\">") {
                return Err(format!(
                    "the install ended without a restart: {}",
                    message_of(&t)
                ));
            }
        }
        if Instant::now() > end {
            return Err("the console did not restart into its update in 600 s".into());
        }
    }
}

/// Wait for the guest to restart: its QEMU ends.
fn ends(guest: &mut Guest<'_>, within: Duration) -> Result<(), String> {
    guest.wait_exit(Instant::now() + within).map(|_| ())
}

/// The level the console's row of Code updates says is installed.
fn installed_level(b: &mut Browser) -> Result<String, String> {
    let t = page(b, "/qsmc/updates/code")?;
    let r = row(&t, CONSOLE_ROW);
    Ok(["V1R0M0", "V1R0M1"]
        .into_iter()
        .find(|l| r.contains(&format!("{l} build")))
        .unwrap_or("unknown")
        .to_string())
}

/// The name of the update image at `path`.
pub(super) fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// The images the suites install.
fn images<'p>(run: &'p Run<'_>) -> Result<(&'p PathBuf, &'p PathBuf), String> {
    let next = run
        .image
        .next
        .as_ref()
        .ok_or("the next variant is not built (`build --test-image --variant next all`)")?;
    let broken = run
        .image
        .broken
        .as_ref()
        .ok_or("the broken variant is not built (`build --test-image --variant broken all`)")?;
    Ok((next, broken))
}

/// Suite 8.
#[allow(clippy::too_many_lines, reason = "the roll back, then the update")]
pub fn update(run: &Run<'_>) -> Result<String, String> {
    let (next, broken) = images(run)?;
    let mut lab = lab(run, "serv-console", &[qsmc_release(broken)?])?;
    let mut said = Vec::new();
    user(&mut lab.c.browser, "witness", "Operator", "Observe-pass-42")?;
    // The image whose web server cannot start.
    download(&mut lab.c.browser, CONSOLE_ROW, "qsmc", &file_name(broken))?;
    let before = state_sums(&mut lab.c.guest)?;
    install_console(&mut lab.c.browser)?;
    let Lab { fleet, c, streams } = lab;
    let Console {
        mut guest,
        mut machine,
        ..
    } = c;
    ends(&mut guest, Duration::from_secs(600))?;
    drop(guest);
    machine.oem = vec![qsmchook::oem_string(qsmchook::SHORT_TRIAL)];
    let mut guest = run.boot(&machine)?;
    guest.expect(
        "IPL slot B (qsmc.slot=B)",
        Instant::now() + Duration::from_secs(300),
    )?;
    guest.seen(
        &refcode::UPDATE_STATE_KEPT.tag(),
        Instant::now() + Duration::from_secs(120),
    )?;
    // Its web server never answers: after the trial, the slot it ran.
    ends(&mut guest, Duration::from_secs(600))?;
    let trial_log = guest.log().to_string();
    drop(guest);
    if trial_log.contains(&refcode::SLOT_CONFIRMED.tag()) {
        return Err("the slot whose web server cannot start confirmed".into());
    }
    machine.oem.clear();
    let mut c = boot(run, machine)?;
    if !c.guest.said("IPL slot A (qsmc.slot=A)") {
        return Err("the console did not come back on slot A".into());
    }
    c.guest.seen(
        &refcode::UPDATE_STATE_RESTORED.tag(),
        Instant::now() + Duration::from_secs(10),
    )?;
    // Before a sign-on, which writes its time in `webusers.toml`.
    let after = state_sums(&mut c.guest)?;
    signed_on(&mut c.browser, ADMIN, PASSWORD)?;
    let changed = differ(&before, &after);
    if !changed.is_empty() {
        return Err(format!(
            "after the roll back the state differs: {changed:?}"
        ));
    }
    let mut lab = Lab { fleet, c, streams };
    let b = &mut lab.c.browser;
    if installed_level(b)? != "V1R0M0" {
        return Err("after the roll back the console does not run V1R0M0".into());
    }
    // The problem service reads the codes on its own turn.
    let fell = refcode::SLOT_FELL_BACK.code;
    until(
        "the console's problem for the slot",
        Duration::from_secs(120),
        || Ok(page(b, "/qsmc/events?rows=100")?.contains(fell)),
    )?;
    until(
        "a console dump of the slot",
        Duration::from_secs(120),
        || Ok(page(b, "/qsmc/dumps")?.contains(consdump::KIND.prefix)),
    )?;
    let progress = page(b, "/qsmc/updates/code/console/progress")?;
    if !progress.contains("rolled back") {
        return Err("the progress page does not say the update rolled back".into());
    }
    said.push(format!(
        "an image whose web server cannot start: slot B on trial, not confirmed, the console back \
         on slot A with its {} state files as they were, {fell} open, a dump taken, the progress \
         page saying it rolled back",
        before.len()
    ));
    // V1R0M1 from the stream.
    publish(&lab.streams, 3, &[qsmc_release(next)?]);
    super::check(&mut lab.c.browser)?;
    download(&mut lab.c.browser, CONSOLE_ROW, "qsmc", &file_name(next))?;
    // A setting changed writes web.toml, at this release's schema 1.
    let r = crate::suite::parts::post_form(
        &mut lab.c.browser,
        "/qsmc/console/locale",
        ("apply", "Apply"),
        &[("to", "45")],
    )?;
    crate::suite::web::follow(&mut lab.c.browser, &r)?;
    let ch = channel(&mut lab.c.guest)?;
    let web_before = ch.text("/data/state/web.toml")?;
    let users = ch.text("/data/state/webusers.toml")?;
    let systems = ch.text("/data/mgmt/systems.toml")?;
    if !web_before.contains("schema_version = 1") {
        return Err(format!("web.toml before: {web_before}"));
    }
    install_console(&mut lab.c.browser)?;
    let Lab { fleet, c, streams } = lab;
    let Console {
        mut guest, machine, ..
    } = c;
    ends(&mut guest, Duration::from_secs(600))?;
    drop(guest);
    let mut c = boot(run, machine)?;
    let mut lab = Lab {
        fleet,
        c: {
            signed_on(&mut c.browser, ADMIN, PASSWORD)?;
            c
        },
        streams,
    };
    c_confirmed(&mut lab.c.guest)?;
    // Each service asks PID 1 every 5 s whether its slot confirmed.
    let _ = lab.c.guest.seen(
        &refcode::STATE_MIGRATED.tag(),
        Instant::now() + Duration::from_secs(60),
    );
    let log = lab.c.guest.log().to_string();
    let at = |t: &str| log.find(t);
    let (Some(confirmed), Some(migrated)) = (
        at(&refcode::UPDATE_CONFIRMED.tag()),
        at(&refcode::STATE_MIGRATED.tag()),
    ) else {
        let web = channel(&mut lab.c.guest)
            .and_then(|ch| ch.text("/data/state/web.toml"))
            .unwrap_or_default();
        let said = crate::suite::mgmt::logs(
            &mut lab.c.guest,
            &["web.toml", "migrat", "this release's form"],
        );
        return Err(format!(
            "the serial log lacks the confirmation or the migration; web.toml:\n{web}\nthe logs:\n{said}"
        ));
    };
    if migrated < confirmed {
        return Err("state was migrated before the slot confirmed".into());
    }
    let ch = channel(&mut lab.c.guest)?;
    let web_after = ch.text("/data/state/web.toml")?;
    if !web_after.contains("schema_version = 2") {
        return Err(format!("web.toml after the confirmation: {web_after}"));
    }
    // What a sign-on or an event writes moves on; the users and the
    // systems themselves do not.
    let users_after = ch.text("/data/state/webusers.toml")?;
    let systems_after = ch.text("/data/mgmt/systems.toml")?;
    if kept(&users_after, &["last_sign_on", "last_from"])
        != kept(&users, &["last_sign_on", "last_from"])
        || kept(&systems_after, &["last_seq"]) != kept(&systems, &["last_seq"])
    {
        return Err(format!(
            "the users or the managed systems changed across the update:\n{users}\n{users_after}\n{systems}\n{systems_after}"
        ));
    }
    let b = &mut lab.c.browser;
    if installed_level(b)? != "V1R0M1" {
        return Err("Code updates does not say the console runs V1R0M1".into());
    }
    until(
        "both systems connected again",
        Duration::from_secs(120),
        || {
            let t = page(b, "/qsmc/connectivity")?;
            Ok(row(&t, VS).contains(">Connected<")
                && row(&t, super::STANDALONE).contains(">Connected<"))
        },
    )?;
    let mut w = b.fresh();
    signed_on(&mut w, "witness", "Observe-pass-42")?;
    said.push(
        "V1R0M1 found on the stream, downloaded, installed and activated: slot B confirmed when \
         its web server answered, Code updates says V1R0M1, web.toml migrated to schema 2 after \
         the confirmation (C3007075 after C3007060), users and managed systems as before, both \
         systems connected again and the witness signed on"
            .into(),
    );
    lab.c.guest.kill();
    Ok(said.join("; "))
}

/// `text`'s lines but those that set one of `moving`.
fn kept(text: &str, moving: &[&str]) -> Vec<String> {
    text.lines()
        .filter(|l| {
            !moving
                .iter()
                .any(|m| l.trim_start().starts_with(&format!("{m} ")))
        })
        .map(str::to_string)
        .collect()
}

/// Wait until the console's update confirmed.
fn c_confirmed(guest: &mut Session) -> Result<(), String> {
    guest.seen(
        &refcode::UPDATE_CONFIRMED.tag(),
        Instant::now() + Duration::from_secs(120),
    )
}

/// The development key the test image is signed with.
fn dev_key(run: &Run<'_>) -> Result<ed25519_dalek::SigningKey, String> {
    let path = run
        .image
        .key
        .as_ref()
        .ok_or("the test image's key is not known")?;
    let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    let seed: [u8; 32] = hex::decode(text.trim())
        .map_err(|e| e.to_string())?
        .try_into()
        .map_err(|_| "the key is not 32 bytes".to_string())?;
    Ok(ed25519_dalek::SigningKey::from_bytes(&seed))
}

/// The manifest and both slots' UKIs of the update image at `path`.
fn parts_of(path: &Path) -> Result<(update::manifest::Manifest, Vec<u8>, Vec<u8>), String> {
    let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
    let p = update::image::parse(&qsmcupdd::FORMAT, &bytes).map_err(|e| e.0)?;
    let text = std::str::from_utf8(&bytes[p.manifest.clone()]).map_err(|e| e.to_string())?;
    let m = update::manifest::Manifest::parse(&qsmcupdd::FORMAT, text)?;
    let file = |n: &str| {
        p.files
            .iter()
            .find(|(f, _)| f == n)
            .map(|(_, r)| bytes[r.clone()].to_vec())
            .ok_or(format!("the image has no {n}"))
    };
    Ok((
        m,
        file(update::verify::SLOT_A)?,
        file(update::verify::SLOT_B)?,
    ))
}

/// A removable volume of `files` as a further disk, `name`.
pub fn volume(run: &Run<'_>, name: &str, files: &[(String, Vec<u8>)]) -> Result<Disk, String> {
    use diskimg::fat;
    let total: u64 = files.iter().map(|(_, b)| b.len() as u64).sum();
    let spec = fat::Spec {
        label: fat::label("QSMCVOL").map_err(|e| e.to_string())?,
        ..fat::fat32_for(total + (64 << 20))
    };
    let refs: Vec<(&str, &[u8])> = files
        .iter()
        .map(|(n, b)| (n.as_str(), b.as_slice()))
        .collect();
    let image = fat::build(&spec, &refs).map_err(|e| e.to_string())?;
    let path = run.file(&format!("{name}.img"));
    std::fs::write(&path, image).map_err(|e| e.to_string())?;
    Ok(Disk {
        path,
        serial: "QSMCVOL01".into(),
        snapshot: false,
    })
}

/// An installed console's machine with `extra` disks after its own.
pub(super) fn with_disks(
    run: &Run<'_>,
    tag: &str,
    kind: Kind,
    extra: Vec<Disk>,
) -> Result<Machine, String> {
    let mut disks = install::installed(run, kind, tag)?;
    disks.extend(extra);
    let mut m = install::installed_machine(run, &disks);
    m.forwards = vec![
        (crate::web::free_port()?, 443),
        (crate::web::free_port()?, 80),
    ];
    Ok(m)
}

/// Option 8, 4 for the image `file` on device `vdb`: the message it
/// ends with, or the confirmation shown.
fn stage_from_volume(guest: &mut Guest<'_>, file: &str) -> Result<String, String> {
    let deadline = Instant::now() + Duration::from_secs(60);
    option(
        guest,
        4,
        "Install an Update from a Removable Volume",
        deadline,
    )?;
    guest.type_keys(&format!("vdb{}{}", key::FIELD_EXIT, key::ENTER))?;
    let list = titled(guest, "Choose an Update Image", deadline)?;
    let at = list
        .lines()
        .filter(|l| l.contains(".upd"))
        .position(|l| l.contains(file))
        .ok_or_else(|| format!("{file} is not listed:\n{list}"))?;
    let mut keys = String::new();
    for _ in 0..at {
        keys.push_str(key::TAB);
    }
    keys.push('1');
    keys.push_str(key::ENTER);
    guest.type_keys(&keys)?;
    let shown = guest.screen(
        "the image staged or refused",
        Instant::now() + Duration::from_secs(120),
        |g| g.row(1).trim() == "Confirm Install Update" || !g.message().trim().is_empty(),
    )?;
    Ok(shown)
}

/// Suite 9.
#[allow(clippy::too_many_lines, reason = "five images, one refusal each")]
pub fn refusals(run: &Run<'_>) -> Result<String, String> {
    let own = run
        .image
        .update
        .as_ref()
        .ok_or("the test image's update image is not built")?;
    let key = dev_key(run)?;
    let (m, a, b) = parts_of(own)?;
    let sign = |m: &update::manifest::Manifest, k: &ed25519_dalek::SigningKey| {
        update::make(&qsmcupdd::FORMAT, m, &a, &b, |t| {
            ed25519_dalek::Signer::sign(k, t).to_bytes()
        })
    };
    let newer = update::manifest::Manifest {
        release: "V1R0M2".into(),
        ..m.clone()
    };
    let mut damaged = sign(&newer, &key);
    let mid = damaged.len() / 2;
    damaged[mid] ^= 0x5a;
    let stranger = sign(&newer, &crate::streams::keys::stranger());
    let other = sign(
        &update::manifest::Manifest {
            product: "QS/VS".into(),
            ..newer.clone()
        },
        &key,
    );
    let older = sign(
        &update::manifest::Manifest {
            release: "V0R9M0".into(),
            ..m.clone()
        },
        &key,
    );
    let mut schemas = newer.schemas.clone();
    for (f, v) in &mut schemas {
        if f == "webusers.toml" {
            *v = 0;
        }
    }
    let unreadable = sign(
        &update::manifest::Manifest {
            schemas,
            ..newer.clone()
        },
        &key,
    );
    let cases: Vec<(&str, Vec<u8>, &str)> = vec![
        ("A-DAMAGED.upd", damaged, "damaged"),
        (
            "B-STRANGER.upd",
            stranger,
            "signed by a key this release does not trust",
        ),
        ("C-OTHER.upd", other, "another product's"),
        ("D-OLDER.upd", older, "older than what runs"),
        (
            "E-STATE.upd",
            unreadable,
            "cannot read this console's state",
        ),
    ];
    let files: Vec<(String, Vec<u8>)> = cases
        .iter()
        .map(|(n, b, _)| ((*n).to_string(), b.clone()))
        .collect();
    let vol = volume(run, "refusals-volume", &files)?;
    let m = with_disks(run, "serv-refusals", Kind::Single, vec![vol])?;
    let mut c = boot(run, m)?;
    let deadline = Instant::now() + Duration::from_secs(60);
    sign_on(&mut c.guest, USER, PASSWORD, deadline)?;
    let mut said = Vec::new();
    for (name, _, what) in &cases {
        option(&mut c.guest, 8, "Service Functions", deadline)?;
        let shown = stage_from_volume(&mut c.guest, name)?;
        if shown
            .lines()
            .nth(1)
            .is_some_and(|t| t.trim() == "Confirm Install Update")
            && !shown
                .lines()
                .any(|l| l.contains("Verified") && l.trim_end().ends_with(" no"))
        {
            return Err(format!("{name} ({what}) was taken:\n{shown}"));
        }
        let message = install::message(&mut c.guest);
        let text = if message.is_empty() {
            shown
                .lines()
                .filter(|l| l.contains("QMC") || l.contains("QVS") || l.contains("refused"))
                .collect::<Vec<_>>()
                .join(" ")
        } else {
            message
        };
        if text.is_empty() {
            return Err(format!(
                "{name} ({what}) refused without a reason:\n{shown}"
            ));
        }
        said.push(format!("{what}: `{}`", text.trim()));
        c.guest.type_keys(key::F3)?;
        titled(&mut c.guest, install::MAIN_MENU, deadline)?;
    }
    // Nothing was installed: no slot pending, the console as it was.
    let status = channel(&mut c.guest)?.status()?;
    let record = crate::suite::base::fact_of(&status, release::fact::RECORD)?.to_string();
    if record.contains("pending A") || record.contains("pending B") {
        return Err(format!("a refused image left a slot pending: {record}"));
    }
    c.guest.kill();
    Ok(format!(
        "five images refused from a removable volume, nothing pending ({record}): {}",
        said.join("; ")
    ))
}

/// F9 at the serial port.
const F9: &str = "\x1b[20~";

/// Cancel a slot an install left pending, never activated (power pulled
/// after it was written), from option 8, 5: whether there was one.
fn cancel_pending(c: &mut Console<'_>) -> Result<bool, String> {
    let st = channel(&mut c.guest)?.status()?;
    let record = crate::suite::base::fact_of(&st, release::fact::RECORD)?.to_string();
    if !(record.contains("pending A") || record.contains("pending B")) {
        return Ok(false);
    }
    let d = Instant::now() + Duration::from_secs(60);
    sign_on(&mut c.guest, USER, PASSWORD, d)?;
    option(&mut c.guest, 8, "Service Functions", d)?;
    option(&mut c.guest, 5, "Display Code Levels", d)?;
    c.guest.type_keys(F9)?;
    install::said(&mut c.guest, "pending no more", d)?;
    c.guest.type_keys(key::F3)?;
    titled(&mut c.guest, install::MAIN_MENU, d)?;
    c.guest.type_keys(key::F3)?;
    Ok(true)
}

/// How many times power is pulled.
const PULLS: usize = 30;

/// A pseudo-random generator of moments.
struct Moments(u64);

impl Moments {
    fn ms(&mut self, lo: u64, hi: u64) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        lo + self.0 % (hi - lo).max(1)
    }
}

/// A mirrored console with V1R0M1 downloaded, booted and signed on.
fn fresh<'a>(
    run: &Run<'a>,
    n: usize,
    streams: &crate::streams::server::Server,
    next: &Path,
) -> Result<Console<'a>, String> {
    let m = with_disks(run, &format!("serv-power-{n}"), Kind::Mirrored, Vec::new())?;
    let mut c = boot(run, m)?;
    signed_on(&mut c.browser, ADMIN, PASSWORD)?;
    user(&mut c.browser, "witness", "Operator", "Observe-pass-42")?;
    super::point(&mut c.browser, &super::server_url(streams))?;
    super::check(&mut c.browser)?;
    download(&mut c.browser, CONSOLE_ROW, "qsmc", &file_name(next))?;
    Ok(c)
}

/// The console came back whole: ready, its web server answering, the
/// witness and the Administrator signing on: the level it runs.
fn whole<'a>(run: &Run<'a>, machine: Machine) -> Result<(Console<'a>, String), String> {
    let mut c = boot(run, machine)?;
    crate::suite::base::all_ready(&channel(&mut c.guest)?.status()?)?;
    signed_on(&mut c.browser, ADMIN, PASSWORD)?;
    let mut w = c.browser.fresh();
    signed_on(&mut w, "witness", "Observe-pass-42")?;
    let level = installed_level(&mut c.browser)?;
    Ok((c, level))
}

/// Whether both disks of `m` hold the next image's slot B in their ESP.
fn on_both(m: &Machine, slot_b: &[u8]) -> Result<bool, String> {
    for d in m.disks.iter().take(2) {
        let esp = crate::suite::base::slot::esp(&d.path)?;
        let mut f = std::fs::File::open(&d.path).map_err(|e| e.to_string())?;
        let mut bytes = vec![0u8; usize::try_from(esp.sectors * 512).unwrap_or(0)];
        std::io::Seek::seek(&mut f, std::io::SeekFrom::Start(esp.first_lba * 512))
            .map_err(|e| e.to_string())?;
        std::io::Read::read_exact(&mut f, &mut bytes).map_err(|e| e.to_string())?;
        let path = release::names::SLOT_B_PATH
            .trim_start_matches('\\')
            .replace('\\', "/");
        let got = diskimg::fat::read_file(&bytes, &path).map_err(|e| e.to_string())?;
        if got != slot_b {
            return Ok(false);
        }
    }
    Ok(true)
}

/// Suite 10.
#[allow(
    clippy::too_many_lines,
    reason = "the pulls, and what each boot after shows"
)]
pub fn power(run: &Run<'_>) -> Result<String, String> {
    let (next, _) = images(run)?;
    let (_, _, slot_b) = parts_of(next)?;
    let streams = crate::streams::server::Server::start("127.0.0.1:0");
    publish(&streams, 2, &[qsmc_release(next)?]);
    let mut moments = Moments(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(7, |d| u64::from(d.subsec_nanos()) | 1),
    );
    let mut fresh_n = 0;
    let mut c = fresh(run, fresh_n, &streams, next)?;
    let (mut in_install, mut in_trial, mut confirmed, mut both) = (0, 0, 0, false);
    let mut cancelled = 0;
    for pull in 0..PULLS {
        let at = |e: String| format!("after {pull} pulls: {e}");
        // A slot activated and pulled is the next IPL's, which falls
        // back; one written and never activated is cancelled here.
        if cancel_pending(&mut c).map_err(at)? {
            cancelled += 1;
        }
        install_console(&mut c.browser).map_err(at)?;
        let machine = if pull % 2 == 0 {
            // During the install: staged, verified, the slot written.
            std::thread::sleep(Duration::from_millis(moments.ms(0, 6000)));
            let Console {
                mut guest, machine, ..
            } = c;
            guest.kill();
            in_install += 1;
            machine
        } else {
            // During the pending slot's first IPL.
            let Console {
                mut guest,
                machine,
                mut browser,
            } = c;
            restarts(&mut guest, &mut browser).map_err(at)?;
            drop(guest);
            let mut g = run.boot(&machine).map_err(at)?;
            g.expect("IPL slot B", Instant::now() + Duration::from_secs(300))
                .map_err(at)?;
            std::thread::sleep(Duration::from_millis(moments.ms(0, 20_000)));
            g.kill();
            in_trial += 1;
            machine
        };
        let (mut back, level) = whole(run, machine).map_err(at)?;
        if level == "V1R0M1" {
            confirmed += 1;
            if !both {
                both = on_both(&back.machine, &slot_b).map_err(at)?;
                if !both {
                    return Err(at("V1R0M1 runs but slot B is not on both disks".into()));
                }
            }
            back.guest.kill();
            fresh_n += 1;
            back = fresh(run, fresh_n, &streams, next).map_err(at)?;
        }
        c = back;
    }
    // One install left to finish: it is on both disks.
    if !both {
        cancel_pending(&mut c)?;
        install_console(&mut c.browser)?;
        ends(&mut c.guest, Duration::from_secs(600))?;
        let Console { guest, machine, .. } = c;
        drop(guest);
        let (back, level) = whole(run, machine)?;
        if level != "V1R0M1" || !on_both(&back.machine, &slot_b)? {
            return Err("the finished install is not V1R0M1 on both disks".into());
        }
        c = back;
    }
    c.guest.kill();
    Ok(format!(
        "{PULLS} power pulls on a mirrored load source, {in_install} during an install and \
         {in_trial} during the pending slot's first IPL: after each the console came back ready \
         on a good slot with its web users signing on ({confirmed} times on V1R0M1, the rest on \
         V1R0M0; {cancelled} times a slot written but never activated, cancelled at option 8, \
         5); V1R0M1's slot B on both disks"
    ))
}

/// Phase 8's `sec-rotation` (`docs/RELEASING.md`): a console signed by
/// the first key and trusting the next refuses V1R0M1 signed by a key
/// it does not trust, with nothing staged, then installs V1R0M1 signed
/// by the next key from the stream, and its systems' sessions resume.
pub fn rotation(run: &Run<'_>) -> Result<String, String> {
    let t = &run.image;
    let rotated = t
        .rotated
        .as_ref()
        .ok_or("the rotated variant is not built (`build --test-image --variant rotated all`)")?;
    let stranger = t
        .stranger
        .as_ref()
        .ok_or("the stranger variant is not built (`build --test-image --variant stranger all`)")?;
    let mut lab = lab(run, "sec-rotation", &[qsmc_release(stranger)?])?;
    super::check(&mut lab.c.browser)?;
    download(&mut lab.c.browser, CONSOLE_ROW, "qsmc", &file_name(stranger))?;
    // The stream's index verifies, the file is what it names; its image
    // is signed by a key the console does not trust.
    let b = &mut lab.c.browser;
    let path = "/qsmc/confirm/update-install?t=console";
    let html = b.get(path)?.text();
    let r = post_on(b, &html, path, ("ok", "OK"), &[])?;
    let first = crate::suite::web::follow(b, &r)?.text();
    let mut refused = first.contains(qmcmsg::UPD_SIGNATURE.id);
    let end = Instant::now() + Duration::from_secs(120);
    while !refused && Instant::now() < end {
        let t = b.get("/qsmc/updates/code/console/progress")?.text();
        refused = t.contains(qmcmsg::UPD_SIGNATURE.id);
    }
    if !refused {
        return Err(format!(
            "V1R0M1 signed by a key the console does not trust was not refused with {}",
            qmcmsg::UPD_SIGNATURE.id
        ));
    }
    if installed_level(b)? != "V1R0M0" {
        return Err("a refused image changed what the console runs".into());
    }
    // The next key's release.
    publish(&lab.streams, 3, &[qsmc_release(rotated)?]);
    super::check(&mut lab.c.browser)?;
    download(&mut lab.c.browser, CONSOLE_ROW, "qsmc", &file_name(rotated))?;
    install_console(&mut lab.c.browser)?;
    let Lab { fleet, c, streams } = lab;
    let Console {
        mut guest, machine, ..
    } = c;
    ends(&mut guest, Duration::from_secs(600))?;
    drop(guest);
    let mut c = boot(run, machine)?;
    signed_on(&mut c.browser, ADMIN, PASSWORD)?;
    c_confirmed(&mut c.guest)?;
    let b = &mut c.browser;
    if installed_level(b)? != "V1R0M1" {
        return Err("Code updates does not say the console runs the rotated V1R0M1".into());
    }
    until(
        "both systems connected again",
        Duration::from_secs(120),
        || {
            let t = page(b, "/qsmc/connectivity")?;
            Ok(row(&t, VS).contains(">Connected<")
                && row(&t, super::STANDALONE).contains(">Connected<"))
        },
    )?;
    drop((fleet, streams));
    c.guest.kill();
    Ok(format!(
        "V1R0M1 signed by a key not trusted refused ({}) with V1R0M0 still running; V1R0M1 \
         signed by the next key found on the stream, installed, confirmed, and both systems \
         connected again",
        qmcmsg::UPD_SIGNATURE.id
    ))
}
