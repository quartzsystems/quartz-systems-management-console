// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! Add managed system (`add-system.html`) and its result
//! (`add-system-result.html`); Reset connection, the same form for a
//! system kept; Remove, through the confirmation page. Connect pairs
//! with the one-time password the managed system showed its operator:
//! the password is used once by the managed-systems service and never
//! kept or logged. The result shows what the system says of itself and
//! its certificate's fingerprint, to compare with the one its operator
//! sees; Add keeps it, Cancel forgets it.

use qsweb::frame::{Ctx, Page};
use qsweb::html::esc;
use qsweb::http::Response;
use qsweb::msg::{Kind, Message};
use qsweb::partials::{
    Button, Cell, Col, List, Prop, PropGroup, buttons, form, list_table, mono, props_table, status,
};

use mgmtproto::Kept;
use mgmtproto::kept::Kind as SystemKind;

use super::{refused, system, unreached};
use crate::app::routes::Rq;

/// The QSI port a system listens on unless told otherwise.
pub const PORT: u16 = 2300;

/// The form's values.
#[derive(Clone, Debug, Default)]
pub struct AddForm {
    /// The address typed.
    pub host: String,
    /// The port typed.
    pub port: String,
    /// The system being paired again, for Reset connection.
    pub reset: Option<String>,
    /// The message rows.
    pub messages: Vec<Message>,
}

fn crumb(last: &str) -> Vec<String> {
    vec!["Systems Management".into(), "Servers".into(), last.into()]
}

/// The form.
pub fn form_page(ctx: &Ctx, v: &AddForm) -> Page {
    let h1 = match &v.reset {
        Some(name) => format!("Reset connection: {name}"),
        None => "Add managed system".to_string(),
    };
    let host = match &v.reset {
        Some(_) => mono(&v.host),
        None => format!(
            "<input type=\"text\" name=\"host\" value=\"{}\" size=\"30\">",
            esc(&v.host)
        ),
    };
    let port = match &v.reset {
        Some(_) => mono(&v.port),
        None => format!(
            "<input type=\"text\" name=\"port\" value=\"{}\" size=\"6\">",
            esc(&v.port)
        ),
    };
    let g = PropGroup {
        title: Some("Connection".into()),
        rows: vec![
            Prop::new("IP address or host name", host).required(),
            Prop::new(
                "Service interface password",
                "<input type=\"password\" name=\"pw\" size=\"20\">".into(),
            )
            .required(),
            Prop::new("Service interface port", port),
            Prop::new(
                "Type",
                "<span class=\"grey\">Detected on connect (standalone QS/400 or QS/VS host)</span>".into(),
            ),
            Prop::new(
                "Verify TLS certificate",
                "<span class=\"grey\">The system's own certificate, pinned when paired: compare its fingerprint with the one the system shows</span>".into(),
            ),
        ],
    };
    let keep = v.reset.as_ref().map_or_else(String::new, |n| {
        format!("<input type=\"hidden\" name=\"ms\" value=\"{}\">", esc(n))
    });
    let body = form(
        ctx,
        "",
        &format!(
            "{keep}{}\n{}",
            props_table(ctx.product, &[g], true),
            buttons(&[
                Button::new("connect", "Connect"),
                Button::new("cancel", "Cancel")
            ])
        ),
    );
    Page {
        title: h1.clone(),
        crumb: crumb(&h1),
        h1,
        desc: Some(if v.reset.is_some() {
            "Pair this console with the managed system again, with a new password its operator makes there.".into()
        } else {
            "Connect this console to the service interface of a managed system.".into()
        }),
        messages: v.messages.clone(),
        body,
        help: "add-system".into(),
        nav: None,
        ..Page::default()
    }
}

