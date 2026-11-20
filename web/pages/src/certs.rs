// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! Certificates (`console-certs.html`): the web certificate in use, the
//! trusted certificate authorities, and the import form. View adds the
//! chosen certificate's every field under the buttons, as the users'
//! forms are (`docs/OPEN_QUESTIONS.md`, item 23).

use qsweb::frame::{Ctx, Page};
use qsweb::html::{esc, query};
use qsweb::msg::Message;
use qsweb::partials::{
    Button, Cell, Col, List, Prop, PropGroup, list_table, mono, props, props_table, status,
    upload_form,
};

/// One trusted certificate authority as the list shows it.
#[derive(Clone, Debug, Default)]
pub struct Ca {
    /// Its label.
    pub label: String,
    /// Its subject.
    pub subject: String,
    /// When it expires, formatted.
    pub expires: String,
}

/// What Certificates shows.
#[derive(Clone, Debug, Default)]
pub struct Certs {
    /// The certificate's subject.
    pub subject: String,
    /// Its issuer.
    pub issuer: String,
    /// `from to until`.
    pub valid: String,
    /// The SHA-256 fingerprint, as the page shows it.
    pub sha256: String,
    /// The key: `ECDSA P-256`.
    pub key: String,
    /// The status word and whether it is good.
    pub status: (String, Option<bool>),
    /// What follows the word.
    pub note: String,
    /// The trusted authorities.
    pub cas: Vec<Ca>,
    /// A certificate's every field, when View was asked.
    pub details: Option<(String, Vec<(String, String)>)>,
    /// The message rows.
    pub messages: Vec<Message>,
}

/// The page.
pub fn page(ctx: &Ctx, v: &Certs) -> Page {
    let here = ctx.url("/console/certs");
    let group = PropGroup {
        title: Some("Console TLS certificate".into()),
        rows: vec![
            Prop::new("Subject", mono(&v.subject)),
            Prop::new("Issuer", mono(&v.issuer)),
            Prop::new("Valid", esc(&v.valid)),
            Prop::new("Fingerprint (SHA-256)", mono(&v.sha256)),
            Prop::new("Key", esc(&v.key)),
            Prop::new(
                "Status",
                format!("{}{}", status(&v.status.0, v.status.1), esc(&v.note)),
            ),
        ],
    };
    let mut body = props(
        ctx,
        &[group],
        &[
            Button::new("view", "View"),
            Button::new("generate_self-signed...", "Generate self-signed..."),
            Button::new("import...", "Import..."),
            Button::new("download_ca_certificate", "Download CA certificate"),
        ],
    );
    if let Some((title, rows)) = &v.details {
        let g = PropGroup {
            title: Some(title.clone()),
            rows: rows.iter().map(|(k, val)| Prop::new(k, esc(val))).collect(),
        };
        body.push_str(&props_table(ctx.product, &[g], true));
    }
    let list = List {
        cols: ["Actions", "Label", "Subject", "Expires"]
            .iter()
            .map(|l| Col::plain(l))
            .collect(),
        rows: v
            .cas
            .iter()
            .map(|c| {
                let q = query(&[("ca", &c.label)]);
                let r = query(&[("l", &c.label)]);
                vec![
                    Cell::act(format!(
                        "<a href=\"{here}{q}\">View</a> | <a href=\"{}{r}\">Remove</a>",
                        ctx.url("/confirm/ca-remove")
                    )),
                    Cell::text(&c.label),
                    Cell::mono(&c.subject),
                    Cell::text(&c.expires),
                ]
            })
            .collect(),
        empty: "No certificate authority is trusted. Import one's certificate below.".into(),
        keep: Vec::new(),
    };
    body.push_str(
        "<h2>Trusted certificate authorities (for managed systems and the update server)</h2>",
    );
    body.push_str(&list_table(&list));
    body.push_str(&upload_form(
        ctx,
        "",
        "<h3>Import certificate</h3><input type=\"file\" name=\"f\" size=\"40\"> PKCS#12 password: <input type=\"password\" name=\"pw\" size=\"16\"> <input type=\"submit\" value=\"Import\">",
    ));
    Page {
        title: "Certificates".into(),
        crumb: vec!["Console Management".into(), "Certificates".into()],
        h1: "Certificates".into(),
        messages: v.messages.clone(),
        body,
        help: "certs".into(),
        nav: Some(("Console Management", Some("/console/certs"))),
        ..Page::default()
    }
}
