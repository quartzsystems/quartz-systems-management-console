// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! The vres family: storage pools managed, virtual disks, the media
//! library and drives, switches, processor pools.

use qsi::any::{AnyEvent, AnyReply, AnyRequest};
use qsi::host::vres::{
    self, Bond, Designate, ImageRename, ImageUpload, Load, PoolCreate, PoolDelete, PoolExtend,
    PoolRename, ProcPoolDelete, ProcPoolSpec, Protection, Request, SpareAdd, SpareRemove,
    SwitchDelete, SwitchRename, SwitchSpec, UnitReplace, Unload, Uplink, UplinkSet, VdiskCreate,
    VdiskDelete, VdiskNamed, VdiskResize, VolumeCreate, VolumeDelete,
};

use super::{ask, events, host, ids};
use crate::system::System;
use crate::system::answer::{Answer, answer};

fn v(r: Request) -> AnyRequest {
    AnyRequest::Vres(r)
}

fn vres(s: &mut System, r: Request) -> Result<AnyReply, Vec<(String, String)>> {
    ask(s, v(r))
}

fn create(name: &str, protection: Protection, disks: &[&str], confirm: &[&str]) -> Request {
    Request::PoolCreate(PoolCreate {
        name: name.into(),
        protection,
        disks: disks.iter().map(|d| (*d).into()).collect(),
        spares: Vec::new(),
        confirm: confirm.iter().map(|d| (*d).into()).collect(),
    })
}

#[test]
fn the_pools_and_their_disks_are_the_pages() {
    let mut s = host();
    let r = vres(&mut s, Request::Pools(qsi::Empty)).unwrap();
    let AnyReply::Vres(vres::Reply::Pools(p)) = r else {
        panic!("{r:?}");
    };
    assert_eq!(
        (p[0].name.as_str(), p[0].units.len(), p[0].bytes >> 30),
        ("POOL1", 6, 5400)
    );
    assert_eq!(
        (
            (p[0].bytes - p[0].allocated) >> 30,
            (p[1].bytes - p[1].allocated) >> 30
        ),
        (2530, 6250)
    );
    assert!(p[1].repository && !p[0].repository);
    let r = vres(&mut s, Request::Disks(qsi::Empty)).unwrap();
    let AnyReply::Vres(vres::Reply::Disks(d)) = r else {
        panic!("{r:?}");
    };
    assert_eq!(d.iter().filter(|d| d.free).count(), 1);
    assert!(d.iter().any(|d| d.name == "sdj" && d.foreign && !d.free));
}

#[test]
fn a_pool_is_made_of_free_disks_and_a_foreign_one_confirmed() {
    let mut s = host();
    assert_eq!(
        ids(vres(
            &mut s,
            create("POOL3", Protection::Raid5, &["sdi"], &[])
        )),
        ["QSI1604", "QVS3004"]
    );
    assert_eq!(
        ids(vres(
            &mut s,
            create("POOL3", Protection::Mirrored, &["sdi", "sdj"], &[])
        )),
        ["QSI1607", "QVS3007"]
    );
    assert_eq!(
        ids(vres(
            &mut s,
            create("POOL3", Protection::Mirrored, &["sdi", "sdc"], &[])
        )),
        ["QSI1606", "QVS3006"]
    );
    assert_eq!(
        ids(vres(
            &mut s,
            create("POOL1", Protection::None, &["sdi"], &[])
        )),
        ["QSI1602", "QVS3002"]
    );
    let at = s.log.next();
    let r = vres(
        &mut s,
        create("POOL3", Protection::Mirrored, &["sdi", "sdj"], &["sdj"]),
    )
    .unwrap();
    assert!(
        matches!(r, AnyReply::Vres(vres::Reply::PoolCreate(p)) if p.protection == "mirrored" && p.units.len() == 2)
    );
    assert!(events(&s, at).iter().any(|e| matches!(e, AnyEvent::Vres(vres::Event::Pool(p)) if p.name == "POOL3" && p.pool.is_some())));
    assert_eq!(s.vres.disks.iter().filter(|d| d.pool == "POOL3").count(), 2);
    // Empty, it is renamed and deleted.
    vres(
        &mut s,
        Request::PoolRename(PoolRename {
            name: "POOL3".into(),
            to: "SCRATCH".into(),
        }),
    )
    .unwrap();
    vres(
        &mut s,
        Request::PoolDelete(PoolDelete {
            name: "SCRATCH".into(),
        }),
    )
    .unwrap();
    assert!(s.vres.disks.iter().any(|d| d.name == "sdj" && d.free));
}

