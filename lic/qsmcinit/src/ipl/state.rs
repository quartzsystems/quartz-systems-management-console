// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! The state step of the IPL: the state files with their defaults, the
//! hardware inventory written to `inventory.toml`, the serial number
//! (the machine's, or one derived once and kept), the host name.

use appliance::time::Zone;
use state::{Host, Identity, Inventory, Network, Store};

/// What the IPL learned from the state files.
pub struct Loaded {
    /// The host name in use.
    pub host_name: String,
    /// The display zone.
    pub zone: Zone,
    /// The serial number, empty when there is none.
    pub serial: String,
    /// The status screen's line of the machine.
    pub machine: String,
    /// The state files that were refused, and why: their defaults are
    /// in use.
    pub refused: Vec<String>,
}

/// The status screen's line of the machine `inventory` describes.
pub fn machine(inventory: &Inventory) -> String {
    let count = |n: usize, one: &str| {
        if n == 1 {
            format!("1 {one}")
        } else {
            format!("{n} {one}s")
        }
    };
    let serial = match inventory.system.serial_source.as_str() {
        "derived" => format!("serial {} (derived)", inventory.system.serial),
        _ if inventory.system.serial.is_empty() => "no serial".to_string(),
        _ => format!("serial {}", inventory.system.serial),
    };
    format!(
        "{serial}, {}, {} MiB, {}, {}",
        count(inventory.processors.threads as usize, "thread"),
        inventory.memory.total_bytes >> 20,
        count(inventory.disks.len(), "disk"),
        count(inventory.interfaces.len(), "interface")
    )
}

fn save<D: state::Document>(store: &Store, doc: &D, refused: &mut Vec<String>) {
    if let Err(e) = store.save(doc) {
        refused.push(e.to_string());
    }
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

/// Read the state files, write the inventory, keep a derived serial
/// number, set the host name.
#[cfg(target_os = "linux")]
pub fn load(store: &Store, host_prefix: &str) -> Loaded {
    let mut refused = Vec::new();
    let host: Host = load_or_default(store, &mut refused);
    let mut identity: Identity = load_or_default(store, &mut refused);
    // The network service reads its own file; a bad one is said here.
    let _: Network = load_or_default(store, &mut refused);

    let inventory = inventory::read(&inventory::Roots::default(), identity.serial.as_deref());
    // A serial derived from the first link is derived once and kept, in
    // a file of its own: `host.toml` is the operator's.
    if identity.serial.is_none() && inventory.system.serial_source == "derived" {
        identity.serial = Some(inventory.system.serial.clone());
        save(store, &identity, &mut refused);
    }
    save(store, &inventory, &mut refused);

    let first = appliance::links::ethernet().first().map(|l| l.mac);
    let host_name = host.name_or_default(host_prefix, first);
    let _ = rustix::system::sethostname(host_name.as_bytes());
    Loaded {
        host_name,
        zone: host.zone(),
        machine: machine(&inventory),
        serial: inventory.system.serial,
        refused,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_machine_line_counts_what_the_inventory_has() {
        let mut inv = Inventory::default();
        inv.system.serial = inventory::derived_serial([0x52, 0x54, 0x00, 0x12, 0x34, 0x56]);
        inv.system.serial_source = "derived".into();
        inv.processors.threads = 2;
        inv.memory.total_bytes = 1 << 30;
        inv.disks.push(state::doc::inventory::Disk::default());
        assert_eq!(
            machine(&inv),
            "serial M525400123456 (derived), 2 threads, 1024 MiB, 1 disk, 0 interfaces"
        );
        inv.system.serial = "S-1".into();
        inv.system.serial_source = "dmi".into();
        assert!(machine(&inv).starts_with("serial S-1, "));
    }

    #[test]
    fn the_default_host_name_is_the_prefix_and_the_mac() {
        // The default host name is the base crate's: the prefix and the
        // MAC's last six digits.
        let name = Host::default().name_or_default("QSMC", Some([0x52, 0x54, 0, 0x12, 0x34, 0x56]));
        assert_eq!(name, "QSMC123456");
    }
}
