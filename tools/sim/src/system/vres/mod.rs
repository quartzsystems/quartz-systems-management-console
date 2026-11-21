// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! A host's virtual resources and the vres family as QS/VS answers it:
//! storage pools and their disks, virtual disks, the media library and
//! partitions' drives, switches and their ports, processor pools.

pub mod media;
pub mod procpool;
pub mod storage;
pub mod switches;

use qsi::Message as _;
use qsi::any::AnyEvent;
use qsi::host::vres::{
    Image, Limits, PoolDisk, Reply, Request, StoragePool, Switch, TapeDrive, Vdisk, Volume, op,
};

use super::System;
use super::refuse::{Refusal, led_by, pools_lead, switches_lead};

/// What a host keeps of its virtual resources.
#[derive(Clone, Debug, Default)]
pub struct Resources {
    /// Its storage pools.
    pub pools: Vec<StoragePool>,
    /// Its whole disks, as a pool sees them.
    pub disks: Vec<PoolDisk>,
    /// Its virtual disks.
    pub vdisks: Vec<Vdisk>,
    /// The media library's optical images.
    pub images: Vec<Image>,
    /// The media library's tape volumes.
    pub volumes: Vec<Volume>,
    /// Its physical tape drives.
    pub tape_drives: Vec<TapeDrive>,
    /// Its switches.
    pub switches: Vec<Switch>,
    /// The switches' limits.
    pub limits: Limits,
}

impl Resources {
    /// The pool that holds the media library.
    pub fn repository(&self) -> Option<&StoragePool> {
        self.pools.iter().find(|p| p.repository)
    }
}

/// What a vres request comes to. A refusal of managing pools or
/// switches is led by QSI's identifier, as QS/VS leads it.
pub fn answer(s: &mut System, r: &Request) -> Result<Reply, Refusal> {
    let op = r.op();
    let answered = if op::is_pools(op) || matches!(r, Request::Pools(_)) {
        storage::pools(s, r)
    } else if (op::VDISKS..=op::VDISK_NAMED).contains(&op) {
        storage::vdisks(s, r)
    } else if (op::IMAGES..=op::TAPE_DRIVES).contains(&op) {
        media::answer(s, r)
    } else if (op::SWITCHES..=op::UPLINK_CLEAR).contains(&op) {
        switches::answer(s, r)
    } else {
        procpool::answer(s, r)
    };
    answered.map_err(|n| {
        if op::is_pools(op) {
            led_by(n, pools_lead)
        } else if op::is_switches(op) {
            led_by(n, switches_lead)
        } else {
            n
        }
    })
}

/// Say a storage pool changed.
pub fn pool_changed(s: &mut System, name: &str) {
    let pool = s.vres.pools.iter().find(|p| p.name == name).cloned();
    s.push(&AnyEvent::Vres(qsi::host::vres::Event::Pool(
        qsi::host::vres::PoolChanged {
            name: name.into(),
            pool,
            time: super::now_s(),
        },
    )));
}