/// What the result shows.
#[derive(Clone, Debug, Default)]
pub struct Found {
    /// Its name.
    pub name: String,
    /// `QS/VS host` or `Standalone QS/400`.
    pub kind: String,
    /// Its type and model.
    pub model: String,
    /// Its serial number.
    pub serial: String,
    /// `QS/VS version` or `QS/400 version`.
    pub version_label: String,
    /// Its release.
    pub release: String,
    /// Its build.
    pub build: String,
    /// `Operating`, ...
    pub state: String,
    /// `8 installed, 8 configurable`.
    pub processors: String,
    /// `131,072 MB installed`.
    pub memory: String,
    /// `0 defined`.
    pub partitions: String,
    /// Its current code.
    pub code: String,
    /// Its certificate's SHA-256.
    pub fingerprint: String,
    /// The message rows.
    pub messages: Vec<Message>,
}

/// The fingerprint as pairs of hexadecimal digits.
pub fn pairs(hex: &str) -> String {
    hex.as_bytes()
        .chunks(2)
        .map(|c| String::from_utf8_lossy(c).to_uppercase())
        .collect::<Vec<_>>()
        .join(":")
}

/// The result.
pub fn result_page(ctx: &Ctx, v: &Found) -> Page {
    let rows = vec![
        Prop::new("Name", mono(&v.name)),
        Prop::new("Type", esc(&v.kind)),
        Prop::new("Type-Model", mono(&v.model)),
        Prop::new("Serial", mono(&v.serial)),
        Prop::new(
            &v.version_label,
            format!("{} build {}", mono(&v.release), esc(&v.build)),
        ),
        Prop::new("State", status(&v.state, Some(v.state == "Operating"))),
        Prop::new("Processors", esc(&v.processors)),
        Prop::new("Memory", esc(&v.memory)),
        Prop::new("Partitions", esc(&v.partitions)),
        Prop::new(
            "Reference code",
            format!(
                "<a class=\"mono\" href=\"{}\">{}</a>",
                ctx.url(&format!("/refcode/{}", esc(&v.code))),
                esc(&v.code)
            ),
        ),
        Prop::new("Certificate SHA-256", mono(&pairs(&v.fingerprint))),
    ];
    let body = format!(
        "<h2>Found</h2>{}<p class=\"small grey\">Add registers the system in the Servers list and starts monitoring. Nothing on the managed system is changed.</p>",
        form(
            ctx,
            "",
            &format!(
                "<input type=\"hidden\" name=\"ms\" value=\"{}\">{}\n{}",
                esc(&v.name),
                props_table(ctx.product, &[PropGroup { title: None, rows }], true),
                buttons(&[Button::new("add", "Add"), Button::new("cancel", "Cancel")])
            )
        )
    );
    Page {
        title: "Add managed system".into(),
        crumb: crumb("Add managed system"),
        h1: "Add managed system".into(),
        messages: v.messages.clone(),
        body,
        help: "add-system".into(),
        nav: None,
        ..Page::default()
    }
}

/// `n` grouped as the console's number format has it.
fn number(rq: &Rq<'_>, n: u64) -> String {
    crate::fmt::number(n, &rq.app.settings().number_format)
}

/// What the result shows of a kept system.
pub fn found(rq: &Rq<'_>, k: &Kept) -> Found {
    let host = k.host.as_ref();
    let (procs, memory) = match host {
        Some(h) => (
            format!(
                "{} installed, {} configurable",
                h.processors.installed / 100,
                h.processors.configurable / 100
            ),
            format!("{} MB installed", number(rq, h.memory.installed)),
        ),
        None => ("not reported".into(), "not reported".into()),
    };
    let standalone = k.kind == SystemKind::Standalone;
    Found {
        name: k.name.clone(),
        kind: k.kind.words().into(),
        model: k.model().into(),
        serial: k
            .identity
            .as_ref()
            .map_or_else(|| k.serial.clone(), |i| i.serial.clone()),
        version_label: if standalone {
            "QS/400 version"
        } else {
            "QS/VS version"
        }
        .into(),
        release: k.release().into(),
        build: k
            .identity
            .as_ref()
            .map(|i| i.build.clone())
            .unwrap_or_default(),
        state: k.status().into(),
        processors: procs,
        memory,
        partitions: if standalone {
            "1 (the system itself)".into()
        } else {
            format!("{} defined", k.partitions.len())
        },
        code: k.code(),
        fingerprint: k.fingerprint.clone(),
        messages: Vec::new(),
    }
}

