// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! The product streams' keys a slot's image trusts (`docs/UPDATES.md`):
//! `/qsmc/keys/streams/<stream>.pub`, a line of 64 hexadecimal
//! characters a key. A test image trusts the harness's test keys. A
//! release image trusts the keys `QSMC_STREAM_KEYS` names (a folder of
//! `qsmc.pub`, `qsvs.pub` and `qs400.pub`, the three streams' release
//! keys, which are Cody's to make); built without them it is refused
//! when it is signed with a release key (`QSMC_SIGNING_KEY`), and
//! otherwise is a development build that trusts development keys the
//! build makes from its own development signing key, saying so.

use std::fs;
use std::path::Path;

use anyhow::{Context, Result, bail};
use buildkit::Ctx;
use ed25519_dalek::SigningKey;
use tracing::warn;

use super::Image;

/// The folder of the streams' release keys.
pub const ENV: &str = "QSMC_STREAM_KEYS";

/// The streams, by name.
pub const STREAMS: [&str; 3] = conformance::streams::keys::STREAMS;

/// QS/MC's release keys' names: `qsmc-release.ed25519`, `qsmc-db.cer`.
pub const NAMES: buildaid::release::Names = buildaid::release::Names {
    prefix: "qsmc",
    sb_subject: "CN=Quartz Systems QS/MC Secure Boot db 2026,O=Quartz Systems",
    stream: "qsmc",
};

/// `build keys DIR`: the release signing key and a Secure Boot db pair,
/// made once on a trusted machine by `base/buildaid`. Nothing is written
/// over.
pub fn make(dir: &Path) -> Result<()> {
    buildaid::release::keys(dir, &NAMES)
}

/// Whether `text` is a keys' file: at least one line, each 64
/// hexadecimal characters.
fn keys_file(text: &str) -> bool {
    let lines: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).collect();
    !lines.is_empty()
        && lines
            .iter()
            .all(|l| l.trim().len() == 64 && hex::decode(l.trim()).is_ok())
}

/// A development key of stream `name`: made from the build's own
/// signing key (a development key itself here), so the same key makes
/// the same image; never a release's.
fn development(ctx: &Ctx, name: &str) -> Result<SigningKey> {
    use sha2::{Digest, Sha256};
    let build = buildkit::image::manifest::signing_key(ctx)?;
    let mut h = Sha256::new();
    h.update(b"QS/MC development stream key ");
    h.update(name.as_bytes());
    h.update(build.to_bytes());
    let seed: [u8; 32] = h.finalize().into();
    warn!("the {name} stream's key is a development key: a release needs {ENV}");
    Ok(SigningKey::from_bytes(&seed))
}

/// Each stream's keys' file for `image`: its name and text.
pub fn files(ctx: &Ctx, image: Image) -> Result<Vec<(String, String)>> {
    let pubfile = conformance::streams::keys::public_file;
    if image == Image::Test {
        return Ok(STREAMS
            .iter()
            .map(|s| {
                (
                    (*s).to_string(),
                    pubfile(&conformance::streams::keys::test_key(s)),
                )
            })
            .collect());
    }
    if let Ok(dir) = std::env::var(ENV) {
        let dir = Path::new(&dir);
        return STREAMS
            .iter()
            .map(|s| {
                let p = dir.join(format!("{s}.pub"));
                let text = fs::read_to_string(&p)
                    .with_context(|| format!("{ENV}: cannot read {}", p.display()))?;
                if !keys_file(&text) {
                    bail!(
                        "{ENV}: {} is not a file of keys, 64 hexadecimal characters a line",
                        p.display()
                    );
                }
                Ok(((*s).to_string(), text))
            })
            .collect();
    }
    if std::env::var_os(ctx.product.env_name("SIGNING_KEY")).is_some() {
        bail!(
            "a release image needs the three streams' release keys: set {ENV} to the folder of \
             qsmc.pub, qsvs.pub and qs400.pub (NEEDS-CODY)"
        );
    }
    STREAMS
        .iter()
        .map(|s| Ok(((*s).to_string(), pubfile(&development(ctx, s)?))))
        .collect()
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_keys_file_is_lines_of_keys() {
        assert!(super::keys_file(&format!("{}\n", "ab".repeat(32))));
        assert!(super::keys_file(&format!(
            "{}\n{}\n",
            "ab".repeat(32),
            "cd".repeat(32)
        )));
        assert!(!super::keys_file(""));
        assert!(!super::keys_file("not a key\n"));
        assert!(!super::keys_file(&"ab".repeat(31)));
    }
}
