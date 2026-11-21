// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! Virtual Storage (`vr-storage.html`): the host's storage pools with
//! their units and use, and every virtual disk with its size, pool,
//! the partition and adapter that hold it, its provisioning and state.

use std::fmt::Write as _;

use qsweb::frame::{Ctx, Page};
use qsweb::html::esc;
use qsweb::http::Response;
use qsweb::msg::Message;
use qsweb::partials::{Cell, Col, List, list_table, status};

use mgmtproto::Kept;
use qsi::host::vres::{StoragePool, Vdisk};

use crate::app::routes::Rq;

/// A pool's row.
#[derive(Clone, Debug, Default)]
pub struct PoolRow {
    /// Its name.
    pub name: String,
    /// What it is made of: `Mirrored (2 x 1.9 TB)`.
    pub backing: String,
    /// Its capacity, GB.
    pub capacity: String,
    /// Used, GB.
    pub used: String,
    /// Tenths of a percent used.
    pub permille: u32,
    /// `Thin provisioned`, `Thick`.
    pub kind: String,
}

/// A virtual disk's row.
#[derive(Clone, Debug, Default)]
pub struct VdiskRow {
    /// Its name.
    pub name: String,
    /// Its size, GB.
    pub size: String,
    /// Its pool.
    pub pool: String,
    /// The partition holding it; empty for none.
    pub partition: String,
    /// The client adapter's slot; 0 for none.
    pub slot: u32,
    /// `Thin`, `Thick`.
    pub provisioning: String,
    /// `Operating` while held; empty otherwise.
    pub status: String,
}

/// What the tab shows.
#[derive(Clone, Debug, Default)]
pub struct Storage {
    /// The host.
    pub host: String,
    /// Its pools.
    pub pools: Vec<PoolRow>,
    /// Its virtual disks.
    pub vdisks: Vec<VdiskRow>,
    /// The sort of the disks: `n`, `s`, `p`, `a`.
    pub sort: String,
    /// The messages.
    pub messages: Vec<Message>,
}

/// The one-row bar of `permille` used: red at 70 % and above.
pub fn bar(permille: u32) -> String {
    let pct = (permille + 5) / 10;
    let pct = pct.min(100);
    let fill = if permille >= 700 { "hot" } else { "fill" };
    let mut s = String::from("<table class=\"bar\" cellspacing=\"0\" cellpadding=\"0\"><tr>");
    let _ = write!(
        s,
        "<td class=\"{fill}\" width=\"{pct}%\"></td><td width=\"{}%\"></td></tr></table>",
        100 - pct
    );
    s
}

/// Tenths of a percent as `53.1`.
pub fn tenths(permille: u32) -> String {
    format!("{}.{}", permille / 10, permille % 10)
}

/// The page.
#[allow(clippy::too_many_lines, reason = "two tables, a column a line")]
pub fn page(ctx: &Ctx, v: &Storage) -> Page {
    let pools = List {
        cols: [
            "Actions",
            "Pool",
            "Backing devices",
            "Capacity (GB)",
            "Used (GB)",
            "% used",
            "",
            "Type",
        ]
        .iter()
        .map(|c| Col::plain(c))
        .collect(),
        rows: v
            .pools
            .iter()
            .map(|p| {
                let props = ctx.url(&format!(
                    "/server/{}/virtual/storage/pool?n={}",
                    v.host, p.name
                ));
                let extend = ctx.url(&format!(
                    "/server/{}/virtual/storage/pool?n={}&extend=1",
                    v.host, p.name
                ));
                vec![
                    Cell::act(format!(
                        "<a href=\"{}\">Properties</a> | <a href=\"{}\">Extend</a>",
                        esc(&props),
                        esc(&extend)
                    )),
                    Cell::mono(&p.name),
                    Cell::text(&p.backing),
                    Cell::num(&p.capacity),
                    Cell::num(&p.used),
                    Cell::num(&tenths(p.permille)),
                    Cell::html(bar(p.permille)),
                    Cell::text(&p.kind),
                ]
            })
            .collect(),
        empty: "The host has no storage pools.".into(),
        keep: Vec::new(),
    };
    let disks = List {
        cols: vec![
            Col::plain("Actions"),
            Col::sorted("Name", "n"),
            Col::sorted("Size (GB)", "s"),
            Col::sorted("Pool", "p"),
            Col::sorted("Assigned partition", "a"),
            Col::plain("Client adapter"),
            Col::plain("Provisioning"),
            Col::plain("Status"),
        ],
        rows: v
            .vdisks
            .iter()
            .map(|d| {
                let q = format!("?ms={}&p={}&n={}", v.host, d.pool, d.name);
                let props = ctx.url(&format!("/server/{}/virtual/storage/resize{q}", v.host));
                let resize = ctx.url(&format!(
                    "/server/{}/virtual/storage/resize{q}&grow=1",
                    v.host
                ));
                let unassign = ctx.url(&format!("/confirm/vdisk-unassign{q}"));
                let delete = ctx.url(&format!("/confirm/vdisk-delete{q}"));
                vec![
                    Cell::act(format!(
                        "<a href=\"{}\">Properties</a> | <a href=\"{}\">Resize</a> | <a href=\"{}\">Unassign</a> | <a href=\"{}\">Delete</a>",
                        esc(&props),
                        esc(&resize),
                        esc(&unassign),
                        esc(&delete)
                    )),
                    Cell::mono(&d.name),
                    Cell::num(&d.size),
                    Cell::mono(&d.pool),
                    Cell::mono(if d.partition.is_empty() {
                        "(unassigned)"
                    } else {
                        &d.partition
                    }),
                    Cell::num(&if d.slot == 0 {
                        String::new()
                    } else {
                        d.slot.to_string()
                    }),
                    Cell::text(&d.provisioning),
                    Cell::html(if d.status.is_empty() {
                        String::new()
                    } else {
                        status(&d.status, Some(true))
                    }),
                ]
            })
            .collect(),
        empty: "The host has no virtual disks.".into(),
        keep: Vec::new(),
    };
    let body = format!(
        "<h2>Storage pools</h2>{}<h2>Virtual disks</h2>{}<form method=\"post\" action=\"{}\">{}<div class=\"buttons\"><input type=\"submit\" name=\"vdisk\" value=\"Create virtual disk...\"><input type=\"submit\" name=\"pool\" value=\"Create storage pool...\"></div></form>",
        list_table(&pools),
        list_table(&disks),
        esc(&ctx.url(&format!("/server/{}/virtual/storage/create", v.host))),
        ctx.csrf_field(),
    );
    super::page(ctx, &v.host, "storage", v.messages.clone(), &body)
}

