// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! The profiles family: revisions, the default, a running partition's
//! configuration saved, and a profile's validation.

use qsi::any::{AnyEvent, AnyReply, AnyRequest};
use qsi::host::model::Profile;
use qsi::host::profiles::{
    self, Activated, Delete, Get, List, Put, Request, SaveCurrent, SetDefault,
};

use super::{ask, events, host, ids};
use crate::system::System;

fn get(s: &mut System, id: u32, name: &str) -> (Profile, u64) {
    let r = ask(
        s,
        AnyRequest::Profiles(Request::Get(Get {
            id,
            name: name.into(),
        })),
    )
    .unwrap();
    let AnyReply::Profiles(profiles::Reply::Get(rev)) = r else {
        panic!("{r:?}");
    };
    (rev.profile, rev.revision)
}

fn put(
    s: &mut System,
    id: u32,
    profile: Profile,
    revision: Option<u64>,
) -> Result<AnyReply, Vec<(String, String)>> {
    ask(
        s,
        AnyRequest::Profiles(Request::Put(Put {
            id,
            profile,
            revision,
        })),
    )
}

fn saved(r: &AnyReply) -> u64 {
    match r {
        AnyReply::Profiles(profiles::Reply::Put(s) | profiles::Reply::SaveCurrent(s)) => s.revision,
        r => panic!("{r:?}"),
    }
}

#[test]
fn the_profiles_are_listed_by_name_with_the_default() {
    let mut s = host();
    let r = ask(&mut s, AnyRequest::Profiles(Request::List(List { id: 1 }))).unwrap();
    let AnyReply::Profiles(profiles::Reply::List(v)) = r else {
        panic!("{r:?}");
    };
    let names: Vec<_> = v
        .iter()
        .map(|e| (e.name.as_str(), e.revision, e.default))
        .collect();
    assert_eq!(
        names,
        [
            ("big-test", 5, false),
            ("default", 7, true),
            ("install", 3, false)
        ]
    );
    let r = ask(
        &mut s,
        AnyRequest::Profiles(Request::Activated(Activated { id: 1 })),
    )
    .unwrap();
    assert!(
        matches!(r, AnyReply::Profiles(profiles::Reply::Activated(v)) if v[1].last == Some(1_793_340_000))
    );
    assert_eq!(
        ids(ask(
            &mut s,
            AnyRequest::Profiles(Request::Get(Get {
                id: 1,
                name: "nope".into()
            }))
        )),
        ["QVS2004"]
    );
}

#[test]
fn a_put_is_checked_against_its_revision() {
    let mut s = host();
    let (mut p, rev) = get(&mut s, 1, "big-test");
    p.memory.desired_mib = 24_576;
    // A name there and no revision; a stale revision.
    assert_eq!(ids(put(&mut s, 1, p.clone(), None)), ["QSI0402", "QVS2012"]);
    assert_eq!(
        ids(put(&mut s, 1, p.clone(), Some(rev - 1))),
        ["QSI0401", "QVS2013"]
    );
    let at = s.log.next();
    let now = saved(&put(&mut s, 1, p.clone(), Some(rev)).unwrap());
    assert!(now > rev);
    assert_eq!(get(&mut s, 1, "big-test"), (p.clone(), now));
    assert!(events(&s, at).iter().any(|e| matches!(e, AnyEvent::Profiles(profiles::Event::Changed(c)) if c.name == "big-test" && c.revision == Some(now) && !c.default)));
    // The revision read before is stale now.
    assert_eq!(ids(put(&mut s, 1, p, Some(rev))), ["QSI0401", "QVS2013"]);
}

#[test]
fn a_new_profile_gets_its_macs_and_is_checked_whole() {
    let mut s = host();
    let (mut p, _) = get(&mut s, 1, "default");
    p.name = "extra".into();
    p.ethernet[1].mac = None;
    p.ethernet[1].slot = 7;
    let r = put(&mut s, 1, p.clone(), None).unwrap();
    assert!(saved(&r) > 0);
    let (made, _) = get(&mut s, 1, "extra");
    assert_eq!(made.ethernet[1].mac.as_deref(), Some("02:51:53:00:01:07"));
    // Every problem at once: a VLAN the switch does not carry, memory
    // out of order.
    p.name = "bad".into();
    p.ethernet[0].vlan = 77;
    p.memory.minimum_mib = 65_536;
    let got = ids(put(&mut s, 1, p.clone(), None));
    assert!(
        got.contains(&"QVS1224".into()) && got.contains(&"QVS1111".into()),
        "{got:?}"
    );
    p.name = "much-too-long".into();
    assert_eq!(ids(put(&mut s, 1, p, None))[0], "QVS1101");
}

#[test]
fn the_default_moves_and_is_not_deleted() {
    let mut s = host();
    let del = |name: &str| {
        AnyRequest::Profiles(Request::Delete(Delete {
            id: 1,
            name: name.into(),
        }))
    };
    assert_eq!(ids(ask(&mut s, del("default"))), ["QVS2007"]);
    let at = s.log.next();
    ask(
        &mut s,
        AnyRequest::Profiles(Request::SetDefault(SetDefault {
            id: 1,
            name: "install".into(),
        })),
    )
    .unwrap();
    assert_eq!(s.part_ref(1).unwrap().definition.default_profile, "install");
    let changed: Vec<_> = events(&s, at)
        .into_iter()
        .filter_map(|e| match e {
            AnyEvent::Profiles(profiles::Event::Changed(c)) => Some((c.name, c.default)),
            _ => None,
        })
        .collect();
    assert_eq!(
        changed,
        [
            ("default".to_string(), false),
            ("install".to_string(), true)
        ]
    );
    ask(&mut s, del("default")).unwrap();
    assert!(s.part_ref(1).unwrap().kept("default").is_none());
    assert_eq!(ids(ask(&mut s, del("default"))), ["QVS2004"]);
}

#[test]
fn a_running_partition_s_configuration_is_saved() {
    let mut s = host();
    let save = |name: &str, id, revision| {
        AnyRequest::Profiles(Request::SaveCurrent(SaveCurrent {
            id,
            name: name.into(),
            revision,
        }))
    };
    let r = ask(&mut s, save("now", 1, None)).unwrap();
    assert!(saved(&r) > 0);
    let (p, rev) = get(&mut s, 1, "now");
    assert_eq!(
        (
            p.processors.desired_units,
            p.processors.desired,
            p.memory.desired_mib
        ),
        (Some(100), 2, 16_384)
    );
    assert_eq!(
        ids(ask(&mut s, save("now", 1, None))),
        ["QSI0402", "QVS2012"]
    );
    assert_eq!(
        ids(ask(&mut s, save("now", 1, Some(rev + 9)))),
        ["QSI0401", "QVS2013"]
    );
    assert!(ask(&mut s, save("now", 1, Some(rev))).is_ok());
    assert_eq!(
        ids(ask(&mut s, save("now", 2, None))),
        ["QSI0403", "QVS2015"]
    );
}
