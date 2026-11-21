// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! The partition family: create, change, delete, settings,
//! entitlement; the host family's settings and devices; the partitions
//! family's adapters.

use qsi::any::{AnyEvent, AnyReply, AnyRequest};
use qsi::host::host;
use qsi::host::model::Sharing;
use qsi::host::partition::{
    self, Change, Create, CreateWith, Delete, Entitlement, GetSettings, SetSettings, Settings,
    WithHost,
};
use qsi::host::partitions::{self, Adapters};

use super::{ask, events, host, ids};
use crate::system::System;

fn part(r: partition::Request) -> AnyRequest {
    AnyRequest::Partition(r)
}

fn settings(text: &str) -> Settings {
    Settings {
        description: text.into(),
        panel: false,
        with_host: WithHost::Immediate,
        delay: 60,
    }
}

fn entitle(s: &mut System, id: u32, units: u32) -> Result<AnyReply, Vec<(String, String)>> {
    ask(
        s,
        part(partition::Request::Entitlement(Entitlement {
            id,
            units: Some(units),
            sharing: None,
            weight: None,
        })),
    )
}

#[test]
fn a_partition_is_created_at_the_lowest_free_id_or_the_one_asked() {
    let mut s = host();
    let at = s.log.next();
    let r = ask(
        &mut s,
        part(partition::Request::Create(Create {
            name: "QSTEST02".into(),
        })),
    )
    .unwrap();
    let AnyReply::Partition(partition::Reply::Create(d)) = r else {
        panic!("{r:?}");
    };
    assert_eq!((d.id, d.serial.as_str()), (4, "K2M4104"));
    assert!(events(&s, at).iter().any(|e| matches!(e, AnyEvent::Partitions(partitions::Event::Defined(d)) if d.exists && d.id == 4)));
    assert_eq!(s.host.as_ref().unwrap().partitions, 4);
    let with = |id, name: &str| {
        part(partition::Request::CreateWith(CreateWith {
            id,
            name: name.into(),
            settings: settings("made at an ID"),
        }))
    };
    let r = ask(&mut s, with(Some(7), "QSTEST07")).unwrap();
    assert!(matches!(r, AnyReply::Partition(partition::Reply::CreateWith(d)) if d.id == 7));
    assert_eq!(s.part_ref(7).unwrap().settings.description, "made at an ID");
    assert_eq!(
        ids(ask(&mut s, with(Some(1), "QSX"))),
        ["QSI0362", "QVS2016"]
    );
    assert_eq!(
        ids(ask(&mut s, with(Some(65), "QSX"))),
        ["QSI0363", "QVS2017"]
    );
    assert_eq!(ids(ask(&mut s, with(None, "qsx"))), ["QVS1001"]);
    assert_eq!(ids(ask(&mut s, with(None, "QSDEV01"))), ["QVS1005"]);
}

#[test]
fn a_definition_changes_only_while_not_activated() {
    let mut s = host();
    let mut d = s.part_ref(2).unwrap().definition.clone();
    d.default_profile = "default".into();
    ask(
        &mut s,
        part(partition::Request::Change(Change {
            definition: d.clone(),
        })),
    )
    .unwrap();
    assert_eq!(s.part_ref(2).unwrap().definition.default_profile, "default");
    d.default_profile = "nope".into();
    assert_eq!(
        ids(ask(
            &mut s,
            part(partition::Request::Change(Change {
                definition: d.clone()
            }))
        )),
        ["QVS1007"]
    );
    d.default_profile = "default".into();
    d.serial = "X".into();
    assert_eq!(
        ids(ask(
            &mut s,
            part(partition::Request::Change(Change { definition: d }))
        )),
        ["QVS1006"]
    );
    let d = s.part_ref(1).unwrap().definition.clone();
    assert_eq!(
        ids(ask(
            &mut s,
            part(partition::Request::Change(Change { definition: d }))
        )),
        ["QVS2002"]
    );
}

#[test]
fn a_partition_is_deleted_with_its_disks_or_without() {
    let mut s = host();
    assert_eq!(
        ids(ask(
            &mut s,
            part(partition::Request::Delete(Delete { id: 1, disks: true }))
        )),
        ["QVS2002"]
    );
    // QSTEST01's disk VD04: named by its profiles only, so it goes too.
    s.release(2);
    let at = s.log.next();
    ask(
        &mut s,
        part(partition::Request::Delete(Delete { id: 2, disks: true })),
    )
    .unwrap();
    assert!(s.part_ref(2).is_none());
    assert!(!s.vres.vdisks.iter().any(|v| v.name == "VD04"));
    let ev = events(&s, at);
    assert!(ev.iter().any(|e| matches!(e, AnyEvent::Partitions(partitions::Event::Defined(d)) if !d.exists && d.id == 2)));
    assert!(ev.iter().any(|e| matches!(e, AnyEvent::Profiles(qsi::host::profiles::Event::Changed(c)) if c.revision.is_none())));
}

