// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! The host families against the fixture's `QSHQVS1`, a request at a
//! time: each family's answers, its refusals by QS/VS's identifiers
//! with QSI's first where QSI names them, and the events that follow.

mod activation;
mod partition;
mod profiles;
mod vres;

use qsi::any::{AnyEvent, AnyReply, AnyRequest};

use super::System;
use super::answer::{Answer, answer};

/// The fixture's host, unlocked for the test.
pub fn host() -> System {
    let shared = crate::fixture::qshqvs1();
    let Ok(m) = std::sync::Arc::try_unwrap(shared) else {
        panic!("the fixture has one owner");
    };
    m.into_inner()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// Ask `r` of `s` as `qsadmin`.
#[allow(
    clippy::needless_pass_by_value,
    reason = "each test builds its request in place"
)]
pub fn ask(s: &mut System, r: AnyRequest) -> Result<AnyReply, Vec<(String, String)>> {
    match answer(s, &r, "qsadmin") {
        Answer::Reply(rep) => Ok(rep),
        Answer::Refuse(n) => Err(n),
        a => panic!("not a reply: {a:?}"),
    }
}

/// A refusal's identifiers, in order.
pub fn ids(r: Result<AnyReply, Vec<(String, String)>>) -> Vec<String> {
    match r {
        Ok(rep) => panic!("not refused: {rep:?}"),
        Err(n) => n.into_iter().map(|(id, _)| id).collect(),
    }
}

/// Every event the system pushed from `from` on.
pub fn events(s: &System, from: u64) -> Vec<AnyEvent> {
    s.log
        .from(from)
        .filter_map(|e| e.decode().ok().flatten())
        .collect()
}

/// The codes posted of partition `id`, oldest first.
pub fn codes_of(s: &System, id: u32) -> Vec<String> {
    s.part_ref(id)
        .map(|p| p.history.iter().map(|c| c.refcode.code.clone()).collect())
        .unwrap_or_default()
}

#[test]
fn the_fixture_holds_what_its_partitions_hold() {
    let mut s = host();
    let at = s.log.next();
    let before = (s.vres.clone(), s.devices.clone(), s.host.clone());
    s.rehold();
    assert!(events(&s, at).is_empty(), "{:?}", events(&s, at));
    assert_eq!(before.0.vdisks, s.vres.vdisks);
    assert_eq!(before.0.images, s.vres.images);
    assert_eq!(before.1, s.devices);
    assert_eq!(before.2, s.host);
    // Every profile of the fixture is one QS/VS takes.
    for p in &s.parts {
        for k in &p.profiles {
            let v = super::profiles::check(&s, p.definition.id, &k.profile).unwrap();
            assert!(v.is_empty(), "{} {}: {v:?}", p.status.name, k.profile.name);
        }
    }
}

#[test]
fn a_family_is_refused_when_told() {
    let shared = crate::fixture::qshqvs1();
    let all = vec![shared.clone()];
    crate::control::command(&all, "refuse-family QSHQVS1 vres/0x0020 QVS3201 No media.");
    let mut s = super::lock(&shared);
    let images = AnyRequest::Vres(qsi::host::vres::Request::Images(qsi::Empty));
    assert_eq!(ids(ask(&mut s, images)), ["QVS3201"]);
    let pools = AnyRequest::Vres(qsi::host::vres::Request::Pools(qsi::Empty));
    assert!(ask(&mut s, pools).is_ok());
    drop(s);
    crate::control::command(&all, "allow QSHQVS1");
    let mut s = super::lock(&shared);
    let images = AnyRequest::Vres(qsi::host::vres::Request::Images(qsi::Empty));
    assert!(ask(&mut s, images).is_ok());
}

#[test]
fn what_is_kept_is_listed() {
    let all = vec![crate::fixture::qshqvs1()];
    let pools = crate::control::command(&all, "kept QSHQVS1 pools");
    assert!(pools.contains("POOL1 raid6 Active") && pools.contains("POOL2 none Active"));
    assert!(
        pools
            .lines()
            .nth(1)
            .is_some_and(|l| l.ends_with("repository"))
    );
    let profiles = crate::control::command(&all, "kept QSHQVS1 profiles 1");
    assert_eq!(
        profiles,
        "default revision 7 default\ninstall revision 3\nbig-test revision 5"
    );
    let v = crate::control::command(&all, "kept QSHQVS1 vdisks");
    assert!(v.contains("POOL1/VD04 thin") && v.contains("held 2/3"));
}
