// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! Manage profiles (`manage-profiles.html`): a partition's profiles as
//! `mgmtd` keeps them, with each one's last activation; Activate, Edit
//! (the partition's pages with that profile), Copy, Delete (through its
//! confirmation), a new profile from the default, the running
//! configuration saved as a profile, and the default set.

use std::fmt::Write as _;

use qsweb::frame::{Ctx, Page};
use qsweb::html::esc;
use qsweb::http::Response;
use qsweb::msg::{Kind, Message};
use qsweb::partials::{
    Button, Cell, Col, List, Prop, PropGroup, buttons, form, hidden, list_table, props_table,
};

use mgmtproto::Kept;
use qsi::any::AnyRequest;
use qsi::host::model::{ProcessorMode, Profile, Sharing};
use qsi::host::profiles;
use webusers::Task;

use crate::app::routes::{Rq, lacks};
use crate::systems::props::units;

/// One profile's row.
#[derive(Clone, Debug, Default)]
pub struct Row {
    /// Its name.
    pub name: String,
    /// `Shared 1.00 (0.25-2.00), uncapped`.
    pub processors: String,
    /// Desired memory, MB: `16,384`.
    pub memory: String,
    /// How many virtual adapters.
    pub adapters: usize,
    /// When last activated; empty for never.
    pub last: String,
    /// It is the default.
    pub default: bool,
}

/// What the page shows.
#[derive(Clone, Debug, Default)]
pub struct Profiles {
    /// The host.
    pub host: String,
    /// The partition.
    pub name: String,
    /// Its ID.
    pub id: u32,
    /// Its profiles.
    pub rows: Vec<Row>,
    /// The messages.
    pub messages: Vec<Message>,
}

/// A profile's processors in a few words.
pub fn processors(p: &Profile) -> String {
    let x = &p.processors;
    if x.mode == ProcessorMode::Dedicated {
        return format!("Dedicated {} ({}-{})", x.desired, x.minimum, x.maximum);
    }
    let u = |v: Option<u32>| units(u64::from(v.unwrap_or(0)));
    let sharing = match x.sharing {
        Some(Sharing::Uncapped) if x.weight.is_some() => ", uncapped",
        Some(Sharing::Capped) => ", capped",
        _ => "",
    };
    format!(
        "Shared {} ({}-{}){sharing}",
        u(x.desired_units),
        u(x.minimum_units),
        u(x.maximum_units)
    )
}

/// How many virtual adapters a profile has, the console's two serial
/// adapters with them.
pub fn adapters(p: &Profile) -> usize {
    2 + p.ethernet.len() + p.scsi.len() + p.optical.len() + p.tape.len()
}

/// The page.
pub fn page(ctx: &Ctx, v: &Profiles) -> Page {
    let rows = v
        .rows
        .iter()
        .map(|r| {
            let q = |page: &str, extra: &str| {
                ctx.url(&format!(
                    "/partition/{}/{}/{page}?prof={}{extra}",
                    v.host,
                    v.id,
                    qsweb::http::form::encode(&r.name)
                ))
            };
            let delete = ctx.url(&format!(
                "/confirm/profile-delete?ms={}&p={}&n={}",
                v.host,
                v.id,
                qsweb::http::form::encode(&r.name)
            ));
            vec![
                Cell::act(format!(
                    "<a href=\"{}\">Activate</a> | <a href=\"{}\">Edit</a> | <a href=\"{}\">Copy</a> | <a href=\"{}\">Delete</a>",
                    esc(&q("activate", "")),
                    esc(&ctx.url(&format!(
                        "/partition/{}/{}/processors?profile={}",
                        v.host,
                        v.id,
                        qsweb::http::form::encode(&r.name)
                    ))),
                    esc(&q("profiles", "&copy=1")),
                    esc(&delete)
                )),
                Cell::mono(&r.name),
                Cell::text(&r.processors),
                Cell::num(&r.memory),
                Cell::num(&r.adapters.to_string()),
                Cell::text(&r.last),
                Cell::text(if r.default { "Yes" } else { "No" }),
            ]
        })
        .collect();
    let list = List {
        cols: vec![
            Col::plain("Actions"),
            Col::sorted("Profile", "n"),
            Col::plain("Processors"),
            Col::plain("Memory (MB)"),
            Col::plain("Virtual adapters"),
            Col::sorted("Last activated", "l"),
            Col::plain("Default"),
        ],
        rows,
        empty: "The partition has no profiles.".into(),
        keep: Vec::new(),
    };
    let body = format!(
        "{}<form method=\"post\">{}<div class=\"buttons\"><input type=\"submit\" name=\"create\" value=\"Create profile...\"><input type=\"submit\" name=\"save\" value=\"Save current configuration as profile...\"><input type=\"submit\" name=\"default\" value=\"Set default\"></div></form>",
        list_table(&list),
        ctx.csrf_field()
    );
    super::sub_page(
        &v.name,
        "Manage profiles",
        &format!("Manage profiles: {}", v.name),
        v.messages.clone(),
        body,
    )
}

