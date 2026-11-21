// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! Shared processor pools (`vr-pools.html`): the host's pools as its
//! resources list them, the cores given to each, its maximum, the
//! partitions in it and what they are entitled to now.

use qsweb::frame::{Ctx, Page};
use qsweb::html::esc;
use qsweb::http::Response;
use qsweb::msg::Message;
use qsweb::partials::{Cell, Col, List, list_table};

use mgmtproto::Kept;

use crate::app::routes::Rq;
use crate::systems::props::units;

/// A pool's row.
#[derive(Clone, Debug, Default)]
pub struct PoolRow {
    /// Its number on the page.
    pub id: u32,
    /// Its name; `(unused)` for a place with none.
    pub name: String,
    /// Units reserved for it: its cores.
    pub reserved: String,
    /// Its most.
    pub maximum: String,
    /// The partitions in it; `(none)`.
    pub partitions: String,
    /// What they are entitled to now; empty for an unused place.
    pub used: String,
}

/// What the tab shows.
#[derive(Clone, Debug, Default)]
pub struct Pools {
    /// The host.
    pub host: String,
    /// The pools.
    pub pools: Vec<PoolRow>,
    /// The messages.
    pub messages: Vec<Message>,
}

/// The page.
pub fn page(ctx: &Ctx, v: &Pools) -> Page {
    let list = List {
        cols: [
            "Actions",
            "Pool ID",
            "Name",
            "Reserved units",
            "Maximum units",
            "Partitions",
            "Current use",
        ]
        .iter()
        .map(|c| Col::plain(c))
        .collect(),
        rows: v
            .pools
            .iter()
            .map(|p| {
                let change = ctx.url(&format!(
                    "/server/{}/virtual/pools/change?id={}",
                    v.host, p.id
                ));
                vec![
                    Cell::act(format!("<a href=\"{}\">Change</a>", esc(&change))),
                    Cell::num(&p.id.to_string()),
                    Cell::mono(&p.name),
                    Cell::num(&p.reserved),
                    Cell::num(&p.maximum),
                    Cell::mono(&p.partitions),
                    Cell::num(&p.used),
                ]
            })
            .collect(),
        empty: "The host has no shared processor pools.".into(),
        keep: Vec::new(),
    };
    super::page(
        ctx,
        &v.host,
        "pools",
        v.messages.clone(),
        &list_table(&list),
    )
}

/// The tab of `k`, from what is kept: the default pool first.
pub fn view(k: &Kept, messages: Vec<Message>) -> Pools {
    let Some(h) = k.host.as_ref() else {
        return Pools {
            host: k.name.clone(),
            pools: Vec::new(),
            messages,
        };
    };
    let pools = h
        .pools
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let names: Vec<String> = k
                .partitions
                .iter()
                .filter(|s| match &s.current {
                    Some(c) => c.pool == p.name,
                    None => k
                        .profile(s.id)
                        .is_some_and(|pr| pr.processors.pool.as_deref() == Some(p.name.as_str())),
                })
                .map(|s| s.name.clone())
                .collect();
            PoolRow {
                id: u32::try_from(i).unwrap_or(u32::MAX),
                name: p.name.clone(),
                reserved: units(u64::try_from(p.cores.len()).unwrap_or(0) * 100),
                maximum: units(u64::from(p.maximum)),
                partitions: if names.is_empty() {
                    "(none)".into()
                } else {
                    names.join(", ")
                },
                used: units(u64::from(p.entitled)),
            }
        })
        .collect();
    Pools {
        host: k.name.clone(),
        pools,
        messages,
    }
}

/// `GET /server/<host>/virtual/pools`.
pub fn route(rq: &Rq<'_>, k: &Kept) -> Response {
    let mut messages = Vec::new();
    if let Some(m) = super::lacking(k, qsi::caps::HOST) {
        messages.push(m);
    }
    rq.page(page(&rq.ctx, &view(k, messages)))
}
