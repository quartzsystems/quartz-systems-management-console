// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! What a reference code means, from data and never restated: a QS/VS
//! host's codes and messages from the catalogue its release writes
//! (`catalogue.json`, in the image at `release::CATALOGUE`), a
//! partition's from QS/400's `srcdata`, the console's own from
//! `refcode`. A code none of them knows is shown with its text as
//! posted and the line that this release has no explanation for it.

use std::collections::HashMap;

use serde::Deserialize;

/// Whose a code is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Whose {
    /// A QS/VS host's (`?2`).
    Host,
    /// A QS/400 system's, on a partition or standalone.
    System,
    /// This console's own (`?3`).
    Console,
    /// No code: the system is operating.
    None,
}

/// A code explained.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Explained {
    /// The code.
    pub code: String,
    /// Its family in words: `attention`, `progress`, ...
    pub family: String,
    /// Its title: the text it is posted with, before any detail.
    pub title: String,
    /// What it means.
    pub meaning: String,
    /// Why it is posted; empty where the data has none.
    pub cause: String,
    /// What to do; empty where the data has none.
    pub recovery: String,
    /// The message a host's problem opens with it, `QVS5112`.
    pub message: String,
}

#[derive(Debug, Deserialize)]
struct Recovery {
    #[serde(default)]
    message: String,
    #[serde(default)]
    description: String,
    action: String,
}

#[derive(Debug, Deserialize)]
struct Entry {
    code: String,
    family: String,
    meaning: String,
    cause: String,
    recovery: Recovery,
}

#[derive(Debug, Deserialize)]
struct Msg {
    id: String,
    text: String,
    help: String,
}

#[derive(Debug, Deserialize)]
struct File {
    schema: String,
    release: String,
    codes: Vec<Entry>,
    messages: Vec<Msg>,
}

/// The schema this release reads.
pub const SCHEMA: &str = "quartz.catalogue/1";

/// A QS/VS release's catalogue, read.
#[derive(Debug, Default)]
pub struct Catalogue {
    /// The QS/VS release it is of.
    pub release: String,
    codes: HashMap<String, Explained>,
    messages: HashMap<String, (String, String)>,
}

impl Catalogue {
    /// The catalogue in `json`.
    ///
    /// # Errors
    /// It does not read, or is of another schema.
    pub fn from_json(json: &str) -> Result<Self, String> {
        let f: File = serde_json::from_str(json).map_err(|e| e.to_string())?;
        if f.schema != SCHEMA {
            return Err(format!("schema {} is not {SCHEMA}", f.schema));
        }
        let codes = f
            .codes
            .into_iter()
            .map(|c| {
                let recovery = if c.recovery.description.is_empty() {
                    c.recovery.action
                } else {
                    format!("{} {}", c.recovery.description, c.recovery.action)
                };
                (
                    c.code.clone(),
                    Explained {
                        code: c.code,
                        family: c.family,
                        title: c.meaning.clone(),
                        meaning: c.meaning,
                        cause: c.cause,
                        recovery,
                        message: c.recovery.message,
                    },
                )
            })
            .collect();
        let messages = f
            .messages
            .into_iter()
            .map(|m| (m.id, (m.text, m.help)))
            .collect();
        Ok(Self {
            release: f.release,
            codes,
            messages,
        })
    }

    /// A host's message: its first- and second-level texts.
    pub fn message(&self, id: &str) -> Option<&(String, String)> {
        self.messages.get(id)
    }

    /// How many codes it explains.
    pub fn len(&self) -> usize {
        self.codes.len()
    }

    /// Whether it explains none.
    pub fn is_empty(&self) -> bool {
        self.codes.is_empty()
    }
}

/// The code a system shows when it posted none.
pub const NONE: &str = "00000000";

/// Whose `code` is, by its form.
pub fn whose(code: &str) -> Whose {
    if code == NONE || code.is_empty() {
        return Whose::None;
    }
    match code.as_bytes().get(1) {
        Some(b'2') => Whose::Host,
        Some(b'3') => Whose::Console,
        _ => Whose::System,
    }
}

fn family_of(code: &str) -> &'static str {
    match code.as_bytes().first() {
        Some(b'A') => "attention",
        Some(b'B') => "fatal",
        Some(b'C') => "progress",
        Some(b'D') => "power down",
        _ => "",
    }
}

/// `code` explained, when this release can.
pub fn explain(cat: &Catalogue, code: &str) -> Option<Explained> {
    match whose(code) {
        Whose::None => Some(Explained {
            code: NONE.into(),
            family: String::new(),
            title: "Operating".into(),
            meaning: "Operating; no attention required.".into(),
            cause: "The system has posted no code since it began running, or its last code was cleared.".into(),
            recovery: "None.".into(),
            message: String::new(),
        }),
        Whose::Host => cat.codes.get(code).cloned(),
        Whose::Console => refcode::find(code).map(|c| Explained {
            code: c.code.into(),
            family: family_of(c.code).into(),
            title: c.text.into(),
            meaning: c.text.into(),
            cause: String::new(),
            recovery: c.action.into(),
            message: String::new(),
        }),
        Whose::System => srcdata::lookup(code).map(|s| Explained {
            code: s.code.into(),
            family: family_of(s.code).into(),
            title: s.text.into(),
            meaning: s.meaning.into(),
            cause: s.cause.unwrap_or_default().into(),
            recovery: s.recovery.unwrap_or_default().into(),
            message: String::new(),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ONE: &str = r#"{"schema":"quartz.catalogue/1","product":"QS/VS","release":"V1R0M0","build":"x",
        "codes":[{"code":"A2002010","family":"attention","meaning":"Management interface has no carrier",
        "cause":"the cable is out","recovery":{"problem":true,"message":"QVS5112",
        "description":"Management interface &1 has no carrier.","action":"Check the cable."}}],
        "messages":[{"id":"QVS5112","text":"Management interface &1 has no carrier.","help":"Check the cable."}]}"#;

    #[test]
    fn each_code_from_its_own_data() {
        let cat = Catalogue::from_json(ONE).unwrap();
        let host = explain(&cat, "A2002010").unwrap();
        assert_eq!(host.meaning, "Management interface has no carrier");
        assert_eq!(host.message, "QVS5112");
        assert!(host.recovery.ends_with("Check the cable."));
        let system = explain(&cat, "A6005008").unwrap();
        assert!(!system.meaning.is_empty());
        assert_eq!(whose("A6005008"), Whose::System);
        let console = explain(&cat, "A3001015").unwrap();
        assert!(console.meaning.starts_with("Processor"));
        assert_eq!(
            explain(&cat, NONE).unwrap().meaning,
            "Operating; no attention required."
        );
        assert!(explain(&cat, "A2999999").is_none());
        assert!(explain(&cat, "Z9999999").is_none());
        assert_eq!(cat.message("QVS5112").unwrap().1, "Check the cable.");
    }
}
