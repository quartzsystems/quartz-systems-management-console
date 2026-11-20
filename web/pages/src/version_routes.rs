// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! Serving Console version: the release and build from `release`, the
//! slot from PID 1, the machine from `inventory.toml`, the interfaces
//! from the network service. No fix is installed before Phase 6.

use qsweb::http::Response;

use crate::app::routes::Rq;
use crate::version::{Version, page, platform};

const MB: u64 = 1024 * 1024;
const GB: u64 = 1024 * MB;

fn view(rq: &Rq<'_>) -> Version {
    let app = rq.app;
    let s = app.settings();
    let zone = app.host().zone();
    let inv = app
        .store()
        .load::<state::Inventory>()
        .ok()
        .flatten()
        .unwrap_or_default();
    let up = app.console.uptime();
    let since = i64::try_from(app.console.now().saturating_sub(up)).unwrap_or(0);
    let sys = &inv.system;
    let platform = platform(&sys.manufacturer, &sys.product, &sys.serial);
    let p = &inv.processors;
    let processors = if platform.starts_with("Virtual machine") {
        format!("{} vCPU", p.threads.max(1))
    } else {
        format!(
            "{} socket{}, {} cores, {} threads",
            p.sockets,
            if p.sockets == 1 { "" } else { "s" },
            p.cores,
            p.threads
        )
    };
    let processors = if p.model.is_empty() {
        processors
    } else {
        format!("{processors} ({})", p.model.trim())
    };
    let disk = app.console.data_space().map_or_else(
        || "not known".to_string(),
        |(all, used)| {
            let pct = (used * 100).checked_div(all).unwrap_or(0);
            format!("{} GB ({pct}% used)", all.div_ceil(GB))
        },
    );
    let interfaces = app
        .console
        .interfaces()
        .iter()
        .filter(|i| !i.interface.is_empty())
        .map(|i| {
            let role = if i.role.is_empty() {
                "public"
            } else if i.role == "private" {
                "private management"
            } else {
                &i.role
            };
            format!("{} ({role})", i.interface)
        })
        .collect::<Vec<_>>()
        .join(", ");
    let kernel = app.console.kernel();
    Version {
        release: release::RELEASE.to_string(),
        build: release::BUILD_ID.into(),
        built: String::new(),
        fixes: "None".into(),
        kernel: if kernel.is_empty() {
            "not known".into()
        } else {
            format!("Linux {kernel}")
        },
        web: format!("qsmcwebd {}", release::RELEASE),
        uptime: format!(
            "{} (since {} {}), slot {}",
            crate::pages::uptime(up),
            crate::fmt::date(since, zone, &s.date_format),
            crate::fmt::time(since, zone, &s.time_format, false),
            Some(app.console.fact(release::fact::SLOT))
                .filter(|f| !f.is_empty())
                .unwrap_or_else(|| "not known".into())
        ),
        platform,
        processors,
        memory: format!(
            "{} MB",
            crate::fmt::number(inv.memory.total_bytes / MB, &s.number_format)
        ),
        disk,
        interfaces: if interfaces.is_empty() {
            "none".into()
        } else {
            interfaces
        },
        serial: format!(
            "{}, 115200, the local console",
            qsweb::partials::mono("ttyS0")
        ),
        messages: Vec::new(),
    }
}

/// `/console/version`.
pub fn version(rq: &Rq<'_>) -> Response {
    rq.page(page(&rq.ctx, &view(rq)))
}
