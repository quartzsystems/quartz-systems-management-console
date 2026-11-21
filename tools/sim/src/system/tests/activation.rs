// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! Activations, shut downs and restarts: their codes, states, what is
//! given and given back, and their refusals.

use qsi::any::{AnyEvent, AnyReply, AnyRequest};
use qsi::host::model::{IplSource, PartitionState};
use qsi::host::partition::{self, Activate, Restart, ShutDown, ShutdownKind};
use qsi::host::{partitions, vres};

use super::{ask, codes_of, events, host, ids};
use crate::system::System;

fn activate(
    s: &mut System,
    id: u32,
    profile: &str,
    source: Option<IplSource>,
) -> Result<AnyReply, Vec<(String, String)>> {
    ask(
        s,
        AnyRequest::Partition(partition::Request::Activate(Activate {
            id,
            profile: profile.into(),
            source,
            keylock: None,
        })),
    )
}

fn shut_down(
    s: &mut System,
    id: u32,
    kind: ShutdownKind,
    restart: bool,
) -> Result<AnyReply, Vec<(String, String)>> {
    ask(
        s,
        AnyRequest::Partition(partition::Request::ShutDown(ShutDown {
            id,
            kind,
            delay: 300,
            restart,
        })),
    )
}

fn states(s: &System, from: u64, id: u32) -> Vec<PartitionState> {
    events(s, from)
        .into_iter()
        .filter_map(|e| match e {
            AnyEvent::Partitions(partitions::Event::State(c)) if c.id == id => Some(c.state),
            _ => None,
        })
        .collect()
}

fn state(s: &System, id: u32) -> PartitionState {
    s.part_ref(id).map(|p| p.status.state).unwrap()
}

#[test]
#[allow(
    clippy::many_single_char_names,
    reason = "the system and what it gives"
)]
fn an_activation_from_a_side_runs_through() {
    let mut s = host();
    let at = s.log.next();
    let r = activate(&mut s, 2, "default", None).unwrap();
    let AnyReply::Partition(partition::Reply::Activate(c)) = r else {
        panic!("{r:?}");
    };
    assert_eq!(
        (c.units, c.virtuals, c.memory_mib, c.capped),
        (150, 2, 8192, true)
    );
    assert_eq!(
        states(&s, at, 2),
        [PartitionState::Starting, PartitionState::Running]
    );
    let codes = codes_of(&s, 2);
    let i = codes.iter().position(|c| c == "C2005000").unwrap();
    assert!(codes[i..].starts_with(&["C2005000".into(), "C2005010".into()]));
    assert!(codes.iter().any(|c| c == "C2005090"));
    assert!(codes.last().is_some_and(|c| c.starts_with("C6")));
    // What it held from its stopped IPL was given back first: the same
    // amounts after.
    let h = s.host.clone().unwrap();
    assert_eq!((h.processors.available, h.memory.available), (350, 40_960));
    let p = s.part_ref(2).unwrap();
    assert_eq!(p.status.profile, "default");
    assert!(p.since.is_some() && p.status.ethernet.iter().all(|l| l.link));
    let ev = events(&s, at);
    assert!(
        ev.iter().any(
            |e| matches!(e, AnyEvent::Vres(vres::Event::Port(p)) if p.open && p.partition == 2)
        )
    );
    assert!(
        ev.iter()
            .any(|e| matches!(e, AnyEvent::Host(qsi::host::host::Event::Resources(_))))
    );
}

#[test]
fn an_activation_from_d_waits_at_the_install_screen() {
    let mut s = host();
    let at = s.log.next();
    activate(&mut s, 2, "install", None).unwrap();
    assert_eq!(states(&s, at, 2), [PartitionState::Starting]);
    let p = s.part_ref(2).unwrap();
    assert_eq!(p.status.code, "A6005008");
    let read = p
        .history
        .iter()
        .find(|c| c.refcode.code == "C2005045")
        .unwrap();
    assert!(read.refcode.text.contains("QS400_V1R0M0_INSTALL.iso"));
    assert!(read.refcode.text.contains("QS400_V1R0M0"));
}

#[test]
fn an_ipl_from_d_with_no_medium_does_not_ipl() {
    let mut s = host();
    activate(&mut s, 2, "default", Some(IplSource::D)).unwrap();
    let codes = codes_of(&s, 2);
    assert!(codes.ends_with(&["A2005032".into(), "A2005100".into()]));
    assert_eq!(state(&s, 2), PartitionState::NotActivated);
    let p = s.part_ref(2).unwrap();
    assert!(p.holds.is_none() && p.status.current.is_none());
    assert_eq!(s.host.as_ref().unwrap().processors.available, 500);
}

