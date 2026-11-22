// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! A serviceable event (`event-detail.html`): the problem read from the
//! system that owns it, its reference code explained, what it is about
//! as the FRU table, its analysis log; Close through its confirmation,
//! Add note, Export as a text file, and Repair, a page of the recovery
//! text with the resources named, which changes nothing. Close and a
//! note go to the owning system in the signed-on user's name.

use std::fmt::Write as _;

use qsweb::frame::{Ctx, Page};
use qsweb::html::esc;
use qsweb::http::Response;
use qsweb::msg::{Kind, Message};
use qsweb::partials::{Cell, Col, List, hidden, list_table, mono, status};
use webusers::Task;

use super::{Ev, Owner};
use crate::app::routes::Rq;

/// A row of the FRU table.
#[derive(Clone, Debug, Default)]
pub struct Fru {
    /// `High`, `Medium`, `Low`.
    pub priority: String,
    /// What it is, in words.
    pub fru: String,
    /// Where it is.
    pub location: String,
    /// Its part or identifier.
    pub part: String,
    /// What to do with it.
    pub action: String,
}

/// What the page shows.
#[derive(Clone, Debug, Default)]
pub struct Event {
    /// The console's number.
    pub number: String,
    /// The owning system's own number; empty when it is the console's.
    pub theirs: String,
    /// Open.
    pub open: bool,
    /// When it was reported.
    pub reported: String,
    /// The managed system and its page; no page for the console.
    pub system: (String, String),
    /// The partition, its page and ID; none for the system.
    pub partition: Option<(String, String, u32)>,
    /// The reference code and its explanation's link.
    pub code: (String, String),
    /// Words 2 to 9.
    pub words: Vec<String>,
    /// How grave, in words.
    pub severity: String,
    /// The description.
    pub description: String,
    /// The code's meaning, cause and recovery.
    pub explained: Option<(String, String, String)>,
    /// What it is about.
    pub frus: Vec<Fru>,
    /// The analysis log, a line each.
    pub log: Vec<String>,
    /// The message rows.
    pub messages: Vec<Message>,
}

/// The page.
#[allow(clippy::too_many_lines, reason = "the page top to bottom")]
pub fn page(ctx: &Ctx, v: &Event) -> Page {
    let mut t = String::from(
        "<table class=\"props\" cellspacing=\"0\">\n<tr class=\"subrow\"><td colspan=\"2\">Problem</td></tr>\n",
    );
    let _ = writeln!(
        t,
        "<tr><th>Problem number</th><td>{}</td></tr>",
        esc(&v.number)
    );
    if !v.theirs.is_empty() {
        let _ = writeln!(
            t,
            "<tr><th>Managed system's number</th><td>{}</td></tr>",
            esc(&v.theirs)
        );
    }
    let _ = writeln!(
        t,
        "<tr><th>Status</th><td>{}</td></tr>",
        if v.open {
            status("Open", Some(false))
        } else {
            "Closed".into()
        }
    );
    let _ = writeln!(t, "<tr><th>Reported</th><td>{}</td></tr>", esc(&v.reported));
    let system = if v.system.1.is_empty() {
        esc(&v.system.0)
    } else {
        format!(
            "<a href=\"{}\">{}</a>",
            esc(&ctx.url(&v.system.1)),
            esc(&v.system.0)
        )
    };
    let _ = writeln!(t, "<tr><th>Managed system</th><td>{system}</td></tr>");
    if let Some((name, link, id)) = &v.partition {
        let _ = writeln!(
            t,
            "<tr><th>Partition</th><td><a href=\"{}\">{}</a> (ID {id})</td></tr>",
            esc(&ctx.url(link)),
            esc(name)
        );
    }
    let mut words = v.words.clone();
    words.resize(8, "00000000".into());
    let _ = writeln!(
        t,
        "<tr><th>Reference code</th><td><a class=\"mono\" href=\"{}\">{}</a> &nbsp; words 2-9: {}</td></tr>",
        esc(&ctx.url(&v.code.1)),
        esc(&v.code.0),
        mono(&words.join(" "))
    );
    let _ = writeln!(t, "<tr><th>Severity</th><td>{}</td></tr>", esc(&v.severity));
    t.push_str("</table>");
    let _ = write!(
        t,
        "<h2>Problem description</h2><p>{}</p>",
        esc(&v.description)
    );
    let _ = write!(t, "<h2>Reference code {}</h2><p>", esc(&v.code.0));
    match &v.explained {
        Some((meaning, cause, recovery)) => {
            t.push_str(&esc(meaning));
            if !cause.is_empty() {
                let _ = write!(t, " <b>Cause:</b> {}", esc(cause));
            }
            if !recovery.is_empty() {
                let _ = write!(t, " <b>Recovery:</b> {}", esc(recovery));
            }
        }
        None => t.push_str("This release has no explanation of the code."),
    }
    let _ = write!(
        t,
        " <a href=\"{}\">Full explanation</a></p><h2>Field replaceable units</h2>",
        esc(&ctx.url(&v.code.1))
    );
    let cols = ["Priority", "FRU", "Location", "Part / identifier", "Action"]
        .map(Col::plain)
        .to_vec();
    let rows = v
        .frus
        .iter()
        .map(|f| {
            vec![
                Cell::text(&f.priority),
                Cell::text(&f.fru),
                Cell::text(&f.location),
                Cell::text(&f.part),
                Cell::text(&f.action),
            ]
        })
        .collect();
    t.push_str(&list_table(&List {
        cols,
        rows,
        empty: "The problem names no resource.".into(),
        ..List::default()
    }));
    let log: Vec<String> = v.log.iter().map(|l| esc(l)).collect();
    let _ = write!(
        t,
        "<h2>Analysis log</h2><pre class=\"out\">{}</pre>",
        log.join("\n")
    );
    let mut bs = vec![qsweb::partials::Button::new("repair", "Repair...")];
    if v.open {
        bs.push(qsweb::partials::Button::new("close_event", "Close event"));
        bs.push(qsweb::partials::Button::new("add_note", "Add note"));
    }
    bs.push(qsweb::partials::Button::new("export", "Export"));
    t.push_str(&qsweb::partials::form(
        ctx,
        "",
        &qsweb::partials::buttons(&bs),
    ));
    Page {
        title: format!("Serviceable event {}", v.number),
        crumb: vec![
            "Service Management".into(),
            "Serviceable events".into(),
            v.number.clone(),
        ],
        h1: format!("Serviceable event {}", v.number),
        messages: v.messages.clone(),
        body: t,
        help: "events".into(),
        nav: Some(("Service Management", Some("/events"))),
        ..Page::default()
    }
}

