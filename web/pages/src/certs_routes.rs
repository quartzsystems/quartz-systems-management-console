// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! Serving Certificates. The web certificate is `webd`'s (`webd::tls`):
//! a new self-signed one or an imported chain and key is served to the
//! next connection. A certificate imported without a key that is an
//! authority's is added to the trusted authorities, `trusted.toml`,
//! which the update server's connection uses (Phase 6).

use qsweb::frame::Page;
use qsweb::http::Response;
use qsweb::msg::{Kind, Message};
use serde::{Deserialize, Serialize};
use x509kit::bundle::{Bundle, read_pem};
use x509kit::crypto::keys::KeyAlgorithm;
use x509kit::x509::CertInfo;

use crate::app::routes::Rq;
use crate::certs::{Ca, Certs, page};
use crate::user_routes::outcome;

/// One trusted authority.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Authority {
    /// Its label: the common name, or the subject.
    pub label: String,
    /// Its certificate, PEM.
    pub pem: String,
}

/// `trusted.toml`: the trusted certificate authorities.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Trusted {
    /// The authorities, in the order added.
    #[serde(default, rename = "authority")]
    pub authorities: Vec<Authority>,
}

/// How many authorities may be trusted.
pub const MOST: usize = 32;

impl state::Document for Trusted {
    const FILE: &'static str = "trusted.toml";
    const VERSION: u32 = 1;
    const MIGRATIONS: &'static [state::Migration] = &[];

    fn validate(&self) -> Result<(), String> {
        if self.authorities.len() > MOST {
            return Err(format!("more than {MOST} trusted authorities"));
        }
        for a in &self.authorities {
            info_of(&a.pem)?;
        }
        Ok(())
    }
}

/// The certificate of `pem`.
fn info_of(pem: &str) -> Result<CertInfo, String> {
    let b = read_pem(pem, None).map_err(|e| e.to_string())?;
    b.chain
        .first()
        .map(|c| c.info.clone())
        .ok_or_else(|| "no certificate".into())
}

fn trusted(rq: &Rq<'_>) -> Trusted {
    rq.app
        .store()
        .load::<Trusted>()
        .ok()
        .flatten()
        .unwrap_or_default()
}

fn key_of(info: &CertInfo) -> String {
    match info.key_algorithm {
        Some(KeyAlgorithm::EcdsaP256) => "ECDSA P-256".into(),
        Some(KeyAlgorithm::EcdsaP384) => "ECDSA P-384".into(),
        Some(_) => format!("RSA {}", info.key_bits),
        None => format!("{} bits", info.key_bits),
    }
}

/// `AA:BB:CC:DD:...:YY:ZZ`, as the page shows a fingerprint.
fn short(colons: &str) -> String {
    let parts: Vec<&str> = colons.split(':').collect();
    if parts.len() <= 6 {
        return colons.to_string();
    }
    format!(
        "{}:...:{}",
        parts[..4].join(":"),
        parts[parts.len() - 2..].join(":")
    )
}

fn names(info: &CertInfo) -> String {
    if info.san.is_empty() {
        "none".into()
    } else {
        info.san.join(", ")
    }
}

fn details(rq: &Rq<'_>, info: &CertInfo, sha256: &str, sha1: &str) -> Vec<(String, String)> {
    let (zone, s) = (rq.app.host().zone(), rq.app.settings());
    let when = |t: u64| {
        let t = i64::try_from(t).unwrap_or(0);
        format!(
            "{} {}",
            crate::fmt::date(t, zone, &s.date_format),
            crate::fmt::time(t, zone, &s.time_format, false)
        )
    };
    vec![
        ("Subject".into(), info.subject.clone()),
        ("Issuer".into(), info.issuer.clone()),
        ("Names".into(), names(info)),
        ("Serial number".into(), info.serial.clone()),
        ("Valid from".into(), when(info.not_before)),
        ("Valid to".into(), when(info.not_after)),
        ("Key".into(), key_of(info)),
        ("Fingerprint (SHA-256)".into(), sha256.into()),
        ("Fingerprint (SHA-1)".into(), sha1.into()),
    ]
}

