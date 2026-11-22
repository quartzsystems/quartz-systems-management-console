// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! Suite 5: a partition's dump streamed from the host through the
//! console to the browser, nothing of it kept; a download abandoned;
//! delete only through its confirmation; dump space. Suite 12: the
//! console's own dump opens with a standard tool, holds what its
//! manifest says and no secret.

use std::process::Command;
use std::time::Duration;

use sha2::{Digest, Sha256};

use super::{Lab, STANDALONE, VS, lab};
use crate::qemu::session::Session;
use crate::suite::base::channel;
use crate::suite::mgmt::{page, until};
use crate::suite::parts::{expect, post_on};
use crate::suite::web::{PASSWORD, message_of, shows, submit};

/// The large dump's size, MiB.
const BIG_MIB: u64 = 256;
/// Its name.
const BIG: &str = "SYSDUMP.QSTEST01.20261121.090000";

/// A process's resident memory and the bytes it has written to storage.
fn use_of(guest: &mut Session, pid: u32) -> Result<(u64, u64), String> {
    let ch = channel(guest)?;
    let status = ch.text(&format!("/proc/{pid}/status"))?;
    let rss = status
        .lines()
        .find_map(|l| l.strip_prefix("VmRSS:"))
        .and_then(|v| v.split_whitespace().next()?.parse::<u64>().ok())
        .unwrap_or(0)
        * 1024;
    let io = ch.text(&format!("/proc/{pid}/io"))?;
    let written = io
        .lines()
        .find_map(|l| l.strip_prefix("write_bytes:"))
        .and_then(|v| v.trim().parse().ok())
        .unwrap_or(0);
    Ok((rss, written))
}

/// The pids of the web server and the managed-systems service.
fn pids(guest: &mut Session) -> Result<[u32; 2], String> {
    let status = channel(guest)?.status()?;
    let pid = |n: &str| status.service(n).map(|s| s.pid).ok_or(format!("no {n}"));
    Ok([pid("webd")?, pid("mgmtd")?])
}

/// The SHA-256 of what the simulator streams of dump `name` of `bytes`.
fn expected(name: &str, bytes: u64) -> String {
    let mut h = Sha256::new();
    let mut at = 0;
    while at < bytes {
        let n = (bytes - at).min(1 << 20);
        h.update(sim::system::servicing::content(
            name,
            at,
            usize::try_from(n).unwrap_or(0),
        ));
        at += n;
    }
    hex::encode(h.finalize())
}