#[test]
fn settings_are_read_and_changed_running_or_not() {
    let mut s = host();
    let r = ask(
        &mut s,
        part(partition::Request::GetSettings(GetSettings { id: 1 })),
    )
    .unwrap();
    assert!(
        matches!(r, AnyReply::Partition(partition::Reply::GetSettings(st)) if st.description == "Development partition" && st.panel)
    );
    ask(
        &mut s,
        part(partition::Request::SetSettings(SetSettings {
            id: 1,
            settings: settings("x"),
        })),
    )
    .unwrap();
    assert!(!s.part_ref(1).unwrap().settings.panel);
    // Panel functions from a console are refused now.
    let f = AnyRequest::Panel(qsi::host::panel::Request::Function(
        qsi::host::panel::Function {
            id: 1,
            function: 11,
            args: qsi::host::panel::PanelArgs::default(),
        },
    ));
    assert_eq!(ids(ask(&mut s, f)), ["QVS2318"]);
}

#[test]
fn an_entitlement_changes_within_its_profile_and_pool() {
    let mut s = host();
    let at = s.log.next();
    let r = entitle(&mut s, 1, 150).unwrap();
    assert!(
        matches!(r, AnyReply::Partition(partition::Reply::Entitlement(c)) if c.units == 150 && !c.capped)
    );
    let h = s.host.as_ref().unwrap();
    assert_eq!((h.processors.available, h.pools[0].entitled), (300, 300));
    let ev = events(&s, at);
    assert!(ev.iter().any(|e| matches!(e, AnyEvent::Partition(partition::Event::Entitlement(c)) if c.before.units == 100 && c.current.units == 150 && c.by == "qsadmin")));
    assert!(
        ev.iter()
            .any(|e| matches!(e, AnyEvent::Partitions(partitions::Event::Resources(_))))
    );
    // Capped: its weight is 0.
    let r = ask(
        &mut s,
        part(partition::Request::Entitlement(Entitlement {
            id: 1,
            units: None,
            sharing: Some(Sharing::Capped),
            weight: None,
        })),
    )
    .unwrap();
    assert!(
        matches!(r, AnyReply::Partition(partition::Reply::Entitlement(c)) if c.capped && c.weight == 0)
    );
    assert_eq!(ids(entitle(&mut s, 3, 100)), ["QSI1402", "QVS2502"]);
    assert_eq!(ids(entitle(&mut s, 2, 100)), ["QSI1401", "QVS2501"]);
    assert_eq!(ids(entitle(&mut s, 1, 250)), ["QSI1403", "QVS2503"]);
    if let Some(h) = &mut s.host {
        h.pools[0].available = 0;
    }
    assert_eq!(ids(entitle(&mut s, 1, 200)), ["QSI1405", "QVS2505"]);
}

#[test]
fn an_entitlement_suits_the_virtual_processors() {
    let mut s = host();
    // A profile wider than TESTPOOL's two cores: two virtual
    // processors given, its maximum units beyond their worth.
    let mut wide = s
        .part_ref(1)
        .unwrap()
        .kept("default")
        .unwrap()
        .profile
        .clone();
    wide.name = "wide".into();
    let p = &mut wide.processors;
    (p.minimum, p.desired, p.maximum) = (1, 4, 4);
    (p.minimum_units, p.desired_units, p.maximum_units) = (Some(50), Some(100), Some(400));
    p.pool = Some("TESTPOOL".into());
    crate::system::profiles::put(&mut s, 1, wide, None).unwrap();
    s.release(1);
    s.set_state(1, qsi::host::model::PartitionState::NotActivated);
    let a = partition::Activate {
        id: 1,
        profile: "wide".into(),
        source: None,
        keylock: None,
    };
    let c = s.activate(&a).unwrap();
    assert_eq!((c.virtuals, c.units, c.pool.as_str()), (2, 100, "TESTPOOL"));
    assert_eq!(ids(entitle(&mut s, 1, 300)), ["QSI1404", "QVS2504"]);
}

#[test]
fn the_host_s_settings_devices_and_adapters() {
    let mut s = host();
    let r = ask(
        &mut s,
        AnyRequest::Host(host::Request::Settings(qsi::Empty)),
    )
    .unwrap();
    assert!(
        matches!(r, AnyReply::Host(host::Reply::Settings(st)) if st.mac_prefix.as_deref() == Some("02:51:53"))
    );
    let r = ask(&mut s, AnyRequest::Host(host::Request::Devices(qsi::Empty))).unwrap();
    let AnyReply::Host(host::Reply::Devices(v)) = r else {
        panic!("{r:?}");
    };
    assert_eq!(v.len(), 5);
    let c3 = v.iter().find(|d| d.location.ends_with("-C3")).unwrap();
    assert!(
        c3.released && !c3.assignable && c3.holder.as_ref().is_some_and(|h| h.name == "QSPROD01")
    );
    let adapters = |s: &mut System, id| {
        let r = ask(
            s,
            AnyRequest::Partitions(partitions::Request::Adapters(Adapters { id })),
        )
        .unwrap();
        let AnyReply::Partitions(partitions::Reply::Adapters(v)) = r else {
            panic!("{r:?}");
        };
        v
    };
    assert_eq!(adapters(&mut s, 0).len(), 10);
    let dev = adapters(&mut s, 1);
    assert_eq!(dev.len(), 4);
    assert!(
        dev.iter()
            .any(|a| a.profile == "default" && a.slot == 4 && a.switch == "ETHSW1" && a.vlan == 99)
    );
}
