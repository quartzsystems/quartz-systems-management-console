// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! What a stream sends, read as hostile: its index (`base/stream`) or
//! catalog (`net/updcat`) used only if its signature verifies against
//! the stream's keys in the image, it names this stream, its sequence is
//! not below the highest seen, and every file it names is a plain name;
//! and a downloaded file used only if its size and SHA-256 are what the
//! index names. Nothing here is written: the service keeps what passes.

use std::io::Read;
use std::path::Path;

use msg::Issued;
use sha2::{Digest, Sha256};

use crate::proto::{Entry, Level};
use crate::streams::{Stream, file_name};

/// The most an index or catalog may be.
pub const MAX_INDEX: u64 = 1 << 20;

/// What verified.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Taken {
    /// Its sequence.
    pub sequence: u64,
    /// When it was made.
    pub generated: u64,
    /// An index's releases.
    pub entries: Vec<Entry>,
    /// A catalog's latest level of each release.
    pub levels: Vec<Level>,
}

fn form(stream: Stream, why: &str) -> Issued {
    let m = qmcmsg::base_of(&qmcmsg::STREAM_FORM);
    m.with(&[stream.name(), why])
}

/// `bytes` as `stream` sent them, verified against `keys`, its sequence
/// at least `seen`.
pub fn index(stream: Stream, bytes: &[u8], keys: &[[u8; 32]], seen: u64) -> Result<Taken, Issued> {
    let not_signed = || qmcmsg::base_of(&qmcmsg::STREAM_NOT_SIGNED).with(&[stream.name()]);
    let older = |n: u64| {
        qmcmsg::base_of(&qmcmsg::STREAM_OLDER).with(&[
            stream.name(),
            &n.to_string(),
            &seen.to_string(),
        ])
    };
    if bytes.len() as u64 > MAX_INDEX {
        return Err(form(stream, "it is larger than an index may be"));
    }
    if stream.is_catalog() {
        let text = std::str::from_utf8(bytes).map_err(|_| form(stream, "it is not UTF-8"))?;
        let c = updcat::Catalog::parse(text, keys).map_err(|e| match e {
            updcat::Error::Verify(_) => not_signed(),
            updcat::Error::Parse(why) => form(stream, &why),
        })?;
        if c.sequence < seen {
            return Err(older(c.sequence));
        }
        let mut releases: Vec<String> = c.groups.iter().map(|g| g.release.clone()).collect();
        releases.sort();
        releases.dedup();
        let levels = releases
            .iter()
            .filter_map(|r| {
                let l = c.latest(r)?;
                Some(Level {
                    release: r.clone(),
                    name: l.name,
                    level: l.level,
                    date: l.date.map(|d| d.to_string()).unwrap_or_default(),
                })
            })
            .collect();
        return Ok(Taken {
            sequence: c.sequence,
            generated: c.generated,
            entries: Vec::new(),
            levels,
        });
    }
    let i = stream::verify(bytes, keys, seen).map_err(|r| match r {
        stream::Refused::Unsigned | stream::Refused::NotTrusted => not_signed(),
        stream::Refused::Form(why) => form(stream, &why),
        stream::Refused::Older { sequence, .. } => older(sequence),
    })?;
    if i.stream != stream.name() {
        return Err(form(stream, &format!("it is the {:?} stream's", i.stream)));
    }
    if let Some(e) = i
        .releases
        .iter()
        .find(|e| !file_name(&e.update) || !(e.notes.is_empty() || file_name(&e.notes)))
    {
        return Err(form(
            stream,
            &format!("release {} names a file that is not one", e.release),
        ));
    }
    Ok(Taken {
        sequence: i.sequence,
        generated: i.generated,
        entries: i.releases.iter().map(Entry::of).collect(),
        levels: Vec::new(),
    })
}

