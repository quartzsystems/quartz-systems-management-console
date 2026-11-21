// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! Suite 6: an image of gigabytes streamed through the console to the
//! host while the console's memory and its disk's writes stay flat; one
//! cut midway leaves nothing; one said to be past 64 GiB is refused
//! before it is read; a slow host paces the browser.

use std::time::{Duration, Instant};

use super::{VS, expect, kept, lab};
use crate::qemu::session::Session;
use crate::suite::base::channel;
use crate::suite::mgmt::until;
use crate::suite::web::message_of;
use crate::web::stream::Upload;

const OPTICAL: &str = "/qsmc/server/QSHQVS1/virtual/optical";
/// The big image: several gigabytes.
const BIG: u64 = 3 << 30;
/// The paced image.
const PACED: u64 = 40 << 20;

/// What the console uses, at one time.
#[derive(Clone, Copy, Debug, Default)]
struct Use {
    /// The web server's resident memory, KiB.
    webd: u64,
    /// `mgmtd`'s, KiB.
    mgmtd: u64,
    /// The memory the kernel has to give, KiB.
    available: u64,
    /// Sectors written to every block device.
    written: u64,
}

/// A `/proc` file's field `key`'s number (`VmRSS:`, `MemAvailable:`).
fn kib(text: &str, key: &str) -> u64 {
    text.lines()
        .find(|l| l.starts_with(key))
        .and_then(|l| l.split_whitespace().nth(1))
        .and_then(|n| n.parse().ok())
        .unwrap_or(0)
}

fn sample(guest: &mut Session) -> Result<Use, String> {
    let ch = channel(guest)?;
    let status = ch.status()?;
    let rss = |ch: &mut crate::qemu::channel::Channel, name: &str| -> Result<u64, String> {
        let pid = status
            .service(name)
            .map(|s| s.pid)
            .ok_or(format!("no {name}"))?;
        Ok(kib(&ch.text(&format!("/proc/{pid}/status"))?, "VmRSS:"))
    };
    let webd = rss(ch, "webd")?;
    let mgmtd = rss(ch, "mgmtd")?;
    let available = kib(&ch.text("/proc/meminfo")?, "MemAvailable:");
    let written = ch
        .text("/proc/diskstats")?
        .lines()
        .filter_map(|l| {
            l.split_whitespace()
                .nth(9)
                .and_then(|n| n.parse::<u64>().ok())
        })
        .sum();
    Ok(Use {
        webd,
        mgmtd,
        available,
        written,
    })
}

/// Upload `u` while sampling the console every two seconds: the
/// answer's page, the most each grew, and the time it took.
fn watched(
    lab: &mut super::Lab<'_>,
    u: Upload,
) -> Result<(Option<crate::web::Answer>, Use, Duration), String> {
    let before = sample(&mut lab.c.guest)?;
    let sender = lab.c.browser.clone();
    let guest = &mut lab.c.guest;
    let t0 = Instant::now();
    let (answer, peak) = std::thread::scope(|s| {
        let h = s.spawn(move || sender.stream_upload(OPTICAL, &u));
        let mut peak = Use::default();
        let mut low = before.available;
        while !h.is_finished() {
            if let Ok(now) = sample(guest) {
                peak.webd = peak.webd.max(now.webd.saturating_sub(before.webd));
                peak.mgmtd = peak.mgmtd.max(now.mgmtd.saturating_sub(before.mgmtd));
                low = low.min(now.available);
            }
            std::thread::sleep(Duration::from_secs(2));
        }
        peak.available = before.available.saturating_sub(low);
        (
            h.join()
                .map_err(|_| "the upload's thread panicked".to_string()),
            peak,
        )
    });
    let took = t0.elapsed();
    let after = sample(&mut lab.c.guest)?;
    let mut peak = peak;
    peak.written = after.written.saturating_sub(before.written);
    Ok((answer??, peak, took))
}

fn upload(name: &str, bytes: u64, csrf: &str) -> Upload {
    Upload {
        csrf: csrf.into(),
        name: name.into(),
        bytes,
        announce: None,
        cut: None,
        head_only: false,
        label: name.trim_end_matches(".iso").into(),
    }
}

