// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! Manage dumps (`dumps.html`): every dump the systems hold, listed from
//! what `mgmtd` keeps of them, and the console's own, with type, size,
//! date, source and status, and the space they take. Download streams a
//! dump from the system through the console to the browser, the console
//! keeping no copy; Delete goes through its confirmation; Copy to remote
//! host answers that this release does not.

use std::fmt::Write as _;

use qsweb::frame::{Ctx, Page};
use qsweb::html::esc;
use qsweb::http::{Body, Response};
use qsweb::msg::{Kind, Message};
use qsweb::partials::{Button, Cell, Col, List, buttons, hidden, list_table, mono};
use webusers::Task;

use crate::app::routes::Rq;

/// A dump, wherever it is.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Dump {
    /// The managed system; empty for the console's own.
    pub system: String,
    /// The partition whose dump it is; 0 for the system's own.
    pub id: u32,
    /// Its partition's name.
    pub partition: String,
    /// Its name.
    pub name: String,
    /// Its type in words.
    pub kind: String,
    /// Its size.
    pub bytes: u64,
    /// When it was taken.
    pub time: u64,
}

impl Dump {
    /// Its key in a form: `<system>:<id>:<name>`.
    pub fn key(&self) -> String {
        format!("{}:{}:{}", self.system, self.id, self.name)
    }

    /// The dump of `key`.
    pub fn of_key(key: &str) -> Option<(String, u32, String)> {
        let mut p = key.splitn(3, ':');
        let system = p.next()?.to_string();
        let id = p.next()?.parse().ok()?;
        let name = p.next().filter(|n| !n.is_empty())?.to_string();
        Some((system, id, name))
    }
}

/// One row.
#[derive(Clone, Debug, Default)]
pub struct Row {
    /// The checkbox's value.
    pub key: String,
    /// The dump's name.
    pub name: String,
    /// Its type.
    pub kind: String,
    /// Its size in MB.
    pub size: String,
    /// `10/29/26 22:15`.
    pub date: String,
    /// `QSTEST01 on QSHQVS1`.
    pub source: String,
    /// `Ready`.
    pub status: String,
}

/// What the page shows.
#[derive(Clone, Debug, Default)]
pub struct Dumps {
    /// The rows.
    pub rows: Vec<Row>,
    /// `Dump space: 12,472 MB of 40,000 MB used.`
    pub space: String,
    /// Each system's space, a line each.
    pub per_system: Vec<String>,
    /// The message rows.
    pub messages: Vec<Message>,
}

/// The page.
pub fn page(ctx: &Ctx, v: &Dumps) -> Page {
    let cols = vec![
        Col::plain("Select"),
        Col::sorted("Name", "n"),
        Col::sorted("Type", "t"),
        Col::sorted("Size (MB)", "s"),
        Col::sorted("Date", "d"),
        Col::sorted("Source", "src"),
        Col::plain("Status"),
    ];
    let rows = v
        .rows
        .iter()
        .enumerate()
        .map(|(i, r)| {
            vec![
                Cell::act(format!(
                    "<input type=\"checkbox\" name=\"d{i}\" value=\"{}\">",
                    esc(&r.key)
                )),
                Cell::mono(&r.name),
                Cell::text(&r.kind),
                Cell::num(&r.size),
                Cell::mono(&r.date),
                Cell::text(&r.source),
                Cell::text(&r.status),
            ]
        })
        .collect();
    let table = list_table(&List {
        cols,
        rows,
        empty: "No dumps.".into(),
        ..List::default()
    });
    let mut inner = table;
    let mut bs = buttons(&[
        Button::new("download", "Download"),
        Button::new("copy", "Copy to remote host..."),
        Button::new("delete", "Delete"),
    ]);
    // The space beside the buttons, as the design has it.
    bs.insert_str(
        bs.len() - "</div>".len(),
        &format!(
            " &nbsp; <span class=\"small grey\">{}</span>",
            esc(&v.space)
        ),
    );
    inner.push_str(&bs);
    let mut body = qsweb::partials::form(ctx, "", &inner);
    if !v.per_system.is_empty() {
        let _ = write!(
            body,
            "<p class=\"small grey\">{}</p>",
            v.per_system
                .iter()
                .map(|l| esc(l))
                .collect::<Vec<_>>()
                .join("<br>")
        );
    }
    Page {
        title: "Manage dumps".into(),
        crumb: vec!["Service Management".into(), "Manage dumps".into()],
        h1: "Manage dumps".into(),
        messages: v.messages.clone(),
        body,
        help: "dumps".into(),
        nav: Some(("Service Management", Some("/dumps"))),
        ..Page::default()
    }
}