/// A resource's kind in words, and what to do with one.
fn kind_words(kind: qsi::service::ResourceKind) -> (&'static str, &'static str) {
    use qsi::service::ResourceKind as K;
    match kind {
        K::DiskUnit => ("Disk unit", "Check the unit; replace its disk if it failed"),
        K::StoragePool => ("Storage pool", "Check the pool and its units"),
        K::VirtualDisk => ("Virtual disk", "Check its pool; run storage verification"),
        K::NetworkInterface => ("Network interface", "Check the cable and the switch port"),
        K::VirtualSwitch => ("Virtual switch", "Check the switch and its uplink"),
        K::Partition => (
            "Partition",
            "See the partition's reference codes and terminal window",
        ),
        K::CodeSlot => ("Code slot", "See the system's code levels"),
        K::System => ("Managed system", "See the system's reference codes"),
        K::Other(_) => ("Resource", "See the system's reference codes"),
    }
}

/// A console resource's kind (`supervise::control::about`) in words.
fn about_words(kind: &str) -> (&'static str, &'static str) {
    use supervise::control::about as a;
    match kind {
        a::DISK_UNIT => ("Load-source unit", "Return or replace the disk of the unit"),
        a::NETWORK_INTERFACE => ("Network interface", "Check the cable and the switch port"),
        a::CODE_SLOT => ("Code slot", "See the console's code levels"),
        a::SERVICE => (
            "Service",
            "Restart the console; report it with a console dump",
        ),
        a::SYSTEM => ("Managed system", "See Connectivity"),
        _ => ("Console", "See the console's reference codes"),
    }
}

/// What is read of one event now: the row, its resources as FRU rows,
/// its log as lines, its system's partition ID.
struct Read {
    ev: Ev,
    frus: Vec<Fru>,
    log: Vec<String>,
}

fn line(rq: &Rq<'_>, time: u64, text: &str) -> String {
    format!("{}  {text}", crate::systems::when(rq, time))
}

