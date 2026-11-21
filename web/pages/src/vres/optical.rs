// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! Virtual Optical (`vr-optical.html`): the host's media library, each
//! partition's optical devices and what is loaded in them, an image
//! uploaded from the browser (`upload`), and Load and Unload.

use qsweb::frame::{Ctx, Page};
use qsweb::html::esc;
use qsweb::http::Response;
use qsweb::msg::{Kind, Message};
use qsweb::partials::{Cell, Col, List, list_table, mono};

use mgmtproto::Kept;
use qsi::any::AnyRequest;
use qsi::host::vres::{self, DriveKind};

use crate::app::routes::Rq;

/// An image's row.
#[derive(Clone, Debug, Default)]
pub struct ImageRow {
    /// Its name.
    pub name: String,
    /// Its size, MB.
    pub size: String,
    /// When it was uploaded; empty when the host does not say.
    pub uploaded: String,
    /// The partition it is loaded in; empty for none.
    pub partition: String,
    /// That partition's device: `OPT01`.
    pub drive: String,
    /// Partitions read it only.
    pub read_only: bool,
}

/// A partition's optical device.
#[derive(Clone, Debug, Default)]
pub struct DriveRow {
    /// The partition.
    pub partition: String,
    /// Its name on the page: `OPT01`.
    pub drive: String,
    /// Its slot.
    pub slot: u32,
    /// What is loaded; empty for nothing.
    pub image: String,
}

/// What the tab shows.
#[derive(Clone, Debug, Default)]
pub struct Optical {
    /// The host.
    pub host: String,
    /// The library.
    pub images: Vec<ImageRow>,
    /// The devices.
    pub drives: Vec<DriveRow>,
    /// The messages.
    pub messages: Vec<Message>,
}

/// The page.
pub fn page(ctx: &Ctx, v: &Optical) -> Page {
    let images = List {
        cols: vec![
            Col::plain("Actions"),
            Col::sorted("Image", "n"),
            Col::sorted("Size (MB)", "s"),
            Col::plain("Uploaded"),
            Col::plain("Mounted to"),
            Col::plain("Virtual drive"),
            Col::plain("Read-only"),
        ],
        rows: v
            .images
            .iter()
            .map(|i| {
                let load = ctx.url(&format!(
                    "/server/{}/virtual/optical/load?n={}",
                    v.host, i.name
                ));
                let unload = ctx.url(&format!(
                    "/server/{}/virtual/optical/load?n={}&unload=1",
                    v.host, i.name
                ));
                let delete = ctx.url(&format!("/confirm/image-delete?ms={}&n={}", v.host, i.name));
                vec![
                    Cell::act(format!(
                        "<a href=\"{}\">Load</a> | <a href=\"{}\">Unload</a> | <a href=\"{}\">Delete</a>",
                        esc(&load),
                        esc(&unload),
                        esc(&delete)
                    )),
                    Cell::mono(&i.name),
                    Cell::num(&i.size),
                    Cell::text(&i.uploaded),
                    Cell::html(if i.partition.is_empty() {
                        String::new()
                    } else {
                        mono(&i.partition)
                    }),
                    Cell::mono(&i.drive),
                    Cell::text(if i.read_only { "Yes" } else { "No" }),
                ]
            })
            .collect(),
        empty: "The media library has no images.".into(),
        keep: Vec::new(),
    };
    let drives = List {
        cols: ["Partition", "Drive", "Adapter ID", "Loaded image"]
            .iter()
            .map(|c| Col::plain(c))
            .collect(),
        rows: v
            .drives
            .iter()
            .map(|d| {
                vec![
                    Cell::mono(&d.partition),
                    Cell::mono(&d.drive),
                    Cell::num(&d.slot.to_string()),
                    Cell::mono(&d.image),
                ]
            })
            .collect(),
        empty: "No partition has a virtual optical device.".into(),
        keep: Vec::new(),
    };
    let body = format!(
        "<h2>Media library</h2>{}<h2>Virtual optical drives</h2>{}<form method=\"post\" enctype=\"multipart/form-data\">{}<h3>Upload image</h3><input type=\"file\" name=\"iso\" size=\"40\"> <input type=\"checkbox\" name=\"ro\" checked disabled> Read-only <input type=\"submit\" name=\"upload\" value=\"Upload\"> <span class=\"small grey\">or from URL: </span><input type=\"text\" name=\"url\" value=\"https://update.quartz.systems/images/\" size=\"40\" form=\"fetch\"> <input type=\"submit\" name=\"fetch\" value=\"Fetch\" form=\"fetch\"></form><form id=\"fetch\" method=\"post\">{}</form>",
        list_table(&images),
        list_table(&drives),
        ctx.csrf_field(),
        ctx.csrf_field(),
    );
    super::page(ctx, &v.host, "optical", v.messages.clone(), &body)
}

/// Each partition's optical devices: its profile's, with what a running
/// one has loaded now. Named `OPT01`, `OPT02` in slot order.
pub fn drives(k: &Kept) -> Vec<DriveRow> {
    let mut out = Vec::new();
    for p in &k.partitions {
        let live = k.resources.drives.iter().find(|d| d.id == p.id);
        let mut slots: Vec<(u32, String)> = match live {
            Some(l) => l
                .drives
                .iter()
                .filter(|d| d.kind == DriveKind::Optical)
                .map(|d| (d.slot, d.name.clone()))
                .collect(),
            None => k
                .profile(p.id)
                .map(|pr| {
                    pr.optical
                        .iter()
                        .map(|m| (m.slot, m.media.clone().unwrap_or_default()))
                        .collect()
                })
                .unwrap_or_default(),
        };
        slots.sort_by_key(|(s, _)| *s);
        for (i, (slot, image)) in slots.into_iter().enumerate() {
            out.push(DriveRow {
                partition: p.name.clone(),
                drive: format!("OPT{:02}", i + 1),
                slot,
                image,
            });
        }
    }
    out
}