/// Suite 6.
#[allow(clippy::too_many_lines, reason = "four uploads")]
pub fn upload_image(run: &crate::suite::base::Run<'_>) -> Result<String, String> {
    let mut lab = lab(run, "parts-upload")?;
    let csrf = lab.c.browser.csrf(OPTICAL)?;
    // Several gigabytes, streamed.
    let (a, grew, took) = watched(&mut lab, upload("BIG.iso", BIG, &csrf))?;
    let a = a.ok_or("the big upload had no answer")?;
    expect(&mut lab.c.browser, &a, "QMC2401")?;
    let up = lab.fleet.tell(0, "uploaded");
    let images = kept(&lab.fleet, "images");
    if !up.starts_with(&format!("BIG.iso {BIG} ")) || !up.ends_with(" ok") {
        return Err(format!("the host's upload: {up}"));
    }
    if !images.contains(&format!("BIG.iso BIG bytes {BIG}")) {
        return Err(format!("the image is not in the library: {images}"));
    }
    let mib = |k: u64| k / 1024;
    if grew.webd > 64 * 1024 || grew.mgmtd > 64 * 1024 || grew.available > 256 * 1024 {
        return Err(format!(
            "memory grew during the upload: webd {} MiB, mgmtd {} MiB, available fell {} MiB",
            mib(grew.webd),
            mib(grew.mgmtd),
            mib(grew.available)
        ));
    }
    let written = grew.written * 512;
    if written > BIG / 16 {
        return Err(format!(
            "{} MiB were written to the console's disks during the upload",
            written >> 20
        ));
    }
    let rate = (BIG / took.as_secs().max(1)) >> 20;
    // Cut midway: nothing kept at the host.
    let mut cut = upload("CUT.iso", 512 << 20, &csrf);
    cut.cut = Some(200 << 20);
    let answer = lab.c.browser.stream_upload(OPTICAL, &cut)?;
    if answer.is_some() {
        return Err("a cut upload was answered as if whole".into());
    }
    until(
        "the cut upload ended at the host",
        Duration::from_secs(60),
        || {
            let u = lab.fleet.tell(0, "uploaded");
            Ok(u.starts_with("CUT.iso ") && u.contains("QSI0702"))
        },
    )?;
    if kept(&lab.fleet, "images").contains("CUT.iso") {
        return Err("the cut image is in the library".into());
    }
    let cut_end = lab.fleet.tell(0, "uploaded");
    // Past 64 GiB: refused from the parts before the file.
    let mut huge = upload("HUGE.iso", 0, &csrf);
    huge.announce = Some(65 << 30);
    huge.head_only = true;
    let t0 = Instant::now();
    let a = lab
        .c
        .browser
        .stream_upload(OPTICAL, &huge)?
        .ok_or("the large upload had no answer")?;
    let quick = t0.elapsed();
    let t = expect(&mut lab.c.browser, &a, "QMC2404")?;
    if super::host(&lab.fleet)
        .lines
        .iter()
        .any(|l| l.contains("HUGE.iso"))
    {
        return Err("the host was asked for the image past the limit".into());
    }
    // Paced by a slow host: the browser waits, nothing grows.
    lab.fleet.tell(0, "upload-rate 4000000");
    let (a, slow, paced) = watched(&mut lab, upload("PACED.iso", PACED, &csrf))?;
    lab.fleet.tell(0, "upload-rate off");
    expect(
        &mut lab.c.browser,
        &a.ok_or("the paced upload had no answer")?,
        "QMC2401",
    )?;
    if paced < Duration::from_secs(7) {
        return Err(format!(
            "40 MiB at 4 MB/s took {} s: the console did not wait on the host",
            paced.as_secs()
        ));
    }
    if slow.webd > 32 * 1024 || slow.mgmtd > 32 * 1024 {
        return Err(format!(
            "memory grew while paced: webd {} MiB, mgmtd {} MiB",
            mib(slow.webd),
            mib(slow.mgmtd)
        ));
    }
    let status = channel(&mut lab.c.guest)?.status()?;
    let failures: u32 = status
        .services
        .iter()
        .filter(|s| s.name == "webd" || s.name == "mgmtd")
        .map(|s| s.failures)
        .sum();
    if failures != 0 {
        return Err(format!("the services failed {failures} times"));
    }
    lab.c.guest.kill();
    Ok(format!(
        "a 3 GiB image streamed to {VS} in {} s ({rate} MiB/s), in its library (QMC2401), while \
         webd grew {} MiB, mgmtd {} MiB, available memory fell {} MiB and {} MiB were written to \
         the console's disks; one cut after 200 MiB left nothing (`{cut_end}`); one said to be \
         65 GiB was refused in {} ms from its first parts ({}); 40 MiB at the host's 4 MB/s took \
         {} s with webd growing {} MiB; webd and mgmtd never failed",
        took.as_secs(),
        mib(grew.webd),
        mib(grew.mgmtd),
        mib(grew.available),
        written >> 20,
        quick.as_millis(),
        message_of(&t),
        paced.as_secs(),
        mib(slow.webd)
    ))
}
