// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! Date and time (`console-datetime.html`): the clock, the display
//! offset (the page's time zone: `host.toml` keeps an offset from UTC),
//! the time servers or the clock set by hand, and how the time servers
//! answer. Serving time to the managed systems is not in V1.

use qsweb::frame::{Ctx, Page};
use qsweb::html::esc;
use qsweb::msg::Message;
use qsweb::partials::{Button, Prop, PropGroup, checkbox, input, props, radio, select, status};

/// What Date and time shows.
#[derive(Clone, Debug, Default)]
pub struct DateTime {
    /// The date and time now, with the zone.
    pub now: String,
    /// The zones offered.
    pub zones: Vec<String>,
    /// The zone in use.
    pub zone: String,
    /// Whether the time servers set the clock.
    pub from_servers: bool,
    /// The time servers, comma-separated.
    pub servers: String,
    /// The date to set by hand.
    pub date: String,
    /// The time to set by hand.
    pub time: String,
    /// The servers' status word and whether it is good.
    pub status: (String, Option<bool>),
    /// What follows the word: the last synchronization.
    pub detail: String,
    /// Whether time is served to the managed systems.
    pub serve: bool,
    /// The message rows.
    pub messages: Vec<Message>,
}

/// The page.
pub fn page(ctx: &Ctx, v: &DateTime) -> Page {
    let zones: Vec<&str> = v.zones.iter().map(String::as_str).collect();
    let from = format!(
        "{} NTP servers: {}<br>{} Manually: {} {}",
        radio("s", "ntp", v.from_servers),
        input("ntp", &v.servers, 40),
        radio("s", "manual", !v.from_servers),
        input("md", &v.date, 8),
        input("mt", &v.time, 5)
    );
    let group = PropGroup {
        title: None,
        rows: vec![
            Prop::new("Current date/time", esc(&v.now)),
            Prop::new("Time zone", select("tz", &zones, &v.zone)),
            Prop::new("Set time from", from),
            Prop::new(
                "NTP status",
                format!("{}{}", status(&v.status.0, v.status.1), esc(&v.detail)),
            ),
            Prop::new(
                "Serve time to managed systems",
                format!(
                    "{} Act as NTP server on the private management network",
                    checkbox("serve", v.serve)
                ),
            ),
        ],
    };
    let body = props(
        ctx,
        &[group],
        &[
            Button::new("apply", "Apply"),
            Button::new("ok", "OK"),
            Button::new("cancel", "Cancel"),
        ],
    );
    Page {
        title: "Date and time".into(),
        crumb: vec!["Console Management".into(), "Date and time".into()],
        h1: "Date and time".into(),
        messages: v.messages.clone(),
        body,
        help: "datetime".into(),
        nav: Some(("Console Management", Some("/console/datetime"))),
        ..Page::default()
    }
}

/// The display offsets offered: each half hour from UTC-12:00 to
/// UTC+14:00, and the three that are quarter hours.
pub fn zones() -> Vec<String> {
    let mut minutes: Vec<i32> = (-24..=28).map(|h| h * 30).collect();
    minutes.extend([345, 525, 765]);
    minutes.sort_unstable();
    minutes
        .into_iter()
        .map(|m| appliance::time::Zone { minutes: m }.to_string())
        .collect()
}