/// Suite 5.
#[allow(
    clippy::too_many_lines,
    reason = "the download, the abandon, the delete"
)]
pub fn dumps(run: &crate::suite::base::Run<'_>) -> Result<String, String> {
    let mut lab = lab(run, "serv-dumps", &[])?;
    let said = lab.fleet.tell(0, &format!("dump 2 {BIG} {BIG_MIB}"));
    if said != "dump taken" {
        return Err(format!("the simulator said `{said}`"));
    }
    let key = format!("{VS}:2:{BIG}");
    until("the large dump listed", Duration::from_secs(60), || {
        Ok(page(&mut lab.c.browser, "/qsmc/dumps")?.contains(BIG))
    })?;
    let t = page(&mut lab.c.browser, "/qsmc/dumps")?;
    for who in [VS, STANDALONE, "the console"] {
        if !t.contains(&format!("{who}: ")) {
            return Err(format!("no dump space shown for {who}"));
        }
    }
    if !t.contains("Dump space: ") {
        return Err("the page shows no dump space".into());
    }
    // The download, watched from the guest while it runs.
    let [webd, mgmtd] = pids(&mut lab.c.guest)?;
    let before = [
        use_of(&mut lab.c.guest, webd)?,
        use_of(&mut lab.c.guest, mgmtd)?,
    ];
    let path = format!("/qsmc/dumps/download?d={key}");
    let browser = lab.c.browser.clone_session();
    let (got, sha, peak) = std::thread::scope(|s| -> Result<_, String> {
        let h = s.spawn(move || {
            let mut sha = Sha256::new();
            let got = browser.stream_get(&path, None, |b| sha.update(b));
            got.map(|g| (g, hex::encode(sha.finalize())))
        });
        let mut peak = [0u64; 2];
        while !h.is_finished() {
            for (i, pid) in [webd, mgmtd].iter().enumerate() {
                if let Ok((rss, _)) = use_of(&mut lab.c.guest, *pid) {
                    peak[i] = peak[i].max(rss);
                }
            }
            std::thread::sleep(Duration::from_millis(500));
        }
        let (got, sha) = h.join().map_err(|_| "the download's thread panicked")??;
        Ok((got, sha, peak))
    })?;
    let after = [
        use_of(&mut lab.c.guest, webd)?,
        use_of(&mut lab.c.guest, mgmtd)?,
    ];
    let bytes = BIG_MIB << 20;
    if got.status != 200 || got.read != bytes || got.length != bytes {
        return Err(format!(
            "the download answered {} with {} of {} bytes",
            got.status, got.read, got.length
        ));
    }
    if sha != expected(BIG, bytes) {
        return Err("the download's bytes are not the host's".into());
    }
    let grew: Vec<u64> = (0..2)
        .map(|i| peak[i].saturating_sub(before[i].0))
        .collect();
    let wrote: Vec<u64> = (0..2)
        .map(|i| after[i].1.saturating_sub(before[i].1))
        .collect();
    if grew.iter().any(|g| *g > 32 << 20) || wrote.iter().any(|w| *w > 4 << 20) {
        return Err(format!(
            "the console held the dump: memory grew {grew:?} bytes, {wrote:?} bytes written (webd, mgmtd)"
        ));
    }
    // Abandoned by the browser after 8 MiB: the host's stream ends.
    let path = format!("/qsmc/dumps/download?d={key}");
    let cut = lab.c.browser.stream_get(&path, Some(8 << 20), |_| {})?;
    let ended = format!("download {BIG} ended by the console after");
    until("the host's stream ended", Duration::from_secs(60), || {
        Ok(lab.fleet.lines(0).contains(&ended))
    })?;
    let line = lab
        .fleet
        .lines(0)
        .lines()
        .rfind(|l| l.contains(&ended))
        .unwrap_or_default()
        .to_string();
    // Delete: a POST of the list's form leads to the confirmation, and
    // only its OK deletes.
    let b = &mut lab.c.browser;
    let token = b.csrf("/qsmc/dumps")?;
    let r = b.post(
        "/qsmc/dumps",
        &[("csrf", &token), ("d0", &key), ("delete", "Delete")],
    )?;
    let to = r.header("location").unwrap_or_default().to_string();
    if r.status != 303 || !to.contains("/confirm/dump-delete") {
        return Err(format!("Delete answered {} to `{to}`", r.status));
    }
    let confirm = format!("/qsmc/confirm/dump-delete?d={key}");
    let r = submit(b, &confirm, &[("d", &key), ("cancel", "Cancel")])?;
    let _ = crate::suite::web::follow(b, &r)?;
    let held = |lab: &Lab<'_>| {
        sim::lock(&lab.fleet.systems[0])
            .parts
            .iter()
            .any(|p| p.dumps.iter().any(|d| d.name == BIG))
    };
    if !held(&lab) {
        return Err("Cancel deleted the dump".into());
    }
    let b = &mut lab.c.browser;
    let html = page(b, &confirm)?;
    let r = post_on(b, &html, &confirm, ("ok", "OK"), &[])?;
    expect(b, &r, qmcmsg::DUMP_DELETED.id)?;
    if held(&lab) {
        return Err("OK did not delete the dump at the host".into());
    }
    // Copy to remote host answers with its message.
    let b = &mut lab.c.browser;
    let html = page(b, "/qsmc/dumps")?;
    let r = post_on(
        b,
        &html,
        "/qsmc/dumps",
        ("copy", "Copy to remote host..."),
        &[],
    )?;
    let t = crate::suite::web::follow(b, &r)?.text();
    if !shows(&t, qmcmsg::NO_REMOTE_COPY.id) {
        return Err(format!("Copy to remote host: {}", message_of(&t)));
    }
    lab.c.guest.kill();
    Ok(format!(
        "{BIG_MIB} MiB from QSTEST01's host through the console, the host's bytes (sha256 {}); \
         at its peak the web server and mgmtd grew {grew:?} bytes and wrote {wrote:?}; abandoned \
         after {} bytes, the host said `{line}`; Delete led to its confirmation, Cancel kept the \
         dump, OK deleted it at the host (QMC7501); Copy to remote host answered QMC7503; dump \
         space shown for QSHQVS1, QSHQ400 and the console",
        &sha[..16],
        cut.read
    ))
}

/// What `unzip` says of `args`, or why it failed.
fn unzip(args: &[&str]) -> Result<Vec<u8>, String> {
    let out = Command::new("unzip")
        .args(args)
        .output()
        .map_err(|e| format!("unzip: {e}"))?;
    if out.status.success() {
        Ok(out.stdout)
    } else {
        Err(format!(
            "unzip {args:?}: {}",
            String::from_utf8_lossy(&out.stderr)
        ))
    }
}