/// The page of partition `id` of `k`, from what is kept.
pub fn view(rq: &Rq<'_>, k: &Kept, id: u32, messages: Vec<Message>) -> Profiles {
    let s = rq.app.settings();
    let zone = rq.app.host().zone();
    let when = |secs: u64| {
        let t = i64::try_from(secs).unwrap_or(0);
        format!(
            "{} {}",
            crate::fmt::date(t, zone, &s.date_format),
            crate::fmt::time(t, zone, &s.time_format, false)
        )
    };
    let mut rows: Vec<Row> = k
        .profile_list(id)
        .iter()
        .map(|e| {
            let body = k.profile_body(id, &e.name);
            Row {
                name: e.name.clone(),
                processors: body.map(|b| processors(&b.profile)).unwrap_or_default(),
                memory: body
                    .map(|b| crate::fmt::number(b.profile.memory.desired_mib, &s.number_format))
                    .unwrap_or_default(),
                adapters: body.map_or(0, |b| adapters(&b.profile)),
                last: k.last_used(id, &e.name).map(when).unwrap_or_default(),
                default: e.default,
            }
        })
        .collect();
    match rq.q("sort").as_str() {
        "n" => rows.sort_by(|a, b| a.name.cmp(&b.name)),
        "l" => rows.sort_by(|a, b| b.last.cmp(&a.last)),
        _ => {}
    }
    Profiles {
        host: k.name.clone(),
        name: super::name_of(k, id),
        id,
        rows,
        messages,
    }
}

/// A short form of one name field (and a choice): Copy, Create, Save
/// current configuration, Set default.
fn name_form(rq: &Rq<'_>, k: &Kept, id: u32, title: &str, what: &str, extra: &str) -> Response {
    let names: Vec<String> = k.profile_list(id).iter().map(|e| e.name.clone()).collect();
    let opts: Vec<&str> = names.iter().map(String::as_str).collect();
    let field = if what == "default" {
        qsweb::partials::select("n", &opts, opts.first().copied().unwrap_or(""))
    } else {
        "<input type=\"text\" name=\"n\" value=\"\" size=\"20\">".into()
    };
    let groups = [PropGroup {
        title: None,
        rows: vec![
            Prop::new(
                if what == "default" {
                    "Profile"
                } else {
                    "New profile's name"
                },
                field,
            )
            .required(),
        ],
    }];
    let body = form(
        &rq.ctx,
        "",
        &format!(
            "{}{extra}{}\n{}",
            hidden("do", what),
            props_table(rq.ctx.product, &groups, true),
            buttons(&[Button::new("ok", "OK"), Button::new("cancel", "Cancel")])
        ),
    );
    let name = super::name_of(k, id);
    rq.page(super::sub_page(
        &name,
        title,
        &format!("{title}: {name}"),
        Vec::new(),
        body,
    ))
}