/// The tab of `k`, from what is kept.
pub fn view(rq: &Rq<'_>, k: &Kept, messages: Vec<Message>) -> Optical {
    let drives = drives(k);
    let mut images: Vec<ImageRow> = k
        .resources
        .images
        .iter()
        .map(|i| {
            let at = drives.iter().find(|d| d.image == i.name);
            ImageRow {
                name: i.name.clone(),
                size: super::mb(rq, i.bytes),
                uploaded: String::new(),
                partition: at.map(|d| d.partition.clone()).unwrap_or_default(),
                drive: at.map(|d| d.drive.clone()).unwrap_or_default(),
                read_only: true,
            }
        })
        .collect();
    match rq.q("sort").as_str() {
        "s" => images.sort_by_key(|i| {
            std::cmp::Reverse(i.size.replace(',', "").parse::<u64>().unwrap_or(0))
        }),
        "n" => images.sort_by(|a, b| a.name.cmp(&b.name)),
        _ => {}
    }
    Optical {
        host: k.name.clone(),
        images,
        drives,
        messages,
    }
}

/// `GET /server/<host>/virtual/optical`; a plain post is Fetch, which
/// the console does not do (an upload's post is streamed: `upload`).
pub fn route(rq: &Rq<'_>, k: &Kept) -> Response {
    let mut messages = Vec::new();
    if rq.post() {
        if let Some(no) = crate::app::routes::lacks(rq, webusers::Task::VirtualResources) {
            return no;
        }
        rq.audit(&format!("fetch an image to {}", k.name), "refused");
        messages.push(qmcmsg::message(Kind::Error, &qmcmsg::NO_FETCH, &[]));
    }
    if let Some(m) = super::lacking(k, qsi::caps::VRES) {
        messages.push(m);
    }
    rq.page(page(&rq.ctx, &view(rq, k, messages)))
}

/// `GET`, `POST /server/<host>/virtual/optical/load?n=&unload=`: load
/// image `n` into a partition's device, or unload it.
#[allow(
    clippy::too_many_lines,
    reason = "one form and its request, step by step"
)]
pub fn load(rq: &Rq<'_>, k: &Kept) -> Response {
    let f = rq.req.form();
    let get = |n: &str| {
        if rq.post() {
            f.str(n).to_string()
        } else {
            rq.q(n)
        }
    };
    let name = get("n");
    let unload = get("unload") == "1";
    let back = format!("/server/{}/virtual/optical", k.name);
    let devices = drives(k);
    if rq.post() {
        if f.has("cancel") {
            return Response::redirect(&rq.ctx.url(&back));
        }
        if let Some(no) = crate::app::routes::lacks(rq, webusers::Task::VirtualResources) {
            return no;
        }
        let Some(d) = devices
            .iter()
            .find(|d| format!("{} {}", d.partition, d.drive) == f.str("d"))
        else {
            let m = qmcmsg::message(Kind::Error, &qmcmsg::NOT_VALID, &["Choose a drive."]);
            return rq.done(&format!("{back}/load?n={name}"), m);
        };
        let id = k
            .partitions
            .iter()
            .find(|p| p.name == d.partition)
            .map_or(0, |p| p.id);
        let (req, action, done) = if unload {
            (
                AnyRequest::Vres(vres::Request::Unload(vres::Unload {
                    id,
                    slot: d.slot,
                    forced: f.has("force"),
                })),
                format!("unload {} from {}", d.drive, d.partition),
                &qmcmsg::IMAGE_UNLOADED,
            )
        } else {
            (
                AnyRequest::Vres(vres::Request::Load(vres::Load {
                    id,
                    slot: d.slot,
                    name: name.clone(),
                })),
                format!("load {name} into {} of {}", d.drive, d.partition),
                &qmcmsg::IMAGE_LOADED,
            )
        };
        return match super::act(rq, k, &action, &req) {
            Ok(_) => rq.done(
                &back,
                qmcmsg::message(Kind::Done, done, &[&name, &d.partition]),
            ),
            Err(ms) => rq.done_all(
                &format!("{back}/load?n={name}&unload={}", u8::from(unload)),
                ms,
            ),
        };
    }
    let options: Vec<String> = devices
        .iter()
        .filter(|d| !unload || d.image == name)
        .map(|d| format!("{} {}", d.partition, d.drive))
        .collect();
    let opts: Vec<&str> = options.iter().map(String::as_str).collect();
    let title = if unload { "Unload image" } else { "Load image" };
    let groups = [qsweb::partials::PropGroup {
        title: None,
        rows: vec![
            qsweb::partials::Prop::new("Image", mono(&name)),
            qsweb::partials::Prop::new(
                "Partition and drive",
                qsweb::partials::select("d", &opts, opts.first().copied().unwrap_or("")),
            ),
        ],
    }];
    let body = qsweb::partials::form(
        &rq.ctx,
        "",
        &format!(
            "{}{}\n{}",
            qsweb::partials::hidden("n", &name),
            qsweb::partials::props_table(rq.ctx.product, &groups, true)
                + &qsweb::partials::hidden("unload", if unload { "1" } else { "0" }),
            qsweb::partials::buttons(&[
                qsweb::partials::Button::new("ok", "OK"),
                qsweb::partials::Button::new("cancel", "Cancel"),
            ])
        ),
    );
    rq.page(super::form_page(
        &k.name,
        "optical",
        title,
        Vec::new(),
        body,
    ))
}