/// Suite 12.
#[allow(
    clippy::too_many_lines,
    reason = "the dump read through, then each secret"
)]
pub fn dump_file(run: &crate::suite::base::Run<'_>) -> Result<String, String> {
    let mut lab = lab(run, "serv-dumpfile", &[])?;
    let b = &mut lab.c.browser;
    let token = b.csrf("/qsmc/dumps")?;
    let r = b.post("/qsmc/dumps/take", &[("csrf", &token)])?;
    let t = expect(b, &r, qmcmsg::DUMP_TAKEN.id)?;
    let name = t
        .split(consdump::KIND.prefix)
        .nth(1)
        .and_then(|r| r.split(".zip").next())
        .map(|r| format!("{}{r}.zip", consdump::KIND.prefix))
        .ok_or("the message names no dump")?;
    let file = run.file(&name);
    let mut out = std::fs::File::create(&file).map_err(|e| e.to_string())?;
    let mut werr = None;
    let got = b.stream_get(
        &format!("/qsmc/dumps/download?d=:0:{name}"),
        None,
        |bytes| {
            if let Err(e) = std::io::Write::write_all(&mut out, bytes) {
                werr = Some(e);
            }
        },
    )?;
    if let Some(e) = werr {
        return Err(e.to_string());
    }
    drop(out);
    if got.status != 200 || got.read == 0 {
        return Err(format!("the console's dump answered {}", got.status));
    }
    let path = file.to_string_lossy().into_owned();
    unzip(&["-tq", &path])?;
    let listed = String::from_utf8_lossy(&unzip(&["-Z1", &path])?).into_owned();
    let names: Vec<&str> = listed.lines().collect();
    let manifest = String::from_utf8_lossy(&unzip(&["-p", &path, "MANIFEST.txt"])?).into_owned();
    // Each entry the manifest names with its size is in the archive.
    let in_manifest: Vec<&str> = manifest
        .lines()
        .filter(|l| l.ends_with(" bytes") && l.starts_with("  "))
        .filter_map(|l| l.trim().split("  ").next())
        .collect();
    if in_manifest.is_empty() {
        return Err(format!("the manifest names nothing: {manifest}"));
    }
    if let Some(n) = in_manifest.iter().find(|n| !names.contains(n)) {
        return Err(format!(
            "the manifest names {n}, which is not in the archive"
        ));
    }
    for want in ["state/web.toml", "MANIFEST.txt"] {
        if !names.iter().any(|n| n.ends_with(want)) {
            return Err(format!("the dump lacks {want}: {names:?}"));
        }
    }
    // Each file of a secret is said to be left out, and nothing in the
    // archive holds one.
    for (n, _) in servicedump::take::LEFT_OUT.iter().chain(consdump::LEFT_OUT) {
        if names.iter().any(|e| e.ends_with(n)) {
            return Err(format!("the dump holds {n}"));
        }
    }
    for (n, _) in consdump::LEFT_OUT {
        if !manifest.contains(&format!("{n}: ")) {
            return Err(format!("the manifest does not say {n} was left out"));
        }
    }
    let ch = channel(&mut lab.c.guest)?;
    let mut secrets = vec![PASSWORD.to_string(), "PRIVATE KEY".into(), "$argon2".into()];
    let key = ch.text("/data/state/identity.key").unwrap_or_default();
    secrets.extend(
        key.lines()
            .filter(|l| l.len() > 32 && !l.starts_with("-----"))
            .map(str::to_string),
    );
    let systems = ch.text("/data/mgmt/systems.toml").unwrap_or_default();
    let pins: Vec<String> = systems
        .lines()
        .filter_map(|l| l.strip_prefix("fingerprint = "))
        .map(|v| v.trim_matches('"').to_string())
        .filter(|v| v.len() >= 32)
        .collect();
    if pins.is_empty() {
        return Err("systems.toml names no pin to look for".into());
    }
    secrets.extend(pins.iter().cloned());
    let all = unzip(&["-p", &path])?;
    let all = String::from_utf8_lossy(&all);
    if let Some(s) = secrets.iter().find(|s| all.contains(s.as_str())) {
        let shown = &s[..s.len().min(12)];
        return Err(format!("the dump holds a secret ({shown}...)"));
    }
    lab.c.guest.kill();
    Ok(format!(
        "{name} ({} bytes) taken from Manage dumps, downloaded, tested and listed by unzip: {} \
         entries, each the manifest names among them; webusers.toml, the web key, systems.toml, \
         identity.key and users.db left out and said so; no password, key, hash or the {} pins in \
         any entry",
        got.read,
        names.len(),
        pins.len()
    ))
}
