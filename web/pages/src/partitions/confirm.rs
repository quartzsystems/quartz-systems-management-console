// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! The partitions' destructive actions, each through its confirmation
//! page and a POST with its token: a partition deleted, with its
//! virtual disks kept or deleted as chosen there; a virtual adapter
//! taken off its default profile; a profile deleted.

use std::fmt::Write as _;

use qsweb::http::Response;
use qsweb::msg::Kind;
use qsweb::partials::{hidden, mono};

use mgmtproto::Kept;
use qsi::any::AnyRequest;
use qsi::host::{partition, profiles};

use crate::app::routes::Rq;

/// The confirmations this module answers.
pub const ROUTES: [&str; 3] = [
    "/confirm/partition-delete",
    "/confirm/adapter-remove",
    "/confirm/profile-delete",
];

fn value(rq: &Rq<'_>, n: &str) -> String {
    if rq.post() {
        rq.req.form().str(n).to_string()
    } else {
        rq.q(n)
    }
}

/// The disks partition `id`'s profiles name: `POOL/NAME`.
fn disks(k: &Kept, id: u32) -> Vec<String> {
    let mut v: Vec<String> = k
        .profile_bodies
        .iter()
        .filter(|b| b.id == id)
        .flat_map(|b| {
            b.profile
                .scsi
                .iter()
                .flat_map(|a| a.luns.iter().map(|l| l.backing.clone()))
        })
        .collect();
    v.sort();
    v.dedup();
    v
}

/// `GET`, `POST /confirm/<what>?ms=&p=&s=&n=`.
#[allow(clippy::too_many_lines, reason = "three confirmations")]
pub fn route(rq: &Rq<'_>, route: &str) -> Response {
    let host = value(rq, "ms");
    let Some(k) = crate::systems::system(rq, &host) else {
        return crate::app::routes::error(rq, 404, &qmcmsg::NO_SUCH_SYSTEM, &[&host]);
    };
    let id: u32 = value(rq, "p").parse().unwrap_or(0);
    let Some(p) = k.part(id) else {
        return crate::app::routes::error(rq, 404, &qmcmsg::NOT_FOUND, &[&rq.req.path]);
    };
    let (slot, name) = (value(rq, "s"), value(rq, "n"));
    let (back, title, sentence, rows) = match route {
        "/confirm/partition-delete" => (
            "/partitions".to_string(),
            "Confirm Delete Partition",
            "The partition and its profiles are deleted from the host. Its virtual disks are kept unless you choose to delete them too.",
            vec![
                ("Partition", format!("{} (ID {id})", mono(&p.name))),
                ("Managed system", mono(&k.name)),
                ("Virtual disks", mono(&disks(&k, id).join(", "))),
            ],
        ),
        "/confirm/adapter-remove" => (
            format!("/partition/{}/{id}/vadapters", k.name),
            "Confirm Remove Virtual Adapter",
            "The adapter is taken off the partition's default profile; a running partition keeps it until its next activation.",
            vec![
                ("Partition", mono(&p.name)),
                ("Adapter ID", qsweb::html::esc(&slot)),
            ],
        ),
        _ => (
            format!("/partition/{}/{id}/profiles", k.name),
            "Confirm Delete Profile",
            "The profile is deleted from the host. The default profile cannot be deleted.",
            vec![("Partition", mono(&p.name)), ("Profile", mono(&name))],
        ),
    };
    if let Some(no) = crate::app::routes::lacks(rq, webusers::Task::CreatePartitions) {
        return no;
    }
    if rq.post() {
        let f = rq.req.form();
        if f.get("ok").is_none() {
            return Response::redirect(&rq.ctx.url(&back));
        }
        let r = match route {
            "/confirm/partition-delete" => {
                let with = f.has("disks");
                let what = format!(
                    "delete partition {} of {}{}",
                    p.name,
                    k.name,
                    if with { " and its virtual disks" } else { "" }
                );
                let req = AnyRequest::Partition(partition::Request::Delete(partition::Delete {
                    id,
                    disks: with,
                }));
                let r = crate::systems::ask(rq, &k.name, "Delete partition", &req);
                rq.audit(&what, if r.is_ok() { "deleted" } else { "refused" });
                r.map(|_| qmcmsg::message(Kind::Done, &qmcmsg::PARTITION_DELETED, &[&p.name]))
            }
            "/confirm/adapter-remove" => {
                let s: u32 = slot.parse().unwrap_or(0);
                if s < 2 {
                    Err(vec![qmcmsg::message(
                        Kind::Error,
                        &qmcmsg::SERIAL_FIXED,
                        &[],
                    )])
                } else {
                    crate::vres::confirm::edit_default(rq, &k, id, |pr| {
                        pr.ethernet.retain(|a| a.slot != s);
                        pr.scsi.retain(|a| a.slot != s);
                        pr.optical.retain(|a| a.slot != s);
                        pr.tape.retain(|a| a.slot != s);
                    })
                    .map(|()| qmcmsg::message(Kind::Done, &qmcmsg::PARTITION_CHANGED, &[&p.name]))
                }
            }
            _ => {
                let req = AnyRequest::Profiles(profiles::Request::Delete(profiles::Delete {
                    id,
                    name: name.clone(),
                }));
                let r = crate::systems::ask(rq, &k.name, "Delete profile", &req);
                rq.audit(
                    &format!("delete profile {name} of {} on {}", p.name, k.name),
                    if r.is_ok() { "deleted" } else { "refused" },
                );
                r.map(|_| qmcmsg::message(Kind::Done, &qmcmsg::PROFILE_DELETED, &[&name]))
            }
        };
        return match r {
            Ok(m) => rq.done(&back, m),
            Err(ms) => rq.done_all(&back, ms),
        };
    }
    let mut table = String::from("<table class=\"props\" cellspacing=\"0\">");
    for (l, v) in rows {
        let _ = write!(table, "<tr><th>{l}</th><td>{v}</td></tr>");
    }
    table.push_str("</table>");
    let mut options = hidden("ms", &k.name)
        + &hidden("p", &id.to_string())
        + &hidden("s", &slot)
        + &hidden("n", &name);
    if route == "/confirm/partition-delete" {
        options.push_str(
            "<p><input type=\"checkbox\" name=\"disks\"> Delete its virtual disks too</p>",
        );
    }
    let body = qsweb::pages::confirm_body(
        &rq.ctx,
        &format!("{sentence} Press OK to do it, or Cancel to return without it."),
        &table,
        &options,
    );
    rq.page(super::sub_page(&p.name, title, title, Vec::new(), body))
}
