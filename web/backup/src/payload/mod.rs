// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! The payload: `QSMCPAY1`, the manifest as `key value` lines ended by
//! an empty line, then each file as a big-endian `u16` path length, the
//! path, a `u32` mode, a `u32` length and the bytes.

use crate::Error;

/// The payload's first bytes.
pub const MAGIC: &[u8; 8] = b"QSMCPAY1";
/// The most files a backup holds.
pub const MOST_FILES: usize = 4096;
/// The most bytes a payload holds: under the web server's upload limit.
pub const MOST_BYTES: usize = 7 * 1024 * 1024;
/// The top folders a path may begin with: `/data/state`, `/data/web`,
/// the managed-systems service's `/data/mgmt` and the download
/// service's settings in `/data/fetch`. Downloaded images and dumps are
/// never in a backup.
pub const TOPS: [&str; 4] = ["state", "web", "mgmt", "fetch"];

/// What a backup says of itself.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Manifest {
    /// The console's host name.
    pub console: String,
    /// Its release, `V1R0M0`.
    pub release: String,
    /// Its build.
    pub build: String,
    /// When it was made, Unix seconds.
    pub made: u64,
}

/// One file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    /// Its path under `/data`: `state/host.toml`.
    pub path: String,
    /// Its permission bits.
    pub mode: u32,
    /// Its bytes.
    pub data: Vec<u8>,
}

/// A backup's contents.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Backup {
    /// What it says of itself.
    pub manifest: Manifest,
    /// Its files.
    pub entries: Vec<Entry>,
}

/// Whether `path` may be in a backup: under one of [`TOPS`], each part
/// letters, digits, `.`, `_` or `-`, none `.` or `..`.
pub fn valid_path(path: &str) -> bool {
    let mut parts = path.split('/');
    let top_ok = parts.next().is_some_and(|t| TOPS.contains(&t));
    let rest: Vec<&str> = parts.collect();
    top_ok
        && !rest.is_empty()
        && rest.len() <= 4
        && path.len() <= 255
        && rest.iter().all(|p| {
            !p.is_empty()
                && *p != "."
                && *p != ".."
                && p.bytes()
                    .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
        })
}

fn line_ok(v: &str) -> bool {
    !v.contains(['\n', '\r']) && v.len() <= 255
}

impl Backup {
    /// The payload's bytes.
    pub fn encode(&self) -> Result<Vec<u8>, Error> {
        let m = &self.manifest;
        if ![&m.console, &m.release, &m.build]
            .iter()
            .all(|v| line_ok(v))
        {
            return Err(Error::Damaged("a manifest value has a line break".into()));
        }
        if self.entries.len() > MOST_FILES {
            return Err(Error::Damaged(format!("more than {MOST_FILES} files")));
        }
        let mut out = MAGIC.to_vec();
        out.extend_from_slice(
            format!(
                "console {}\nrelease {}\nbuild {}\nmade {}\n\n",
                m.console, m.release, m.build, m.made
            )
            .as_bytes(),
        );
        for e in &self.entries {
            if !valid_path(&e.path) {
                return Err(Error::Damaged(format!("`{}` is not a backup path", e.path)));
            }
            let len = u32::try_from(e.data.len())
                .map_err(|_| Error::Damaged(format!("{} is too large", e.path)))?;
            out.extend_from_slice(&u16::try_from(e.path.len()).unwrap_or(0).to_be_bytes());
            out.extend_from_slice(e.path.as_bytes());
            out.extend_from_slice(&(e.mode & 0o777).to_be_bytes());
            out.extend_from_slice(&len.to_be_bytes());
            out.extend_from_slice(&e.data);
        }
        if out.len() > MOST_BYTES {
            return Err(Error::Damaged(format!(
                "{} bytes, more than a backup holds ({MOST_BYTES})",
                out.len()
            )));
        }
        Ok(out)
    }