/// Event `number` of `owner`, read from whoever keeps it.
fn read(rq: &Rq<'_>, owner: Owner, number: u64) -> Result<Read, Message> {
    match owner {
        Owner::Fleet => {
            let d = rq
                .app
                .console
                .mgmt()
                .problem(number, rq.user().into())
                .map_err(|e| crate::systems::unreached(&e))?
                .map_err(|r| {
                    crate::systems::refused(&r)
                        .into_iter()
                        .next()
                        .unwrap_or_else(|| {
                            qmcmsg::message(Kind::Error, &qmcmsg::NO_REPLY, &["mgmtd"])
                        })
                })?;
            let ev = Ev::fleet(&d.row);
            let frus = d
                .resources
                .iter()
                .enumerate()
                .map(|(i, r)| {
                    let (kind, action) = kind_words(r.kind);
                    // A virtual resource is named as one by its system.
                    let fru = if r.name.to_lowercase().starts_with(&kind.to_lowercase()) {
                        r.name.clone()
                    } else {
                        format!("{kind} {}", r.name)
                    };
                    Fru {
                        priority: ev.grave.priority(i).into(),
                        fru,
                        location: r.location.clone(),
                        part: if r.serial.is_empty() {
                            r.name.clone()
                        } else {
                            r.serial.clone()
                        },
                        action: action.into(),
                    }
                })
                .collect();
            let log = d.log.iter().map(|l| line(rq, l.time, &l.text)).collect();
            Ok(Read { ev, frus, log })
        }
        Owner::Console => {
            let p = rq
                .app
                .console
                .problems()
                .get_rated(number)
                .map_err(|e| {
                    qmcmsg::message(
                        Kind::Error,
                        &qmcmsg::SERVICE_DOWN,
                        &["The console's problem service", &e.to_string()],
                    )
                })?
                .map_err(|is| issued(&is))?;
            let name = rq.app.console.name();
            let ev = Ev::console(
                &problem::Rated {
                    severity: p.severity,
                    summary: p.problem.summary.clone(),
                },
                &name,
            );
            let frus = p
                .problem
                .resources
                .iter()
                .enumerate()
                .map(|(i, a)| {
                    let (kind, action) = about_words(&a.kind);
                    Fru {
                        priority: ev.grave.priority(i).into(),
                        fru: format!("{kind} {}", a.name),
                        location: a.location.clone(),
                        part: if a.serial.is_empty() {
                            a.name.clone()
                        } else {
                            a.serial.clone()
                        },
                        action: action.into(),
                    }
                })
                .collect();
            let log = p
                .problem
                .log
                .iter()
                .map(|l| line(rq, l.time, &l.text))
                .collect();
            Ok(Read { ev, frus, log })
        }
    }
}

/// The console's own refusals as message rows.
fn issued(is: &[msg::Issued]) -> Message {
    is.first().map_or_else(
        || qmcmsg::message(Kind::Error, &qmcmsg::PRB_NOT_KEPT, &[]),
        |i| qsweb::msg::Message::new(Kind::Error, &i.id, &i.text),
    )
}

/// The event's view.
fn view(rq: &Rq<'_>, r: &Read) -> Event {
    let ev = &r.ev;
    let explained = crate::refcodes::explain::explain(rq.app.catalogue(), &ev.code)
        .map(|x| (x.meaning, x.cause, x.recovery));
    let partition = (ev.partition_id != 0).then(|| {
        (
            ev.partition.clone(),
            format!("/partition/{}/{}/general", ev.system, ev.partition_id),
            ev.partition_id,
        )
    });
    let (system_link, code_link) = match ev.owner {
        Owner::Fleet => (
            format!("/server/{}/general", ev.system),
            format!(
                "/refcode/{}?ms={}&p={}",
                ev.code, ev.system, ev.partition_id
            ),
        ),
        Owner::Console => (String::new(), format!("/refcode/{}", ev.code)),
    };
    Event {
        number: ev.number.to_string(),
        theirs: if ev.owner == Owner::Fleet {
            ev.theirs.to_string()
        } else {
            String::new()
        },
        open: ev.open,
        reported: crate::systems::when(rq, ev.first),
        system: (ev.system.clone(), system_link),
        partition,
        code: (ev.code.clone(), code_link),
        words: ev.words.clone(),
        severity: ev.grave.words().into(),
        description: ev.description.clone(),
        explained,
        frus: r.frus.clone(),
        log: r.log.clone(),
        messages: Vec::new(),
    }
}

/// Which event a route names: `/event/<n>` or `/event/console/<n>`, and
/// what follows it.
fn named(route: &str) -> Option<(Owner, u64, &str)> {
    let rest = route.strip_prefix("/event/")?;
    let (owner, rest) = match rest.strip_prefix("console/") {
        Some(r) => (Owner::Console, r),
        None => (Owner::Fleet, rest),
    };
    let (n, tail) = rest.split_once('/').unwrap_or((rest, ""));
    Some((owner, n.parse().ok().filter(|n| *n > 0)?, tail))
}