/// Megabytes, rounded up, with the console's thousands mark.
fn mb(rq: &Rq<'_>, bytes: u64) -> String {
    crate::fmt::number(bytes.div_ceil(1_048_576), &rq.app.settings().number_format)
}

/// Every dump: the systems' as kept, then the console's own; and each
/// one's space: who, bytes, limit.
/// Each system's dump space: who, bytes, limit.
type Spaces = Vec<(String, u64, u64)>;

fn every(rq: &Rq<'_>) -> (Vec<Dump>, Spaces, Vec<Message>) {
    let mut messages = Vec::new();
    let kept = crate::systems::systems(rq).unwrap_or_else(|m| {
        messages.push(m);
        Vec::new()
    });
    let mut out = Vec::new();
    let mut space = Vec::new();
    for k in &kept {
        for d in &k.dumps {
            out.push(Dump {
                system: k.name.clone(),
                id: 0,
                partition: String::new(),
                name: d.name.clone(),
                kind: format!("{} dump", capital(&d.kind_name)),
                bytes: d.bytes,
                time: d.time,
            });
        }
        for p in &k.part_dumps {
            let partition = k
                .partitions
                .iter()
                .find(|x| x.id == p.id)
                .map_or_else(|| format!("partition {}", p.id), |x| x.name.clone());
            for d in &p.dumps {
                let why = match d.reason.as_str() {
                    "function22" => " (function 22)",
                    "function34" => " (function 34)",
                    _ => "",
                };
                out.push(Dump {
                    system: k.name.clone(),
                    id: p.id,
                    partition: partition.clone(),
                    name: d.name.clone(),
                    kind: format!("Partition main storage dump{why}"),
                    bytes: d.bytes,
                    time: d.time,
                });
            }
        }
        let mut used = 0;
        let mut limit = 0;
        if let Some(s) = &k.dump_space {
            used += s.bytes;
            limit += s.limit;
        }
        for p in &k.part_space {
            used += p.space.bytes;
            limit += p.space.limit;
        }
        if k.dump_space.is_some() || !k.part_space.is_empty() {
            space.push((k.name.clone(), used, limit));
        }
    }
    if let Some(dir) = rq.app.console.dumps() {
        let store = servicedump::Store::at(dir, &consdump::KIND);
        for i in store.list() {
            out.push(Dump {
                system: String::new(),
                id: 0,
                partition: String::new(),
                name: i.name,
                kind: "Console dump".into(),
                bytes: i.bytes,
                time: i.time,
            });
        }
        let (used, limit, _, _) = store.space();
        space.push((rq.app.console.name(), used, limit));
    }
    out.sort_by_key(|d| std::cmp::Reverse(d.time));
    (out, space, messages)
}

fn capital(s: &str) -> String {
    let mut c = s.chars();
    c.next()
        .map_or_else(String::new, |f| f.to_uppercase().chain(c).collect())
}

/// The dumps the form chose.
fn chosen(rq: &Rq<'_>) -> Vec<(String, u32, String)> {
    rq.req
        .form()
        .pairs()
        .iter()
        .filter(|(k, _)| {
            k.starts_with('d') && k[1..].bytes().all(|b| b.is_ascii_digit()) && k.len() > 1
        })
        .filter_map(|(_, v)| Dump::of_key(v))
        .collect()
}