#[test]
fn a_pool_is_fixed_in_size_and_kept_while_in_use() {
    let mut s = host();
    let extend = |pool: &str| {
        Request::PoolExtend(PoolExtend {
            pool: pool.into(),
            disks: vec!["sdi".into()],
            confirm: Vec::new(),
        })
    };
    assert_eq!(ids(vres(&mut s, extend("POOL1"))), ["QSI1616", "QVS3019"]);
    assert_eq!(ids(vres(&mut s, extend("NOPE"))), ["QSI1603", "QVS3003"]);
    assert_eq!(
        ids(vres(
            &mut s,
            Request::PoolDelete(PoolDelete {
                name: "POOL1".into()
            })
        )),
        ["QSI1612", "QVS3012"]
    );
    assert_eq!(
        ids(vres(
            &mut s,
            Request::PoolRename(PoolRename {
                name: "POOL1".into(),
                to: "FAST".into()
            })
        )),
        ["QSI1613", "QVS3013"]
    );
    assert_eq!(
        ids(vres(
            &mut s,
            Request::Designate(Designate {
                pool: "POOL1".into()
            })
        )),
        ["QSI1618", "QVS3205"]
    );
}

#[test]
fn spares_and_a_replaced_unit() {
    let mut s = host();
    let add = |pool: &str| {
        Request::SpareAdd(SpareAdd {
            pool: pool.into(),
            disk: "sdi".into(),
            confirm: false,
        })
    };
    assert_eq!(ids(vres(&mut s, add("POOL2"))), ["QSI1610", "QVS3017"]);
    vres(&mut s, add("POOL1")).unwrap();
    assert_eq!(s.vres.pools[0].units[6].state, "Hot spare");
    vres(
        &mut s,
        Request::SpareRemove(SpareRemove {
            pool: "POOL1".into(),
            disk: "sdi".into(),
        }),
    )
    .unwrap();
    assert_eq!(s.vres.pools[0].units.len(), 6);
    let replace = |unit| {
        Request::UnitReplace(UnitReplace {
            pool: "POOL1".into(),
            unit,
            disk: "sdi".into(),
            confirm: false,
        })
    };
    assert_eq!(ids(vres(&mut s, replace(2))), ["QSI1615", "QVS3015"]);
    let all = vec![crate::fixture::shared(std::mem::replace(&mut s, host()))];
    crate::control::command(&all, "fail QSHQVS1 POOL1 2");
    let mut s = crate::system::lock(&all[0]);
    assert_eq!(s.vres.pools[0].state, "Degraded");
    vres(&mut s, replace(2)).unwrap();
    assert_eq!(
        (
            s.vres.pools[0].state.as_str(),
            s.vres.pools[0].units[1].disk.as_str()
        ),
        ("Active", "sdi")
    );
    assert!(s.vres.disks.iter().any(|d| d.name == "sdd" && d.free));
}

