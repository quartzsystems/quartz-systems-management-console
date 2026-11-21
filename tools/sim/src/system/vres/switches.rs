// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! Virtual switches as QS/VS's switch service keeps them
//! (`docs/NETWORK.md`): VLAN-aware, an uplink of the host's interfaces
//! by location code or none; the partitions' ports on them, worked out
//! from the profiles in force.

use qsi::Empty;
use qsi::any::AnyEvent;
use qsi::host::host::{self, ItemKind, Owner};
use qsi::host::vres::{
    self, Bond, Counters, FdbRow, Member, Port, Reply, Request, Switch, SwitchSpec, Uplink,
    VlanPort, VlanUse,
};

use crate::system::refuse::{Refusal, name_ok, no, no_such};
use crate::system::{System, now_s};

/// The most interfaces of an uplink.
pub const MAX_UPLINK: usize = 8;
/// The host's MTU range.
pub const MTU: std::ops::RangeInclusive<u32> = 1280..=9000;

fn no_switch(name: &str) -> Refusal {
    no("QVS4003", format!("There is no switch named {name}."))
}

/// The partitions' ports, every switch's or `switch`'s.
pub fn ports(s: &System, switch: &str) -> Vec<Port> {
    let mut v = Vec::new();
    for p in &s.parts {
        let Some(pr) = p.in_force() else {
            continue;
        };
        let link = p.active() && p.holds.is_some();
        for e in &pr.ethernet {
            let Some(sw) = &e.switch else {
                continue;
            };
            if !switch.is_empty() && sw != switch {
                continue;
            }
            let seed = u64::from(p.definition.id) * 1000 + u64::from(e.slot);
            let counters = if link {
                Counters {
                    sent_bytes: seed * 7_340_033,
                    sent_packets: seed * 5_021,
                    sent_dropped: 0,
                    received_bytes: seed * 9_437_189,
                    received_packets: seed * 6_113,
                    received_dropped: seed % 3,
                }
            } else {
                Counters::default()
            };
            v.push(Port {
                partition: p.definition.id,
                slot: e.slot,
                switch: sw.clone(),
                vlan: u16::try_from(e.vlan).unwrap_or(0),
                tagged: e
                    .tagged
                    .iter()
                    .filter_map(|t| u16::try_from(*t).ok())
                    .collect(),
                mac: e.mac.clone().unwrap_or_default(),
                link,
                counters,
            });
        }
    }
    v
}

/// The switches, each with its ports counted.
fn listed(s: &System) -> Vec<Switch> {
    s.vres
        .switches
        .iter()
        .map(|w| {
            let mut w = w.clone();
            w.ports = u32::try_from(ports(s, &w.name).len()).unwrap_or(0);
            w
        })
        .collect()
}

/// What the switches' requests come to.
pub fn answer(s: &mut System, r: &Request) -> Result<Reply, Refusal> {
    Ok(match r {
        Request::Switches(_) => Reply::Switches(listed(s)),
        Request::Ports(q) => {
            if !q.switch.is_empty() {
                find(s, &q.switch)?;
            }
            Reply::Ports(ports(s, &q.switch))
        }
        Request::Fdb(q) => Reply::Fdb(fdb(s, &q.switch)?),
        Request::Vlans(_) => Reply::Vlans(vlans(s)),
        Request::Limits(_) => Reply::Limits(s.vres.limits),
        Request::SwitchCreate(w) => Reply::SwitchCreate(create(s, w)?),
        Request::SwitchChange(w) => {
            change(s, w)?;
            Reply::SwitchChange(Empty)
        }
        Request::SwitchRename(w) => {
            rename(s, &w.name, &w.to)?;
            Reply::SwitchRename(Empty)
        }
        Request::SwitchDelete(w) => {
            delete(s, &w.name, w.force)?;
            Reply::SwitchDelete(Empty)
        }
        Request::UplinkSet(u) => {
            set_uplink(s, &u.switch, Some(&u.uplink))?;
            Reply::UplinkSet(Empty)
        }
        Request::UplinkClear(u) => {
            set_uplink(s, &u.switch, None)?;
            Reply::UplinkClear(Empty)
        }
        _ => return Err(no_such("operation")),
    })
}