/// `GET`, `POST /servers/add`: the form, and Connect.
pub fn add(rq: &Rq<'_>) -> Response {
    let f = rq.req.form();
    // The result's Add or Cancel, posted back here.
    if rq.post() && f.has("ms") {
        return result(rq);
    }
    if rq.post() && f.has("cancel") {
        return Response::redirect(&rq.ctx.url("/servers"));
    }
    let mut v = AddForm {
        host: f.str("host").trim().to_string(),
        port: Some(f.str("port").trim())
            .filter(|p| !p.is_empty())
            .unwrap_or("2300")
            .to_string(),
        ..AddForm::default()
    };
    if !rq.post() {
        return rq.page(form_page(&rq.ctx, &v));
    }
    let Ok(port) = v.port.parse::<u16>() else {
        v.messages = vec![qmcmsg::message(
            Kind::Error,
            &qmcmsg::NOT_VALID,
            &["The service interface port is a number from 1 to 65535."],
        )];
        return rq.page(form_page(&rq.ctx, &v));
    };
    if v.host.is_empty() {
        v.messages = vec![qmcmsg::message(
            Kind::Error,
            &qmcmsg::NOT_VALID,
            &["Type the managed system's IP address or host name."],
        )];
        return rq.page(form_page(&rq.ctx, &v));
    }
    let pw = zeroize::Zeroizing::new(f.str("pw").to_string());
    let done = rq
        .app
        .console
        .mgmt()
        .add(v.host.clone(), port, pw.to_string(), rq.user().into());
    let what = format!("add managed system at {}:{port}", v.host);
    match done {
        Ok(Ok(k)) => {
            rq.audit(&what, &format!("paired with {}", k.name));
            let mut r = found(rq, &k);
            r.messages = vec![qmcmsg::message(
                Kind::Done,
                &qmcmsg::SYSTEM_CONNECTED,
                &[&k.name, k.release()],
            )];
            rq.page(result_page(&rq.ctx, &r))
        }
        Ok(Err(no)) => {
            rq.audit(
                &what,
                &format!("refused: {} {}", no.main.msgid, no.main.text),
            );
            v.messages = refused(&no);
            rq.page(form_page(&rq.ctx, &v))
        }
        Err(e) => {
            v.messages = vec![unreached(&e)];
            rq.page(form_page(&rq.ctx, &v))
        }
    }
}

/// `POST /servers/add/result`: Add keeps the system; Cancel forgets it.
pub fn result(rq: &Rq<'_>) -> Response {
    let f = rq.req.form();
    let name = f.str("ms").to_string();
    if !rq.post() {
        return match system(rq, &name) {
            Some(k) => rq.page(result_page(&rq.ctx, &found(rq, &k))),
            None => crate::app::routes::error(rq, 404, &qmcmsg::NO_SUCH_SYSTEM, &[&name]),
        };
    }
    if f.has("cancel") {
        let done = rq.app.console.mgmt().remove(name.clone(), rq.user().into());
        rq.audit(
            &format!("cancel adding {name}"),
            match &done {
                Ok(Ok(())) => "forgotten",
                _ => "not forgotten",
            },
        );
        return Response::redirect(&rq.ctx.url("/servers/add"));
    }
    rq.audit(&format!("add {name}"), "kept");
    rq.done(
        "/servers",
        qmcmsg::message(Kind::Done, &qmcmsg::SYSTEM_ADDED, &[&name]),
    )
}

