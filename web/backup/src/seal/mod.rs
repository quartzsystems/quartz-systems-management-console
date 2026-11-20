// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! The file: a header, then the payload under AES-256-GCM with the
//! header as its additional data. The header is `QSMCBAK1`, Argon2id's
//! memory (KiB), passes and lanes as big-endian `u32`s, a 16-byte salt
//! and the 12-byte nonce. The tag follows the ciphertext.

use argon2::{Algorithm, Argon2, Params, Version};
use graviola::aead::AesGcm;
use zeroize::Zeroizing;

use crate::{Backup, Error};

/// The file's first bytes.
pub const MAGIC: &[u8; 8] = b"QSMCBAK1";
/// The header's length.
pub const HEADER: usize = 8 + 12 + 16 + 12;
/// The tag's length.
const TAG: usize = 16;
/// The file's extension.
pub const EXTENSION: &str = "qmcb";

/// The fewest and the most characters of a passphrase.
pub const PASSPHRASE: (usize, usize) = (12, 128);

/// Argon2id's cost as made: 64 MiB, three passes, one lane.
const MAKE: (u32, u32, u32) = (64 * 1024, 3, 1);
/// The most a file may ask for: a file is not trusted to set the cost.
const MOST: (u32, u32, u32) = (256 * 1024, 10, 4);

/// Whether `p` is a passphrase a backup may be made with.
pub fn passphrase_ok(p: &str) -> bool {
    (PASSPHRASE.0..=PASSPHRASE.1).contains(&p.chars().count())
}

fn key(
    passphrase: &str,
    salt: &[u8],
    (m, t, p): (u32, u32, u32),
) -> Result<Zeroizing<[u8; 32]>, Error> {
    let params =
        Params::new(m, t, p, Some(32)).map_err(|e| Error::NotBackup(format!("its cost: {e}")))?;
    let mut out = Zeroizing::new([0u8; 32]);
    Argon2::new(Algorithm::Argon2id, Version::V0x13, params)
        .hash_password_into(passphrase.as_bytes(), salt, out.as_mut())
        .map_err(|e| Error::NotBackup(format!("its key: {e}")))?;
    Ok(out)
}

fn random<const N: usize>() -> Result<[u8; N], Error> {
    let mut b = [0u8; N];
    graviola::random::fill(&mut b)
        .map_err(|e| Error::Damaged(format!("no random bytes: {e:?}")))?;
    Ok(b)
}

/// The file of `backup` under `passphrase`.
pub fn seal(backup: &Backup, passphrase: &str) -> Result<Vec<u8>, Error> {
    if !passphrase_ok(passphrase) {
        return Err(Error::Passphrase);
    }
    let salt: [u8; 16] = random()?;
    let nonce: [u8; 12] = random()?;
    let mut out = MAGIC.to_vec();
    for v in [MAKE.0, MAKE.1, MAKE.2] {
        out.extend_from_slice(&v.to_be_bytes());
    }
    out.extend_from_slice(&salt);
    out.extend_from_slice(&nonce);
    let key = key(passphrase, &salt, MAKE)?;
    let mut body = Zeroizing::new(backup.encode()?);
    let mut tag = [0u8; TAG];
    AesGcm::new(key.as_ref()).encrypt(&nonce, &out, &mut body, &mut tag);
    out.extend_from_slice(&body);
    out.extend_from_slice(&tag);
    Ok(out)
}

/// A file's header, read and checked before any work is done for it.
#[derive(Debug, PartialEq, Eq)]
pub struct Header<'a> {
    /// Argon2id's memory (KiB), passes and lanes.
    pub cost: (u32, u32, u32),
    /// The salt.
    pub salt: &'a [u8],
    /// The nonce.
    pub nonce: [u8; 12],
    /// The header's bytes: the cipher's additional data.
    pub bytes: &'a [u8],
    /// The ciphertext.
    pub body: &'a [u8],
    /// The tag.
    pub tag: &'a [u8],
}

/// The header of the file `bytes`: refused when the file is short, is
/// not one, or asks for more work than a backup takes.
pub fn header(bytes: &[u8]) -> Result<Header<'_>, Error> {
    let not = |why: &str| Error::NotBackup(why.to_string());
    if bytes.len() < HEADER + TAG || !bytes.starts_with(MAGIC) {
        return Err(not("it does not begin as one"));
    }
    let (header, rest) = bytes.split_at(HEADER);
    let word = |i: usize| {
        let at = 8 + 4 * i;
        u32::from_be_bytes([header[at], header[at + 1], header[at + 2], header[at + 3]])
    };
    let cost = (word(0), word(1), word(2));
    if cost.0 > MOST.0 || cost.1 > MOST.1 || cost.2 > MOST.2 {
        return Err(not("it asks for more work than a backup takes"));
    }
    let nonce: [u8; 12] = header[36..48].try_into().map_err(|_| not("its nonce"))?;
    let (body, tag) = rest.split_at(rest.len() - TAG);
    Ok(Header {
        cost,
        salt: &header[20..36],
        nonce,
        bytes: header,
        body,
        tag,
    })
}

/// Read the file `bytes` with `passphrase`.
pub fn open(bytes: &[u8], passphrase: &str) -> Result<Backup, Error> {
    let h = header(bytes)?;
    let key = key(passphrase, h.salt, h.cost)?;
    let mut plain = Zeroizing::new(h.body.to_vec());
    AesGcm::new(key.as_ref())
        .decrypt(&h.nonce, h.bytes, &mut plain, h.tag)
        .map_err(|_| Error::Passphrase)?;
    Backup::decode(&plain)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Entry, Manifest};

    fn sample() -> Backup {
        Backup {
            manifest: Manifest {
                console: "QSHQMC1".into(),
                release: "V1R0M0".into(),
                build: "b".into(),
                made: 1,
            },
            entries: vec![Entry {
                path: "state/webusers.toml".into(),
                mode: 0o600,
                data: b"secret hash".to_vec(),
            }],
        }
    }

    const PASS: &str = "correct horse battery";

    #[test]
    fn sealed_opens_with_its_passphrase_only() {
        let file = seal(&sample(), PASS).unwrap();
        assert_eq!(open(&file, PASS).unwrap(), sample());
        assert_eq!(open(&file, "another passphrase"), Err(Error::Passphrase));
        // Nothing of the payload shows.
        assert!(!file.windows(11).any(|w| w == b"secret hash"));
        assert!(!file.windows(8).any(|w| w == crate::payload::MAGIC));
    }

    #[test]
    fn a_changed_byte_is_refused() {
        let file = seal(&sample(), PASS).unwrap();
        for i in [0, 9, 25, 40, HEADER + 1, file.len() - 1] {
            let mut f = file.clone();
            f[i] ^= 1;
            assert!(open(&f, PASS).is_err(), "byte {i}");
        }
        assert!(matches!(open(b"QSMCBAK1", PASS), Err(Error::NotBackup(_))));
    }

    #[test]
    fn a_short_passphrase_makes_nothing() {
        assert_eq!(seal(&sample(), "short"), Err(Error::Passphrase));
    }
}
