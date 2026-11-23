// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! The release's own stream layout, found by a console (`docs/RELEASING.md`),
//! run by name after `build release` and `build stream add`:
//! `QSMC_STREAM_DIR` is the directory `stream add` wrote, served as the
//! `qsmc` stream at the paths `fetchd` reads, and the console's Code
//! updates shows the release's entry. The index is the release's, read
//! with the key that signed it (`QSMC_SIGNING_KEY`), and signed again
//! with the harness's `qsmc` stream key, which a test image trusts:
//! `build stream verify` checks the release's own signature.

use std::path::PathBuf;

use crate::streams::keys::test_key;
use crate::suite::base::Run;
use crate::suite::serv::{CONSOLE_ROW, check, lab, row};

/// The suite.
pub fn stream(run: &Run<'_>) -> Result<String, String> {
    let dir = PathBuf::from(
        std::env::var_os("QSMC_STREAM_DIR")
            .ok_or("QSMC_STREAM_DIR is not set: the directory `build stream add` wrote")?,
    );
    let key_file = std::env::var_os("QSMC_SIGNING_KEY")
        .ok_or("QSMC_SIGNING_KEY is not set: the key the release was signed with")?;
    let seed = hex::decode(
        std::fs::read_to_string(&key_file)
            .map_err(|e| e.to_string())?
            .trim(),
    )
    .map_err(|e| e.to_string())?;
    let seed: [u8; 32] = seed
        .try_into()
        .map_err(|_| "the signing key is not 32 bytes")?;
    let signer = ed25519_dalek::SigningKey::from_bytes(&seed);
    let bytes = std::fs::read(dir.join("index.toml")).map_err(|e| e.to_string())?;
    let index = stream::verify(&bytes, &[signer.verifying_key().to_bytes()], 0)
        .map_err(|e| format!("the release's index does not verify: {e:?}"))?;
    let entry = index
        .releases
        .iter()
        .find(|e| e.release == release::RELEASE.to_string())
        .cloned()
        .ok_or("the stream has no entry of this release")?;
    let mut lab = lab(run, "sec-stream", &[])?;
    let s = &lab.streams;
    let again = stream::Index {
        sequence: index.sequence + 100,
        ..index.clone()
    };
    s.put(
        "/qsmc/v1/index",
        stream::sign(&again, &test_key("qsmc")).into_bytes(),
    );
    for name in [&entry.update, &entry.notes] {
        if name.is_empty() {
            continue;
        }
        let file = std::fs::read(dir.join(name)).map_err(|e| format!("{name}: {e}"))?;
        s.put(&format!("/qsmc/v1/file/{name}"), file);
    }
    let page = check(&mut lab.c.browser)?;
    if page.contains("QMC7201") {
        return Err("the console refused the stream's index".into());
    }
    let console = row(&page, CONSOLE_ROW);
    if entry.notes.is_empty() || !console.contains(&entry.notes) {
        return Err(format!(
            "Code updates does not show the release's entry ({}): {console}",
            entry.notes
        ));
    }
    lab.c.guest.kill();
    Ok(format!(
        "{} {} of the release's stream (sequence {}, {} entries) found by the console: its \
         row shows the entry and its notes ({})",
        entry.release,
        entry.build,
        index.sequence,
        index.releases.len(),
        entry.notes
    ))
}
