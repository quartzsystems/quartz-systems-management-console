// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! The confirmations of the virtual resources' destructive actions:
//! `/confirm/<what>?ms=&...`, a GET page that lists what is affected and
//! a POST with its token that does it. Nothing is done by GET.

use std::fmt::Write as _;

use qsweb::html::esc;
use qsweb::http::Response;
use qsweb::msg::Kind;
use qsweb::partials::hidden;

use mgmtproto::Kept;
use qsi::any::{AnyReply, AnyRequest};
use qsi::host::{partitions, profiles, vres};

use crate::app::routes::{Rq, lacks};

/// The confirmations this module answers, by route.
pub const ROUTES: [&str; 7] = [
    "/confirm/switch-delete",
    "/confirm/vdisk-delete",
    "/confirm/vdisk-unassign",
    "/confirm/image-delete",
    "/confirm/tape-remove",
    "/confirm/volume-delete",
    "/confirm/proc-pool-delete",
];

/// What one confirmation does.
struct What {
    title: &'static str,
    sentence: String,
    rows: Vec<(&'static str, String)>,
    tab: &'static str,
}

fn value(rq: &Rq<'_>, n: &str) -> String {
    if rq.post() {
        rq.req.form().str(n).to_string()
    } else {
        rq.q(n)
    }
}

/// `GET`, `POST /confirm/<what>`.
pub fn route(rq: &Rq<'_>, route: &str) -> Response {
    let host = value(rq, "ms");
    let Some(k) = crate::systems::system(rq, &host) else {
        return crate::app::routes::error(rq, 404, &qmcmsg::NO_SUCH_SYSTEM, &[&host]);
    };
    let (name, pool, slot) = (value(rq, "n"), value(rq, "p"), value(rq, "s"));
    let Some(what) = describe(&k, route, &name, &pool, &slot) else {
        return crate::app::routes::error(rq, 404, &qmcmsg::NOT_FOUND, &[&rq.req.path]);
    };
    let back = format!("/server/{}/virtual/{}", k.name, what.tab);
    if let Some(no) = lacks(rq, webusers::Task::VirtualResources) {
        return no;
    }
    if rq.post() {
        if rq.req.form().get("ok").is_none() {
            return Response::redirect(&rq.ctx.url(&back));
        }
        let force = rq.req.form().has("force");
        return match carry_out(rq, &k, route, &name, &pool, &slot, force) {
            Ok(m) => rq.done(&back, m),
            Err(ms) => rq.done_all(&back, ms),
        };
    }
    let table = format!(
        "<table class=\"props\" cellspacing=\"0\">{}</table>",
        what.rows.iter().fold(String::new(), |mut out, (l, v)| {
            let _ = write!(out, "<tr><th>{l}</th><td>{v}</td></tr>");
            out
        })
    );
    let mut options =
        hidden("ms", &k.name) + &hidden("n", &name) + &hidden("p", &pool) + &hidden("s", &slot);
    if route == "/confirm/vdisk-delete" || route == "/confirm/switch-delete" {
        options.push_str(
            "<p><input type=\"checkbox\" name=\"force\"> Even when a profile names it</p>",
        );
    }
    let body = qsweb::pages::confirm_body(
        &rq.ctx,
        &format!(
            "{} Press OK to do it, or Cancel to return without it.",
            what.sentence
        ),
        &table,
        &options,
    );
    rq.page(super::form_page(
        &k.name,
        what.tab,
        what.title,
        Vec::new(),
        body,
    ))
}

#[allow(
    clippy::many_single_char_names,
    reason = "the fields as the form names them"
)]
fn describe(k: &Kept, route: &str, n: &str, p: &str, s: &str) -> Option<What> {
    let host = super::forms::named(&k.name);
    let r = &k.resources;
    Some(match route {
        "/confirm/switch-delete" => {
            let sw = r.switches.iter().find(|x| x.name == n)?;
            What {
                title: "Confirm Delete Virtual Switch",
                sentence: "The virtual switch is deleted from the host; a partition's adapter on it no longer connects.".into(),
                rows: vec![
                    ("Switch", super::forms::named(&sw.name)),
                    ("Host", host),
                    ("VLAN IDs", esc(&sw.vlans.iter().map(u16::to_string).collect::<Vec<_>>().join(", "))),
                ],
                tab: "ethernet",
            }
        }
        "/confirm/vdisk-delete" | "/confirm/vdisk-unassign" => {
            let d = r.vdisks.iter().find(|x| x.name == n && x.pool == p)?;
            let delete = route.ends_with("delete");
            What {
                title: if delete {
                    "Confirm Delete Virtual Disk"
                } else {
                    "Confirm Unassign Virtual Disk"
                },
                sentence: if delete {
                    "The virtual disk and everything on it are deleted."
                } else {
                    "The virtual disk is taken off the partition's profile; the disk and its data stay in the pool."
                }
                .into(),
                rows: vec![
                    ("Virtual disk", super::forms::named(&format!("{}/{}", d.pool, d.name))),
                    ("Host", host),
                    ("Size", format!("{} MiB", d.bytes >> 20)),
                ],
                tab: "storage",
            }
        }
        "/confirm/image-delete" => {
            let i = r.images.iter().find(|x| x.name == n)?;
            What {
                title: "Confirm Delete Image",
                sentence: "The image is deleted from the host's media library.".into(),
                rows: vec![("Image", super::forms::named(&i.name)), ("Host", host)],
                tab: "optical",
            }
        }
        "/confirm/tape-remove" => What {
            title: "Confirm Remove Tape Device",
            sentence: "The tape device is taken off the partition's default profile; its volume stays in the media library.".into(),
            rows: vec![
                ("Partition", super::forms::named(p)),
                ("Adapter ID", esc(s)),
                ("Host", host),
            ],
            tab: "tape",
        },
        "/confirm/volume-delete" => {
            let v = r.volumes.iter().find(|x| x.name == n)?;
            What {
                title: "Confirm Delete Tape Volume",
                sentence: "The tape volume and everything on it are deleted.".into(),
                rows: vec![("Volume", super::forms::named(&v.name)), ("Host", host)],
                tab: "tape",
            }
        }
        "/confirm/proc-pool-delete" => What {
            title: "Confirm Delete Processor Pool",
            sentence: "The shared processor pool is deleted; its cores go back to the host's default pool.".into(),
            rows: vec![("Pool", super::forms::named(n)), ("Host", host)],
            tab: "pools",
        },
        _ => return None,
    })
}

