// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! Activate (`activate.html`) and its confirmation
//! (`confirm-activate.html`): the profile, a terminal window opened
//! after, and the keylock, IPL source and IPL type for this activation.
//! The confirmation is a GET that lists what the partition is given and
//! POSTs back with its token; a refusal shows the host's messages under
//! the console's.

use std::fmt::Write as _;

use qsweb::frame::{Ctx, Page};
use qsweb::html::esc;
use qsweb::http::{Form, Response};
use qsweb::msg::{Kind, Message};
use qsweb::partials::{Cell, Col, List, list_table, mono, status};

use mgmtproto::Kept;
use qsi::any::{AnyReply, AnyRequest};
use qsi::base::Keylock;
use qsi::host::model::{IplSource, IplType, ProcessorMode};
use qsi::host::{partition, profiles};
use webusers::Task;

use super::list::{good, state_word};
use crate::app::routes::{Rq, lacks};
use crate::systems::props::units;

/// The form.
#[derive(Clone, Debug, Default)]
pub struct Activate {
    /// The host.
    pub host: String,
    /// The partition.
    pub name: String,
    /// Its ID.
    pub id: u32,
    /// `Not activated`.
    pub state: String,
    /// Its profiles: `install (default)`, `default`.
    pub profiles: Vec<String>,
    /// The keylock chosen first: `Manual`.
    pub keylock: String,
    /// The IPL source first: `D (optical/install)`.
    pub source: String,
    /// The IPL type first: `Attended`.
    pub ipl_type: String,
    /// The messages.
    pub messages: Vec<Message>,
}

