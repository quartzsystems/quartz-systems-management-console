// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! Code updates (`code-updates.html`): the console and every QS/VS host,
//! each level installed beside what its stream offers, with the release
//! date, status and notes; Download, Install through its confirmation,
//! Schedule; the update server's settings and Check now.

use std::fmt::Write as _;

use fetchd::Stream;
use fetchd::proto::Config;
use fetchd::settings::Check;
use qsweb::frame::{Ctx, Page};
use qsweb::html::esc;
use qsweb::http::Response;
use qsweb::msg::{Kind, Message};
use qsweb::partials::{
    Button, Cell, Col, List, buttons, checkbox, hidden, input, list_table, status,
};
use webusers::Task;

use super::{CONSOLE, date_of, downloaded, downloading, fetched, host_level, level, offered};
use crate::app::routes::Rq;

/// A row of either table.
#[derive(Clone, Debug, Default)]
pub struct Line {
    /// `QS/MC`, `Console fixes`, a host's name.
    pub component: String,
    /// What is installed.
    pub installed: String,
    /// What the stream offers.
    pub available: String,
    /// Its release date.
    pub released: String,
    /// Its status and whether that is good.
    pub status: (String, Option<bool>),
    /// Its notes' link; none for no notes.
    pub notes: Option<String>,
    /// Its actions: each link and its words.
    pub actions: Vec<(String, &'static str)>,
}

/// What the page shows.
#[derive(Clone, Debug, Default)]
pub struct Updates {
    /// The console's rows: its release, its fixes.
    pub console: Vec<Line>,
    /// Each host's row.
    pub hosts: Vec<Line>,
    /// The link of the progress page in the note under the hosts.
    pub progress: String,
    /// The update server, as typed.
    pub server: String,
    /// `Daily 04:00`, `Weekly`, `Never`.
    pub check: String,
    /// Download automatically.
    pub download: bool,
    /// The message rows.
    pub messages: Vec<Message>,
}

/// The check's choices in the design's order.
pub const CHECKS: [&str; 3] = ["Daily 04:00", "Weekly", "Never"];

fn table(ctx: &Ctx, first: &str, lines: &[Line], act: bool, mono_first: bool) -> String {
    let cols = vec![
        Col::plain(first),
        Col::plain("Installed level"),
        Col::plain("Available level"),
        Col::plain("Released"),
        Col::plain("Status"),
        Col::plain("Release notes"),
        Col::plain("Actions"),
    ];
    let rows = lines
        .iter()
        .map(|l| {
            let hrefs: Vec<(String, &str)> =
                l.actions.iter().map(|(h, t)| (ctx.url(h), *t)).collect();
            vec![
                if mono_first {
                    Cell::mono(&l.component)
                } else {
                    Cell::text(&l.component)
                },
                Cell::mono(&l.installed),
                Cell::mono(&l.available),
                Cell::text(&l.released),
                Cell::html(status(&l.status.0, l.status.1)),
                Cell::html(l.notes.as_ref().map_or_else(String::new, |n| {
                    format!("<a href=\"{}\">Notes</a>", esc(&ctx.url(n)))
                })),
                if act {
                    Cell::act(qsweb::partials::actions(&hrefs))
                } else {
                    Cell::html(qsweb::partials::actions(&hrefs))
                },
            ]
        })
        .collect();
    list_table(&List {
        cols,
        rows,
        empty: "No QS/VS host is managed.".into(),
        ..List::default()
    })
}

/// A select whose first option is the default: marked only when another
/// is chosen, as the design's are.
fn select(name: &str, options: &[&str], chosen: &str) -> String {
    let mut s = format!("<select name=\"{}\">", esc(name));
    for (i, o) in options.iter().enumerate() {
        let sel = if *o == chosen && i > 0 {
            " selected"
        } else {
            ""
        };
        let _ = write!(s, "<option{sel}>{}</option>", esc(o));
    }
    s.push_str("</select>");
    s
}

/// The page.
pub fn page(ctx: &Ctx, v: &Updates) -> Page {
    let mut body = format!(
        "<h2>QS/MC console updates</h2>{}<h2>QS/VS host updates</h2>{}",
        table(ctx, "Component", &v.console, false, false),
        table(ctx, "Managed system", &v.hosts, true, true)
    );
    let _ = write!(
        body,
        "<p class=\"small grey\">Installing a QS/VS update restarts the host into its other slot; its running partitions are shut down first, as the confirmation page lists them. Installation goes through the confirmation page, then the <a href=\"{}\">progress page</a>.</p>",
        esc(&ctx.url(&v.progress))
    );
    let settings = format!(
        "<table class=\"props\" cellspacing=\"0\">\n<tr class=\"subrow\"><td colspan=\"2\">Update server</td></tr>\n<tr><th>Server</th><td>{}</td></tr>\n<tr><th>Check automatically</th><td>{}</td></tr>\n<tr><th>Download automatically</th><td>{} Download new levels when found (install stays manual)</td></tr>\n</table>\n{}",
        input("s", &v.server, 30),
        select("a", &CHECKS, &v.check),
        checkbox("dl", v.download),
        buttons(&[
            Button::new("apply", "Apply"),
            Button::new("check_now", "Check now")
        ])
    );
    body.push_str(&qsweb::partials::form(ctx, "", &settings));
    Page {
        title: "Code updates".into(),
        crumb: vec!["Service Management".into(), "Code updates".into()],
        h1: "Code updates".into(),
        messages: v.messages.clone(),
        body,
        help: "code-updates".into(),
        nav: Some(("Service Management", Some("/updates/code"))),
        ..Page::default()
    }
}

/// A level's status and whether it is good, and the actions on it.
type State = ((String, Option<bool>), Vec<(String, &'static str)>);

/// The status of a level the stream offers, and the actions on it.
fn state_of(
    rq: &Rq<'_>,
    f: &fetchd::proto::Status,
    stream: Stream,
    target: &str,
    entry: Option<&fetchd::proto::Entry>,
    known: bool,
) -> State {
    let running = rq
        .app
        .installs
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .get(target)
        .is_some_and(|r| r.done.is_none());
    let progress = format!("/updates/code/{target}/progress");
    match entry {
        _ if running => (("Installing".into(), None), vec![(progress, "Progress")]),
        None if !known => (("Unknown".into(), None), Vec::new()),
        None => (("Current".into(), Some(true)), Vec::new()),
        Some(e) => {
            let schedule = if target == CONSOLE {
                "/schedule/add?op=update".to_string()
            } else {
                format!("/schedule/add?op=update&ms={target}")
            };
            let install = (format!("/confirm/update-install?t={target}"), "Install...");
            if downloaded(f, stream, e) {
                (
                    ("Downloaded".into(), None),
                    vec![install, (schedule, "Schedule")],
                )
            } else if downloading(f, stream, e) {
                (("Downloading".into(), None), Vec::new())
            } else {
                let q = qsweb::html::query(&[("s", stream.name()), ("f", e.update.as_str())]);
                (
                    ("Update available".into(), Some(false)),
                    vec![
                        (format!("/updates/code/download{q}"), "Download"),
                        install,
                        (schedule, "Schedule"),
                    ],
                )
            }
        }
    }
}

fn notes_of(stream: Stream, e: &fetchd::proto::Entry) -> Option<String> {
    (!e.notes.is_empty()).then(|| format!("/updates/notes/{}/{}", stream.name(), e.notes))
}

/// The page's view from the download service and what is kept of the
/// hosts.
#[allow(clippy::too_many_lines, reason = "the page top to bottom")]
fn view(rq: &Rq<'_>, mut messages: Vec<Message>) -> Updates {
    let f = fetched(rq).unwrap_or_else(|m| {
        messages.push(m);
        fetchd::proto::Status::default()
    });
    let mine = super::known(&f, Stream::Qsmc);
    let running = release::RELEASE.to_string();
    let newest = offered(&f, Stream::Qsmc, &running);
    let now = mine.and_then(|k| k.entries.iter().find(|e| e.release == running));
    let shown = newest.or(now);
    let (st, actions) = state_of(rq, &f, Stream::Qsmc, CONSOLE, newest, mine.is_some());
    let installed = level(&running, release::BUILD_ID);
    let fixes = rq
        .app
        .console
        .updd()
        .status()
        .ok()
        .and_then(|s| s.slots.into_iter().find(|s| s.running))
        .map(|s| s.fixes)
        .unwrap_or_default();
    let mut console = vec![Line {
        component: release::NAME.into(),
        installed: installed.clone(),
        available: shown.map_or_else(|| installed.clone(), |e| level(&e.release, &e.build)),
        released: shown.map(|e| date_of(rq, &e.released)).unwrap_or_default(),
        status: st,
        notes: shown.and_then(|e| notes_of(Stream::Qsmc, e)),
        actions,
    }];
    let new_fixes: Vec<String> = newest
        .map(|e| {
            e.fixes
                .iter()
                .filter(|x| !fixes.contains(x))
                .cloned()
                .collect()
        })
        .unwrap_or_default();
    console.push(Line {
        component: "Console fixes".into(),
        installed: if fixes.is_empty() {
            "(none)".into()
        } else {
            fixes.join(", ")
        },
        available: if new_fixes.is_empty() {
            "(none pending)".into()
        } else {
            new_fixes.join(", ")
        },
        released: String::new(),
        status: if new_fixes.is_empty() {
            ("Current".into(), Some(true))
        } else {
            ("Update available".into(), Some(false))
        },
        notes: None,
        actions: Vec::new(),
    });
    let kept = crate::systems::systems(rq).unwrap_or_else(|m| {
        messages.push(m);
        Vec::new()
    });
    let known = super::known(&f, Stream::Qsvs).is_some();
    let mut hosts = Vec::new();
    for k in kept
        .iter()
        .filter(|k| k.kind == mgmtproto::kept::Kind::Host)
    {
        let (release, build) = host_level(k);
        let newest = offered(&f, Stream::Qsvs, &release);
        let now = super::known(&f, Stream::Qsvs)
            .and_then(|kn| kn.entries.iter().find(|e| e.release == release));
        let shown = newest.or(now);
        let (st, actions) = state_of(
            rq,
            &f,
            Stream::Qsvs,
            &k.name,
            newest,
            known && !release.is_empty(),
        );
        let installed = level(&release, &build);
        hosts.push(Line {
            component: k.name.clone(),
            installed: installed.clone(),
            available: shown.map_or_else(
                || {
                    if known {
                        installed.clone()
                    } else {
                        "unknown".into()
                    }
                },
                |e| level(&e.release, &e.build),
            ),
            released: shown.map(|e| date_of(rq, &e.released)).unwrap_or_default(),
            status: st,
            notes: shown.and_then(|e| notes_of(Stream::Qsvs, e)),
            actions,
        });
    }
    let progress = hosts.first().map_or_else(
        || format!("/updates/code/{CONSOLE}/progress"),
        |h| format!("/updates/code/{}/progress", h.component),
    );
    Updates {
        console,
        hosts,
        progress,
        server: f.config.server.clone(),
        check: f.config.check.words().into(),
        download: f.config.download,
        messages,
    }
}

/// QMC7001 for a check that ended at `at`, with what is available.
fn checked(rq: &Rq<'_>, f: &fetchd::proto::Status) -> Message {
    let kept = crate::systems::systems(rq).unwrap_or_default();
    let hosts = kept
        .iter()
        .filter(|k| k.kind == mgmtproto::kept::Kind::Host)
        .filter(|k| offered(f, Stream::Qsvs, &host_level(k).0).is_some())
        .count();
    let mine = offered(f, Stream::Qsmc, &release::RELEASE.to_string()).is_some();
    let mut said = Vec::new();
    if mine {
        said.push("A console update available.".to_string());
    }
    match hosts {
        0 => {}
        1 => said.push("1 QS/VS host update available.".into()),
        n => said.push(format!("{n} QS/VS host updates available.")),
    }
    if said.is_empty() {
        said.push("No update is available.".into());
    }
    if let Some(e) = f.streams.iter().find_map(|k| k.error.as_ref()) {
        return qsweb::msg::Message::new(Kind::Error, &e.id, &e.text);
    }
    let bare = f
        .config
        .server
        .split("://")
        .nth(1)
        .unwrap_or(&f.config.server);
    let server = bare.split('/').next().unwrap_or(bare).to_string();
    qmcmsg::message(
        Kind::Info,
        &qmcmsg::CHECKED,
        &[
            &server,
            &crate::systems::when(rq, f.checked),
            &said.join(" "),
        ],
    )
}

/// `GET /updates/code`, and its settings' form.
pub fn route(rq: &Rq<'_>) -> Response {
    if rq.post() {
        let f = rq.req.form();
        if f.get("check_now").is_some() {
            let known = rq.app.console.fetch().check(rq.user().into());
            rq.audit("product streams", "checked");
            let ms: Vec<Message> = match (known, fetched(rq)) {
                (Ok(_), Ok(st)) => vec![checked(rq, &st)],
                (Err(e), _) => vec![qmcmsg::message(
                    Kind::Error,
                    &qmcmsg::SERVICE_DOWN,
                    &["The download service", &e.to_string()],
                )],
                (_, Err(m)) => vec![m],
            };
            return rq.done_all("/updates/code", ms);
        }
        let check = Check::of_words(f.str("a")).unwrap_or_default();
        let c = Config {
            server: f.str("s").trim().to_string(),
            check,
            download: f.get("dl").is_some(),
        };
        return match rq
            .app
            .console
            .fetch()
            .configure(c.clone(), rq.user().into())
        {
            Ok(Ok(())) => {
                rq.audit("update settings", "changed");
                rq.done(
                    "/updates/code",
                    qmcmsg::message(
                        Kind::Done,
                        &qmcmsg::UPDATE_SETTINGS,
                        &[
                            &c.server,
                            check.words(),
                            if c.download {
                                "automatically"
                            } else {
                                "on request"
                            },
                        ],
                    ),
                )
            }
            Ok(Err(is)) => rq.done_all(
                "/updates/code",
                is.iter()
                    .map(|i| qsweb::msg::Message::new(Kind::Error, &i.id, &i.text))
                    .collect(),
            ),
            Err(e) => rq.done(
                "/updates/code",
                qmcmsg::message(
                    Kind::Error,
                    &qmcmsg::SERVICE_DOWN,
                    &["The download service", &e.to_string()],
                ),
            ),
        };
    }
    let mut messages = Vec::new();
    if let Ok(f) = fetched(rq)
        && f.checked > 0
    {
        messages.push(checked(rq, &f));
    }
    rq.page(page(&rq.ctx, &view(rq, messages)))
}

/// `GET /updates/code/download?s=&f=`: the download begun.
pub fn download(rq: &Rq<'_>) -> Response {
    if let Some(no) = crate::app::routes::lacks(rq, Task::CodeUpdates) {
        return no;
    }
    let Some(stream) = Stream::of(&rq.q("s")) else {
        return crate::app::routes::error(rq, 404, &qmcmsg::NOT_FOUND, &[&rq.req.path]);
    };
    let name = rq.q("f");
    match rq
        .app
        .console
        .fetch()
        .download(stream, name.clone(), rq.user().into())
    {
        Ok(Ok(())) => {
            rq.audit(&format!("{} {name}", stream.name()), "download begun");
            rq.done(
                "/updates/code",
                qmcmsg::message(Kind::Done, &qmcmsg::DOWNLOAD_BEGUN, &[&name]),
            )
        }
        Ok(Err(is)) => rq.done_all(
            "/updates/code",
            is.iter()
                .map(|i| qsweb::msg::Message::new(Kind::Error, &i.id, &i.text))
                .collect(),
        ),
        Err(e) => rq.done(
            "/updates/code",
            qmcmsg::message(
                Kind::Error,
                &qmcmsg::SERVICE_DOWN,
                &["The download service", &e.to_string()],
            ),
        ),
    }
}

/// `GET /updates/notes/<stream>/<file>`: a release's notes, as text,
/// served from the download service's folder.
pub fn notes(rq: &Rq<'_>, rest: &str) -> Response {
    let (s, file) = rest.split_once('/').unwrap_or((rest, ""));
    let ok = Stream::of(s).is_some() && fetchd::streams::file_name(file);
    let path = rq
        .app
        .console
        .downloads()
        .map(|d| d.join(s).join("notes").join(file));
    match path.filter(|_| ok).and_then(|p| std::fs::read(p).ok()) {
        Some(bytes) => {
            let mut r = Response::new(200, "text/plain; charset=utf-8", bytes);
            r.headers
                .push(("X-Content-Type-Options".into(), "nosniff".into()));
            r
        }
        None => rq.done(
            "/updates/code",
            qmcmsg::message(Kind::Error, &qmcmsg::NO_NOTES, &[file]),
        ),
    }
}

/// `/confirm/update-install?t=`: what installing `t`'s update does, the
/// partitions it shuts down listed; OK begins it.
#[allow(clippy::too_many_lines, reason = "the check, the POST and the page")]
pub fn confirm(rq: &Rq<'_>) -> Response {
    if let Some(no) = crate::app::routes::lacks(rq, Task::CodeUpdates) {
        return no;
    }
    let form = rq.req.form();
    let target = if rq.post() {
        form.str("t").to_string()
    } else {
        rq.q("t")
    };
    let f = match fetched(rq) {
        Ok(f) => f,
        Err(m) => return rq.done("/updates/code", m),
    };
    let plan = if target == CONSOLE {
        schedd::update::console_plan(&rq.app.console.name(), &f, release::RELEASE)
    } else {
        match crate::systems::system(rq, &target) {
            Some(k) => schedd::update::host_plan(&k, &f),
            None => return crate::app::routes::error(rq, 404, &qmcmsg::NOT_FOUND, &[&rq.req.path]),
        }
    };
    let plan = match plan {
        Ok(p) => p,
        Err(i) => {
            return rq.done(
                "/updates/code",
                qsweb::msg::Message::new(Kind::Error, &i.id, &i.text),
            );
        }
    };
    if rq.post() {
        if form.get("ok").is_none() {
            return Response::redirect(&rq.ctx.url("/updates/code"));
        }
        let timeout: u32 = if form.get("shut").is_some() {
            form.str("timeout")
                .trim()
                .parse()
                .unwrap_or(600)
                .min(99_999)
        } else {
            0
        };
        if !plan.running.is_empty() && timeout == 0 {
            let names: Vec<&str> = plan.running.iter().map(|(_, n)| n.as_str()).collect();
            return rq.done(
                "/updates/code",
                qmcmsg::message(
                    Kind::Error,
                    &qmcmsg::PARTITIONS_RUN,
                    &[&target, &names.join(", ")],
                ),
            );
        }
        super::begin(rq, &target, timeout);
        rq.audit(
            &format!("code update {} on {target}", plan.entry.release),
            "install begun",
        );
        return rq.done_all(
            &format!("/updates/code/{target}/progress"),
            vec![
                qmcmsg::message(
                    Kind::Done,
                    &qmcmsg::INSTALL_BEGUN,
                    &[&level(&plan.entry.release, &plan.entry.build), &plan.target],
                ),
                qmcmsg::message(Kind::Info, &qmcmsg::NO_PLAN_SAVED, &[]),
            ],
        );
    }
    let console = target == CONSOLE;
    let sentence = if console {
        "The update is installed into the console's other slot on every load-source disk, and the console restarts into it; it confirms when its web server answers, or the console comes back on the slot it runs now, with its state as it was. Press OK to install it, or Cancel to return without it."
    } else {
        "The update is brought to the host, verified there, installed into its other slot and activated by a restart of the host. Every partition listed is shut down first. Press OK to install it, or Cancel to return without it."
    };
    let mut table = format!(
        "<table class=\"props\" cellspacing=\"0\"><tr><th>System</th><td>{}</td></tr><tr><th>Installed level</th><td>{}</td></tr><tr><th>New level</th><td>{}</td></tr><tr><th>Image</th><td>{}</td></tr><tr><th>System plan</th><td>{}</td></tr>",
        qsweb::partials::mono(&plan.target),
        qsweb::partials::mono(&plan.from),
        qsweb::partials::mono(&level(&plan.entry.release, &plan.entry.build)),
        qsweb::partials::mono(&plan.entry.update),
        esc("Not saved: system plans come with a later release")
    );
    for (id, name) in &plan.running {
        let _ = write!(
            table,
            "<tr><th>Shut down</th><td>{} (ID {id})</td></tr>",
            qsweb::partials::mono(name)
        );
    }
    table.push_str("</table>");
    let mut options = hidden("t", &target);
    if !plan.running.is_empty() {
        let _ = write!(
            options,
            "<table class=\"props\" cellspacing=\"0\"><tr><th>Partitions</th><td>{} Shut down through their operating systems first, each given {} seconds</td></tr></table>",
            checkbox("shut", true),
            input("timeout", "600", 6)
        );
    }
    let body = qsweb::pages::confirm_body(&rq.ctx, sentence, &table, &options);
    rq.page(Page {
        title: "Confirm Install Update".into(),
        crumb: vec![
            "Service Management".into(),
            "Code updates".into(),
            "Install".into(),
        ],
        h1: "Confirm Install Update".into(),
        body,
        help: "code-updates".into(),
        nav: Some(("Service Management", Some("/updates/code"))),
        ..Page::default()
    })
}