fn find<'a>(s: &'a System, name: &str) -> Result<&'a Switch, Refusal> {
    s.vres
        .switches
        .iter()
        .find(|w| w.name == name)
        .ok_or_else(|| no_switch(name))
}

fn fdb(s: &System, switch: &str) -> Result<Vec<FdbRow>, Refusal> {
    let w = find(s, switch)?;
    let mut v: Vec<FdbRow> = ports(s, switch)
        .into_iter()
        .filter(|p| p.link)
        .map(|p| FdbRow {
            mac: p.mac,
            vlan: p.vlan,
            port: format!("partition {} slot {}", p.partition, p.slot),
            partition: p.partition,
            slot: p.slot,
            origin: "learned".into(),
        })
        .collect();
    if w.uplink.is_some() {
        v.push(FdbRow {
            mac: "00:1b:21:aa:0f:01".into(),
            vlan: w.vlans.first().copied().unwrap_or(0),
            port: "uplink".into(),
            partition: 0,
            slot: 0,
            origin: "learned".into(),
        });
    }
    v.push(FdbRow {
        mac: format!("02:51:53:ff:00:{:02x}", w.id & 0xff),
        vlan: 0,
        port: "switch".into(),
        partition: 0,
        slot: 0,
        origin: "local".into(),
    });
    Ok(v)
}

fn vlans(s: &System) -> Vec<VlanUse> {
    let mut v = Vec::new();
    for w in &s.vres.switches {
        let on = ports(s, &w.name);
        for vlan in &w.vlans {
            v.push(VlanUse {
                switch: w.name.clone(),
                vlan: *vlan,
                native: w.uplink.as_ref().and_then(|u| u.native_vlan) == Some(*vlan),
                ports: on
                    .iter()
                    .filter(|p| p.vlan == *vlan || p.tagged.contains(vlan))
                    .map(|p| VlanPort {
                        partition: p.partition,
                        slot: p.slot,
                        tagged: p.vlan != *vlan,
                    })
                    .collect(),
            });
        }
    }
    v
}

/// A switch's VLANs and MTU, checked.
fn check_spec(s: &System, w: &SwitchSpec) -> Result<u32, Refusal> {
    let mut seen = Vec::new();
    for vlan in &w.vlans {
        if !(1..=4094).contains(vlan) || seen.contains(vlan) {
            return Err(no(
                "QVS4004",
                format!("VLAN {vlan} of switch {} is not valid.", w.name),
            ));
        }
        seen.push(*vlan);
    }
    let most = s.vres.limits.vlans_per_switch;
    if w.vlans.len() > most as usize {
        return Err(no(
            "QVS4005",
            format!(
                "Switch {} would carry {} VLANs; a switch may carry {most}.",
                w.name,
                w.vlans.len()
            ),
        ));
    }
    let mtu = if w.mtu == 0 { 1500 } else { w.mtu };
    if !MTU.contains(&mtu) {
        return Err(no(
            "QVS4006",
            format!("MTU {mtu} of switch {} is not valid.", w.name),
        ));
    }
    Ok(mtu)
}

/// Say a switch changed.
fn changed(s: &mut System, name: &str) {
    let switch = listed(s).into_iter().find(|w| w.name == name);
    s.push(&AnyEvent::Vres(vres::Event::Switch(vres::SwitchChanged {
        name: name.into(),
        switch,
        time: now_s(),
    })));
}

/// The first profile that names switch `name` (on `vlan`, when given):
/// partition, its name, the profile and the slot.
fn naming(s: &System, name: &str, vlan: Option<u16>) -> Option<(String, String, u32)> {
    s.parts.iter().find_map(|p| {
        p.profiles.iter().find_map(|k| {
            k.profile
                .ethernet
                .iter()
                .find(|e| {
                    e.switch.as_deref() == Some(name)
                        && vlan.is_none_or(|v| {
                            e.vlan == u32::from(v) || e.tagged.contains(&u32::from(v))
                        })
                })
                .map(|e| (p.status.name.clone(), k.profile.name.clone(), e.slot))
        })
    })
}

