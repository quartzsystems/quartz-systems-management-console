// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! A boot entry written into a copy of OVMF's variable store, so a test
//! boot goes by a firmware boot entry (`Boot0000` first in
//! `BootOrder`) the way an installed console's does. The store is a
//! firmware volume holding an authenticated variable store (EDK II's
//! `VariableFormat.h`); variables are appended after the ones there.
//! The load option itself is QS/400's `diskimg::efi::load_option`.

use uuid::{Uuid, uuid};

/// The firmware volume header's signature, at offset 40.
const FV_SIGNATURE: &[u8; 4] = b"_FVH";
/// The authenticated variable store's GUID.
const AUTH_STORE: Uuid = uuid!("aaf32c78-947b-439a-a180-2e144ec37792");
/// The EFI global variable GUID.
const GLOBAL: Uuid = uuid!("8be4df61-93ca-11d2-aa0d-00e098032b8c");
/// Bytes of the variable store header.
const STORE_HEADER: usize = 28;
/// A formatted, healthy store.
const STORE_FORMATTED: u8 = 0x5A;
const STORE_HEALTHY: u8 = 0xFE;
/// What every variable header starts with.
const START_ID: u16 = 0x55AA;
/// Bytes of an authenticated variable header.
const VAR_HEADER: usize = 60;
/// A variable that was added and is valid.
const VAR_ADDED: u8 = 0x3F;
/// Non-volatile, boot service and runtime access.
const ATTRS: u32 = 0x7;

fn u16_at(b: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_le_bytes(b.get(at..at + 2)?.try_into().ok()?))
}

fn u32_at(b: &[u8], at: usize) -> Option<usize> {
    let v = u32::from_le_bytes(b.get(at..at + 4)?.try_into().ok()?);
    usize::try_from(v).ok()
}

fn align4(n: usize) -> usize {
    n.div_ceil(4) * 4
}

/// The variable area of `store`: where its first variable is and where
/// it ends.
fn area(store: &[u8]) -> Result<(usize, usize), String> {
    if store.get(40..44) != Some(FV_SIGNATURE) {
        return Err("not a firmware volume (no _FVH)".into());
    }
    let header = usize::from(u16_at(store, 48).ok_or("short firmware volume header")?);
    let head = store
        .get(header..header + STORE_HEADER)
        .ok_or("no variable store header")?;
    if head[..16] != AUTH_STORE.to_bytes_le() {
        return Err("not an authenticated variable store".into());
    }
    if (head[20], head[21]) != (STORE_FORMATTED, STORE_HEALTHY) {
        return Err("the variable store is not formatted and healthy".into());
    }
    let size = u32_at(head, 16).ok_or("no variable store size")?;
    let end = header + size;
    if size < STORE_HEADER || end > store.len() {
        return Err("the variable store's size is out of range".into());
    }
    Ok((header + STORE_HEADER, end))
}

/// The offset after the last variable in the area.
fn free(store: &[u8], start: usize, end: usize) -> Result<usize, String> {
    let mut at = start;
    while at + VAR_HEADER <= end && u16_at(store, at) == Some(START_ID) {
        let name = u32_at(store, at + 36).ok_or("short variable header")?;
        let data = u32_at(store, at + 40).ok_or("short variable header")?;
        at = align4(at + VAR_HEADER + name + data);
    }
    if at > end {
        return Err("a variable runs past the store".into());
    }
    Ok(at)
}

/// One variable as stored: header, UCS-2 name, data.
fn variable(name: &str, data: &[u8]) -> Result<Vec<u8>, String> {
    let name: Vec<u8> = name
        .encode_utf16()
        .chain([0])
        .flat_map(u16::to_le_bytes)
        .collect();
    let len32 = |n: usize| {
        u32::try_from(n)
            .map(u32::to_le_bytes)
            .map_err(|_| "a variable of 4 GiB or more".to_string())
    };
    let mut v = Vec::with_capacity(VAR_HEADER + name.len() + data.len());
    v.extend_from_slice(&START_ID.to_le_bytes());
    v.extend_from_slice(&[VAR_ADDED, 0]);
    v.extend_from_slice(&ATTRS.to_le_bytes());
    v.extend_from_slice(&[0; 8]); // monotonic count
    v.extend_from_slice(&[0; 16]); // time stamp
    v.extend_from_slice(&[0; 4]); // public key index
    v.extend_from_slice(&len32(name.len())?);
    v.extend_from_slice(&len32(data.len())?);
    v.extend_from_slice(&GLOBAL.to_bytes_le());
    v.extend_from_slice(&name);
    v.extend_from_slice(data);
    Ok(v)
}

/// `store` with the global variables `vars` (name, data) appended.
pub fn with_variables(store: &[u8], vars: &[(&str, &[u8])]) -> Result<Vec<u8>, String> {
    let (start, end) = area(store)?;
    let mut at = free(store, start, end)?;
    let mut out = store.to_vec();
    for (name, data) in vars {
        let bytes = variable(name, data)?;
        let next = at + bytes.len();
        if next > end {
            return Err(format!("no room for {name} in the variable store"));
        }
        if out[at..next].iter().any(|b| *b != 0xFF) {
            return Err("the variable store's free space is not erased".into());
        }
        out[at..next].copy_from_slice(&bytes);
        at = align4(next);
    }
    Ok(out)
}

/// `store` with one boot entry, `Boot0000`, first in `BootOrder`.
pub fn with_boot_entry(store: &[u8], load_option: &[u8]) -> Result<Vec<u8>, String> {
    with_variables(
        store,
        &[
            ("Boot0000", load_option),
            ("BootOrder", &0u16.to_le_bytes()),
        ],
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An empty store: the volume header, the store header, erased space.
    fn empty() -> Vec<u8> {
        let mut s = vec![0xFF; 4096];
        s[..72].fill(0);
        s[40..44].copy_from_slice(FV_SIGNATURE);
        s[48..50].copy_from_slice(&72u16.to_le_bytes());
        s[72..88].copy_from_slice(&AUTH_STORE.to_bytes_le());
        s[88..92].copy_from_slice(&2048u32.to_le_bytes());
        s[92] = STORE_FORMATTED;
        s[93] = STORE_HEALTHY;
        s[94..100].fill(0);
        s
    }

    #[test]
    fn a_boot_entry_lands_after_the_header() {
        let out = with_boot_entry(&empty(), b"option").unwrap();
        let first = 72 + STORE_HEADER;
        assert_eq!(u16_at(&out, first), Some(START_ID));
        assert_eq!(&out[first + 44..first + 60], &GLOBAL.to_bytes_le());
        // Both variables are there, one after the other.
        let after = free(&out, first, 72 + 2048).unwrap();
        assert!(after > first + 2 * VAR_HEADER);
        // Appending again goes after them.
        let again = with_boot_entry(&out, b"option").unwrap();
        assert!(free(&again, first, 72 + 2048).unwrap() > after);
    }

    #[test]
    fn what_is_not_a_store_is_refused() {
        assert!(with_boot_entry(&[0; 128], b"x").is_err());
        let mut s = empty();
        s[92] = 0;
        assert!(with_boot_entry(&s, b"x").is_err());
    }
}