/// Whether the file at `path` is `bytes` long with SHA-256 `sha256`;
/// why not when it is not.
pub fn file(path: &Path, bytes: u64, sha256: &str) -> Result<(), String> {
    let mut f = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let len = f.metadata().map_err(|e| e.to_string())?.len();
    if len != bytes {
        return Err(format!("it is {len} bytes, and the index names {bytes}"));
    }
    let mut h = Sha256::new();
    let mut buf = vec![0u8; 1 << 16];
    loop {
        let n = f.read(&mut buf).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    let got = hex::encode(h.finalize());
    if got.eq_ignore_ascii_case(sha256) {
        Ok(())
    } else {
        Err(format!(
            "its SHA-256 is {got}, and the index names {sha256}"
        ))
    }
}

/// One input of the fuzzer: `data` as each stream's index, verified
/// against a key that signed nothing of it.
pub fn fuzz(data: &[u8]) {
    for s in Stream::ALL {
        let _ = index(s, data, &[[0x42; 32]], 0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::SigningKey;

    fn key(n: u8) -> SigningKey {
        SigningKey::from_bytes(&[n; 32])
    }

    fn index_of(name: &str, sequence: u64) -> stream::Index {
        stream::Index {
            format: stream::FORMAT,
            stream: name.into(),
            sequence,
            generated: 1_800_000_000,
            releases: vec![stream::Entry {
                release: "V1R0M1".into(),
                build: "b1".into(),
                fixes: vec!["QM00001".into()],
                updates_from: "V1R0M0".into(),
                published: 1_800_000_000,
                update: "QSMC-V1R0M1-update-ab.upd".into(),
                update_bytes: 3,
                update_sha256: hex::encode(Sha256::digest(b"abc")),
                iso: "QSMC-V1R0M1-ab.iso".into(),
                iso_bytes: 1,
                iso_sha256: "00".repeat(32),
                released: "2027-01-02".into(),
                notes: "RELEASE_NOTES_V1R0M1.md".into(),
            }],
        }
    }

    #[test]
    fn an_index_is_taken_only_signed_by_the_stream_s_key_and_never_older() {
        let k = key(1);
        let pk = k.verifying_key().to_bytes();
        let good = stream::sign(&index_of("qsmc", 5), &k);
        let t = index(Stream::Qsmc, good.as_bytes(), &[pk], 4).unwrap();
        assert_eq!((t.sequence, t.entries.len()), (5, 1));
        assert_eq!(t.entries[0].released, "2027-01-02");
        // Another key, or none.
        let e = index(
            Stream::Qsmc,
            good.as_bytes(),
            &[key(2).verifying_key().to_bytes()],
            0,
        )
        .unwrap_err();
        assert_eq!(e.id, "QMC7201");
        assert_eq!(
            index(Stream::Qsmc, good.as_bytes(), &[], 0).unwrap_err().id,
            "QMC7201"
        );
        // A byte changed.
        let bad = good.replacen("V1R0M1", "V1R0M2", 1);
        assert_eq!(
            index(Stream::Qsmc, bad.as_bytes(), &[pk], 0)
                .unwrap_err()
                .id,
            "QMC7201"
        );
        // Older than seen.
        let e = index(Stream::Qsmc, good.as_bytes(), &[pk], 6).unwrap_err();
        assert_eq!(
            (e.id.as_str(), e.text.contains("sequence 5")),
            ("QMC7202", true)
        );
        // Another stream's index, signed by this stream's key.
        let other = stream::sign(&index_of("qsvs", 9), &k);
        assert_eq!(
            index(Stream::Qsmc, other.as_bytes(), &[pk], 0)
                .unwrap_err()
                .id,
            "QMC7203"
        );
        // A file named by a path.
        let mut i = index_of("qsmc", 7);
        i.releases[0].update = "../x".into();
        let path = stream::sign(&i, &k);
        assert_eq!(
            index(Stream::Qsmc, path.as_bytes(), &[pk], 0)
                .unwrap_err()
                .id,
            "QMC7203"
        );
    }

    #[test]
    fn a_catalog_gives_each_release_s_latest_level() {
        let k = key(3);
        let pk = k.verifying_key().to_bytes();
        let c = updcat::Catalog {
            format: updcat::FORMAT,
            sequence: 42,
            generated: 1_800_000_000,
            products: Vec::new(),
            ptfs: Vec::new(),
            groups: vec![updcat::CatalogGroup {
                name: "C26290110".into(),
                kind: updcat::GroupKind::Cumulative,
                level: 1,
                release: "V1R1M0".into(),
                text: "Cumulative".into(),
                ptfs: Vec::new(),
                related: Vec::new(),
            }],
        };
        let text = c.sign(&k);
        let t = index(Stream::Qs400, text.as_bytes(), &[pk], 0).unwrap();
        assert_eq!(t.levels.len(), 1);
        assert_eq!(t.levels[0].name, "C26290110");
        assert_eq!(t.levels[0].date, "2026-10-17");
        assert_eq!(
            index(Stream::Qs400, text.as_bytes(), &[pk], 43)
                .unwrap_err()
                .id,
            "QMC7202"
        );
        assert_eq!(
            index(Stream::Qs400, text.as_bytes(), &[[9; 32]], 0)
                .unwrap_err()
                .id,
            "QMC7201"
        );
    }

    #[test]
    fn a_file_is_its_index_s_size_and_hash() {
        let d = std::env::temp_dir().join(format!("fetchd-verify-{}", std::process::id()));
        std::fs::write(&d, b"abc").unwrap();
        let h = hex::encode(Sha256::digest(b"abc"));
        assert_eq!(file(&d, 3, &h), Ok(()));
        assert!(file(&d, 4, &h).unwrap_err().contains("names 4"));
        assert!(
            file(&d, 3, &"00".repeat(32))
                .unwrap_err()
                .contains("SHA-256")
        );
        let _ = std::fs::remove_file(&d);
    }

    #[test]
    fn hostile_bytes_are_refused_not_followed() {
        fuzz(b"");
        fuzz(b"# signature zz");
        fuzz(&[0xff; 64]);
        fuzz(b"format = 1\nstream = \"qsmc\"\n# signature 00\n");
    }
}