/// A pool's units in words: `Mirrored (2 x 1.9 TB)`.
fn backing(p: &StoragePool) -> String {
    let n = p.units.len();
    let each = p.units.iter().map(|u| u.bytes).max().unwrap_or(0);
    let size = if each >= 1_000_000_000_000 {
        format!(
            "{}.{} TB",
            each / 1_000_000_000_000,
            each / 100_000_000_000 % 10
        )
    } else {
        format!("{} GB", each / 1_000_000_000)
    };
    format!("{} ({n} x {size})", p.protection)
}

/// The tab of `k`, from what is kept.
pub fn view(rq: &Rq<'_>, k: &Kept, sort: &str, messages: Vec<Message>) -> Storage {
    let r = &k.resources;
    let pools = r
        .pools
        .iter()
        .map(|p| {
            let permille = if p.bytes == 0 {
                0
            } else {
                u32::try_from(u128::from(p.allocated) * 1000 / u128::from(p.bytes)).unwrap_or(1000)
            };
            let thin = r.vdisks.iter().any(|d| d.pool == p.name && !d.thick);
            PoolRow {
                name: p.name.clone(),
                backing: backing(p),
                capacity: super::gb(rq, p.bytes),
                used: super::gb(rq, p.allocated),
                permille,
                kind: if thin { "Thin provisioned" } else { "Thick" }.into(),
            }
        })
        .collect();
    let mut disks: Vec<&Vdisk> = r.vdisks.iter().collect();
    let holder = |d: &Vdisk| {
        k.partitions
            .iter()
            .find(|p| p.id == d.holder.partition && d.holder.partition != 0)
            .map(|p| p.name.clone())
            .unwrap_or_default()
    };
    match sort {
        "s" => disks.sort_by_key(|d| std::cmp::Reverse(d.bytes)),
        "p" => disks.sort_by(|a, b| (&a.pool, &a.name).cmp(&(&b.pool, &b.name))),
        "a" => disks.sort_by_key(|d| (holder(d), d.name.clone())),
        _ => disks.sort_by(|a, b| a.name.cmp(&b.name)),
    }
    let vdisks = disks
        .into_iter()
        .map(|d| {
            let partition = holder(d);
            VdiskRow {
                name: d.name.clone(),
                size: super::gb(rq, d.bytes),
                pool: d.pool.clone(),
                slot: if partition.is_empty() {
                    0
                } else {
                    d.holder.slot
                },
                status: if partition.is_empty() {
                    String::new()
                } else {
                    "Operating".into()
                },
                partition,
                provisioning: if d.thick { "Thick" } else { "Thin" }.into(),
            }
        })
        .collect();
    Storage {
        host: k.name.clone(),
        pools,
        vdisks,
        sort: sort.into(),
        messages,
    }
}

/// `GET /server/<host>/virtual/storage`.
pub fn route(rq: &Rq<'_>, k: &Kept) -> Response {
    let mut messages = Vec::new();
    if let Some(m) = super::lacking(k, qsi::caps::VRES) {
        messages.push(m);
    }
    rq.page(page(&rq.ctx, &view(rq, k, &rq.q("sort"), messages)))
}
