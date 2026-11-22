// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! System plans (`docs/PLANS.md`): a saved configuration of a QS/VS
//! host and its partitions, built only from what QSI's host family reads
//! and writes. It holds configuration and nothing else: no disk's
//! contents, no image, no dump, no password or key.
//!
//! - [`doc`]: the document, its version, its strict reader and writer.
//! - [`view`]: a host as it is read now through `mgmtd`.
//! - [`capture`]: a plan of a host.
//! - [`compare`]: what differs between two plans.
//! - [`validate`]: every item of a plan against a host, OK, Warning or
//!   Error, by the rules table.
//! - [`deploy`]: the steps a validated plan and a choice of partitions
//!   give, and the request of each.
//! - [`index`], [`job`]: what the scheduler keeps of plans and of a
//!   deployment, as its socket carries them.

#![forbid(unsafe_code)]

pub mod capture;
pub mod compare;
pub mod deploy;
pub mod doc;
pub mod index;
pub mod job;
pub mod validate;
pub mod view;

pub use doc::Plan;
pub use validate::{Item, Status};
pub use view::HostView;

/// The fuzzer's target `system-plan`: an imported file's bytes. What
/// reads writes again and reads back the same; what reads is validated
/// against a bare host and its steps made, without a panic.
pub fn fuzz(data: &[u8]) {
    use qsi::host::host::{Amount, Host, ProcPool};
    let Ok(p) = doc::text::read(data) else {
        return;
    };
    let t = doc::text::write(&p).expect("a plan read writes");
    assert_eq!(doc::text::read(t.as_bytes()).as_ref(), Ok(&p));
    let amount = |n| Amount {
        installed: n,
        configurable: n,
        available: n,
    };
    let v = HostView {
        name: "QSFUZZ".into(),
        is_host: true,
        connected: true,
        serial: p.host.serial.clone(),
        host: Some(Host {
            sockets: 1,
            cores: 8,
            threads_per_core: 1,
            processors: amount(800),
            memory: amount(131_072),
            memory_block_mib: 256,
            reserved_cores: Vec::new(),
            reserved_memory_mib: 0,
            pools: vec![ProcPool {
                name: "DefaultPool".into(),
                cores: (0..8).collect(),
                maximum: 800,
                entitled: 0,
                available: 800,
                partitions: 0,
            }],
            partitions: 0,
            max_partitions: 64,
        }),
        ..HostView::default()
    };
    let ids: Vec<u32> = p.partitions.iter().map(|x| x.id).collect();
    let val = validate::validate(&p, &v, &[], Some(&ids));
    for s in deploy::steps(&p, &val, &ids, true) {
        if let Some(a) = &s.action {
            let _ = deploy::request(a);
            let _ = deploy::uplink(a);
        }
    }
}