fn view(rq: &Rq<'_>, show_details: bool, messages: Vec<Message>) -> Certs {
    let app = rq.app;
    let s = app.settings();
    let zone = app.host().zone();
    let date = |t: u64| crate::fmt::date(i64::try_from(t).unwrap_or(0), zone, &s.date_format);
    let now = app.console.now();
    let mut v = Certs {
        cas: trusted(rq)
            .authorities
            .iter()
            .filter_map(|a| {
                let i = info_of(&a.pem).ok()?;
                Some(Ca {
                    label: a.label.clone(),
                    subject: i.subject,
                    expires: date(i.not_after),
                })
            })
            .collect(),
        messages,
        ..Certs::default()
    };
    let Some(certs) = app.certs.get() else {
        v.status = ("Not made yet".into(), Some(false));
        return v;
    };
    let shown = certs.shown();
    let i = &shown.info;
    v.subject.clone_from(&i.subject);
    v.issuer.clone_from(&i.issuer);
    v.valid = format!("{} to {}", date(i.not_before), date(i.not_after));
    v.sha256 = short(&shown.sha256);
    v.key = key_of(i);
    let days = i.not_after.saturating_sub(now) / 86_400;
    v.status = if now > i.not_after {
        ("Expired".into(), Some(false))
    } else if now < i.not_before {
        ("Not yet valid".into(), Some(false))
    } else {
        ("Active".into(), Some(true))
    };
    if now <= i.not_after {
        v.note = format!(
            " (expires in {days} day{})",
            if days == 1 { "" } else { "s" }
        );
    }
    if i.self_signed {
        v.note.push_str(", self-signed");
    }
    if show_details {
        v.details = Some((
            "Certificate in use".into(),
            details(rq, i, &shown.sha256, &shown.sha1),
        ));
    }
    v
}

fn show(rq: &Rq<'_>, full: bool, messages: Vec<Message>) -> Response {
    let mut v = view(rq, full, messages);
    let q = rq.req.query();
    let chosen = q.get("ca").and_then(|label| {
        let t = trusted(rq);
        let a = t.authorities.iter().find(|a| a.label == label)?;
        Some((label.to_string(), read_pem(&a.pem, None).ok()?))
    });
    if let Some((label, b)) = chosen {
        let c = &b.chain[0];
        let sha256 = webd::tls::colons(&x509kit::crypto::sha256(&c.der));
        let sha1 = webd::tls::colons(&x509kit::crypto::sha1(&c.der));
        v.details = Some((
            format!("Certificate authority {label}"),
            details(rq, &c.info, &sha256, &sha1),
        ));
    }
    rq.page(page(&rq.ctx, &v))
}

fn refused(text: &str) -> Message {
    qmcmsg::message(Kind::Error, &qmcmsg::CERT_REFUSED, &[text])
}

/// The open addresses, as the certificate names them.
fn addresses(rq: &Rq<'_>) -> Vec<String> {
    let line = rq.app.console.fact(release::fact::MANAGEMENT);
    crate::pages::address(&line)
        .split_once('/')
        .map(|(a, _)| vec![a.to_string()])
        .unwrap_or_default()
}

fn import(rq: &Rq<'_>) -> Response {
    let parts = rq.req.multipart().unwrap_or_default();
    let field = |n: &str| parts.iter().find(|p| p.name == n);
    let data = field("f").map(|p| p.data.as_slice()).unwrap_or_default();
    let password = zeroize::Zeroizing::new(
        field("pw")
            .map(|p| String::from_utf8_lossy(&p.data).into_owned())
            .unwrap_or_default(),
    );
    let bundle = match webd::tls::read_bundle(data, &password) {
        Ok(b) => b,
        Err(e) => {
            rq.audit("import a certificate", &e);
            return show(rq, false, vec![refused(&e)]);
        }
    };
    if bundle.key.is_none() {
        return add_authority(rq, &bundle);
    }
    let Some(certs) = rq.app.certs.get() else {
        return show(rq, false, vec![refused("the web server has not started")]);
    };
    let done = certs.import(bundle);
    rq.audit(
        "import the web certificate",
        &done
            .as_ref()
            .map_or_else(Clone::clone, |s| format!("done, SHA-256 {}", s.sha256)),
    );
    match done {
        Ok(s) => rq.done(
            "/console/certs",
            qmcmsg::message(Kind::Done, &qmcmsg::CERT_IMPORTED, &[&short(&s.sha256)]),
        ),
        Err(e) => show(rq, false, vec![refused(&e)]),
    }
}

fn add_authority(rq: &Rq<'_>, bundle: &Bundle) -> Response {
    let c = bundle.leaf();
    if !c.info.is_ca {
        let e = "the file has no private key, and its certificate is not an authority's";
        rq.audit("import a certificate", e);
        return show(rq, false, vec![refused(e)]);
    }
    let label = c
        .info
        .common_name
        .clone()
        .unwrap_or_else(|| c.info.subject.clone());
    let mut t = trusted(rq);
    let done = if t.authorities.iter().any(|a| a.label == label) {
        Err(format!("{label} is trusted already"))
    } else {
        t.authorities.push(Authority {
            label: label.clone(),
            pem: x509kit::pem::encode("CERTIFICATE", &c.der),
        });
        rq.app.store().save(&t).map_err(|e| e.to_string())
    };
    rq.audit(&format!("trust the authority {label}"), outcome(&done));
    match done {
        Ok(()) => rq.done(
            "/console/certs",
            qmcmsg::message(Kind::Done, &qmcmsg::CA_ADDED, &[&label]),
        ),
        Err(e) => show(rq, false, vec![refused(&e)]),
    }
}

