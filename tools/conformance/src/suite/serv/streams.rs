// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! Suite 6: a stream that is not to be trusted, a file that is not what
//! its index says, a server absent or slow: each its message, nothing
//! changed. A download resumes. The download service is the one process
//! that connects outward, and only to the server it was given.

use std::time::Duration;

use sha2::{Digest, Sha256};

use super::{QSVS_IMAGE, VS, check, lab, qsvs_image, qsvs_release, row, status_is};
use crate::qemu::session::Session;
use crate::streams::keys::{stranger, test_key};
use crate::streams::server::Mode;
use crate::suite::base::channel;
use crate::suite::mgmt::guard::{connections, sockets};
use crate::suite::mgmt::{HOST, page, until};
use crate::suite::parts::expect;
use crate::suite::web::{message_of, shows};
use crate::web::Browser;

/// What the console keeps of the `qsvs` stream.
fn kept(guest: &mut Session) -> Result<Vec<u8>, String> {
    channel(guest)?.cat("/data/updates/qsvs/index")
}

/// Check now, which must show `id` and leave the kept index as it was.
fn refused(b: &mut Browser, guest: &mut Session, id: &str, what: &str) -> Result<String, String> {
    let was = kept(guest)?;
    let t = check(b)?;
    if !shows(&t, id) {
        return Err(format!("{what}: not {id} but {}", message_of(&t)));
    }
    if kept(guest)? != was {
        return Err(format!("{what} changed the kept index"));
    }
    let code = page(b, "/qsmc/updates/code")?;
    if !row(&code, VS).contains(super::QSVS_NEXT) {
        return Err(format!(
            "{what}: the host's row no longer offers {}",
            super::QSVS_NEXT
        ));
    }
    Ok(format!("{what}: {id}"))
}

/// [`refused`] for a server too slow for the page to wait on: Check now
/// posted by a browser of its own, its message read from the page after.
fn refused_slowly(
    b: &mut Browser,
    guest: &mut Session,
    id: &str,
    what: &str,
) -> Result<String, String> {
    let was = kept(guest)?;
    let mut other = b.clone_session();
    let token = b.csrf("/qsmc/updates/code")?;
    let posted = std::thread::spawn(move || {
        let _ = other.post(
            "/qsmc/updates/code",
            &[("csrf", &token), ("check_now", "Check now")],
        );
    });
    until(
        &format!("{what}: {id} shown"),
        Duration::from_secs(240),
        || Ok(shows(&page(b, "/qsmc/updates/code")?, id)),
    )?;
    let _ = posted.join();
    if kept(guest)? != was {
        return Err(format!("{what} changed the kept index"));
    }
    Ok(format!("{what}: {id}"))
}

/// Begin the host image's download from its link.
fn begin(b: &mut Browser) -> Result<(), String> {
    let a = b.get(&format!(
        "/qsmc/updates/code/download?s=qsvs&f={QSVS_IMAGE}"
    ))?;
    expect(b, &a, qmcmsg::DOWNLOAD_BEGUN.id).map(|_| ())
}

/// Wait for a stopped download's message `id` on Code updates.
fn stopped(b: &mut Browser, id: &str) -> Result<(), String> {
    until(
        &format!("the download stopped with {id}"),
        Duration::from_secs(120),
        || Ok(shows(&page(b, "/qsmc/updates/code")?, id)),
    )
}

/// Whether the host image is among the downloaded files.
fn downloaded(guest: &mut Session) -> Result<bool, String> {
    Ok(channel(guest)?
        .ls("/data/updates/qsvs")
        .unwrap_or_default()
        .iter()
        .any(|n| n == QSVS_IMAGE))
}

/// Every process's outbound TCP connections: (process, remote address,
/// remote port), the listeners' own left out.
fn outbound(guest: &mut Session) -> Result<Vec<(String, String, u16)>, String> {
    let ch = channel(guest)?;
    let status = ch.status()?;
    let tcp = ch.text("/proc/net/tcp")?;
    let mut out = Vec::new();
    for s in &status.services {
        if s.pid == 0 {
            continue;
        }
        let inodes = sockets(ch, s.pid).unwrap_or_default();
        for (local, addr, port) in connections(&tcp, &inodes) {
            if local == release::HTTPS_PORT || local == release::HTTP_PORT {
                continue;
            }
            out.push((s.name.clone(), addr, port));
        }
    }
    Ok(out)
}