/// `GET`, `POST /partition/<host>/<id>/profiles?copy=&prof=&save=`.
pub fn route(rq: &Rq<'_>, k: &Kept, id: u32) -> Response {
    let f = rq.req.form();
    let here = format!("/partition/{}/{id}/profiles", k.name);
    if rq.post() && f.has("cancel") {
        return Response::redirect(&rq.ctx.url(&here));
    }
    if rq.post() {
        if let Some(no) = lacks(rq, Task::CreatePartitions) {
            return no;
        }
        if f.has("create") {
            return name_form(rq, k, id, "Create profile", "create", "");
        }
        if f.has("save") {
            return name_form(rq, k, id, "Save current configuration", "save", "");
        }
        if f.has("default") {
            return name_form(rq, k, id, "Set default profile", "default", "");
        }
        let n = f.str("n").trim().to_string();
        let r = match f.str("do") {
            "copy" | "create" => {
                let from = if f.str("do") == "copy" {
                    f.str("from").to_string()
                } else {
                    k.profile_list(id)
                        .iter()
                        .find(|e| e.default)
                        .map(|e| e.name.clone())
                        .unwrap_or_default()
                };
                copy(rq, k, id, &from, &n)
            }
            "save" => ask(
                rq,
                k,
                &format!("save the running configuration of partition {id} as {n}"),
                &AnyRequest::Profiles(profiles::Request::SaveCurrent(profiles::SaveCurrent {
                    id,
                    name: n.clone(),
                    revision: k
                        .profile_list(id)
                        .iter()
                        .find(|e| e.name == n)
                        .map(|e| e.revision),
                })),
                qmcmsg::message(Kind::Done, &qmcmsg::PROFILE_SAVED, &[&n]),
            ),
            "default" => ask(
                rq,
                k,
                &format!("set profile {n} default for partition {id}"),
                &AnyRequest::Profiles(profiles::Request::SetDefault(profiles::SetDefault {
                    id,
                    name: n.clone(),
                })),
                qmcmsg::message(Kind::Done, &qmcmsg::PROFILE_DEFAULT, &[&n]),
            ),
            _ => Err(Vec::new()),
        };
        return match r {
            Ok(m) => rq.done(&here, m),
            Err(ms) => rq.done_all(&here, ms),
        };
    }
    if rq.q("copy") == "1" {
        let from = rq.q("prof");
        let extra = hidden("from", &from);
        return name_form(rq, k, id, &format!("Copy profile {from}"), "copy", &extra);
    }
    if rq.q("save") == "1" {
        return name_form(rq, k, id, "Save current configuration", "save", "");
    }
    rq.page(page(&rq.ctx, &view(rq, k, id, Vec::new())))
}

fn ask(
    rq: &Rq<'_>,
    k: &Kept,
    what: &str,
    req: &AnyRequest,
    done: Message,
) -> Result<Message, Vec<Message>> {
    let r = crate::systems::ask(rq, &k.name, "Profile", req);
    rq.audit(
        &format!("{what} on {}", k.name),
        if r.is_ok() { "done" } else { "refused" },
    );
    r.map(|_| done)
}

/// Copy profile `from` of partition `id` as `to`: a new profile, no
/// revision.
fn copy(rq: &Rq<'_>, k: &Kept, id: u32, from: &str, to: &str) -> Result<Message, Vec<Message>> {
    if to.is_empty() {
        return Err(vec![qmcmsg::message(
            Kind::Error,
            &qmcmsg::NOT_VALID,
            &["Type the new profile's name."],
        )]);
    }
    let Some(b) = k.profile_body(id, from) else {
        let mut s = String::new();
        let _ = write!(s, "Profile {from} is not known to the console yet.");
        return Err(vec![qmcmsg::message(
            Kind::Error,
            &qmcmsg::NOT_VALID,
            &[&s],
        )]);
    };
    let mut p = b.profile.clone();
    p.name = to.to_string();
    ask(
        rq,
        k,
        &format!("copy profile {from} of partition {id} as {to}"),
        &AnyRequest::Profiles(profiles::Request::Put(profiles::Put {
            id,
            profile: p,
            revision: None,
        })),
        qmcmsg::message(Kind::Done, &qmcmsg::PROFILE_COPIED, &[from, to]),
    )
}
