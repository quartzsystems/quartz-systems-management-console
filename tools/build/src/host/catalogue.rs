// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! The console's own catalogue as one file (`catalogue.json` in a
//! release's directory, its form in `docs/catalogue.schema.json`): every
//! reference code of the console with its meaning (`refcode`), its cause
//! (`docs/REFCODES.md`'s row) and its recovery (the problem it opens in
//! `qsmcprobd`'s table and its message's second-level text, or where a
//! person sees it), and every `QMC` message with both its texts.
//!
//! [`audit`] is the same join, checked: `build doctor` fails on a code
//! without a cause or a recovery, or a message without both texts. The
//! join is `base/buildaid`'s; the tables are QS/MC's. (The catalogue of
//! a host's codes the pages explain is QS/VS's, `product::catalogue`.)

use std::sync::LazyLock;

use buildaid::catalogue::{Code, Tables};
#[cfg(test)]
use serde_json::Value;

/// The reference codes' table, as the build reads it.
const REFCODES: &str = include_str!("../../../../docs/REFCODES.md");

fn family(f: refcode::Family) -> &'static str {
    match f {
        refcode::Family::Progress => "progress",
        refcode::Family::Attention => "attention",
        refcode::Family::Fatal => "fatal",
        refcode::Family::PowerDown => "power-down",
    }
}

/// Every code of the console, as the catalogue has it.
static CODES: LazyLock<Vec<Code>> = LazyLock::new(|| {
    refcode::ALL
        .iter()
        .map(|c| Code {
            code: c.code,
            family: family(c.family),
            meaning: c.text,
            needs_recovery: matches!(
                c.family,
                refcode::Family::Attention | refcode::Family::Fatal
            ),
        })
        .collect()
});

/// Every `QMC` message in `base/msg`'s form: the first-level text, and
/// what to do as the second.
static MESSAGES: LazyLock<Vec<msg::Message>> =
    LazyLock::new(|| qmcmsg::ALL.iter().map(qmcmsg::base_of).collect());
static MESSAGE_REFS: LazyLock<Vec<&'static msg::Message>> =
    LazyLock::new(|| MESSAGES.iter().collect());

/// QS/MC's tables: its codes, `docs/REFCODES.md` (a console code's
/// second character is `3`), its problem table, its messages.
fn tables() -> Tables<'static> {
    Tables {
        product: release::NAME,
        codes: &CODES,
        document: REFCODES,
        document_name: "docs/REFCODES.md",
        ours: |c| c.as_bytes().get(1) == Some(&b'3'),
        rules: &qsmcprobd::RULES,
        messages: &MESSAGE_REFS,
        no_recovery: "None: the console's progress.",
    }
}

/// The catalogue of `release`, build `build`, as JSON.
pub fn json(release: &str, build: &str) -> String {
    buildaid::catalogue::json(&tables(), release, build)
}

/// What is missing from the catalogue (`buildaid::catalogue::audit`),
/// and a `QMC` message whose second-level text has no cause.
pub fn audit() -> Vec<String> {
    let mut out = buildaid::catalogue::audit(&tables());
    for d in qmcmsg::ALL {
        if d.cause.trim().is_empty() {
            out.push(format!("{} has no cause in its second-level text", d.id));
        }
    }
    out
}

/// The audit as one line for `doctor`.
pub fn summary() -> Result<String, String> {
    let missing = audit();
    if missing.is_empty() {
        Ok(format!(
            "{} codes with meaning, cause and recovery; {} messages with both texts",
            refcode::ALL.len(),
            qmcmsg::ALL.len()
        ))
    } else {
        Err(missing.join("; "))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_code_and_message_is_whole() {
        assert_eq!(audit(), Vec::<String>::new());
    }

    #[test]
    fn the_catalogue_has_every_code_and_message() {
        let v: Value = serde_json::from_str(&json("V1R0M0", "test")).unwrap();
        assert_eq!(v["schema"], buildaid::catalogue::SCHEMA);
        assert_eq!(v["product"], "QS/MC");
        assert_eq!(v["codes"].as_array().unwrap().len(), refcode::ALL.len());
        assert_eq!(v["messages"].as_array().unwrap().len(), qmcmsg::ALL.len());
        let code = |c: &str| {
            v["codes"]
                .as_array()
                .unwrap()
                .iter()
                .find(|x| x["code"] == c)
                .unwrap()
                .clone()
        };
        let lost = code("B3003000");
        assert_eq!(lost["family"], "fatal");
        assert_eq!(lost["recovery"]["problem"], true);
        assert!(!lost["cause"].as_str().unwrap().is_empty());
        let started = code("C3001000");
        assert_eq!(
            started["recovery"]["action"],
            "None: the console's progress."
        );
        // The schema's patterns: a console code, a QMC message.
        for c in v["codes"].as_array().unwrap() {
            assert_eq!(c["code"].as_str().unwrap().as_bytes()[1], b'3', "{c}");
        }
        for m in v["messages"].as_array().unwrap() {
            assert!(m["id"].as_str().unwrap().starts_with("QMC"), "{m}");
        }
    }

    /// The schema in the tree names the form `buildaid` writes.
    #[test]
    fn the_schema_is_buildaid_s_form() {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../docs/catalogue.schema.json"
        );
        let v: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
        assert_eq!(v["$id"], buildaid::catalogue::SCHEMA);
    }
}