#[allow(
    clippy::too_many_arguments,
    clippy::many_single_char_names,
    reason = "the confirmation's fields as the form names them"
)]
fn carry_out(
    rq: &Rq<'_>,
    k: &Kept,
    route: &str,
    n: &str,
    p: &str,
    s: &str,
    force: bool,
) -> Result<qsweb::msg::Message, Vec<qsweb::msg::Message>> {
    let done = |def: &qmcmsg::Def, a: &str| qmcmsg::message(Kind::Done, def, &[a, &k.name]);
    let v = |req: vres::Request| AnyRequest::Vres(req);
    match route {
        "/confirm/switch-delete" => super::act(
            rq,
            k,
            &format!("delete switch {n} on {}", k.name),
            &v(vres::Request::SwitchDelete(vres::SwitchDelete {
                name: n.into(),
                force,
            })),
        )
        .map(|_| done(&qmcmsg::SWITCH_DELETED, n)),
        "/confirm/vdisk-delete" => super::act(
            rq,
            k,
            &format!("delete virtual disk {p}/{n} on {}", k.name),
            &v(vres::Request::VdiskDelete(vres::VdiskDelete {
                pool: p.into(),
                name: n.into(),
                force,
            })),
        )
        .map(|_| done(&qmcmsg::VDISK_DELETED, n)),
        "/confirm/image-delete" => super::act(
            rq,
            k,
            &format!("delete image {n} on {}", k.name),
            &v(vres::Request::ImageDelete(vres::ImageDelete {
                name: n.into(),
            })),
        )
        .map(|_| done(&qmcmsg::IMAGE_DELETED, n)),
        "/confirm/volume-delete" => super::act(
            rq,
            k,
            &format!("delete tape volume {n} on {}", k.name),
            &v(vres::Request::VolumeDelete(vres::VolumeDelete {
                name: n.into(),
            })),
        )
        .map(|_| done(&qmcmsg::VOLUME_DELETED, n)),
        "/confirm/proc-pool-delete" => super::act(
            rq,
            k,
            &format!("delete processor pool {n} on {}", k.name),
            &v(vres::Request::ProcPoolDelete(vres::ProcPoolDelete {
                name: n.into(),
            })),
        )
        .map(|_| done(&qmcmsg::PROC_POOL_DELETED, n)),
        "/confirm/vdisk-unassign" => {
            let backing = format!("{p}/{n}");
            let holder = k
                .resources
                .vdisks
                .iter()
                .find(|d| d.pool == p && d.name == n)
                .map_or(0, |d| d.holder.partition);
            let owners: Vec<u32> = if holder == 0 {
                k.partitions
                    .iter()
                    .filter(|x| {
                        k.profile(x.id).is_some_and(|pr| {
                            pr.scsi
                                .iter()
                                .any(|a| a.luns.iter().any(|l| l.backing == backing))
                        })
                    })
                    .map(|x| x.id)
                    .collect()
            } else {
                vec![holder]
            };
            for id in owners {
                edit_default(rq, k, id, |pr| {
                    for a in &mut pr.scsi {
                        a.luns.retain(|l| l.backing != backing);
                    }
                    pr.scsi.retain(|a| !a.luns.is_empty());
                })?;
            }
            Ok(done(&qmcmsg::VDISK_UNASSIGNED, n))
        }
        "/confirm/tape-remove" => {
            let slot: u32 = s.parse().unwrap_or(0);
            let Some(x) = k.partitions.iter().find(|x| x.name == p) else {
                return Err(Vec::new());
            };
            edit_default(rq, k, x.id, |pr| pr.tape.retain(|t| t.slot != slot))?;
            Ok(qmcmsg::message(Kind::Done, &qmcmsg::TAPE_REMOVED, &[s, p]))
        }
        _ => Err(Vec::new()),
    }
}

