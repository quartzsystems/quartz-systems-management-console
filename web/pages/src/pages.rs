// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! The signed-on pages by route, each filled from the console's state.

use qsweb::http::Response;

use crate::app::routes::Rq;

/// `1 day 3:12`, `41 days 3:12` of `secs`.
pub fn uptime(secs: u64) -> String {
    let days = secs / 86_400;
    let h = secs % 86_400 / 3600;
    let m = secs % 3600 / 60;
    let d = if days == 1 { "day" } else { "days" };
    format!("{days} {d} {h}:{m:02}")
}

fn welcome(rq: &Rq<'_>) -> Response {
    let v = crate::welcome::Welcome {
        console: rq.app.console.name(),
        user: rq.user().into(),
        role: rq.role.label().into(),
        from: rq.from(),
        systems: [0; 6],
        partitions: [0; 6],
        events: 0,
        dumps: 0,
        updates: "0".into(),
        version: release::RELEASE.to_string(),
        build: release::BUILD_ID.into(),
        uptime: uptime(rq.app.console.uptime()),
        connected: "0 of 0".into(),
        messages: vec![qmcmsg::message(
            qsweb::msg::Kind::Info,
            &qmcmsg::NO_SYSTEMS,
            &[],
        )],
    };
    rq.page(crate::welcome::page(&rq.ctx, &v))
}

/// The first word of a status line (`10.0.2.15/24 on eth0, ...`), or
/// `none`.
pub fn address(line: &str) -> String {
    line.split_whitespace()
        .next()
        .filter(|a| a.contains('/'))
        .unwrap_or("none")
        .to_string()
}

fn console(rq: &Rq<'_>) -> Response {
    let app = rq.app;
    let s = app.settings();
    let host = app.host();
    let zone = host.zone();
    let now = i64::try_from(app.console.now()).unwrap_or(0);
    let when = format!(
        "{} {} {zone}",
        crate::fmt::date(now, zone, &s.date_format),
        crate::fmt::time(now, zone, &s.time_format, true)
    );
    let ntp = if host.ntp_servers.is_empty() {
        "none".to_string()
    } else {
        host.ntp_servers.join(", ")
    };
    let cert = app.certs.get().map_or_else(
        || "not made yet".to_string(),
        |c| {
            let i = c.shown().info;
            let name = i.common_name.clone().unwrap_or_else(|| i.subject.clone());
            let until = i64::try_from(i.not_after).unwrap_or(0);
            format!(
                "{name}, expires {}",
                crate::fmt::date(until, zone, &s.date_format)
            )
        },
    );
    let build = release::BUILD_ID;
    let v = crate::console::Settings {
        console: app.console.name(),
        values: vec![
            format!("{} users, 4 roles", app.users().users.len()),
            format!(
                "Public {}, private {}",
                address(&app.console.fact(release::fact::MANAGEMENT)),
                address(&app.console.fact(release::fact::PRIVATE))
            ),
            format!("{when}, NTP {ntp}"),
            cert,
            crate::backup::last(app, zone, &s.date_format),
            format!("{} {} build {build}", release::NAME, release::RELEASE),
            format!("Uptime {}", uptime(app.console.uptime())),
            format!("English (US), {}, {}", s.date_format, s.time_format),
            format!("{} minutes idle", s.idle_minutes),
            format!("{} build {build}", release::RELEASE),
        ],
        messages: Vec::new(),
    };
    rq.page(crate::console::page(&rq.ctx, &v))
}

/// The page for `route`, `None` when there is none.
pub fn serve(rq: &Rq<'_>, route: &str) -> Option<Response> {
    Some(match route {
        "/welcome" => welcome(rq),
        "/console" => console(rq),
        "/console/users" => crate::user_routes::users(rq),
        "/console/roles" => crate::user_routes::roles(rq),
        "/console/network" => crate::network_routes::network(rq),
        "/console/datetime" => crate::datetime_routes::datetime(rq),
        "/console/backup" => crate::backup_routes::backup(rq),
        "/confirm/restore" => crate::backup_routes::confirm(rq),
        "/console/certs" => crate::certs_routes::certs(rq),
        "/console/shutdown" => crate::shutdown_routes::shutdown(rq),
        "/console/locale" => crate::locale_routes::locale(rq),
        "/confirm/cert-new" => crate::certs_routes::renew(rq),
        "/confirm/ca-remove" => crate::certs_routes::remove(rq),
        "/confirm/user-delete" => crate::user_routes::delete(rq),
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    #[test]
    fn uptime_reads_as_the_prototype_s() {
        assert_eq!(
            super::uptime(41 * 86_400 + 3 * 3600 + 12 * 60),
            "41 days 3:12"
        );
        assert_eq!(super::uptime(86_400 + 59), "1 day 0:00");
    }
}
