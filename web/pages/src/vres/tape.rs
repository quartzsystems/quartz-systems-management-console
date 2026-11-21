// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! Virtual Tape (`vr-tape.html`): each partition's tape devices, a
//! volume of the media library or one of the host's own drives passed
//! through (released at the host). The volumes are on their own form
//! (`forms::volume`).

use qsweb::frame::{Ctx, Page};
use qsweb::html::esc;
use qsweb::http::Response;
use qsweb::msg::{Kind, Message};
use qsweb::partials::{Cell, Col, List, list_table, status};

use mgmtproto::Kept;
use qsi::host::model::{PartitionState, TapeSource};

use crate::app::routes::Rq;

/// A tape device's row.
#[derive(Clone, Debug, Default)]
pub struct TapeRow {
    /// Its name on the page: `VTAP01`, `TAP01`.
    pub drive: String,
    /// `Virtual (file)`, `Pass-through`.
    pub kind: String,
    /// What backs it.
    pub backing: String,
    /// The partition.
    pub partition: String,
    /// Its slot.
    pub slot: u32,
    /// `Operating`, `Not activated`.
    pub status: String,
}

/// What the tab shows.
#[derive(Clone, Debug, Default)]
pub struct Tape {
    /// The host.
    pub host: String,
    /// The devices.
    pub drives: Vec<TapeRow>,
    /// The messages.
    pub messages: Vec<Message>,
}

/// The page.
pub fn page(ctx: &Ctx, v: &Tape) -> Page {
    let list = List {
        cols: vec![
            Col::plain("Actions"),
            Col::sorted("Drive", "d"),
            Col::plain("Type"),
            Col::plain("Backing"),
            Col::plain("Assigned partition"),
            Col::plain("Adapter ID"),
            Col::plain("Status"),
        ],
        rows: v
            .drives
            .iter()
            .map(|d| {
                let q = format!("?ms={}&p={}&s={}", v.host, d.partition, d.slot);
                let props = ctx.url(&format!("/server/{}/virtual/tape/volume{q}", v.host));
                let reassign = ctx.url(&format!(
                    "/server/{}/virtual/tape/volume{q}&reassign=1",
                    v.host
                ));
                let remove = ctx.url(&format!("/confirm/tape-remove{q}"));
                vec![
                    Cell::act(format!(
                        "<a href=\"{}\">Properties</a> | <a href=\"{}\">Reassign</a> | <a href=\"{}\">Remove</a>",
                        esc(&props),
                        esc(&reassign),
                        esc(&remove)
                    )),
                    Cell::mono(&d.drive),
                    Cell::text(&d.kind),
                    Cell::mono(&d.backing),
                    Cell::mono(&d.partition),
                    Cell::num(&d.slot.to_string()),
                    Cell::html(status(&d.status, Some(d.status == "Operating"))),
                ]
            })
            .collect(),
        empty: "No partition has a tape device.".into(),
        keep: Vec::new(),
    };
    let body = format!(
        "{}<form method=\"post\">{}<div class=\"buttons\"><input type=\"submit\" name=\"vtape\" value=\"Create virtual tape drive...\"><input type=\"submit\" name=\"ptape\" value=\"Add pass-through drive...\"></div></form>",
        list_table(&list),
        ctx.csrf_field(),
    );
    super::page(ctx, &v.host, "tape", v.messages.clone(), &body)
}

/// A volume's backing in words: `TAPE01 (200 GB)`.
fn volume(rq: &Rq<'_>, k: &Kept, name: &str) -> String {
    k.resources
        .volumes
        .iter()
        .find(|x| x.name == name)
        .map_or_else(
            || name.to_string(),
            |x| {
                format!(
                    "{name} ({} GB)",
                    crate::fmt::number(x.capacity_mib / 1024, &rq.app.settings().number_format)
                )
            },
        )
}

/// The tab of `k`, from what is kept.
pub fn view(rq: &Rq<'_>, k: &Kept, messages: Vec<Message>) -> Tape {
    let (mut virtuals, mut passed) = (0, 0);
    let mut drives = Vec::new();
    for p in &k.partitions {
        let Some(profile) = k.profile(p.id) else {
            continue;
        };
        for t in &profile.tape {
            let (kind, backing, drive) = match t.tape_source() {
                TapeSource::Drive(at) => {
                    passed += 1;
                    let what = k
                        .resources
                        .tape_drives
                        .iter()
                        .find(|d| d.location == at)
                        .map_or_else(
                            || at.to_string(),
                            |d| format!("{at} ({} {})", d.vendor, d.product),
                        );
                    ("Pass-through", what, format!("TAP{passed:02}"))
                }
                TapeSource::Volume(v) => {
                    virtuals += 1;
                    (
                        "Virtual (file)",
                        volume(rq, k, v),
                        format!("VTAP{virtuals:02}"),
                    )
                }
                TapeSource::Empty => {
                    virtuals += 1;
                    (
                        "Virtual (file)",
                        "(none)".into(),
                        format!("VTAP{virtuals:02}"),
                    )
                }
            };
            drives.push(TapeRow {
                drive,
                kind: kind.into(),
                backing,
                partition: p.name.clone(),
                slot: t.slot,
                status: if p.state == PartitionState::Running {
                    "Operating"
                } else {
                    "Not activated"
                }
                .into(),
            });
        }
    }
    Tape {
        host: k.name.clone(),
        drives,
        messages,
    }
}

/// `GET`, `POST /server/<host>/virtual/tape`: the tab; its buttons open
/// the volumes' form, or say how a drive is passed through.
pub fn route(rq: &Rq<'_>, k: &Kept) -> Response {
    let f = rq.req.form();
    let mut messages = Vec::new();
    if rq.post() {
        if f.has("vtape") {
            return Response::redirect(
                &rq.ctx
                    .url(&format!("/server/{}/virtual/tape/volume", k.name)),
            );
        }
        messages.push(qmcmsg::message(Kind::Info, &qmcmsg::PASS_THROUGH, &[]));
    }
    if let Some(m) = super::lacking(k, qsi::caps::VRES) {
        messages.push(m);
    }
    rq.page(page(&rq.ctx, &view(rq, k, messages)))
}
