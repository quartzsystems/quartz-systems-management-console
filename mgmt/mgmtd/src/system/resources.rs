// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! A host's virtual resources, its power-on settings, its partitions'
//! profiles, activation times, drives and control panel functions:
//! read when the session begins and kept by the events, each read only
//! when the system's hello offers it. Where an event does not carry
//! the whole of a thing, its list is read again.

use mgmtproto::kept::{PartDrives, PartFunctions, PartProfiles};
use qsi::Empty;
use qsi::any::{AnyEvent, AnyReply, AnyRequest};
use qsi::caps;
use qsi::host::model::PartitionState;
use qsi::host::partitions::Status;
use qsi::host::{host, panel, partitions, profiles, vres};
use qsisess::Session;

use super::worker::{Mine, Worker};

/// Put `item` in `list` in the place of the one `same` finds, or at the
/// end; take it out when `None`.
fn upsert<T>(list: &mut Vec<T>, item: Option<T>, same: impl Fn(&T) -> bool) {
    match (list.iter().position(&same), item) {
        (Some(i), Some(t)) => list[i] = t,
        (Some(i), None) => {
            list.remove(i);
        }
        (None, Some(t)) => list.push(t),
        (None, None) => {}
    }
}

impl Worker {
    fn has(s: &Session, bits: u64) -> bool {
        s.peer().map_or(0, |h| h.caps) & bits == bits
    }

    /// Ask `req` for `what` unless the same read is out.
    fn read(&mut self, s: &mut Session, what: Mine, req: &AnyRequest) {
        if !self.asking_this(&what) {
            self.mine(s, what, req);
        }
    }

    /// Everything this module keeps, read whole.
    pub(super) fn read_resources(&mut self, s: &mut Session) {
        if Self::has(s, caps::VRES) {
            self.read(
                s,
                Mine::Pools,
                &AnyRequest::Vres(vres::Request::Pools(Empty)),
            );
            self.read_vdisks(s);
            self.read(
                s,
                Mine::Images,
                &AnyRequest::Vres(vres::Request::Images(Empty)),
            );
            self.read(
                s,
                Mine::Volumes,
                &AnyRequest::Vres(vres::Request::Volumes(Empty)),
            );
            self.read_switches(s);
            self.read_ports(s);
        }
        if Self::has(s, caps::VRES | caps::PHYSICAL_IO) {
            self.read_tape_drives(s);
        }
        if Self::has(s, caps::HOST | caps::PHYSICAL_IO) {
            self.read_devices(s);
        }
        if Self::has(s, caps::HOST | caps::POWER_ON) {
            self.read(
                s,
                Mine::PowerOn,
                &AnyRequest::Host(host::Request::PowerOn(Empty)),
            );
        }
        self.read_activations(s);
        // A standalone system's own panel is at ID 0.
        if Self::has(s, caps::PANEL | caps::PANEL_FUNCTIONS) && !Self::has(s, caps::PARTITIONS) {
            self.read_functions(s, 0);
        }
    }

    fn read_vdisks(&mut self, s: &mut Session) {
        let req = AnyRequest::Vres(vres::Request::Vdisks(vres::Vdisks {
            pool: String::new(),
        }));
        self.read(s, Mine::Vdisks, &req);
    }

    fn read_switches(&mut self, s: &mut Session) {
        self.read(
            s,
            Mine::Switches,
            &AnyRequest::Vres(vres::Request::Switches(Empty)),
        );
    }

    fn read_ports(&mut self, s: &mut Session) {
        let req = AnyRequest::Vres(vres::Request::Ports(vres::Ports {
            switch: String::new(),
        }));
        self.read(s, Mine::Ports, &req);
    }

    fn read_tape_drives(&mut self, s: &mut Session) {
        self.read(
            s,
            Mine::TapeDrives,
            &AnyRequest::Vres(vres::Request::TapeDrives(Empty)),
        );
    }

    fn read_devices(&mut self, s: &mut Session) {
        self.read(
            s,
            Mine::Devices,
            &AnyRequest::Host(host::Request::Devices(Empty)),
        );
    }

