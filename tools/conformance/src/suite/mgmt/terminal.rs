// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! Suite 8: the terminal window against the simulator. The picture of
//! the fixture's screen is the prototype's `5250-24x80.png` byte for
//! byte; keys typed reach the system, posted or on the socket; a Viewer
//! cannot type; two browsers see one screen; a window outlives its web
//! session only with the option on; the disconnected page says why.

use std::time::Duration;

use super::{Fleet, keep, page, until};
use crate::suite::web::{ADMIN, PASSWORD, follow, message_of, shows, signed_on, start, submit};

/// The prototype's picture of the fixture's screen.
const GOLDEN: &[u8] =
    include_bytes!("../../../../../design/qsmc-prototype/shared/img/5250-24x80.png");

const WINDOW: &str = "/qsmc/terminal/QSHQVS1/1";

fn keys(fleet: &Fleet) -> String {
    fleet.tell(0, "keys 1")
}

/// Suite 8.
#[allow(clippy::too_many_lines, reason = "one scenario, step by step")]
pub fn terminal(run: &crate::suite::base::Run<'_>) -> Result<String, String> {
    let fleet = Fleet::start(vec![sim::fixture::qshqvs1()])?;
    let mut c = start(run, "mgmt-terminal")?;
    let b = &mut c.browser;
    signed_on(b, ADMIN, PASSWORD)?;
    keep(b, &fleet, 0)?;
    // The window, and its picture.
    let w = b.get(WINDOW)?;
    if w.status != 200 || !w.text().contains("data-socket") {
        return Err(format!("the window answered {}", w.status));
    }
    let png = b.get(&format!("{WINDOW}/screen.png?scale=1"))?;
    if png.body != GOLDEN {
        return Err(format!(
            "the picture differs from the prototype's: {} bytes, the golden {}",
            png.body.len(),
            GOLDEN.len()
        ));
    }
    // A key posted from the keypad reaches the system.
    submit(b, WINDOW, &[("k", "F3")])?;
    until("F3 at the system", Duration::from_secs(10), || {
        Ok(keys(&fleet).contains("Function(3)"))
    })?;
    // The socket: the screen, then typed text reaches the system and
    // both windows see it.
    let mut one = b.websocket(&format!("{WINDOW}/ws"))?;
    let first = one.recv(Duration::from_secs(5));
    if !first.iter().any(|m| m.contains("\"t\":\"screen\"")) {
        return Err(format!("the socket sent no screen: {first:?}"));
    }
    let mut other = b.fresh();
    signed_on(&mut other, ADMIN, PASSWORD)?;
    let mut two = other.websocket(&format!("{WINDOW}/ws"))?;
    let seen = two.recv(Duration::from_secs(5));
    let screen = |ms: &[String]| {
        ms.iter()
            .find(|m| m.contains("\"t\":\"screen\""))
            .map(|m| m.split("\"cells\"").nth(1).unwrap_or("").to_string())
    };
    if screen(&first) != screen(&seen) {
        return Err("two browsers were sent different screens".into());
    }
    one.send("{\"t\":\"type\",\"s\":\"QZ\"}")?;
    until("typed text at the system", Duration::from_secs(10), || {
        Ok(keys(&fleet).contains("Char('Q')") && keys(&fleet).contains("Char('Z')"))
    })?;
    let echoed = two.recv(Duration::from_secs(3));
    if !echoed
        .iter()
        .any(|m| m.contains("\"t\":\"delta\"") || m.contains("\"t\":\"screen\""))
    {
        return Err("the other browser did not see the typing".into());
    }
    // A Viewer sees the window and cannot type.
    submit(
        b,
        "/qsmc/console/users",
        &[
            ("id", "looker"),
            ("role", "Viewer"),
            ("password", "Look-only-42"),
            ("again", "Look-only-42"),
            ("create", "OK"),
        ],
    )?;
    let mut viewer = b.fresh();
    signed_on(&mut viewer, "looker", "Look-only-42")?;
    let vw = page(&mut viewer, WINDOW)?;
    if !vw
        .contains("value=\"F1\" style=\"width:54px;height:22px;font-size:8pt;padding:0\" disabled")
    {
        return Err("the Viewer's keypad is not disabled".into());
    }
    let before = keys(&fleet);
    let refused = submit(&mut viewer, WINDOW, &[("k", "F5")])?;
    let mut vws = viewer.websocket(&format!("{WINDOW}/ws"))?;
    let _ = vws.recv(Duration::from_secs(2));
    vws.send("{\"t\":\"key\",\"k\":\"Enter\"}")?;
    std::thread::sleep(Duration::from_secs(2));
    if refused.status != 403 || keys(&fleet) != before {
        return Err(format!(
            "the Viewer's keys reached the system ({})",
            refused.status
        ));
    }
    // Signing off ends a window unless the option keeps it open.
    other.get("/qsmc/signoff")?;
    let after = two.recv(Duration::from_secs(5));
    if !after.iter().any(|m| m.contains("\"t\":\"closed\"")) && !two.closed {
        return Err("a window outlived its session with the option off".into());
    }
    submit(
        b,
        "/qsmc/console/locale",
        &[
            ("l", "English (US)"),
            ("df", "MM/DD/YY"),
            ("tf", "24-hour"),
            ("nf", "1,234.56"),
            ("to", "30"),
            ("max", "12"),
            ("tw", "on"),
            ("apply", "Apply"),
        ],
    )?;
    let mut third = b.fresh();
    signed_on(&mut third, ADMIN, PASSWORD)?;
    let mut kept = third.websocket(&format!("{WINDOW}/ws"))?;
    let _ = kept.recv(Duration::from_secs(3));
    third.get("/qsmc/signoff")?;
    let _ = kept.recv(Duration::from_secs(5));
    kept.send("{\"t\":\"type\",\"s\":\"K\"}")?;
    until(
        "a key from the window kept open",
        Duration::from_secs(10),
        || Ok(keys(&fleet).contains("Char('K')")),
    )?;
    if kept.closed {
        return Err("the window kept open was closed".into());
    }
    // The console lost: the window says why.
    fleet.tell(0, "drop");
    std::thread::sleep(Duration::from_secs(2));
    let gone = page(b, WINDOW)?;
    let gone = if shows(&gone, "QMC4001") {
        gone
    } else {
        page(b, WINDOW)?
    };
    if !shows(&gone, "QMC4001") && !gone.contains("Not connected") {
        return Err(format!("the disconnected window: {}", message_of(&gone)));
    }
    let _ = follow;
    c.guest.kill();
    Ok(format!(
        "the picture of the fixture's screen is the prototype's 5250-24x80.png, {} bytes, byte \
         for byte; F3 posted and text typed on the socket reached the system; two browsers were \
         sent the same screen and both saw the typing; a Viewer's keypad is disabled, its post \
         403 and its socket's keys dropped; signing off closed a window with the option off and \
         not with it on; a lost console shows QMC4001 with why",
        GOLDEN.len()
    ))
}