#[test]
fn activations_are_refused_in_qsvs_words() {
    let mut s = host();
    assert_eq!(ids(activate(&mut s, 1, "", None)), ["QVS2002"]);
    assert_eq!(ids(activate(&mut s, 9, "", None)), ["QVS2001"]);
    assert_eq!(ids(activate(&mut s, 2, "nope", None)), ["QVS2004"]);
    s.faults.missing.push("POOL1/VD04".into());
    assert_eq!(ids(activate(&mut s, 2, "", None)), ["QVS2110"]);
    s.faults.missing = vec!["ETHSW0".into()];
    assert_eq!(ids(activate(&mut s, 2, "", None)), ["QVS2117"]);
    s.faults.missing.clear();
    // More memory than the host has left.
    s.release(2);
    if let Some(h) = &mut s.host {
        h.memory.available = 1024;
    }
    assert_eq!(ids(activate(&mut s, 2, "", None)), ["QVS2104"]);
}

#[test]
fn each_shut_down_has_its_codes_then_not_activated() {
    for (kind, code) in [
        (ShutdownKind::Delayed, "C2009040"),
        (ShutdownKind::System, "C2009041"),
        (ShutdownKind::Immediate, "A2005140"),
    ] {
        let mut s = host();
        let at = s.log.next();
        shut_down(&mut s, 1, kind, false).unwrap();
        assert_eq!(
            states(&s, at, 1),
            [PartitionState::ShuttingDown, PartitionState::NotActivated]
        );
        let codes = codes_of(&s, 1);
        assert!(codes.iter().any(|c| c == code), "{kind:?}: {codes:?}");
        let p = s.part_ref(1).unwrap();
        assert!(p.holds.is_none() && p.drives.is_empty());
        // Given back: its 1.00 units and 16 GB.
        let h = s.host.as_ref().unwrap();
        assert_eq!((h.processors.available, h.memory.available), (450, 57_344));
        assert_eq!(h.pools[0].entitled, 150);
        // Its disk and media are no longer held.
        assert_eq!(s.vres.vdisks[2].holder.partition, 0);
        assert_eq!(
            ids(shut_down(&mut s, 1, ShutdownKind::Immediate, false)),
            ["QVS2003"]
        );
    }
}

#[test]
fn a_shut_down_with_restart_and_a_restart_come_back_running() {
    let mut s = host();
    shut_down(&mut s, 1, ShutdownKind::Delayed, true).unwrap();
    assert_eq!(state(&s, 1), PartitionState::Running);
    assert_eq!(s.part_ref(1).unwrap().status.profile, "default");
    let at = s.log.next();
    ask(
        &mut s,
        AnyRequest::Partition(partition::Request::Restart(Restart { id: 3 })),
    )
    .unwrap();
    assert!(codes_of(&s, 3).iter().any(|c| c == "C2005110"));
    assert_eq!(
        states(&s, at, 3),
        [PartitionState::Starting, PartitionState::Running]
    );
    // A restart keeps what it holds: its cores.
    let c = s.part_ref(3).unwrap().holds.clone().unwrap();
    assert_eq!(c.cores, [0, 1]);
    assert_eq!(
        ids(ask(
            &mut s,
            AnyRequest::Partition(partition::Request::Restart(Restart { id: 2 }))
        )),
        ["QVS2003"]
    );
}

#[test]
fn a_dedicated_partition_takes_the_default_pool_s_cores_and_gives_them_back() {
    let mut s = host();
    shut_down(&mut s, 3, ShutdownKind::Immediate, false).unwrap();
    assert_eq!(
        s.host.as_ref().unwrap().pools[0].cores,
        [0, 1, 2, 3, 4, 5, 6, 7]
    );
    activate(&mut s, 3, "", None).unwrap();
    let c = s.part_ref(3).unwrap().holds.clone().unwrap();
    // The highest not in TESTPOOL.
    assert_eq!(c.cores, [4, 5]);
    let d = &s.host.as_ref().unwrap().pools[0];
    assert_eq!((d.cores.len(), d.maximum), (6, 600));
    // Its device and tape drive are held again.
    assert_eq!(s.devices[2].holder.as_ref().map(|h| h.partition), Some(3));
    assert_eq!(s.vres.tape_drives[0].holder.partition, 3);
}

#[test]
fn activation_times_are_listed() {
    let mut s = host();
    let r = ask(
        &mut s,
        AnyRequest::Partitions(partitions::Request::Activations(qsi::Empty)),
    )
    .unwrap();
    let AnyReply::Partitions(partitions::Reply::Activations(v)) = r else {
        panic!("{r:?}");
    };
    assert_eq!(v[0].since, Some(crate::fixture::DEV_SINCE));
    assert_eq!((v[1].since, v[1].profile.as_str()), (None, "install"));
    assert_eq!(v[2].profile, "prod");
}