/// `/event/<n>[/repair|/note|/export]`.
pub fn route(rq: &Rq<'_>, route: &str) -> Response {
    let Some((owner, number, tail)) = named(route) else {
        return crate::app::routes::error(rq, 404, &qmcmsg::NOT_FOUND, &[&rq.req.path]);
    };
    let read = match read(rq, owner, number) {
        Ok(r) => r,
        Err(m) => {
            return rq.page(Page {
                title: format!("Serviceable event {number}"),
                crumb: vec!["Service Management".into(), "Serviceable events".into()],
                h1: format!("Serviceable event {number}"),
                messages: vec![m],
                help: "events".into(),
                nav: Some(("Service Management", Some("/events"))),
                ..Page::default()
            });
        }
    };
    let here = read.ev.route();
    match tail {
        "repair" => return repair(rq, &read),
        "note" => return note(rq, &read),
        "export" => return export(rq, &read),
        "" => {}
        _ => return crate::app::routes::error(rq, 404, &qmcmsg::NOT_FOUND, &[&rq.req.path]),
    }
    if rq.post() {
        let f = rq.req.form();
        let to = if f.get("close_event").is_some() {
            read.ev.close_route()
        } else if f.get("add_note").is_some() {
            format!("{here}/note")
        } else if f.get("export").is_some() {
            format!("{here}/export")
        } else {
            format!("{here}/repair")
        };
        return Response::redirect(&rq.ctx.url(&to));
    }
    rq.page(page(&rq.ctx, &view(rq, &read)))
}

/// Export: the problem as a text file, nothing changed.
fn export(rq: &Rq<'_>, r: &Read) -> Response {
    let v = view(rq, r);
    let mut t = format!(
        "Serviceable event {}\nManaged system: {}\n",
        v.number, v.system.0
    );
    if !v.theirs.is_empty() {
        let _ = writeln!(t, "Managed system's number: {}", v.theirs);
    }
    let _ = writeln!(t, "Status: {}", if v.open { "Open" } else { "Closed" });
    let _ = writeln!(t, "Reported: {}", v.reported);
    if let Some((p, _, id)) = &v.partition {
        let _ = writeln!(t, "Partition: {p} (ID {id})");
    }
    let mut words = v.words.clone();
    words.resize(8, "00000000".into());
    let _ = writeln!(
        t,
        "Reference code: {}  words 2-9: {}",
        v.code.0,
        words.join(" ")
    );
    let _ = writeln!(
        t,
        "Severity: {}\n\nDescription\n{}\n",
        v.severity, v.description
    );
    if let Some((m, c, rec)) = &v.explained {
        let _ = writeln!(
            t,
            "Reference code {}\n{m}\nCause: {c}\nRecovery: {rec}\n",
            v.code.0
        );
    }
    t.push_str("Field replaceable units\n");
    for f in &v.frus {
        let _ = writeln!(
            t,
            "{}  {}  {}  {}  {}",
            f.priority, f.fru, f.location, f.part, f.action
        );
    }
    let _ = writeln!(t, "\nAnalysis log\n{}", v.log.join("\n"));
    rq.audit(
        &format!("serviceable event {} of {}", v.number, v.system.0),
        "exported",
    );
    let file = format!("event-{}-{}.txt", v.system.0, v.number);
    Response::download(&file, "text/plain; charset=utf-8", t.into_bytes())
}

