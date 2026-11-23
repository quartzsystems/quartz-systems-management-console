// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! Managed systems as hostile (`docs/SECURITY.md`), beside `mgmt-guard`'s
//! nonsense, oversized frames, floods and withheld credit: a system that
//! names things in markup, lies in its hello, sends console screens that
//! are wrong, and answers slowly. Nothing it sends is shown unescaped,
//! no page about the others waits on it, and no service notices.

use std::time::{Duration, Instant};

use crate::suite::base::channel;
use crate::suite::mgmt::until;
use crate::suite::parts::{Lab, lab};
use crate::web::Browser;

/// What would be markup if it were not escaped.
const RAW: [&str; 4] = [
    "<script>alert",
    "<img src=x",
    "<b>1</b>",
    "V99R99M99<script>",
];

/// `path` answers 200 with none of [`RAW`] in it.
fn shown_as_text(b: &mut Browser, path: &str) -> Result<String, String> {
    let a = b.get(path)?;
    let t = a.text();
    if a.status != 200 {
        return Err(format!("{path} answered {}", a.status));
    }
    if let Some(raw) = RAW.iter().find(|r| t.contains(*r)) {
        return Err(format!("{path} shows {raw:?} unescaped"));
    }
    Ok(t)
}

/// A system's markup is shown as text on every page that names it.
fn markup(lab: &mut Lab<'_>) -> Result<String, String> {
    lab.fleet.tell(0, "markup");
    let b = &mut lab.c.browser;
    until("the partitions' new names", Duration::from_secs(90), || {
        Ok(b.get("/qsmc/partitions")?
            .text()
            .contains("&lt;b&gt;1&lt;/b&gt;"))
    })?;
    let pages = [
        "/qsmc/welcome",
        "/qsmc/servers",
        "/qsmc/partitions",
        "/qsmc/events",
        "/qsmc/refcodes?ms=QSHQVS1",
        "/qsmc/server/QSHQVS1/general",
        "/qsmc/server/QSHQVS1/refcode",
        "/qsmc/partition/QSHQVS1/1/general",
        "/qsmc/partition/QSHQVS1/1/panel",
        "/qsmc/terminal/QSHQVS1/1",
        "/qsmc/connectivity",
    ];
    for p in pages {
        shown_as_text(b, p)?;
    }
    let pic = b.get("/qsmc/terminal/QSHQVS1/1/screen.png")?;
    if pic.status != 200 {
        return Err(format!("the screen's picture answered {}", pic.status));
    }
    Ok(format!(
        "names, a model, a code and console cells of markup shown as text on {} pages",
        pages.len()
    ))
}

/// A hello that lies is taken for what it is: the session is made
/// again, nothing of it is shown unescaped, and no capability it denies
/// is asked of it. The release a page shows is the system's status, not
/// its hello.
fn lies(lab: &mut Lab<'_>) -> Result<String, String> {
    lab.fleet.tell(1, "lie");
    // Its old session closes; the next is made with the lying hello.
    std::thread::sleep(Duration::from_secs(5));
    let b = &mut lab.c.browser;
    until(
        "the lying system connected again",
        Duration::from_secs(90),
        || {
            let t = b.get("/qsmc/connectivity")?.text();
            Ok(crate::suite::serv::row(&t, "QSHQ400").contains(">Connected<"))
        },
    )?;
    for p in [
        "/qsmc/servers",
        "/qsmc/server/QSHQ400/general",
        "/qsmc/welcome",
    ] {
        shown_as_text(b, p)?;
    }
    Ok("a hello of another release, no capability and a build of markup shown as text".into())
}

/// A console screen that is wrong is drawn as far as it goes, or not.
fn bad_screen(lab: &mut Lab<'_>) -> Result<String, String> {
    lab.fleet.tell(0, "bad-screen");
    let b = &mut lab.c.browser;
    let at = Instant::now();
    let pic = b.get("/qsmc/terminal/QSHQVS1/1/screen.png")?;
    if pic.status >= 500 && pic.status != 503 {
        return Err(format!(
            "the wrong screen's picture answered {}",
            pic.status
        ));
    }
    shown_as_text(b, "/qsmc/terminal/QSHQVS1/1")?;
    Ok(format!(
        "a screen of the wrong size, its cursor off the grid, answered {} in {} ms",
        pic.status,
        at.elapsed().as_millis()
    ))
}

/// While one system answers slowly, no page about the others waits.
fn slow(lab: &mut Lab<'_>) -> Result<String, String> {
    for family in [
        "host",
        "partitions",
        "partition",
        "profiles",
        "vres",
        "refcodes",
        "panel",
    ] {
        lab.fleet.tell(0, &format!("slow {family} 60000"));
    }
    let b = &mut lab.c.browser;
    let mut slowest = Duration::ZERO;
    for p in [
        "/qsmc/welcome",
        "/qsmc/servers",
        "/qsmc/partitions",
        "/qsmc/events",
        "/qsmc/connectivity",
        "/qsmc/server/QSHQ400/general",
    ] {
        let at = Instant::now();
        let a = b.get(p)?;
        let took = at.elapsed();
        if a.status != 200 || took > Duration::from_secs(5) {
            return Err(format!(
                "{p} answered {} after {took:?} beside a slow system",
                a.status
            ));
        }
        slowest = slowest.max(took);
    }
    lab.fleet.tell(0, "allow");
    Ok(format!(
        "with QSHQVS1 a minute slow, the other pages answered within {} ms",
        slowest.as_millis()
    ))
}

/// The suite.
pub fn systems(run: &crate::suite::base::Run<'_>) -> Result<String, String> {
    let mut lab = lab(run, "sec-systems")?;
    let before = channel(&mut lab.c.guest)?.status()?;
    let mut said = vec![markup(&mut lab)?];
    said.push(lies(&mut lab)?);
    said.push(bad_screen(&mut lab)?);
    said.push(slow(&mut lab)?);
    let after = channel(&mut lab.c.guest)?.status()?;
    for s in &after.services {
        let was = before.service(&s.name).map_or(0, |b| b.failures);
        if s.failures != was || s.state != "ready" {
            return Err(format!("{} noticed: {:?}", s.name, s));
        }
    }
    lab.c.guest.kill();
    Ok(said.join("; "))
}
