// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! The installation media: the block device whose ISO 9660 volume is
//! this release's, a virtual or real CD or the same image on a USB
//! stick, mounted read-only, and its payload checked before anything is
//! written. The manifest must be signed by a key of this initramfs and
//! each UKI must have the length and hash the manifest gives it.
//!
//! BLOCKED(phase2): the payload's format is `base/loadsource::payload`,
//! but its check is QS/VS's own (`lic/qsvsinstall/src/media.rs`); it
//! belongs beside the format (`docs/OPEN_QUESTIONS.md` item 15).

use std::fs::{self, File};
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use loadsource::payload::{self, Manifest};
use release::LOAD_SOURCE;
use sha2::{Digest, Sha256};

/// Where the media is mounted.
pub const MOUNT: &str = "/run/media";
/// How long the media's device may take to appear: a USB stick is
/// found seconds after the kernel starts.
const WAIT: Duration = Duration::from_secs(20);
/// The primary volume descriptor's place, and its volume identifier.
const DESCRIPTOR: u64 = 16 * 2048;
const VOLUME_ID: std::ops::Range<usize> = 40..72;
/// An ed25519 public key's length.
const KEY_LEN: usize = 32;

/// The payload, checked, in memory.
#[derive(Debug)]
pub struct Payload {
    /// The manifest.
    pub manifest: Manifest,
    /// Slot A's UKI.
    pub slot_a: Vec<u8>,
    /// Slot B's UKI.
    pub slot_b: Vec<u8>,
}

/// The volume identifier of an ISO 9660 file system on `device`.
fn volume_id(device: &Path) -> Option<String> {
    let mut file = File::open(device).ok()?;
    let mut sector = [0u8; 2048];
    file.seek(SeekFrom::Start(DESCRIPTOR)).ok()?;
    file.read_exact(&mut sector).ok()?;
    if sector[0] != 1 || &sector[1..6] != b"CD001" {
        return None;
    }
    Some(
        String::from_utf8_lossy(&sector[VOLUME_ID])
            .trim_end()
            .to_string(),
    )
}

/// The whole block device that holds this release's media.
pub fn find() -> Option<PathBuf> {
    let wanted = payload::volume_id(LOAD_SOURCE.vendor, &release::RELEASE.to_string());
    let mut names: Vec<String> = fs::read_dir("/sys/class/block")
        .ok()?
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| {
            !["loop", "ram", "zram", "ublk"]
                .iter()
                .any(|p| n.starts_with(p))
        })
        .collect();
    names.sort();
    names
        .into_iter()
        .map(|n| Path::new("/dev").join(n))
        .find(|device| volume_id(device).as_deref() == Some(wanted.as_str()))
}

/// The media's device, waited for.
pub fn wait() -> Result<PathBuf, String> {
    let give_up = Instant::now() + WAIT;
    loop {
        if let Some(device) = find() {
            return Ok(device);
        }
        if Instant::now() >= give_up {
            return Err(format!(
                "no device holds the installation media of {} {}",
                release::NAME,
                release::RELEASE
            ));
        }
        std::thread::sleep(Duration::from_millis(250));
    }
}

/// The file named `name` in `dir`, in whatever case the file system
/// shows it, and with a plain ISO 9660 name's version or not.
fn named(dir: &Path, name: &str) -> Result<PathBuf, String> {
    fs::read_dir(dir)
        .map_err(|e| format!("{}: {e}", dir.display()))?
        .flatten()
        .find(|e| {
            let found = e.file_name().to_string_lossy().into_owned();
            let bare = found.split(';').next().unwrap_or_default();
            bare.eq_ignore_ascii_case(name)
        })
        .map(|e| e.path())
        .ok_or_else(|| format!("the media has no {name}"))
}

/// Whether `signature` over `text` is by one of `keys`, 32 bytes each.
pub fn signed(text: &[u8], signature: &[u8], keys: &[u8]) -> bool {
    let Ok(signature) = Signature::from_slice(signature) else {
        return false;
    };
    keys.as_chunks::<KEY_LEN>()
        .0
        .iter()
        .any(|key| VerifyingKey::from_bytes(key).is_ok_and(|k| k.verify(text, &signature).is_ok()))
}

/// Whether `bytes` are the file `name` of `manifest`.
pub fn matches(manifest: &Manifest, name: &str, bytes: &[u8]) -> Result<(), String> {
    let file = manifest
        .file(name)
        .ok_or_else(|| format!("the manifest does not list {name}"))?;
    if file.bytes != bytes.len() as u64 {
        return Err(format!(
            "{name} is {} bytes and the manifest says {}",
            bytes.len(),
            file.bytes
        ));
    }
    if hex::encode(Sha256::digest(bytes)) != file.sha256 {
        return Err(format!("{name} does not have the hash the manifest gives"));
    }
    Ok(())
}