    fn read_activations(&mut self, s: &mut Session) {
        if Self::has(s, caps::PARTITIONS | caps::ACTIVATIONS) {
            self.read(
                s,
                Mine::Activations,
                &AnyRequest::Partitions(partitions::Request::Activations(Empty)),
            );
        }
    }

    fn read_functions(&mut self, s: &mut Session, id: u32) {
        let req = AnyRequest::Panel(panel::Request::Functions(panel::Functions { id }));
        self.read(s, Mine::Functions(id), &req);
    }

    fn read_profile_list(&mut self, s: &mut Session, id: u32) {
        if Self::has(s, caps::PROFILES) {
            let req = AnyRequest::Profiles(profiles::Request::List(profiles::List { id }));
            self.read(s, Mine::ProfileList(id), &req);
        }
    }

    fn read_drives(&mut self, s: &mut Session, id: u32) {
        if Self::has(s, caps::VRES) {
            let req = AnyRequest::Vres(vres::Request::Drives(vres::Drives { id }));
            self.read(s, Mine::Drives(id), &req);
        }
    }

    /// After the partitions' list: the profiles of each partition not
    /// yet listed, the drives of each running one, and the functions
    /// of each not yet read.
    pub(super) fn read_parts(&mut self, s: &mut Session, list: &[Status]) {
        let (lists, functions): (Vec<u32>, Vec<u32>) = {
            let k = self.entry.kept();
            (
                k.profile_lists.iter().map(|p| p.id).collect(),
                k.functions.iter().map(|f| f.id).collect(),
            )
        };
        let panel = Self::has(s, caps::PANEL | caps::PANEL_FUNCTIONS);
        for p in list {
            if !lists.contains(&p.id) {
                self.read_profile_list(s, p.id);
            }
            if p.state == PartitionState::Running {
                self.read_drives(s, p.id);
            }
            if panel && !functions.contains(&p.id) {
                self.read_functions(s, p.id);
            }
        }
        let mut k = self.entry.kept();
        let ids: Vec<u32> = list.iter().map(|p| p.id).collect();
        k.profile_lists.retain(|p| ids.contains(&p.id));
        k.functions.retain(|f| f.id == 0 || ids.contains(&f.id));
        k.activations.retain(|a| ids.contains(&a.id));
        let running: Vec<u32> = list
            .iter()
            .filter(|p| p.state == PartitionState::Running)
            .map(|p| p.id)
            .collect();
        k.resources.drives.retain(|d| running.contains(&d.id));
    }

    /// A reply to one of this module's reads; `false` when it is not one.
    pub(super) fn resources_reply(&mut self, what: &Mine, any: AnyReply) -> bool {
        let mut k = self.entry.kept();
        let r = &mut k.resources;
        match (what, any) {
            (Mine::Pools, AnyReply::Vres(vres::Reply::Pools(v))) => r.pools = v,
            (Mine::Vdisks, AnyReply::Vres(vres::Reply::Vdisks(v))) => r.vdisks = v,
            (Mine::Images, AnyReply::Vres(vres::Reply::Images(v))) => r.images = v,
            (Mine::Volumes, AnyReply::Vres(vres::Reply::Volumes(v))) => r.volumes = v,
            (Mine::Switches, AnyReply::Vres(vres::Reply::Switches(v))) => r.switches = v,
            (Mine::Ports, AnyReply::Vres(vres::Reply::Ports(v))) => r.ports = v,
            (Mine::TapeDrives, AnyReply::Vres(vres::Reply::TapeDrives(v))) => r.tape_drives = v,
            (Mine::Devices, AnyReply::Host(host::Reply::Devices(v))) => r.devices = v,
            (Mine::Drives(id), AnyReply::Vres(vres::Reply::Drives(drives))) => {
                upsert(&mut r.drives, Some(PartDrives { id: *id, drives }), |d| {
                    d.id == *id
                })
            }
            (Mine::PowerOn, AnyReply::Host(host::Reply::PowerOn(p))) => k.power_on = Some(p),
            (Mine::Activations, AnyReply::Partitions(partitions::Reply::Activations(v))) => {
                k.activations = v;
            }
            (Mine::ProfileList(id), AnyReply::Profiles(profiles::Reply::List(entries))) => {
                upsert(
                    &mut k.profile_lists,
                    Some(PartProfiles { id: *id, entries }),
                    |p| p.id == *id,
                );
            }
            (Mine::Functions(id), AnyReply::Panel(panel::Reply::Functions(functions))) => {
                upsert(
                    &mut k.functions,
                    Some(PartFunctions { id: *id, functions }),
                    |f| f.id == *id,
                );
            }
            _ => return false,
        }
        true
    }