/// `/console/certs`.
pub fn certs(rq: &Rq<'_>) -> Response {
    if !rq.post() {
        return show(rq, false, Vec::new());
    }
    if rq.req.multipart().is_some() {
        return import(rq);
    }
    let f = rq.req.form();
    if f.get("generate_self-signed...").is_some() {
        return Response::redirect(&rq.ctx.url("/confirm/cert-new"));
    }
    if f.get("import...").is_some() {
        return show(
            rq,
            false,
            vec![qmcmsg::message(Kind::Info, &qmcmsg::CERT_CHOOSE, &[])],
        );
    }
    if let (Some(_), Some(c)) = (f.get("download_ca_certificate"), rq.app.certs.get()) {
        return Response::download(
            "qsmc-ca.pem",
            "application/x-pem-file",
            c.authority_pem().into_bytes(),
        );
    }
    show(rq, f.get("view").is_some(), Vec::new())
}

/// `/confirm/cert-new`: a new self-signed certificate, then OK.
pub fn renew(rq: &Rq<'_>) -> Response {
    let app = rq.app;
    if rq.post() {
        let f = rq.req.form();
        if f.get("ok").is_none() {
            return Response::redirect(&rq.ctx.url("/console/certs"));
        }
        let Some(certs) = app.certs.get() else {
            return show(rq, false, vec![refused("the web server has not started")]);
        };
        let done = certs.renew(&app.console.name(), &addresses(rq), app.console.now());
        rq.audit(
            "make a new self-signed web certificate",
            &done
                .as_ref()
                .map_or_else(Clone::clone, |s| format!("done, SHA-256 {}", s.sha256)),
        );
        return match done {
            Ok(s) => rq.done(
                "/console/certs",
                qmcmsg::message(Kind::Done, &qmcmsg::CERT_MADE, &[&short(&s.sha256)]),
            ),
            Err(e) => show(rq, false, vec![refused(&e)]),
        };
    }
    let names = std::iter::once(app.console.name())
        .chain(addresses(rq))
        .collect::<Vec<_>>()
        .join(", ");
    let table = format!(
        "<p class=\"small grey\">The new certificate is ECDSA P-256, valid for {} days, and names {}. Browsers warn about it until it is accepted again.</p>",
        webd::tls::SELF_SIGNED_DAYS,
        qsweb::html::esc(&names)
    );
    let body = qsweb::pages::confirm_body(
        &rq.ctx,
        "Making a new self-signed certificate replaces the one in use; new connections are served with it. Press OK to make it, or Cancel to return without changing it.",
        &table,
        "",
    );
    rq.page(confirm_page("Confirm New Certificate", body))
}

fn confirm_page(title: &str, body: String) -> Page {
    Page {
        title: title.into(),
        crumb: vec![
            "Console Management".into(),
            "Certificates".into(),
            title.into(),
        ],
        h1: title.into(),
        body,
        help: "certs".into(),
        nav: Some(("Console Management", Some("/console/certs"))),
        ..Page::default()
    }
}

/// `/confirm/ca-remove?l=<label>`: the authority to remove, then OK.
pub fn remove(rq: &Rq<'_>) -> Response {
    let f = rq.req.form();
    let q = rq.req.query();
    let label = f.get("l").or_else(|| q.get("l")).unwrap_or("").to_string();
    let mut t = trusted(rq);
    if rq.post() {
        if f.get("ok").is_none() {
            return Response::redirect(&rq.ctx.url("/console/certs"));
        }
        let before = t.authorities.len();
        t.authorities.retain(|a| a.label != label);
        let done = if t.authorities.len() == before {
            Err(format!("{label} is not trusted"))
        } else {
            rq.app.store().save(&t).map_err(|e| e.to_string())
        };
        rq.audit(
            &format!("stop trusting the authority {label}"),
            outcome(&done),
        );
        return match done {
            Ok(()) => rq.done(
                "/console/certs",
                qmcmsg::message(Kind::Done, &qmcmsg::CA_REMOVED, &[&label]),
            ),
            Err(e) => rq.done("/console/certs", refused(&e)),
        };
    }
    let Some(a) = t.authorities.iter().find(|a| a.label == label) else {
        return crate::app::routes::error(rq, 404, &qmcmsg::NOT_FOUND, &[&label]);
    };
    let subject = info_of(&a.pem).map(|i| i.subject).unwrap_or_default();
    let list = qsweb::partials::List {
        cols: ["Label", "Subject"]
            .iter()
            .map(|l| qsweb::partials::Col::plain(l))
            .collect(),
        rows: vec![vec![
            qsweb::partials::Cell::text(&a.label),
            qsweb::partials::Cell::mono(&subject),
        ]],
        empty: String::new(),
        keep: Vec::new(),
    };
    let options = format!(
        "<input type=\"hidden\" name=\"l\" value=\"{}\">",
        qsweb::html::esc(&a.label)
    );
    let body = qsweb::pages::confirm_body(
        &rq.ctx,
        "Removing a certificate authority stops the console trusting what it signed. Press OK to remove the authority listed below, or Cancel to return without removing it.",
        &qsweb::partials::list_table(&list),
        &options,
    );
    rq.page(confirm_page("Confirm Remove Authority", body))
}