/// Check the payload under `root`, the mounted media, against `keys`.
pub fn verify(root: &Path, keys: &[u8]) -> Result<Payload, String> {
    let dir = named(root, payload::dir(&LOAD_SOURCE))?;
    let read = |name: &str| -> Result<Vec<u8>, String> {
        fs::read(named(&dir, name)?).map_err(|e| format!("{name}: {e}"))
    };
    let text = read(payload::MANIFEST)?;
    let signature = hex::decode(String::from_utf8_lossy(&read(payload::SIGNATURE)?).trim())
        .map_err(|_| "the manifest's signature is not hexadecimal".to_string())?;
    if !signed(&text, &signature, keys) {
        return Err("the manifest is not signed by a key this installer trusts".into());
    }
    let manifest = Manifest::parse(&LOAD_SOURCE, &String::from_utf8_lossy(&text))?;
    if manifest.release != release::RELEASE.to_string() {
        return Err(format!(
            "the media is of release {}; this installer is of {}",
            manifest.release,
            release::RELEASE
        ));
    }
    let slot_a = read(payload::SLOT_A)?;
    matches(&manifest, payload::SLOT_A, &slot_a)?;
    let slot_b = read(payload::SLOT_B)?;
    matches(&manifest, payload::SLOT_B, &slot_b)?;
    Ok(Payload {
        manifest,
        slot_a,
        slot_b,
    })
}

/// Find, mount and check the media: its device and its payload.
#[cfg(target_os = "linux")]
pub fn open() -> Result<(PathBuf, Payload), String> {
    use rustix::mount::{MountFlags, mount};
    let device = wait()?;
    fs::create_dir_all(MOUNT).map_err(|e| format!("{MOUNT}: {e}"))?;
    let flags = MountFlags::RDONLY | MountFlags::NOSUID | MountFlags::NODEV | MountFlags::NOEXEC;
    mount(&device, MOUNT, "iso9660", flags, None)
        .map_err(|e| format!("{}: the media was not mounted: {e}", device.display()))?;
    let keys =
        fs::read(release::TRUSTED_KEYS).map_err(|e| format!("{}: {e}", release::TRUSTED_KEYS))?;
    let payload = verify(Path::new(MOUNT), &keys)?;
    Ok((device, payload))
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Signer, SigningKey};
    use loadsource::payload::File as Entry;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("qsmcinstall-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("qsmc")).unwrap();
        dir
    }

    /// The payload as the kernel shows a plain ISO 9660: lower case.
    fn write(dir: &Path, key: &SigningKey) {
        let (a, b) = (b"slot a".to_vec(), b"slot b".to_vec());
        let entry = |name: &str, bytes: &[u8]| Entry {
            name: name.into(),
            bytes: bytes.len() as u64,
            sha256: hex::encode(Sha256::digest(bytes)),
        };
        let manifest = Manifest {
            product: "QS/MC".into(),
            release: release::RELEASE.to_string(),
            build: "abc1234".into(),
            files: vec![entry(payload::SLOT_A, &a), entry(payload::SLOT_B, &b)],
        };
        let text = manifest.render(&LOAD_SOURCE);
        let put = |name: &str, bytes: &[u8]| {
            fs::write(dir.join("qsmc").join(name.to_lowercase()), bytes).unwrap();
        };
        put(payload::MANIFEST, text.as_bytes());
        let signature = hex::encode(key.sign(text.as_bytes()).to_bytes());
        put(payload::SIGNATURE, format!("{signature}\n").as_bytes());
        put(payload::SLOT_A, &a);
        put(payload::SLOT_B, &b);
    }

    #[test]
    fn a_payload_signed_by_a_trusted_key_is_taken() {
        let dir = scratch("good");
        let key = SigningKey::from_bytes(&[3; 32]);
        write(&dir, &key);
        let mut keys = SigningKey::from_bytes(&[4; 32])
            .verifying_key()
            .to_bytes()
            .to_vec();
        keys.extend_from_slice(&key.verifying_key().to_bytes());
        let payload = verify(&dir, &keys).unwrap();
        assert_eq!(payload.slot_b, b"slot b");
        assert_eq!(payload.manifest.build, "abc1234");
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_payload_is_refused_for_its_signer_its_text_or_a_file() {
        let dir = scratch("bad");
        let key = SigningKey::from_bytes(&[3; 32]);
        let keys = key.verifying_key().to_bytes();
        let file = |name: &str| dir.join("qsmc").join(name.to_lowercase());

        write(&dir, &SigningKey::from_bytes(&[5; 32]));
        assert!(verify(&dir, &keys).unwrap_err().contains("not signed"));

        write(&dir, &key);
        let text = fs::read_to_string(file(payload::MANIFEST)).unwrap();
        fs::write(file(payload::MANIFEST), text.replace("abc1234", "abc1235")).unwrap();
        assert!(verify(&dir, &keys).unwrap_err().contains("not signed"));

        write(&dir, &key);
        fs::write(file(payload::SLOT_A), b"slot A").unwrap();
        assert!(verify(&dir, &keys).unwrap_err().contains("hash"));
        fs::write(file(payload::SLOT_A), b"slot a, longer").unwrap();
        assert!(verify(&dir, &keys).unwrap_err().contains("bytes"));

        write(&dir, &key);
        fs::remove_file(file(payload::SLOT_B)).unwrap();
        assert!(verify(&dir, &keys).unwrap_err().contains("no SLOTB.EFI"));
        write(&dir, &key);
        fs::write(file(payload::SIGNATURE), "not hex").unwrap();
        assert!(verify(&dir, &keys).unwrap_err().contains("hexadecimal"));
        assert!(!signed(b"text", &[0; 64], &keys));
        assert!(!signed(b"text", &[1; 3], &keys));
        fs::remove_dir_all(&dir).unwrap();
    }
}
