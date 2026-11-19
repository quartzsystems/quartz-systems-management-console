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

/// The page for `route`, `None` when there is none.
pub fn serve(rq: &Rq<'_>, route: &str) -> Option<Response> {
    Some(match route {
        "/welcome" => welcome(rq),
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