/// Change partition `id`'s default profile by `edit`, put back with the
/// revision read.
pub fn edit_default(
    rq: &Rq<'_>,
    k: &Kept,
    id: u32,
    edit: impl FnOnce(&mut qsi::host::model::Profile),
) -> Result<(), Vec<qsweb::msg::Message>> {
    let get = AnyRequest::Partitions(partitions::Request::Get(partitions::Get { id }));
    let AnyReply::Partitions(partitions::Reply::Get(def)) =
        crate::systems::ask(rq, &k.name, "Read partition", &get)?
    else {
        return Err(Vec::new());
    };
    let read = AnyRequest::Profiles(profiles::Request::Get(profiles::Get {
        id,
        name: def.definition.default_profile.clone(),
    }));
    let AnyReply::Profiles(profiles::Reply::Get(mut rev)) =
        crate::systems::ask(rq, &k.name, "Read profile", &read)?
    else {
        return Err(Vec::new());
    };
    edit(&mut rev.profile);
    let put = AnyRequest::Profiles(profiles::Request::Put(profiles::Put {
        id,
        profile: rev.profile,
        revision: Some(rev.revision),
    }));
    super::act(
        rq,
        k,
        &format!(
            "change profile {} of {}",
            def.definition.default_profile, def.definition.name
        ),
        &put,
    )
    .map(|_| ())
}