fn create(s: &mut System, w: &SwitchSpec) -> Result<Switch, Refusal> {
    if !name_ok(&w.name) {
        return Err(no(
            "QVS4001",
            format!("Switch name {} is not valid.", w.name),
        ));
    }
    if s.vres.switches.iter().any(|x| x.name == w.name) {
        return Err(no(
            "QVS4002",
            format!("There is already a switch named {}.", w.name),
        ));
    }
    let most = s.vres.limits.switches;
    if s.vres.switches.len() >= most as usize {
        return Err(no(
            "QVS4007",
            format!(
                "The host would have {} switches; it may have {most}.",
                s.vres.switches.len() + 1
            ),
        ));
    }
    let mtu = check_spec(s, w)?;
    let id = s.vres.switches.iter().map(|x| x.id + 1).max().unwrap_or(0);
    let made = Switch {
        name: w.name.clone(),
        id,
        vlans: w.vlans.clone(),
        mtu,
        uplink: None,
        uplink_state: "None".into(),
        members: Vec::new(),
        ports: 0,
        failure: String::new(),
    };
    s.vres.switches.push(made.clone());
    changed(s, &w.name);
    Ok(made)
}

fn change(s: &mut System, w: &SwitchSpec) -> Result<(), Refusal> {
    let was = find(s, &w.name)?.clone();
    let mut spec = w.clone();
    if spec.mtu == 0 {
        spec.mtu = was.mtu;
    }
    let mtu = check_spec(s, &spec)?;
    for gone in was.vlans.iter().filter(|v| !w.vlans.contains(v)) {
        if let Some((part, profile, slot)) = naming(s, &w.name, Some(*gone)) {
            return Err(no(
                "QVS4009",
                format!(
                    "VLAN {gone} of switch {} is used by partition {part}, profile {profile}, slot {slot}.",
                    w.name
                ),
            ));
        }
    }
    if let Some(n) = was.uplink.as_ref().and_then(|u| u.native_vlan)
        && !w.vlans.contains(&n)
    {
        return Err(no(
            "QVS4010",
            format!("Native VLAN {n} is not a VLAN switch {} carries.", w.name),
        ));
    }
    if let Some(x) = s.vres.switches.iter_mut().find(|x| x.name == w.name) {
        x.vlans.clone_from(&w.vlans);
        x.mtu = mtu;
    }
    changed(s, &w.name);
    Ok(())
}

fn rename(s: &mut System, name: &str, to: &str) -> Result<(), Refusal> {
    find(s, name)?;
    if !name_ok(to) {
        return Err(no("QVS4001", format!("Switch name {to} is not valid.")));
    }
    if s.vres.switches.iter().any(|x| x.name == to) {
        return Err(no(
            "QVS4002",
            format!("There is already a switch named {to}."),
        ));
    }
    if let Some((part, profile, slot)) = naming(s, name, None) {
        return Err(no(
            "QVS4008",
            format!("Switch {name} is named by partition {part}, profile {profile}, slot {slot}."),
        ));
    }
    if let Some(x) = s.vres.switches.iter_mut().find(|x| x.name == name) {
        x.name = to.into();
    }
    own(s, name, to);
    changed(s, name);
    changed(s, to);
    Ok(())
}

fn delete(s: &mut System, name: &str, force: bool) -> Result<(), Refusal> {
    find(s, name)?;
    if let Some(p) = ports(s, name).iter().find(|p| p.link) {
        return Err(no(
            "QVS4015",
            format!(
                "Switch {name} has the port of partition {}, slot {}.",
                s.part_name(p.partition),
                p.slot
            ),
        ));
    }
    if !force && let Some((part, profile, slot)) = naming(s, name, None) {
        return Err(no(
            "QVS4008",
            format!("Switch {name} is named by partition {part}, profile {profile}, slot {slot}."),
        ));
    }
    s.vres.switches.retain(|x| x.name != name);
    own(s, name, "");
    changed(s, name);
    Ok(())
}

