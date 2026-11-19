// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! The state step of the IPL: the state files with their defaults, the
//! serial number the console derives for itself once, the host name.

use std::fmt::Write as _;

use appliance::time::Zone;
use state::{Host, Identity, Network, Store};

/// What the IPL learned from the state files.
pub struct Loaded {
    /// The host name in use.
    pub host_name: String,
    /// The display zone.
    pub zone: Zone,
    /// The serial number, empty when there is none.
    pub serial: String,
    /// The state files that were refused, and why: their defaults are
    /// in use.
    pub refused: Vec<String>,
}

/// The serial number derived from a MAC address: `M` and its twelve
/// hexadecimal digits, as a QS/VS host derives its own.
pub fn derived_serial(mac: [u8; 6]) -> String {
    mac.iter().fold(String::from("M"), |mut out, byte| {
        let _ = write!(out, "{byte:02X}");
        out
    })
}

fn load_or_default<D: state::Document + Default>(store: &Store, refused: &mut Vec<String>) -> D {
    match store.load::<D>() {
        Ok(found) => found.unwrap_or_default(),
        Err(e) => {
            refused.push(e.to_string());
            D::default()
        }
    }
}

/// Read the state files, keep a derived serial number, set the host
/// name.
#[cfg(target_os = "linux")]
pub fn load(store: &Store, host_prefix: &str) -> Loaded {
    let mut refused = Vec::new();
    let host: Host = load_or_default(store, &mut refused);
    let mut identity: Identity = load_or_default(store, &mut refused);
    // The network service reads its own file; a bad one is said here.
    let _: Network = load_or_default(store, &mut refused);

    let first = appliance::links::ethernet().first().map(|l| l.mac);
    // Derived once and kept: `host.toml` is the operator's.
    if identity.serial.is_none()
        && let Some(mac) = first
    {
        identity.serial = Some(derived_serial(mac));
        if let Err(e) = store.save(&identity) {
            refused.push(e.to_string());
        }
    }
    let host_name = host.name_or_default(host_prefix, first);
    let _ = rustix::system::sethostname(host_name.as_bytes());
    Loaded {
        host_name,
        zone: host.zone(),
        serial: identity.serial.unwrap_or_default(),
        refused,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_derived_serial_is_m_and_the_mac() {
        assert_eq!(
            derived_serial([0x52, 0x54, 0x00, 0x12, 0x34, 0x56]),
            "M525400123456"
        );
        // The default host name is the base crate's: the prefix and the
        // MAC's last six digits.
        let name = Host::default().name_or_default("QSMC", Some([0x52, 0x54, 0, 0x12, 0x34, 0x56]));
        assert_eq!(name, "QSMC123456");
    }
}