/// Suite 6.
#[allow(
    clippy::too_many_lines,
    reason = "each refusal, the resume, the connections"
)]
pub fn streams(run: &crate::suite::base::Run<'_>) -> Result<String, String> {
    let mut lab = lab(run, "serv-streams", &[])?;
    let mut said = Vec::new();
    let s = &lab.streams;
    // An index whose signature is not a trusted key's.
    s.publish("qsvs", 3, &[qsvs_release()], &stranger());
    said.push(refused(
        &mut lab.c.browser,
        &mut lab.c.guest,
        qmcmsg::STREAM_NOT_SIGNED.id,
        "an index signed by another key",
    )?);
    // An older sequence, signed as it should be.
    lab.streams
        .publish("qsvs", 1, &[qsvs_release()], &test_key("qsvs"));
    said.push(refused(
        &mut lab.c.browser,
        &mut lab.c.guest,
        qmcmsg::STREAM_OLDER.id,
        "an index of an older sequence",
    )?);
    // A server that answers nothing, then one too slow to wait for.
    lab.streams
        .publish("qsvs", 3, &[qsvs_release()], &test_key("qsvs"));
    lab.streams.set_mode(Mode::Absent);
    said.push(refused(
        &mut lab.c.browser,
        &mut lab.c.guest,
        qmcmsg::STREAM_NOT_REACHED.id,
        "a server that answers nothing",
    )?);
    // Reached again, the message goes, so the slow server's is its own.
    lab.streams.set_mode(Mode::Normal);
    let t = check(&mut lab.c.browser)?;
    if shows(&t, qmcmsg::STREAM_NOT_REACHED.id) {
        return Err("the server reached again still shows QMC7204".into());
    }
    lab.streams.set_mode(Mode::Slow(Duration::from_secs(45)));
    said.push(refused_slowly(
        &mut lab.c.browser,
        &mut lab.c.guest,
        qmcmsg::STREAM_NOT_REACHED.id,
        "a server slower than the download service waits",
    )?);
    lab.streams.set_mode(Mode::Normal);
    let t = check(&mut lab.c.browser)?;
    if !shows(&t, qmcmsg::CHECKED.id) {
        return Err(format!("sequence 3 was not taken: {}", message_of(&t)));
    }
    // A file of the wrong hash, then of the wrong size: removed, with
    // their message and the console's problem.
    lab.streams.set_mode(Mode::Wrong);
    begin(&mut lab.c.browser)?;
    stopped(&mut lab.c.browser, qmcmsg::FILE_BAD.id)?;
    lab.streams.set_mode(Mode::Normal);
    if downloaded(&mut lab.c.guest)? {
        return Err("a file of the wrong hash was kept".into());
    }
    let path = format!("/qsvs/v1/file/{QSVS_IMAGE}");
    let mut short = qsvs_image();
    short.truncate(short.len() - 4096);
    lab.streams.put(&path, short);
    begin(&mut lab.c.browser)?;
    until("the short file refused", Duration::from_secs(120), || {
        let t = page(&mut lab.c.browser, "/qsmc/updates/code")?;
        Ok(shows(&t, qmcmsg::FILE_BAD.id) && !row(&t, VS).contains("Downloading"))
    })?;
    if downloaded(&mut lab.c.guest)? {
        return Err("a file of the wrong size was kept".into());
    }
    // The problem service opens it a moment after the code is posted.
    until(
        "a bad download's problem of the console's",
        Duration::from_secs(60),
        || Ok(page(&mut lab.c.browser, "/qsmc/events?rows=100")?.contains("A3007110")),
    )?;
    said.push(
        "a file of the wrong hash and one of the wrong size: QMC7205, removed, A3007110 open"
            .into(),
    );
    // A download cut short resumes from where it was.
    lab.streams.put(&path, qsvs_image());
    lab.streams.set_mode(Mode::CutOnce(512 * 1024));
    begin(&mut lab.c.browser)?;
    until("the cut download stopped", Duration::from_secs(120), || {
        let t = page(&mut lab.c.browser, "/qsmc/updates/code")?;
        Ok(!row(&t, VS).contains("Downloading"))
    })?;
    // While a slow resume runs, only the download service connects out.
    lab.streams.set_mode(Mode::Slow(Duration::from_millis(400)));
    begin(&mut lab.c.browser)?;
    std::thread::sleep(Duration::from_secs(3));
    let conns = outbound(&mut lab.c.guest)?;
    lab.streams.set_mode(Mode::Normal);
    status_is(
        &mut lab.c.browser,
        VS,
        "Downloaded",
        Duration::from_secs(300),
    )?;
    let ranged: Vec<String> = lab
        .streams
        .asked()
        .into_iter()
        .map(|(_, l)| l)
        .filter(|l| l.contains(QSVS_IMAGE) && l.contains(" range "))
        .collect();
    if ranged.is_empty() {
        return Err("the second download did not resume: no ranged request".into());
    }
    let (len, _) = channel(&mut lab.c.guest)?.sum(&format!("/data/updates/qsvs/{QSVS_IMAGE}"))?;
    let want = qsvs_image();
    if len != want.len() as u64 {
        return Err(format!(
            "the resumed file is {len} bytes, not {}",
            want.len()
        ));
    }
    said.push(format!(
        "a download cut after 512 KiB resumed ({}), whole and verified (sha256 {})",
        ranged[0],
        &hex::encode(Sha256::digest(&want))[..16]
    ));
    let fetching: Vec<_> = conns.iter().filter(|(n, _, _)| n == "fetchd").collect();
    if fetching.is_empty() {
        return Err(format!(
            "no connection of fetchd seen while it downloaded: {conns:?}"
        ));
    }
    let port = lab.streams.port();
    for (name, addr, p) in &conns {
        let ok = match name.as_str() {
            "fetchd" => addr == HOST && *p == port,
            "mgmtd" => addr == HOST && lab.fleet.ports.contains(p),
            _ => false,
        };
        if !ok {
            return Err(format!("{name} connects to {addr}:{p}"));
        }
    }
    said.push(format!(
        "while it downloaded, fetchd's connections were to {HOST}:{port} alone, mgmtd's to its \
         systems, no other process's outward"
    ));
    lab.c.guest.kill();
    Ok(said.join("; "))
}