/// `GET /dumps?sort=`, and its form's POST.
pub fn route(rq: &Rq<'_>) -> Response {
    if rq.post() {
        let f = rq.req.form();
        let picked = chosen(rq);
        if f.get("copy").is_some() {
            return rq.done(
                "/dumps",
                qmcmsg::message(Kind::Info, &qmcmsg::NO_REMOTE_COPY, &[]),
            );
        }
        let what = if f.get("delete").is_some() {
            "Delete"
        } else {
            "Download"
        };
        if picked.is_empty() {
            return rq.done(
                "/dumps",
                qmcmsg::message(Kind::Error, &qmcmsg::SELECT_DUMP, &[&what.to_lowercase()]),
            );
        }
        let keys: Vec<String> = picked
            .iter()
            .map(|(s, i, n)| format!("{s}:{i}:{n}"))
            .collect();
        if what == "Delete" {
            let q: Vec<(&str, &str)> = keys.iter().map(|k| ("d", k.as_str())).collect();
            return Response::redirect(
                &rq.ctx
                    .url(&format!("/confirm/dump-delete{}", qsweb::html::query(&q))),
            );
        }
        if keys.len() != 1 {
            return rq.done(
                "/dumps",
                qmcmsg::message(Kind::Error, &qmcmsg::ONE_DUMP, &[]),
            );
        }
        let q = [("d", keys[0].as_str())];
        return Response::redirect(
            &rq.ctx
                .url(&format!("/dumps/download{}", qsweb::html::query(&q))),
        );
    }
    let (mut all, space, messages) = every(rq);
    match rq.q("sort").as_str() {
        "n" => all.sort_by(|a, b| a.name.cmp(&b.name)),
        "t" => all.sort_by(|a, b| a.kind.cmp(&b.kind)),
        "s" => all.sort_by_key(|d| std::cmp::Reverse(d.bytes)),
        "src" => all.sort_by(|a, b| (&a.system, &a.partition).cmp(&(&b.system, &b.partition))),
        _ => {}
    }
    let console = rq.app.console.name();
    let s = rq.app.settings();
    let zone = rq.app.host().zone();
    let rows = all
        .iter()
        .map(|d| {
            let t = i64::try_from(d.time).unwrap_or(0);
            Row {
                key: d.key(),
                name: d.name.clone(),
                kind: d.kind.clone(),
                size: mb(rq, d.bytes),
                date: format!(
                    "{} {}",
                    crate::fmt::date(t, zone, &s.date_format),
                    crate::fmt::time(t, zone, &s.time_format, false)
                ),
                source: if d.system.is_empty() {
                    console.clone()
                } else if d.id == 0 {
                    d.system.clone()
                } else {
                    format!("{} on {}", d.partition, d.system)
                },
                status: "Ready".into(),
            }
        })
        .collect();
    let used: u64 = space.iter().map(|(_, u, _)| u).sum();
    let limit: u64 = space.iter().map(|(_, _, l)| l).sum();
    let line = |used: u64, limit: u64| {
        if limit == 0 {
            format!("{} MB used", mb(rq, used))
        } else {
            format!("{} MB of {} MB used", mb(rq, used), mb(rq, limit))
        }
    };
    let v = Dumps {
        rows,
        space: format!("Dump space: {}.", line(used, limit)),
        per_system: space
            .iter()
            .map(|(who, u, l)| format!("{who}: {}", line(*u, *l)))
            .collect(),
        messages,
    };
    rq.page(page(&rq.ctx, &v))
}

/// The dump `key` names, if it is still listed.
fn find(rq: &Rq<'_>, key: &str) -> Option<Dump> {
    let (system, id, name) = Dump::of_key(key)?;
    every(rq)
        .0
        .into_iter()
        .find(|d| d.system == system && d.id == id && d.name == name)
}