/// The interfaces switch `from`'s uplink holds go to switch `to` (none
/// when empty), in the hardware's owners.
fn own(s: &mut System, from: &str, to: &str) {
    let mut events = Vec::new();
    if let Some(h) = &mut s.hardware {
        for i in h
            .items
            .iter_mut()
            .filter(|i| i.kind == ItemKind::Interface && i.owner == Owner::Switch)
            .filter(|i| i.owner_name == from)
        {
            if to.is_empty() {
                i.owner = Owner::None;
                i.owner_name.clear();
                i.what.clear();
            } else {
                i.owner_name = to.into();
                i.what = format!("Uplink of {to}");
            }
            events.push(i.clone());
        }
    }
    for item in events {
        s.push(&AnyEvent::Host(host::Event::Item(host::ItemChanged {
            location: item.location.clone(),
            item: Some(item),
            time: now_s(),
        })));
    }
}

#[allow(
    clippy::too_many_lines,
    reason = "each interface checked, then the uplink"
)]
fn set_uplink(s: &mut System, name: &str, up: Option<&Uplink>) -> Result<(), Refusal> {
    let w = find(s, name)?.clone();
    let Some(up) = up else {
        own(s, name, "");
        if let Some(x) = s.vres.switches.iter_mut().find(|x| x.name == name) {
            x.uplink = None;
            x.uplink_state = "None".into();
            x.members.clear();
        }
        changed(s, name);
        return Ok(());
    };
    if matches!(up.bond, Some(Bond::Other(_))) {
        return Err(no_such("bond mode"));
    }
    let n = up.interfaces.len();
    if !(1..=MAX_UPLINK).contains(&n) {
        return Err(no(
            "QVS4106",
            format!("An uplink has 1 to {MAX_UPLINK} interfaces, not {n}."),
        ));
    }
    let items = s
        .hardware
        .as_ref()
        .map(|h| h.items.clone())
        .unwrap_or_default();
    let mut members = Vec::new();
    for (i, loc) in up.interfaces.iter().enumerate() {
        if up.interfaces[..i].contains(loc) {
            return Err(no("QVS4107", format!("Interface {loc} is named twice.")));
        }
        let Some(item) = items
            .iter()
            .find(|x| x.kind == ItemKind::Interface && &x.location == loc)
        else {
            return Err(no(
                "QVS4101",
                format!("There is no network interface at {loc}."),
            ));
        };
        if item.owner == Owner::Host {
            return Err(no(
                "QVS4102",
                format!("Interface {loc} is the management interface."),
            ));
        }
        if item.owner == Owner::Switch && item.owner_name != name {
            return Err(no(
                "QVS4103",
                format!(
                    "Interface {loc} is in the uplink of switch {}.",
                    item.owner_name
                ),
            ));
        }
        members.push(Member {
            location: loc.clone(),
            name: item.name.clone(),
            carrier: item.carrier,
            speed_mbps: item.speed_mbps,
        });
    }
    if n > 1 && up.bond.is_none() {
        return Err(no(
            "QVS4104",
            format!("An uplink of {n} interfaces needs a bond mode."),
        ));
    }
    if let Some(v) = up.native_vlan
        && !w.vlans.contains(&v)
    {
        return Err(no(
            "QVS4010",
            format!("Native VLAN {v} is not a VLAN switch {name} carries."),
        ));
    }
    own(s, name, "");
    let mut events = Vec::new();
    if let Some(h) = &mut s.hardware {
        for i in h
            .items
            .iter_mut()
            .filter(|i| i.kind == ItemKind::Interface && up.interfaces.contains(&i.location))
        {
            i.owner = Owner::Switch;
            i.owner_name = name.into();
            i.what = format!("Uplink of {name}");
            events.push(i.clone());
        }
    }
    for item in events {
        s.push(&AnyEvent::Host(host::Event::Item(host::ItemChanged {
            location: item.location.clone(),
            item: Some(item),
            time: now_s(),
        })));
    }
    let up_count = members.iter().filter(|m| m.carrier).count();
    let state = match up_count {
        0 => "Down",
        c if c == members.len() => "Up",
        _ => "Degraded",
    };
    if let Some(x) = s.vres.switches.iter_mut().find(|x| x.name == name) {
        x.uplink = Some(up.clone());
        x.uplink_state = state.into();
        x.members = members;
    }
    changed(s, name);
    Ok(())
}