/// Repair: what to do for the code, and what it is about. Nothing is
/// changed by it.
fn repair(rq: &Rq<'_>, r: &Read) -> Response {
    let v = view(rq, r);
    let recovery = v
        .explained
        .as_ref()
        .map(|(_, _, rec)| rec.clone())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| {
            "This release has no recovery text for the code: see its full explanation.".into()
        });
    let mut body = format!(
        "<p>{}</p><h2>Recovery for {}</h2><p>{}</p><h2>Resources</h2>",
        esc(
            "Repair is done at the system the problem is about; this page changes nothing. When it is done, the problem's condition clears there, and the problem is closed here."
        ),
        esc(&v.code.0),
        esc(&recovery)
    );
    let cols = ["Priority", "FRU", "Location", "Action"]
        .map(Col::plain)
        .to_vec();
    let rows = v
        .frus
        .iter()
        .map(|f| {
            vec![
                Cell::text(&f.priority),
                Cell::text(&f.fru),
                Cell::text(&f.location),
                Cell::text(&f.action),
            ]
        })
        .collect();
    body.push_str(&list_table(&List {
        cols,
        rows,
        empty: "The problem names no resource.".into(),
        ..List::default()
    }));
    let _ = write!(
        body,
        "<p><a href=\"{}\">Back to serviceable event {}</a></p>",
        esc(&rq.ctx.url(&r.ev.route())),
        esc(&v.number)
    );
    rq.page(Page {
        title: format!("Repair serviceable event {}", v.number),
        crumb: vec![
            "Service Management".into(),
            "Serviceable events".into(),
            v.number.clone(),
            "Repair".into(),
        ],
        h1: format!("Repair serviceable event {}", v.number),
        body,
        help: "events".into(),
        nav: Some(("Service Management", Some("/events"))),
        ..Page::default()
    })
}

/// Add note: a line in the problem's analysis log, at its system, with
/// the user's name.
fn note(rq: &Rq<'_>, r: &Read) -> Response {
    let back = r.ev.route();
    if rq.post() {
        let f = rq.req.form();
        if f.get("cancel").is_some() {
            return Response::redirect(&rq.ctx.url(&back));
        }
        let text = f.str("text").trim().to_string();
        let what = format!("serviceable event {} of {}", r.ev.number, r.ev.system);
        let done =
            match r.ev.owner {
                Owner::Fleet => {
                    let req = qsi::any::AnyRequest::Service(qsi::service::Request::Note(
                        qsi::service::Note {
                            number: r.ev.theirs,
                            text: text.clone(),
                        },
                    ));
                    crate::systems::ask(rq, &r.ev.system, "Add note", &req).map(|_| ())
                }
                Owner::Console => match rq.app.console.problems().note(
                    r.ev.number,
                    text.clone(),
                    rq.user().into(),
                ) {
                    Ok(Ok(_)) => Ok(()),
                    Ok(Err(is)) => Err(vec![issued(&is)]),
                    Err(e) => Err(vec![qmcmsg::message(
                        Kind::Error,
                        &qmcmsg::SERVICE_DOWN,
                        &["The console's problem service", &e.to_string()],
                    )]),
                },
            };
        return match done {
            Ok(()) => {
                rq.audit(&what, "noted");
                rq.done(
                    &back,
                    qmcmsg::message(
                        Kind::Done,
                        &qmcmsg::EVENT_NOTED,
                        &[&r.ev.number.to_string()],
                    ),
                )
            }
            Err(ms) => {
                rq.audit(&what, "note refused");
                rq.done_all(&back, ms)
            }
        };
    }
    let body = qsweb::partials::form(
        &rq.ctx,
        "",
        &format!(
            "<table class=\"props\" cellspacing=\"0\"><tr><th>Serviceable event</th><td>{}</td></tr><tr><th>Note</th><td>{}</td></tr></table>{}",
            esc(&r.ev.number.to_string()),
            qsweb::partials::input("text", "", 64),
            qsweb::partials::buttons(&[
                qsweb::partials::Button::new("ok", "OK"),
                qsweb::partials::Button::new("cancel", "Cancel"),
            ])
        ),
    );
    rq.page(Page {
        title: format!("Add note to serviceable event {}", r.ev.number),
        crumb: vec![
            "Service Management".into(),
            "Serviceable events".into(),
            r.ev.number.to_string(),
        ],
        h1: format!("Add note to serviceable event {}", r.ev.number),
        desc: Some(
            "The note is added to the problem's analysis log at its system, with your user name."
                .into(),
        ),
        body,
        help: "events".into(),
        nav: Some(("Service Management", Some("/events"))),
        ..Page::default()
    })
}