/// `GET /dumps/download?d=`: the dump streamed from its system, or read
/// from the console's own folder; the console keeps no copy.
pub fn download(rq: &Rq<'_>) -> Response {
    if let Some(no) = crate::app::routes::lacks(rq, Task::Events) {
        return no;
    }
    let key = rq.q("d");
    let Some(d) = find(rq, &key) else {
        let (s, _, n) = Dump::of_key(&key).unwrap_or_default();
        return rq.done(
            "/dumps",
            qmcmsg::message(Kind::Error, &qmcmsg::NO_SUCH_DUMP, &[&n, &s]),
        );
    };
    let kind = if std::path::Path::new(&d.name)
        .extension()
        .is_some_and(|x| x.eq_ignore_ascii_case("zip"))
    {
        "application/zip"
    } else {
        "application/octet-stream"
    };
    let what = format!(
        "dump {} of {}",
        d.name,
        if d.system.is_empty() {
            "the console"
        } else {
            &d.system
        }
    );
    let body = if d.system.is_empty() {
        let Some(path) = rq
            .app
            .console
            .dumps()
            .and_then(|dir| servicedump::Store::at(dir, &consdump::KIND).path(&d.name))
        else {
            return rq.done(
                "/dumps",
                qmcmsg::message(
                    Kind::Error,
                    &qmcmsg::NO_SUCH_DUMP,
                    &[&d.name, "the console"],
                ),
            );
        };
        match std::fs::File::open(&path).and_then(|f| f.metadata().map(|m| (m.len(), f))) {
            Ok((len, f)) => Body::Sized(len, Box::new(f)),
            Err(_) => {
                return rq.done(
                    "/dumps",
                    qmcmsg::message(
                        Kind::Error,
                        &qmcmsg::NO_SUCH_DUMP,
                        &[&d.name, "the console"],
                    ),
                );
            }
        }
    } else {
        match from_system(rq, &d) {
            Ok(b) => b,
            Err(ms) => {
                rq.audit(&what, "download refused");
                return rq.done_all("/dumps", ms);
            }
        }
    };
    rq.audit(&what, "downloaded");
    let mut r = Response::download(&d.name, kind, Vec::new());
    r.body = body;
    r
}

/// The dump's bytes from its system through `mgmtd`'s console socket.
fn from_system(rq: &Rq<'_>, d: &Dump) -> Result<Body, Vec<Message>> {
    use mgmtproto::console::{Fetch, Frames, Open};
    let down = |e: &dyn std::fmt::Display| {
        vec![qmcmsg::message(
            Kind::Error,
            &qmcmsg::SERVICE_DOWN,
            &["The managed-systems service", &e.to_string()],
        )]
    };
    let socket = rq
        .app
        .console
        .consoles()
        .ok_or_else(|| down(&"it has no socket"))?;
    #[cfg(unix)]
    {
        let mut s = std::os::unix::net::UnixStream::connect(socket).map_err(|e| down(&e))?;
        let open = Open {
            watch: None,
            upload: None,
            fetch: Some(Fetch {
                system: d.system.clone(),
                id: d.id,
                name: d.name.clone(),
                user: rq.user().into(),
            }),
        };
        ipc::frame::write_frame(&mut s, &ipc::encode(&open)).map_err(|e| down(&e))?;
        let a = ipc::frame::read_frame(&mut s).map_err(|e| down(&e))?;
        let start: Result<u64, mgmtproto::Refusal> = ipc::decode(&a).map_err(|e| down(&e))?;
        let bytes = start.map_err(|no| crate::systems::refused(&no))?;
        let _ = s.set_read_timeout(Some(std::time::Duration::from_mins(2)));
        Ok(Body::Sized(bytes, Box::new(Frames::new(s))))
    }
    #[cfg(not(unix))]
    {
        let _ = (socket, d);
        Err(down(&"not on this host"))
    }
}

