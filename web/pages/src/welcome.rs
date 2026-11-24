// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! Welcome (`welcome.html`): the sections and the status strip. The
//! managed systems and partitions are counted from what `mgmtd` keeps,
//! a standalone system as one partition as the Partitions list has it;
//! the service counts are zero until Phase 6.

use std::fmt::Write as _;

use qsweb::frame::{Ctx, Page};
use qsweb::html::esc;
use qsweb::msg::Message;

/// What Welcome shows.
#[derive(Clone, Debug, Default)]
pub struct Welcome {
    /// The console's name.
    pub console: String,
    /// The signed-on user, its role and where from.
    pub user: String,
    /// Its role's name.
    pub role: String,
    /// The address it came from.
    pub from: String,
    /// Managed systems: total, operating, power off, standby, error,
    /// no connection.
    pub systems: [u32; 6],
    /// Partitions: total, running, not activated, shutting down, error,
    /// open firmware.
    pub partitions: [u32; 6],
    /// Open serviceable events.
    pub events: u32,
    /// Dumps awaiting action.
    pub dumps: u32,
    /// Updates available, as the strip says it.
    pub updates: String,
    /// `V1R0M0`.
    pub version: String,
    /// The build.
    pub build: String,
    /// How long the console has been up.
    pub uptime: String,
    /// Systems connected, `2 of 2`.
    pub connected: String,
    /// The message rows.
    pub messages: Vec<Message>,
}

/// A count: `class` around it when it is not zero.
fn count(n: u32, class: Option<&str>) -> String {
    match class {
        Some(c) if n > 0 => format!("<span class=\"{c}\">{n}</span>"),
        _ => n.to_string(),
    }
}

fn status(rows: &[(&str, String)]) -> String {
    let mut t = String::from("<table class=\"status\" cellspacing=\"0\">");
    for (label, value) in rows {
        let _ = write!(t, "<tr><td class=\"lbl\">{label}</td><td>{value}</td></tr>");
    }
    t.push_str("</table>");
    t
}

/// The page.
#[allow(clippy::too_many_lines, reason = "one page, top to bottom")]
pub fn page(ctx: &Ctx, v: &Welcome) -> Page {
    let url = |r: &str| ctx.url(r);
    let sections: [(&str, &str, &str); 5] = [
        (
            "/partitions",
            "Systems Management",
            "Managed systems and partitions: power, activation, profiles, virtual resources, terminal windows.",
        ),
        (
            "/plans",
            "System Plans",
            "Capture a managed system's configuration as a plan; deploy plans to new hosts.",
        ),
        (
            "/console",
            "Console Management",
            "Users, network, date and time, certificates, backup, updates and shutdown of this console.",
        ),
        (
            "/events",
            "Service Management",
            "Serviceable events, reference code history, dumps, code updates and connectivity.",
        ),
        (
            "/updates",
            "Updates",
            "Current and available code levels across the fleet.",
        ),
    ];
    let mut body = String::from("<table class=\"home\" cellspacing=\"0\">");
    for (route, title, text) in sections {
        let _ = write!(
            body,
            "<tr><td><a class=\"big\" href=\"{}\">{title}</a><br><span class=\"small grey\">{}</span></td></tr>",
            url(route),
            esc(text)
        );
    }
    body.push_str("</table>");
    let [s_total, s_op, s_off, s_standby, s_err, s_none] = v.systems;
    let systems = status(&[
        (
            "Total",
            format!("<a href=\"{}\">{s_total}</a>", url("/servers")),
        ),
        ("Operating", count(s_op, Some("ok"))),
        ("Power off", count(s_off, None)),
        ("Standby", count(s_standby, None)),
        ("Error", count(s_err, Some("err"))),
        ("No connection", count(s_none, Some("err"))),
    ]);
    let [p_total, p_run, p_not, p_down, p_err, p_fw] = v.partitions;
    let partitions = status(&[
        (
            "Total",
            format!("<a href=\"{}\">{p_total}</a>", url("/partitions")),
        ),
        ("Running", count(p_run, Some("ok"))),
        ("Not activated", count(p_not, Some("err"))),
        ("Shutting down", count(p_down, None)),
        ("Error", count(p_err, Some("err"))),
        ("Open firmware", count(p_fw, None)),
    ]);
    let events = if v.events > 0 {
        format!(
            "<a class=\"err\" href=\"{}\">{}</a>",
            url("/events"),
            v.events
        )
    } else {
        format!("<a href=\"{}\">0</a>", url("/events"))
    };
    let service = status(&[
        ("Serviceable events open", events),
        (
            "Dumps awaiting action",
            format!("<a href=\"{}\">{}</a>", url("/dumps"), v.dumps),
        ),
        (
            "Updates available",
            format!("<a href=\"{}\">{}</a>", url("/updates"), esc(&v.updates)),
        ),
    ]);
    let console = status(&[
        (
            "Console",
            format!("<span class=\"mono\">{}</span>", esc(&v.console)),
        ),
        (
            "Version",
            format!(
                "<span class=\"mono\">{}</span> build {} &nbsp; <a href=\"{}\">Details</a>",
                esc(&v.version),
                esc(&v.build),
                url("/console/version")
            ),
        ),
        ("Uptime", esc(&v.uptime)),
        (
            "Connected systems",
            format!(
                "<a href=\"{}\">{}</a>",
                url("/connectivity"),
                esc(&v.connected)
            ),
        ),
    ]);
    let _ = write!(
        body,
        "<table class=\"home\" width=\"100%\"><tr><td width=\"50%\"><h3>Managed systems</h3>{systems}</td><td width=\"50%\"><h3>Partitions</h3>{partitions}</td></tr><tr><td><h3>Service</h3>{service}</td><td><h3>Console</h3>{console}</td></tr></table>"
    );
    Page {
        title: "Welcome".into(),
        crumb: vec!["Welcome".into()],
        h1: "Welcome".into(),
        desc: Some(format!(
            "Quartz Systems/Management Console {}. Signed on as {} ({}) from {}.",
            v.console, v.user, v.role, v.from
        )),
        messages: v.messages.clone(),
        body,
        help: "welcome".into(),
        nav: Some(("Welcome", Some("/welcome"))),
        ..Page::default()
    }
}