/// `/confirm/event-close?n=` or `?c=`: Close, through the confirmation;
/// a condition that still holds is closed only when the page says so.
#[allow(clippy::too_many_lines, reason = "the POST and the page")]
pub fn close(rq: &Rq<'_>) -> Response {
    if let Some(no) = crate::app::routes::lacks(rq, Task::Events) {
        return no;
    }
    let f = rq.req.form();
    let pick = |k: &str| {
        if rq.post() {
            f.str(k).to_string()
        } else {
            rq.q(k)
        }
    };
    let (owner, number) = match (pick("n").parse::<u64>(), pick("c").parse::<u64>()) {
        (Ok(n), _) => (Owner::Fleet, n),
        (_, Ok(c)) => (Owner::Console, c),
        _ => return crate::app::routes::error(rq, 404, &qmcmsg::NOT_FOUND, &[&rq.req.path]),
    };
    let r = match read(rq, owner, number) {
        Ok(r) => r,
        Err(m) => return rq.done("/events", m),
    };
    let ev = &r.ev;
    if rq.post() {
        if f.get("ok").is_none() {
            return Response::redirect(&rq.ctx.url(&ev.route()));
        }
        let what = format!("serviceable event {} of {}", ev.number, ev.system);
        // Closed anyway only when the page said the condition holds.
        let anyway = ev.holds && f.get("anyway").is_some();
        let done = match owner {
            Owner::Fleet => {
                let req = qsi::any::AnyRequest::Service(qsi::service::Request::Close(
                    qsi::service::Close {
                        number: ev.theirs,
                        anyway,
                    },
                ));
                crate::systems::ask(rq, &ev.system, "Close", &req).map(|_| ())
            }
            Owner::Console => {
                match rq
                    .app
                    .console
                    .problems()
                    .close(ev.number, anyway, rq.user().into())
                {
                    Ok(Ok(_)) => Ok(()),
                    Ok(Err(is)) => Err(vec![issued(&is)]),
                    Err(e) => Err(vec![qmcmsg::message(
                        Kind::Error,
                        &qmcmsg::SERVICE_DOWN,
                        &["The console's problem service", &e.to_string()],
                    )]),
                }
            }
        };
        return match done {
            Ok(()) => {
                rq.audit(
                    &what,
                    if anyway {
                        "closed while it holds"
                    } else {
                        "closed"
                    },
                );
                rq.done(
                    "/events",
                    qmcmsg::message(Kind::Done, &qmcmsg::EVENT_CLOSED, &[&ev.number.to_string()]),
                )
            }
            Err(ms) => {
                rq.audit(&what, "close refused");
                rq.done_all(&ev.route(), ms)
            }
        };
    }
    let sentence = if ev.holds {
        "The problem's condition still holds at the system: it is closed only because this page says so, and seen again it opens a new problem. Press OK to close it, or Cancel to return without it."
    } else {
        "The problem is closed at the system that owns it, with your user name; seen again, its condition opens a new problem. Press OK to close it, or Cancel to return without it."
    };
    let holds = if ev.holds {
        "<tr><th>Condition</th><td><span class=\"err\">Still holds</span></td></tr>"
    } else {
        "<tr><th>Condition</th><td>Cleared</td></tr>"
    };
    let table = format!(
        "<table class=\"props\" cellspacing=\"0\"><tr><th>Serviceable event</th><td>{}</td></tr><tr><th>Managed system</th><td>{}</td></tr><tr><th>Reference code</th><td>{}</td></tr><tr><th>Description</th><td>{}</td></tr>{holds}</table>",
        esc(&ev.number.to_string()),
        mono(&ev.system),
        mono(&ev.code),
        esc(&ev.description)
    );
    let key = match owner {
        Owner::Fleet => "n",
        Owner::Console => "c",
    };
    let mut options = hidden(key, &number.to_string());
    if ev.holds {
        options.push_str(&hidden("anyway", "1"));
    }
    let body = qsweb::pages::confirm_body(&rq.ctx, sentence, &table, &options);
    rq.page(Page {
        title: "Confirm Close Serviceable Event".into(),
        crumb: vec![
            "Service Management".into(),
            "Serviceable events".into(),
            ev.number.to_string(),
        ],
        h1: "Confirm Close Serviceable Event".into(),
        body,
        help: "events".into(),
        nav: Some(("Service Management", Some("/events"))),
        ..Page::default()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_event_route_names_whose_it_is() {
        assert!(matches!(
            named("/event/1042"),
            Some((Owner::Fleet, 1042, ""))
        ));
        assert!(matches!(
            named("/event/console/7/repair"),
            Some((Owner::Console, 7, "repair"))
        ));
        assert!(named("/event/0").is_none());
        assert!(named("/event/x").is_none());
    }

    #[test]
    fn a_priority_follows_the_severity() {
        use crate::service::Grave;
        assert_eq!(Grave::Terminal.priority(0), "High");
        assert_eq!(Grave::Terminal.priority(1), "Medium");
        assert_eq!(Grave::Attention.priority(0), "Medium");
        assert_eq!(Grave::Information.priority(3), "Low");
    }
}