    /// An event of what this module keeps; `false` when it is not one.
    pub(super) fn resources_event(&mut self, s: &mut Session, any: &AnyEvent) -> bool {
        match any {
            AnyEvent::Vres(e) => self.vres_event(s, e),
            AnyEvent::Host(host::Event::PowerOn(p)) => {
                self.entry.kept().power_on = Some(p.power_on);
            }
            AnyEvent::Host(
                host::Event::Released(_)
                | host::Event::TakenBack(_)
                | host::Event::Assigned(_)
                | host::Event::Returned(_)
                | host::Event::ResetFailed(_),
            ) => self.read_devices(s),
            AnyEvent::Profiles(profiles::Event::Changed(c)) => {
                self.read_profile_list(s, c.id);
                return false;
            }
            AnyEvent::Partitions(partitions::Event::State(c)) => {
                self.read_activations(s);
                if c.state == PartitionState::Running {
                    self.read_drives(s, c.id);
                }
                return false;
            }
            _ => return false,
        }
        true
    }

    fn vres_event(&mut self, s: &mut Session, event: &vres::Event) {
        match event {
            vres::Event::Uplink(_) => self.read_switches(s),
            vres::Event::Port(_) => self.read_ports(s),
            other => {
                let mut k = self.entry.kept();
                let res = &mut k.resources;
                match other {
                    vres::Event::Pool(ch) => {
                        upsert(&mut res.pools, ch.pool.clone(), |p| p.name == ch.name);
                    }
                    vres::Event::Vdisk(ch) => upsert(&mut res.vdisks, ch.vdisk.clone(), |v| {
                        v.pool == ch.pool && v.name == ch.name
                    }),
                    vres::Event::Image(ch) => {
                        upsert(&mut res.images, ch.image.clone(), |i| i.name == ch.name);
                    }
                    vres::Event::Upload(ch) => {
                        if ch.image.is_some() {
                            upsert(&mut res.images, ch.image.clone(), |i| i.name == ch.name);
                        }
                    }
                    vres::Event::Volume(ch) => {
                        upsert(&mut res.volumes, ch.volume.clone(), |v| v.name == ch.name);
                    }
                    vres::Event::Switch(ch) => {
                        upsert(&mut res.switches, ch.switch.clone(), |w| w.name == ch.name);
                    }
                    vres::Event::Drive(ch) => {
                        if let Some(d) = res.drives.iter_mut().find(|d| d.id == ch.id) {
                            upsert(&mut d.drives, Some(ch.drive.clone()), |x| {
                                x.slot == ch.drive.slot
                            });
                        }
                    }
                    vres::Event::TapeDrive(ch) => {
                        upsert(&mut res.tape_drives, ch.drive.clone(), |t| {
                            t.location == ch.location
                        });
                    }
                    _ => {}
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::upsert;

    #[test]
    fn an_item_is_put_in_its_place_added_or_taken_out() {
        let mut v = vec![(1, "a"), (2, "b")];
        upsert(&mut v, Some((2, "B")), |x| x.0 == 2);
        upsert(&mut v, Some((3, "c")), |x| x.0 == 3);
        upsert(&mut v, None, |x| x.0 == 1);
        upsert(&mut v, None, |x| x.0 == 9);
        assert_eq!(v, vec![(2, "B"), (3, "c")]);
    }
}