/// `GET`, `POST /servers/reset?ms=`: pair a kept system again.
pub fn reset(rq: &Rq<'_>) -> Response {
    let f = rq.req.form();
    let name = if rq.post() {
        f.str("ms").to_string()
    } else {
        rq.q("ms")
    };
    let Some(k) = system(rq, &name) else {
        return crate::app::routes::error(rq, 404, &qmcmsg::NO_SUCH_SYSTEM, &[&name]);
    };
    if rq.post() && f.has("cancel") {
        return Response::redirect(&rq.ctx.url("/connectivity"));
    }
    let mut v = AddForm {
        host: k.address.clone(),
        port: k.port.to_string(),
        reset: Some(k.name.clone()),
        messages: Vec::new(),
    };
    if !rq.post() {
        return rq.page(form_page(&rq.ctx, &v));
    }
    let pw = zeroize::Zeroizing::new(f.str("pw").to_string());
    let done = rq
        .app
        .console
        .mgmt()
        .reset(k.name.clone(), pw.to_string(), rq.user().into());
    let what = format!("reset connection {}", k.name);
    match done {
        Ok(Ok(_)) => {
            rq.audit(&what, "paired again");
            rq.done(
                "/connectivity",
                qmcmsg::message(Kind::Done, &qmcmsg::SYSTEM_PAIRED_AGAIN, &[&k.name]),
            )
        }
        Ok(Err(no)) => {
            rq.audit(
                &what,
                &format!("refused: {} {}", no.main.msgid, no.main.text),
            );
            v.messages = refused(&no);
            rq.page(form_page(&rq.ctx, &v))
        }
        Err(e) => {
            v.messages = vec![unreached(&e)];
            rq.page(form_page(&rq.ctx, &v))
        }
    }
}

/// `/confirm/system-remove?ms=`: Remove, through the confirmation page.
pub fn remove(rq: &Rq<'_>) -> Response {
    let f = rq.req.form();
    let name = if rq.post() {
        f.str("ms").to_string()
    } else {
        rq.q("ms")
    };
    if rq.post() {
        if f.get("ok").is_none() {
            return Response::redirect(&rq.ctx.url("/connectivity"));
        }
        let done = rq.app.console.mgmt().remove(name.clone(), rq.user().into());
        let what = format!("remove managed system {name}");
        return match done {
            Ok(Ok(())) => {
                rq.audit(&what, "removed");
                rq.done(
                    "/connectivity",
                    qmcmsg::message(Kind::Done, &qmcmsg::SYSTEM_REMOVED, &[&name]),
                )
            }
            Ok(Err(no)) => {
                rq.audit(
                    &what,
                    &format!("refused: {} {}", no.main.msgid, no.main.text),
                );
                rq.done("/connectivity", refused(&no).remove(0))
            }
            Err(e) => rq.done("/connectivity", unreached(&e)),
        };
    }
    let Some(k) = system(rq, &name) else {
        return crate::app::routes::error(rq, 404, &qmcmsg::NO_SUCH_SYSTEM, &[&name]);
    };
    let list = List {
        cols: ["Managed system", "Type", "Address", "Partitions"]
            .iter()
            .map(|l| Col::plain(l))
            .collect(),
        rows: vec![vec![
            Cell::mono(&k.name),
            Cell::text(k.kind.words()),
            Cell::mono(&format!("{}:{}", k.address, k.port)),
            Cell::num(&k.partition_count().to_string()),
        ]],
        empty: String::new(),
        keep: Vec::new(),
    };
    let table = format!(
        "{}<p class=\"small grey\">The console forgets the system and the certificate it pinned. Nothing on the managed system is changed: remove this console there too.</p>",
        list_table(&list)
    );
    let options = format!(
        "<input type=\"hidden\" name=\"ms\" value=\"{}\">",
        esc(&k.name)
    );
    let body = qsweb::pages::confirm_body(
        &rq.ctx,
        "Removing a managed system ends the console's session with it. Press OK to remove the system listed below, or Cancel to return without removing it.",
        &table,
        &options,
    );
    rq.page(Page {
        title: "Confirm Remove Managed System".into(),
        crumb: vec![
            "Service Management".into(),
            "Connectivity".into(),
            "Confirm Remove Managed System".into(),
        ],
        h1: "Confirm Remove Managed System".into(),
        body,
        help: "connectivity".into(),
        nav: Some(("Service Management", Some("/connectivity"))),
        ..Page::default()
    })
}
