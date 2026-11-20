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

/// Managed systems by state: total, operating, power off, standby,
/// error, no connection; and partitions by state: total, running, not
/// activated, shutting down, error, starting (the strip's open
/// firmware). A standalone system is one partition, as on the
/// Partitions list.
pub fn counts(kept: &[mgmtproto::Kept]) -> ([u32; 6], [u32; 6]) {
    use qsi::host::model::PartitionState as P;
    let mut s = [0u32; 6];
    let mut p = [0u32; 6];
    for k in kept {
        s[0] += 1;
        let i = match k.status() {
            "Operating" => 1,
            "Power off" => 2,
            "Standby" => 3,
            "Error" => 4,
            _ => 5,
        };
        s[i] += 1;
        if k.kind == mgmtproto::kept::Kind::Standalone {
            p[0] += 1;
            match i {
                1 => p[1] += 1,
                4 => p[4] += 1,
                _ => p[2] += 1,
            }
            continue;
        }
        for part in &k.partitions {
            p[0] += 1;
            let j = match part.state {
                P::Running => 1,
                P::ShuttingDown => 3,
                P::Error => 4,
                P::Starting => 5,
                _ => 2,
            };
            p[j] += 1;
        }
    }
    (s, p)
}

fn welcome(rq: &Rq<'_>) -> Response {
    let mut messages = Vec::new();
    let kept = crate::systems::systems(rq).unwrap_or_else(|m| {
        messages.push(m);
        Vec::new()
    });
    let (systems, partitions) = counts(&kept);
    if kept.is_empty() && messages.is_empty() {
        messages.push(qmcmsg::message(
            qsweb::msg::Kind::Info,
            &qmcmsg::NO_SYSTEMS,
            &[],
        ));
    }
    let v = crate::welcome::Welcome {
        console: rq.app.console.name(),
        user: rq.user().into(),
        role: rq.role.label().into(),
        from: rq.from(),
        systems,
        partitions,
        events: 0,
        dumps: 0,
        updates: "0".into(),
        version: release::RELEASE.to_string(),
        build: release::BUILD_ID.into(),
        uptime: uptime(rq.app.console.uptime()),
        connected: format!(
            "{} of {}",
            systems[1] + systems[2] + systems[3] + systems[4],
            systems[0]
        ),
        messages,
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
        "/console/version" => crate::version_routes::version(rq),
        "/connectivity" => crate::systems::connectivity::route(rq),
        "/servers" => crate::systems::servers::route(rq),
        "/servers/add" => crate::systems::add::add(rq),
        "/servers/add/result" => crate::systems::add::result(rq),
        "/servers/reset" => crate::systems::add::reset(rq),
        "/confirm/system-remove" => crate::systems::add::remove(rq),
        // The pattern reference and its error page: a test image's.
        #[cfg(test_hooks)]
        crate::components::ROUTE => rq.page(crate::components::page(&rq.ctx)),
        #[cfg(test_hooks)]
        crate::components::ERROR_ROUTE => {
            crate::app::routes::error(rq, 404, &qmcmsg::NOT_FOUND, &["/qsmc/dev/example"])
        }
        "/confirm/cert-new" => crate::certs_routes::renew(rq),
        "/confirm/ca-remove" => crate::certs_routes::remove(rq),
        "/confirm/user-delete" => crate::user_routes::delete(rq),
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    #[test]
    fn welcome_counts_systems_and_partitions_by_state() {
        use mgmtproto::Kept;
        use mgmtproto::kept::{Kind, Link};
        use qsi::base::{PowerState, State};
        use qsi::host::model::PartitionState as P;
        use qsi::host::partitions::Status;
        let state = |power| {
            Some(State {
                power,
                refcode: None,
                release: String::new(),
                slot: String::new(),
                keylock: None,
            })
        };
        let part = |state| Status {
            id: 1,
            name: "P".into(),
            state,
            code: String::new(),
            text: String::new(),
            current: None,
            profile: String::new(),
            last_end: String::new(),
            message: None,
            ethernet: Vec::new(),
            channel: qsi::host::model::Channel::default(),
        };
        let host = Kept {
            link: Link::Connected,
            state: state(PowerState::Running),
            partitions: vec![part(P::Running), part(P::NotActivated), part(P::Starting)],
            ..Kept::default()
        };
        let standalone = Kept {
            link: Link::Connected,
            kind: Kind::Standalone,
            state: state(PowerState::Running),
            ..Kept::default()
        };
        let away = Kept::default();
        let (s, p) = super::counts(&[host, standalone, away]);
        assert_eq!(s, [3, 2, 0, 0, 0, 1]);
        assert_eq!(p, [4, 2, 1, 0, 0, 1]);
    }

    #[test]
    fn uptime_reads_as_the_prototype_s() {
        assert_eq!(
            super::uptime(41 * 86_400 + 3 * 3600 + 12 * 60),
            "41 days 3:12"
        );
        assert_eq!(super::uptime(86_400 + 59), "1 day 0:00");
    }
}