#[test]
fn virtual_disks_are_made_grown_and_deleted() {
    let mut s = host();
    let make = |name: &str, mib, thick| {
        Request::VdiskCreate(VdiskCreate {
            pool: "POOL1".into(),
            name: name.into(),
            mib,
            thick,
        })
    };
    let before = s.vres.pools[0].clone();
    let r = vres(&mut s, make("VD07", 120 * 1024, true)).unwrap();
    assert!(
        matches!(r, AnyReply::Vres(vres::Reply::VdiskCreate(d)) if d.thick && d.allocated == 120 << 30)
    );
    assert_eq!(s.vres.pools[0].allocated - before.allocated, 120 << 30);
    assert_eq!(ids(vres(&mut s, make("VD07", 1024, false))), ["QVS3103"]);
    assert_eq!(ids(vres(&mut s, make("7VD", 1024, false))), ["QVS3101"]);
    assert_eq!(ids(vres(&mut s, make("VD08", 0, false))), ["QVS3102"]);
    assert_eq!(
        ids(vres(&mut s, make("VD08", 4000 * 1024, true))),
        ["QVS3107"]
    );
    let resize = |name: &str, mib| {
        Request::VdiskResize(VdiskResize {
            pool: "POOL1".into(),
            name: name.into(),
            mib,
        })
    };
    assert_eq!(ids(vres(&mut s, resize("VD03", 200 * 1024))), ["QVS3105"]);
    assert_eq!(ids(vres(&mut s, resize("VD06", 40 * 1024))), ["QVS3106"]);
    assert!(vres(&mut s, resize("VD06", 100 * 1024)).is_ok());
    let r = vres(
        &mut s,
        Request::VdiskNamed(VdiskNamed {
            pool: "POOL1".into(),
            name: "VD03".into(),
        }),
    )
    .unwrap();
    assert!(
        matches!(r, AnyReply::Vres(vres::Reply::VdiskNamed(n)) if n.len() == 3 && n.iter().all(|x| x.name == "QSDEV01"))
    );
    // A profile names VD06 now: deleted only by force.
    let (mut p, _) = {
        let k = s.part_ref(2).unwrap().kept("test-small").unwrap();
        (k.profile.clone(), k.revision)
    };
    p.name = "six".into();
    p.scsi[0].luns[0].backing = "POOL1/VD06".into();
    crate::system::profiles::put(&mut s, 2, p, None).unwrap();
    let del = |force| {
        Request::VdiskDelete(VdiskDelete {
            pool: "POOL1".into(),
            name: "VD06".into(),
            force,
        })
    };
    assert_eq!(ids(vres(&mut s, del(false))), ["QVS3108"]);
    vres(&mut s, del(true)).unwrap();
    assert!(!s.vres.vdisks.iter().any(|d| d.name == "VD06"));
    assert_eq!(
        ids(vres(
            &mut s,
            Request::VdiskDelete(VdiskDelete {
                pool: "POOL1".into(),
                name: "VD01".into(),
                force: true
            })
        )),
        ["QVS3105"]
    );
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "the library, then the drives, then the volumes"
)]
fn the_media_library_and_a_partition_s_drives() {
    let mut s = host();
    let rename = |name: &str, to: &str| {
        Request::ImageRename(ImageRename {
            name: name.into(),
            to: to.into(),
        })
    };
    assert_eq!(
        ids(vres(&mut s, rename("QS400_V1R0M0_INSTALL.iso", "X.iso"))),
        ["QVS3205"]
    );
    assert_eq!(
        ids(vres(
            &mut s,
            rename("QS400_CUM_26290.iso", "QS400_CUM_26300.iso")
        )),
        ["QVS3202"]
    );
    vres(&mut s, rename("QS400_CUM_26290.iso", "QS400_CUM_26300")).unwrap();
    vres(
        &mut s,
        Request::ImageDelete(vres::ImageDelete {
            name: "blank_4700MB.iso".into(),
        }),
    )
    .unwrap();
    assert_eq!(s.vres.images.len(), 4);
    // An upload's checks before its stream.
    let up = |name: &str, bytes| {
        v(Request::ImageUpload(ImageUpload {
            name: name.into(),
            bytes,
        }))
    };
    assert!(
        matches!(answer(&mut s, &up("NEW", 4096), "qsadmin"), Answer::Upload(n, 4096) if n == "NEW")
    );
    assert_eq!(ids(ask(&mut s, up("NEW", 4097))), ["QVS3207"]);
    assert_eq!(ids(ask(&mut s, up("QS400_CUM_26300", 4096))), ["QVS3203"]);
    // Drives: QSDEV01's, and a load into a full one.
    let r = vres(&mut s, Request::Drives(vres::Drives { id: 1 })).unwrap();
    assert!(
        matches!(&r, AnyReply::Vres(vres::Reply::Drives(d)) if d.len() == 2 && d[0].name == "HLINV_SAVF_20261028.iso")
    );
    assert_eq!(
        ids(vres(&mut s, Request::Drives(vres::Drives { id: 2 }))),
        ["QVS2003"]
    );
    let load = |id, slot, name: &str| {
        Request::Load(Load {
            id,
            slot,
            name: name.into(),
        })
    };
    let unload = |id, slot, forced| Request::Unload(Unload { id, slot, forced });
    assert_eq!(
        ids(vres(&mut s, load(1, 5, "QS400_CUM_26300"))),
        ["QVS2115"]
    );
    assert_eq!(
        ids(vres(&mut s, load(1, 9, "QS400_CUM_26300"))),
        ["QVS2113"]
    );
    assert_eq!(
        ids(vres(&mut s, load(3, 6, "VTAP01"))),
        ["QSI1307", "QVS2421"]
    );
    // The tape is prevented from removal: only forced.
    assert_eq!(ids(vres(&mut s, unload(1, 6, false))), ["QVS2114"]);
    let at = s.log.next();
    let r = vres(&mut s, unload(1, 6, true)).unwrap();
    assert!(matches!(r, AnyReply::Vres(vres::Reply::Unload(u)) if u.name == "VTAP01"));
    assert_eq!(s.vres.volumes[0].holder.partition, 0);
    let ev = events(&s, at);
    assert!(ev.iter().any(|e| matches!(e, AnyEvent::Vres(vres::Event::Drive(d)) if d.id == 1 && d.drive.name.is_empty())));
    assert!(
        ev.iter()
            .any(|e| matches!(e, AnyEvent::Vres(vres::Event::Volume(v)) if v.name == "VTAP01"))
    );
    vres(&mut s, unload(1, 5, false)).unwrap();
    vres(&mut s, load(1, 5, "QS400_CUM_26300")).unwrap();
    assert_eq!(
        s.vres
            .images
            .iter()
            .find(|i| i.name == "QS400_CUM_26300")
            .unwrap()
            .holder
            .partition,
        1
    );
    // Volumes.
    let make = |name: &str, id: &str| {
        Request::VolumeCreate(VolumeCreate {
            name: name.into(),
            identifier: id.into(),
            mib: 1024,
        })
    };
    assert_eq!(ids(vres(&mut s, make("VTAP03", "vtap03"))), ["QVS3208"]);
    vres(&mut s, make("VTAP03", "VTAP03")).unwrap();
    vres(
        &mut s,
        Request::VolumeProtect(vres::VolumeProtect {
            name: "VTAP03".into(),
            on: true,
        }),
    )
    .unwrap();
    assert!(s.vres.volumes[2].protected);
    assert_eq!(
        ids(vres(
            &mut s,
            Request::VolumeDelete(VolumeDelete {
                name: "VTAP02".into()
            })
        )),
        ["QVS3205"]
    );
    vres(
        &mut s,
        Request::VolumeDelete(VolumeDelete {
            name: "VTAP03".into(),
        }),
    )
    .unwrap();
    let r = vres(&mut s, Request::TapeDrives(qsi::Empty)).unwrap();
    assert!(
        matches!(r, AnyReply::Vres(vres::Reply::TapeDrives(t)) if t[0].holder.partition == 3 && t[0].loaded)
    );
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "reads, then each refusal, then the changes"
)]
fn switches_their_ports_and_uplinks() {
    let mut s = host();
    let r = vres(&mut s, Request::Switches(qsi::Empty)).unwrap();
    let AnyReply::Vres(vres::Reply::Switches(w)) = r else {
        panic!("{r:?}");
    };
    assert_eq!(
        (w[0].ports, w[1].ports, w[0].uplink_state.as_str()),
        (3, 2, "Up")
    );
    let r = vres(
        &mut s,
        Request::Ports(vres::Ports {
            switch: "ETHSW0".into(),
        }),
    )
    .unwrap();
    assert!(
        matches!(&r, AnyReply::Vres(vres::Reply::Ports(p)) if p.iter().filter(|p| p.link).count() == 2)
    );
    let r = vres(&mut s, Request::Vlans(qsi::Empty)).unwrap();
    assert!(
        matches!(&r, AnyReply::Vres(vres::Reply::Vlans(v)) if v.iter().any(|u| u.vlan == 20 && u.ports.len() == 2))
    );
    assert!(
        vres(
            &mut s,
            Request::Fdb(vres::Fdb {
                switch: "ETHSW0".into()
            })
        )
        .is_ok()
    );
    let spec = |name: &str, vlans: Vec<u16>| SwitchSpec {
        name: name.into(),
        vlans,
        mtu: 0,
    };
    assert_eq!(
        ids(vres(&mut s, Request::SwitchCreate(spec("sw2", vec![5])))),
        ["QSI1501", "QVS4001"]
    );
    assert_eq!(
        ids(vres(&mut s, Request::SwitchCreate(spec("ETHSW1", vec![5])))),
        ["QSI1502", "QVS4002"]
    );
    assert_eq!(
        ids(vres(&mut s, Request::SwitchCreate(spec("ETHSW2", vec![0])))),
        ["QSI1504", "QVS4004"]
    );
    let r = vres(&mut s, Request::SwitchCreate(spec("ETHSW2", vec![40]))).unwrap();
    assert!(
        matches!(r, AnyReply::Vres(vres::Reply::SwitchCreate(w)) if w.id == 2 && w.mtu == 1500)
    );
    let up = |at: &[&str], bond| {
        Request::UplinkSet(UplinkSet {
            switch: "ETHSW2".into(),
            uplink: Uplink {
                interfaces: at.iter().map(|x| crate::fixture::loc(x)).collect(),
                bond,
                native_vlan: None,
            },
        })
    };
    assert_eq!(
        ids(vres(&mut s, up(&["C4-T1"], None))),
        ["QSI1510", "QVS4102"]
    );
    assert_eq!(
        ids(vres(&mut s, up(&["C1-T1"], None))),
        ["QSI1511", "QVS4103"]
    );
    assert_eq!(
        ids(vres(&mut s, up(&["C1-T2", "C4-T2"], None))),
        ["QSI1512", "QVS4104"]
    );
    assert_eq!(
        ids(vres(&mut s, up(&["C9-T1"], None))),
        ["QSI1509", "QVS4101"]
    );
    vres(&mut s, up(&["C1-T2", "C4-T2"], Some(Bond::ActiveBackup))).unwrap();
    assert_eq!(s.vres.switches[2].uplink_state, "Degraded");
    vres(
        &mut s,
        Request::UplinkClear(vres::UplinkClear {
            switch: "ETHSW2".into(),
        }),
    )
    .unwrap();
    // A VLAN a profile uses, a switch with a running port, a switch a
    // profile names.
    assert_eq!(
        ids(vres(
            &mut s,
            Request::SwitchChange(spec("ETHSW0", vec![10, 30]))
        )),
        ["QSI1507", "QVS4009"]
    );
    assert_eq!(
        ids(vres(
            &mut s,
            Request::SwitchDelete(SwitchDelete {
                name: "ETHSW0".into(),
                force: true
            })
        )),
        ["QSI1508", "QVS4015"]
    );
    assert_eq!(
        ids(vres(
            &mut s,
            Request::SwitchRename(SwitchRename {
                name: "ETHSW1".into(),
                to: "INTERNAL".into()
            })
        )),
        ["QSI1507", "QVS4008"]
    );
    vres(
        &mut s,
        Request::SwitchDelete(SwitchDelete {
            name: "ETHSW2".into(),
            force: false,
        }),
    )
    .unwrap();
    assert_eq!(s.vres.switches.len(), 2);
}

