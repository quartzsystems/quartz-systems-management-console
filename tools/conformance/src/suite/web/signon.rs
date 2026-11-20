// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! Suite 5: the sign-on rules from outside. The cookie's attributes; a
//! form without its token refused; five wrong passwords lock an ID; an
//! address's attempts limited; the idle timeout; a session not usable
//! after sign off. Suite 6: the last enabled Administrator is kept; a
//! disabled user's session ends.

use std::time::Duration;

use webusers::Role;
use webusers::sign::{MAX_FAILURES, PER_ADDRESS, WINDOW_SECS};

use super::{ADMIN, PASSWORD, follow, message_of, shows, signed_on, start, submit};
use crate::web::Browser;

fn make(b: &mut Browser, id: &str, role: Role, pw: &str) -> Result<(), String> {
    let r = submit(
        b,
        "/qsmc/console/users",
        &[
            ("id", id),
            ("role", role.label()),
            ("password", pw),
            ("again", pw),
            ("create", "OK"),
        ],
    )?;
    let page = follow(b, &r)?.text();
    if shows(&page, "QMC8001") {
        Ok(())
    } else {
        Err(format!("{id} was not made: {}", message_of(&page)))
    }
}

/// The page a session that ended answers with.
fn ended(b: &mut Browser) -> Result<bool, String> {
    let a = b.get("/qsmc/welcome")?;
    let page = follow(b, &a)?.text();
    Ok(shows(&page, "QMC9006"))
}

/// The suite of the sign-on rules.
pub fn rules(run: &super::Run<'_>) -> Result<String, String> {
    let mut c = start(run, "web-signon")?;
    let admin = &mut c.browser;
    let r = admin.sign_on(ADMIN, PASSWORD)?;
    let cookie = r.header("set-cookie").unwrap_or("").to_string();
    for want in ["HttpOnly", "Secure", "SameSite=Strict", "Path=/qsmc"] {
        if !cookie.contains(want) {
            return Err(format!("the cookie lacks {want}: {cookie}"));
        }
    }
    // A form without its token changes nothing.
    let r = admin.post("/qsmc/console/locale", &[("to", "1"), ("apply", "Apply")])?;
    if r.status != 403 || !shows(&r.text(), "QMC9011") {
        return Err(format!("a form without its token answered {}", r.status));
    }
    let pw = "Locked-out-pass-3";
    make(admin, "lockme", Role::Viewer, pw)?;
    let mut other = admin.fresh();
    let mut attempts = 0;
    for n in 1..=MAX_FAILURES {
        attempts += 1;
        let page = other.sign_on("lockme", "not the password")?.text();
        // The last of them locks the ID, and may say so at once.
        let said = shows(&page, "QMC9001") || (n == MAX_FAILURES && shows(&page, "QMC9002"));
        if !said {
            return Err(format!("wrong password {n} said: {}", message_of(&page)));
        }
    }
    attempts += 1;
    let page = other.sign_on("lockme", pw)?.text();
    if !shows(&page, "QMC9002") {
        return Err(format!(
            "after {MAX_FAILURES} wrong passwords the right one said: {}",
            message_of(&page)
        ));
    }
    // The address's attempts: the next past the limit is refused.
    let mut limited = false;
    while attempts <= PER_ADDRESS {
        attempts += 1;
        let page = other.sign_on("nobody", "nothing at all")?.text();
        limited = shows(&page, "QMC9004");
        if limited {
            break;
        }
    }
    if !limited {
        return Err(format!(
            "{attempts} attempts from one address were not limited"
        ));
    }
    // An idle minute ends a session.
    let r = submit(
        admin,
        "/qsmc/console/locale",
        &[
            ("l", "English (US)"),
            ("df", "MM/DD/YY"),
            ("tf", "24-hour"),
            ("nf", "1,234.56"),
            ("to", "1"),
            ("max", "12"),
            ("apply", "Apply"),
        ],
    )?;
    if !shows(&r.text(), "QMC8600") {
        return Err(format!(
            "the idle timeout was not set: {}",
            message_of(&r.text())
        ));
    }
    std::thread::sleep(Duration::from_secs(WINDOW_SECS.max(65)));
    if !ended(admin)? {
        return Err("a session idle for a minute did not end".into());
    }
    // Signed off, the cookie is no session.
    let mut b = admin.fresh();
    signed_on(&mut b, ADMIN, PASSWORD)?;
    let kept = b.cookie.clone();
    let off = b.get("/qsmc/signoff")?;
    if !shows(&follow(&mut b, &off)?.text(), "QMC9005") {
        return Err("sign off did not say so".into());
    }
    b.cookie = kept;
    if !ended(&mut b)? {
        return Err("a session was usable after sign off".into());
    }
    let _ = super::super::base::power_off(&mut c.guest);
    Ok(format!(
        "the cookie is {}; a form without its token is refused (QMC9011); {MAX_FAILURES} wrong \
         passwords lock the ID (QMC9002); attempt {attempts} from one address refused (QMC9004); \
         an idle minute ends a session and signing off ends it at once (QMC9006)",
        cookie
            .split(';')
            .skip(1)
            .map(str::trim)
            .collect::<Vec<_>>()
            .join("; ")
    ))
}

/// The suite of the last Administrator and a disabled user.
pub fn users(run: &super::Run<'_>) -> Result<String, String> {
    let mut c = start(run, "web-users")?;
    let admin = &mut c.browser;
    signed_on(admin, ADMIN, PASSWORD)?;
    // The last enabled Administrator: not disabled, lowered or deleted.
    for (what, fields) in [
        (
            "disabled",
            vec![
                ("id", ADMIN),
                ("role", "Administrator"),
                ("properties", "OK"),
            ],
        ),
        (
            "given another role",
            vec![
                ("id", ADMIN),
                ("role", "Operator"),
                ("enabled", "on"),
                ("properties", "OK"),
            ],
        ),
    ] {
        let r = submit(admin, "/qsmc/console/users", &fields)?;
        let page = follow(admin, &r)?.text();
        if !shows(&page, "QMC8010") {
            return Err(format!(
                "the last Administrator was {what}: {}",
                message_of(&page)
            ));
        }
    }
    let r = submit(
        admin,
        "/qsmc/confirm/user-delete?u=admin",
        &[("u", ADMIN), ("ok", "OK")],
    )?;
    if !shows(&follow(admin, &r)?.text(), "QMC8010") {
        return Err("the last Administrator was deleted".into());
    }
    // A disabled user's session ends at its next request.
    let pw = "Shift-worker-pass-55";
    make(admin, "oper", Role::Operator, pw)?;
    let mut oper = admin.fresh();
    signed_on(&mut oper, "oper", pw)?;
    if oper.get("/qsmc/welcome")?.status != 200 {
        return Err("the operator's session does not work".into());
    }
    let r = submit(
        admin,
        "/qsmc/console/users",
        &[("id", "oper"), ("role", "Operator"), ("properties", "OK")],
    )?;
    if !shows(&follow(admin, &r)?.text(), "QMC8006") {
        return Err("the operator was not disabled".into());
    }
    if !ended(&mut oper)? {
        return Err("a disabled user's session went on".into());
    }
    let page = oper.sign_on("oper", pw)?.text();
    if !shows(&page, "QMC9003") {
        return Err(format!("a disabled user signed on: {}", message_of(&page)));
    }
    let _ = super::super::base::power_off(&mut c.guest);
    Ok(
        "the last enabled Administrator was not disabled, given another role or deleted \
        (QMC8010); a disabled Operator's session ended at its next request and it could not \
        sign on again (QMC9003)"
            .into(),
    )
}