/// `/confirm/dump-delete?d=...`: the dumps chosen, deleted on OK.
#[allow(clippy::too_many_lines, reason = "the POST and the page")]
pub fn delete(rq: &Rq<'_>) -> Response {
    if let Some(no) = crate::app::routes::lacks(rq, Task::Events) {
        return no;
    }
    let keys: Vec<String> = if rq.post() {
        rq.req
            .form()
            .pairs()
            .iter()
            .filter(|(k, _)| k == "d")
            .map(|(_, v)| v.clone())
            .collect()
    } else {
        rq.req
            .query()
            .all("d")
            .into_iter()
            .map(str::to_string)
            .collect()
    };
    let dumps: Vec<Dump> = keys.iter().filter_map(|k| find(rq, k)).collect();
    if dumps.is_empty() {
        return rq.done(
            "/dumps",
            qmcmsg::message(Kind::Error, &qmcmsg::SELECT_DUMP, &["delete"]),
        );
    }
    if rq.post() {
        if rq.req.form().get("ok").is_none() {
            return Response::redirect(&rq.ctx.url("/dumps"));
        }
        let mut ms = Vec::new();
        for d in &dumps {
            let who = if d.system.is_empty() {
                rq.app.console.name()
            } else {
                d.system.clone()
            };
            let what = format!("dump {} of {who}", d.name);
            let done = if d.system.is_empty() {
                rq.app
                    .console
                    .dumps()
                    .map_or_else(
                        || Err("no dumps folder".into()),
                        |dir| servicedump::Store::at(dir, &consdump::KIND).delete(&d.name),
                    )
                    .map_err(|e| vec![qmcmsg::message(Kind::Error, &qmcmsg::NOT_VALID, &[&e])])
                    .map(|()| {
                        #[cfg(unix)]
                        refcode::post::remote::code(
                            &refcode::DUMP_REMOVED,
                            &format!("{}, by {}", d.name, rq.user()),
                        );
                    })
            } else {
                let req = if d.id == 0 {
                    qsi::any::AnyRequest::Base(qsi::base::Request::DeleteDump(
                        qsi::base::DeleteDump {
                            name: d.name.clone(),
                        },
                    ))
                } else {
                    qsi::any::AnyRequest::Dumps(qsi::host::dumps::Request::Delete(
                        qsi::host::dumps::Delete {
                            id: d.id,
                            name: d.name.clone(),
                        },
                    ))
                };
                crate::systems::ask(rq, &d.system, "Delete dump", &req).map(|_| ())
            };
            match done {
                Ok(()) => {
                    rq.audit(&what, "deleted");
                    ms.push(qmcmsg::message(
                        Kind::Done,
                        &qmcmsg::DUMP_DELETED,
                        &[&d.name, &who],
                    ));
                }
                Err(e) => {
                    rq.audit(&what, "delete refused");
                    ms.extend(e);
                }
            }
        }
        return rq.done_all("/dumps", ms);
    }
    let mut table = String::from("<table class=\"props\" cellspacing=\"0\">");
    let mut options = String::new();
    for d in &dumps {
        let who = if d.system.is_empty() {
            rq.app.console.name()
        } else if d.id == 0 {
            d.system.clone()
        } else {
            format!("{} on {}", d.partition, d.system)
        };
        let _ = write!(
            table,
            "<tr><th>{}</th><td>{} ({who})</td></tr>",
            esc(&d.kind),
            mono(&d.name)
        );
        options.push_str(&hidden("d", &d.key()));
    }
    table.push_str("</table>");
    let body = qsweb::pages::confirm_body(
        &rq.ctx,
        "Each dump is deleted at the system that holds it and cannot be had again. Press OK to delete them, or Cancel to return without it.",
        &table,
        &options,
    );
    rq.page(Page {
        title: "Confirm Delete Dump".into(),
        crumb: vec!["Service Management".into(), "Manage dumps".into()],
        h1: "Confirm Delete Dump".into(),
        body,
        help: "dumps".into(),
        nav: Some(("Service Management", Some("/dumps"))),
        ..Page::default()
    })
}

/// `POST /dumps/take`: a console dump asked of PID 1.
pub fn take(rq: &Rq<'_>) -> Response {
    if let Some(no) = crate::app::routes::lacks(rq, Task::Events) {
        return no;
    }
    if !rq.post() {
        return Response::redirect(&rq.ctx.url("/dumps"));
    }
    match rq.app.console.take_dump(&format!("web user {}", rq.user())) {
        Ok(name) => {
            rq.audit("console dump", "taken");
            rq.done(
                "/dumps",
                qmcmsg::message(Kind::Done, &qmcmsg::DUMP_TAKEN, &[&name]),
            )
        }
        Err(e) => {
            rq.audit("console dump", "refused");
            rq.done(
                "/dumps",
                qmcmsg::message(Kind::Error, &qmcmsg::NOT_VALID, &[&e]),
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_dump_s_key_names_it_whole() {
        let d = Dump {
            system: "QSHQVS1".into(),
            id: 2,
            name: "QSTEST01-1794000000.core".into(),
            ..Dump::default()
        };
        assert_eq!(
            Dump::of_key(&d.key()),
            Some(("QSHQVS1".into(), 2, "QSTEST01-1794000000.core".into()))
        );
        assert_eq!(
            Dump::of_key(":0:QSMCDUMP-20261122-000000.zip").unwrap().0,
            ""
        );
        assert!(Dump::of_key("x:y:z").is_none());
        assert!(Dump::of_key("x:1:").is_none());
    }
}