#[test]
fn processor_pools_are_made_changed_and_deleted() {
    let mut s = host();
    let spec = |name: &str, cores: Vec<u32>| ProcPoolSpec {
        name: name.into(),
        cores,
        maximum_units: None,
    };
    let make = |p| Request::ProcPoolCreate(p);
    assert_eq!(
        ids(vres(&mut s, make(spec("TESTPOOL", vec![5])))),
        ["QVS2202"]
    );
    assert_eq!(ids(vres(&mut s, make(spec("BATCH", vec![6])))), ["QVS2204"]);
    assert_eq!(ids(vres(&mut s, make(spec("BATCH", vec![0])))), ["QVS2205"]);
    assert_eq!(ids(vres(&mut s, make(spec("BATCH", vec![])))), ["QVS2206"]);
    assert_eq!(
        ids(vres(&mut s, make(spec("DefaultPool", vec![5])))),
        ["QVS2209"]
    );
    let at = s.log.next();
    vres(&mut s, make(spec("BATCH", vec![5]))).unwrap();
    assert!(events(&s, at).iter().any(
        |e| matches!(e, AnyEvent::Host(qsi::host::host::Event::Resources(h)) if h.pools.len() == 3)
    ));
    vres(
        &mut s,
        Request::ProcPoolChange(ProcPoolSpec {
            name: "BATCH".into(),
            cores: vec![4, 5],
            maximum_units: Some(150),
        }),
    )
    .unwrap();
    assert_eq!(s.host.as_ref().unwrap().pools[2].maximum, 150);
    vres(
        &mut s,
        Request::ProcPoolDelete(ProcPoolDelete {
            name: "BATCH".into(),
        }),
    )
    .unwrap();
    assert_eq!(
        ids(vres(
            &mut s,
            Request::ProcPoolDelete(ProcPoolDelete {
                name: "BATCH".into()
            })
        )),
        ["QVS2105"]
    );
}