/// The order of a select: `first` and then the others of `all`.
fn first_then(first: &str, all: &[&'static str]) -> Vec<&'static str> {
    let mut v: Vec<&'static str> = all.iter().copied().filter(|o| *o == first).collect();
    v.extend(all.iter().copied().filter(|o| *o != first));
    v
}

fn options(name: &str, list: &[&str]) -> String {
    let mut s = format!("<select name=\"{name}\">");
    for o in list {
        let _ = write!(s, "<option>{}</option>", esc(o));
    }
    s.push_str("</select>");
    s
}

/// The page.
pub fn page(ctx: &Ctx, v: &Activate) -> Page {
    let profiles: Vec<&str> = v.profiles.iter().map(String::as_str).collect();
    let keylock = first_then(&v.keylock, &["Manual", "Normal"]);
    let source = first_then(&v.source, &["D (optical/install)", "A", "B"]);
    let kind = first_then(&v.ipl_type, &["Attended", "Unattended"]);
    let body = format!(
        "<form method=\"post\">{}<table class=\"props\" cellspacing=\"0\">\n<tr><th>Partition</th><td>{} (ID {}) on {}</td></tr>\n<tr><th>State</th><td>{}</td></tr>\n<tr><th>Profile <span class=\"req\">*</span></th><td>{}</td></tr>\n<tr><th>Terminal window</th><td><input type=\"checkbox\" name=\"tw\" checked> Open a terminal window on activation</td></tr>\n<tr class=\"subrow\"><td colspan=\"2\">Advanced</td></tr>\n<tr><th>Keylock position</th><td>{}</td></tr>\n<tr><th>IPL source</th><td>{}</td></tr>\n<tr><th>IPL type</th><td>{}</td></tr>\n</table>\n<div class=\"buttons\"><input type=\"submit\" name=\"ok\" value=\"OK\"><input type=\"submit\" name=\"cancel\" value=\"Cancel\"></div></form><p class=\"small grey\">OK opens the confirmation page.</p>",
        ctx.csrf_field(),
        mono(&v.name),
        v.id,
        mono(&v.host),
        status(&v.state, good(&v.state)),
        options("prof", &profiles),
        options("key", &keylock),
        options("src", &source),
        options("type", &kind),
    );
    super::sub_page(
        &v.name,
        "Activate",
        &format!("Activate partition: {}", v.name),
        v.messages.clone(),
        body,
    )
}

/// What the confirmation lists.
#[derive(Clone, Debug, Default)]
pub struct Confirm {
    /// The host.
    pub host: String,
    /// The partition.
    pub name: String,
    /// Its ID.
    pub id: u32,
    /// The profile.
    pub profile: String,
    /// `D`.
    pub source: String,
    /// `Manual`.
    pub keylock: String,
    /// `1.50 shared`.
    pub processors: String,
    /// `8,192`.
    pub memory: String,
    /// `OPT01: QS400_V1R0M0_INSTALL.iso`.
    pub optical: String,
    /// A terminal window opens after.
    pub terminal: bool,
    /// The choices, carried.
    pub carried: String,
}

/// The confirmation page.
pub fn confirm_page(ctx: &Ctx, v: &Confirm) -> Page {
    let list = List {
        cols: [
            "Partition",
            "ID",
            "Managed system",
            "Profile",
            "IPL source",
            "Keylock",
            "Processors",
            "Memory (MB)",
            "Virtual optical",
        ]
        .iter()
        .map(|c| Col::plain(c))
        .collect(),
        rows: vec![vec![
            Cell::html(mono(&v.name)),
            Cell::text(&v.id.to_string()),
            Cell::html(mono(&v.host)),
            Cell::text(&v.profile),
            Cell::text(&v.source),
            Cell::text(&v.keylock),
            Cell::text(&v.processors),
            Cell::text(&v.memory),
            Cell::text(&v.optical),
        ]],
        empty: String::new(),
        keep: Vec::new(),
    };
    let after = if v.terminal {
        "<p class=\"small grey\">A terminal window will open after the activation request is accepted.</p>"
    } else {
        ""
    };
    let body = format!(
        "<p>Press OK to activate the partition below.</p>{}{after}<form method=\"post\">{}{}<div class=\"buttons\"><input type=\"submit\" name=\"ok\" value=\"OK\"><input type=\"submit\" name=\"cancel\" value=\"Cancel\"></div></form>",
        list_table(&list),
        ctx.csrf_field(),
        v.carried,
    );
    super::sub_page(
        &v.name,
        "Confirm Activate",
        "Confirm Activate Partition",
        Vec::new(),
        body,
    )
}

/// The choices of a form: the profile's name, the keylock, the source,
/// the IPL type.
fn choices(f: &Form) -> (String, Keylock, IplSource, IplType) {
    let profile = f.str("prof").trim_end_matches(" (default)").to_string();
    let keylock = if f.str("key") == "Normal" {
        Keylock::Normal
    } else {
        Keylock::Manual
    };
    let source = match f.str("src") {
        "A" => IplSource::A,
        "B" => IplSource::B,
        _ => IplSource::D,
    };
    let kind = if f.str("type") == "Unattended" {
        IplType::Unattended
    } else {
        IplType::Attended
    };
    (profile, keylock, source, kind)
}

const CARRIED: [&str; 5] = ["prof", "key", "src", "type", "tw"];

/// `GET`, `POST /partition/<host>/<id>/activate`: the form; its OK
/// shows the confirmation; the confirmation's OK activates.
#[allow(
    clippy::too_many_lines,
    reason = "the form, its confirmation, the request"
)]
pub fn route(rq: &Rq<'_>, k: &Kept, id: u32) -> Response {
    let f = rq.req.form();
    let back = "/partitions";
    let Some(st) = k.part(id) else {
        return crate::app::routes::error(rq, 404, &qmcmsg::NOT_FOUND, &[&rq.req.path]);
    };
    if rq.post() && f.has("cancel") {
        return Response::redirect(&rq.ctx.url(back));
    }
    if rq.post()
        && let Some(no) = lacks(rq, Task::Activate)
    {
        return no;
    }
    let l = match super::props::load(rq, k, id) {
        Ok(l) => l,
        Err(ms) => return rq.done_all(back, ms),
    };
    if rq.post() && f.has("confirmed") {
        let (profile, keylock, source, kind) = choices(&f);
        let req = AnyRequest::Partition(partition::Request::Activate(partition::Activate {
            id,
            profile: profile.clone(),
            source: Some(source),
            keylock: Some(keylock),
        }));
        let what = format!("activate {} on {} with profile {profile}", st.name, k.name);
        // The IPL type is the profile's: an activation that asks
        // another takes it through the profile first.
        let mut ms = Vec::new();
        if kind != l.profile.boot.ipl_type && profile == l.profile.name {
            let mut p = l.profile.clone();
            p.boot.ipl_type = kind;
            let rev = l.revision;
            if let Err(e) = super::props::put(rq, k, id, p, rev) {
                ms = e;
            }
        }
        if !ms.is_empty() {
            return rq.done_all(back, ms);
        }
        return match crate::systems::ask(rq, &k.name, "Activate", &req) {
            Ok(_) => {
                rq.audit(&what, "activated");
                let m = qmcmsg::message(Kind::Done, &qmcmsg::ACTIVATED, &[&st.name]);
                if f.has("tw") {
                    rq.app.sessions.flash(&rq.session.id, m);
                    return Response::redirect(&rq.ctx.url(&format!("/terminal/{}/{id}", k.name)));
                }
                rq.done(back, m)
            }
            Err(mut no) => {
                rq.audit(&what, "refused");
                let first = no.first().map(|m| m.text.clone()).unwrap_or_default();
                no.insert(
                    0,
                    qmcmsg::message(
                        Kind::Error,
                        &qmcmsg::ACTIVATE_FAILED,
                        &[&st.name, first.trim_end_matches('.')],
                    ),
                );
                rq.done_all(back, no)
            }
        };
    }
    if rq.post() {
        // OK of the form: the confirmation, by GET.
        let q: Vec<String> = CARRIED
            .iter()
            .filter(|n| f.has(n))
            .map(|n| format!("{n}={}", qsweb::http::form::encode(f.str(n))))
            .collect();
        return Response::redirect(&rq.ctx.url(&format!(
            "/partition/{}/{id}/activate?confirm=1&{}",
            k.name,
            q.join("&")
        )));
    }
    if rq.q("confirm") == "1" {
        let qf = rq.req.query();
        let (profile, keylock, source, _) = choices(&qf);
        let chosen = if profile.is_empty() {
            l.profile.clone()
        } else {
            match crate::systems::ask(
                rq,
                &k.name,
                "Read profile",
                &AnyRequest::Profiles(profiles::Request::Get(profiles::Get {
                    id,
                    name: profile.clone(),
                })),
            ) {
                Ok(AnyReply::Profiles(profiles::Reply::Get(r))) => r.profile,
                Ok(_) => l.profile.clone(),
                Err(ms) => return rq.done_all(back, ms),
            }
        };
        let procs = &chosen.processors;
        let processors = if procs.mode == ProcessorMode::Dedicated {
            format!("{} dedicated", procs.desired)
        } else {
            format!(
                "{} shared",
                units(u64::from(procs.desired_units.unwrap_or(0)))
            )
        };
        let images = super::super::vres::optical::drives(k);
        let optical = images
            .iter()
            .filter(|d| d.partition == st.name && !d.image.is_empty())
            .map(|d| format!("{}: {}", d.drive, d.image))
            .collect::<Vec<_>>()
            .join(", ");
        let mut carried = String::new();
        for n in CARRIED {
            if let Some(v) = qf.get(n) {
                carried.push_str(&qsweb::partials::hidden(n, v));
            }
        }
        carried.push_str(&qsweb::partials::hidden("confirmed", "1"));
        let v = Confirm {
            host: k.name.clone(),
            name: st.name.clone(),
            id,
            profile: chosen.name.clone(),
            source: match source {
                IplSource::B => "B",
                IplSource::A => "A",
                _ => "D",
            }
            .into(),
            keylock: if keylock == Keylock::Manual {
                "Manual"
            } else {
                "Normal"
            }
            .into(),
            processors,
            memory: crate::fmt::number(chosen.memory.desired_mib, &rq.app.settings().number_format),
            optical,
            terminal: qf.has("tw"),
            carried,
        };
        return rq.page(confirm_page(&rq.ctx, &v));
    }
    let entries: Vec<String> = k.profile_list(id).iter().map(|e| e.name.clone()).collect();
    let default = &l.def.default_profile;
    let mut profiles = vec![format!("{default} (default)")];
    profiles.extend(entries.into_iter().filter(|n| n != default));
    let v = Activate {
        host: k.name.clone(),
        name: st.name.clone(),
        id,
        state: state_word(st.state).into(),
        profiles,
        keylock: if l.def.keylock == Keylock::Manual {
            "Manual"
        } else {
            "Normal"
        }
        .into(),
        source: super::props::source_word(l.profile.boot.source).into(),
        ipl_type: if l.profile.boot.ipl_type == IplType::Attended {
            "Attended"
        } else {
            "Unattended"
        }
        .into(),
        messages: Vec::new(),
    };
    rq.page(page(&rq.ctx, &v))
}
