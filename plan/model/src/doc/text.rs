// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! A plan's text, read and written. An imported file is hostile input:
//! the size is checked before anything is read, the text is UTF-8 TOML
//! with only the document's keys, its version no newer than this
//! release's, and every value in bounds. Nothing of it is acted on here.

use super::Plan;

/// The largest plan, bytes.
pub const MAX_BYTES: usize = 1 << 20;

/// Why a plan's text was refused.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Refused {
    /// Past [`MAX_BYTES`]: its size.
    TooLarge(usize),
    /// Not UTF-8 text.
    NotText,
    /// Written by a newer release: its version.
    Newer(u32),
    /// Not a plan, or a value out of bounds: what is wrong.
    Invalid(String),
}

impl std::fmt::Display for Refused {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TooLarge(n) => write!(f, "{n} bytes, more than the {MAX_BYTES} a plan may be"),
            Self::NotText => f.write_str("it is not text"),
            Self::Newer(v) => write!(
                f,
                "it is schema version {v}, newer than {}, the newest this release reads",
                super::VERSION
            ),
            Self::Invalid(why) => f.write_str(why),
        }
    }
}

/// A plan from its bytes.
pub fn read(bytes: &[u8]) -> Result<Plan, Refused> {
    if bytes.len() > MAX_BYTES {
        return Err(Refused::TooLarge(bytes.len()));
    }
    let text = std::str::from_utf8(bytes).map_err(|_| Refused::NotText)?;
    if text.contains('\0') {
        return Err(Refused::NotText);
    }
    match state::store::parse::<Plan>(text) {
        Ok(p) => Ok(p),
        Err(state::StateError::Newer { found, .. }) => Err(Refused::Newer(found)),
        Err(state::StateError::Invalid { reason, .. }) => {
            Err(Refused::Invalid(first_line(&reason)))
        }
        Err(e) => Err(Refused::Invalid(e.to_string())),
    }
}

/// The TOML parser's message to its first line, without the text it
/// quotes.
fn first_line(reason: &str) -> String {
    let line = reason
        .lines()
        .find(|l| !l.trim().is_empty())
        .unwrap_or(reason);
    line.chars().take(200).collect()
}

/// A plan's text; refused when a value is out of bounds or it is too
/// large.
pub fn write(p: &Plan) -> Result<String, String> {
    let text = state::store::render(p).map_err(|e| match e {
        state::StateError::Invalid { reason, .. } => reason,
        e => e.to_string(),
    })?;
    if text.len() > MAX_BYTES {
        return Err(Refused::TooLarge(text.len()).to_string());
    }
    Ok(text)
}