    /// Read a payload.
    pub fn decode(bytes: &[u8]) -> Result<Self, Error> {
        let damaged = |why: &str| Error::Damaged(why.to_string());
        if bytes.len() > MOST_BYTES {
            return Err(damaged("too large"));
        }
        let rest = bytes
            .strip_prefix(MAGIC.as_slice())
            .ok_or_else(|| damaged("no payload mark"))?;
        let end = rest
            .windows(2)
            .position(|w| w == b"\n\n")
            .ok_or_else(|| damaged("no end to the manifest"))?;
        let head = std::str::from_utf8(&rest[..end]).map_err(|_| damaged("manifest not text"))?;
        let mut manifest = Manifest::default();
        // Lines end at `\n` alone, and no value is one the writer
        // refuses: what reads, writes again the same.
        for line in head.split('\n') {
            let (k, v) = line.split_once(' ').unwrap_or((line, ""));
            if !line_ok(v) {
                return Err(damaged("a manifest value the writer refuses"));
            }
            match k {
                "console" => manifest.console = v.into(),
                "release" => manifest.release = v.into(),
                "build" => manifest.build = v.into(),
                "made" => manifest.made = v.parse().map_err(|_| damaged("made"))?,
                _ => {}
            }
        }
        if manifest.release.is_empty() {
            return Err(damaged("no release in the manifest"));
        }
        let mut at = &rest[end + 2..];
        let mut entries = Vec::new();
        while !at.is_empty() {
            if entries.len() == MOST_FILES {
                return Err(damaged("too many files"));
            }
            let (n, r) = take::<2>(at).ok_or_else(|| damaged("a path's length"))?;
            let n = usize::from(u16::from_be_bytes(n));
            let (path, r) = split(r, n).ok_or_else(|| damaged("a path"))?;
            let path = std::str::from_utf8(path).map_err(|_| damaged("a path not text"))?;
            if !valid_path(path) {
                return Err(Error::Damaged(format!("`{path}` is not a backup path")));
            }
            let (mode, r) = take::<4>(r).ok_or_else(|| damaged("a mode"))?;
            let (len, r) = take::<4>(r).ok_or_else(|| damaged("a length"))?;
            let len = usize::try_from(u32::from_be_bytes(len)).map_err(|_| damaged("a length"))?;
            let (data, r) = split(r, len).ok_or_else(|| damaged("a file cut short"))?;
            if entries.iter().any(|e: &Entry| e.path == path) {
                return Err(Error::Damaged(format!("`{path}` twice")));
            }
            entries.push(Entry {
                path: path.to_string(),
                mode: u32::from_be_bytes(mode) & 0o777,
                data: data.to_vec(),
            });
            at = r;
        }
        Ok(Self { manifest, entries })
    }
}

fn take<const N: usize>(b: &[u8]) -> Option<([u8; N], &[u8])> {
    let (h, r) = split(b, N)?;
    Some((h.try_into().ok()?, r))
}

fn split(b: &[u8], n: usize) -> Option<(&[u8], &[u8])> {
    (b.len() >= n).then(|| b.split_at(n))
}

#[cfg(test)]
mod tests {
    use super::*;

    pub(crate) fn sample() -> Backup {
        Backup {
            manifest: Manifest {
                console: "QSHQMC1".into(),
                release: "V1R0M0".into(),
                build: "0e744f2".into(),
                made: 1_790_000_000,
            },
            entries: vec![
                Entry {
                    path: "state/host.toml".into(),
                    mode: 0o644,
                    data: b"name = \"QSHQMC1\"\n".to_vec(),
                },
                Entry {
                    path: "web/key.pem".into(),
                    mode: 0o600,
                    data: vec![1, 2, 3],
                },
            ],
        }
    }

    #[test]
    fn a_payload_reads_back() {
        let b = sample();
        assert_eq!(Backup::decode(&b.encode().unwrap()).unwrap(), b);
    }

    #[test]
    fn paths_stay_under_data() {
        for bad in [
            "state",
            "log/x",
            "state/../x",
            "state/./x",
            "/state/x",
            "state//x",
            "web/a b",
        ] {
            assert!(!valid_path(bad), "{bad}");
        }
        assert!(valid_path("state/identity/cert.pem"));
        let mut b = sample();
        b.entries[0].path = "state/../../etc/x".into();
        assert!(b.encode().is_err());
    }

    #[test]
    fn every_cut_is_refused() {
        let bytes = sample().encode().unwrap();
        for n in 0..bytes.len() {
            let cut = Backup::decode(&bytes[..n]);
            // A cut at a file's end is a backup with fewer files.
            if let Ok(b) = cut {
                assert!(b.entries.len() < 2, "{n}");
            }
        }
    }

    /// The fuzzer's finding: a carriage return in the manifest was read
    /// and could not be written again.
    #[test]
    fn what_reads_writes_again() {
        assert!(Backup::decode(b"QSMCPAY1release \r\n\n").is_err());
        let long = format!("QSMCPAY1release {}\n\n", "V".repeat(300));
        assert!(Backup::decode(long.as_bytes()).is_err());
        let b = sample();
        let again = Backup::decode(&b.encode().unwrap()).unwrap();
        assert_eq!(again.encode().unwrap(), b.encode().unwrap());
    }
}
