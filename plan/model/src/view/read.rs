// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! A host read now through `mgmtd`, in a user's name: what `mgmtd`
//! keeps of its connection, then the host family's reads it carries
//! (`ask`) and a partition's definition (`partition`). A read the
//! host's hello does not offer is left empty; any other refusal stops
//! the read with the host's message.

use mgmtproto::kept::{Kind, Link};
use mgmtproto::{Asked, Calls, Refusal};
use qsi::Empty;
use qsi::any::{AnyReply, AnyReply as R, AnyRequest, AnyRequest as Q};
use qsi::host::{host, partition, partitions, profiles, vres};

use super::{HostView, PartView};

/// Why a host could not be read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReadError {
    /// `mgmtd` did not answer: why.
    Down(String),
    /// There is no such managed system.
    NoSystem,
    /// The host refused a read: its messages.
    Refused(Refusal),
}

impl std::fmt::Display for ReadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Down(e) => write!(f, "the managed-systems service did not answer: {e}"),
            Self::NoSystem => f.write_str("there is no such managed system"),
            Self::Refused(r) => write!(f, "{} {}", r.main.msgid, r.main.text),
        }
    }
}

struct Reader<'a> {
    mgmt: &'a mut dyn Calls,
    name: String,
    user: String,
    caps: u64,
}

impl Reader<'_> {
    /// `req`'s reply; `None` when the host's hello does not offer it.
    fn ask(&mut self, req: &AnyRequest) -> Result<Option<AnyReply>, ReadError> {
        if mgmtproto::ask::lacks_for(self.caps, req).is_some() {
            return Ok(None);
        }
        let answered = self
            .mgmt
            .ask(self.name.clone(), Asked::of(req), self.user.clone())
            .map_err(|e| ReadError::Down(e.to_string()))?
            .map_err(ReadError::Refused)?;
        answered
            .reply()
            .map(Some)
            .ok_or_else(|| ReadError::Refused(Refusal::new("", "the host's reply did not read")))
    }
}

fn odd() -> ReadError {
    ReadError::Refused(Refusal::new("", "the host answered with another reply"))
}

/// Host `name` as `user` reads it now.
#[allow(clippy::too_many_lines, reason = "a read a family, in order")]
pub fn host(mgmt: &mut dyn Calls, name: &str, user: &str) -> Result<HostView, ReadError> {
    let kept = mgmt
        .system(name.to_string())
        .map_err(|e| ReadError::Down(e.to_string()))?
        .ok_or(ReadError::NoSystem)?;
    let mut v = HostView {
        name: kept.name.clone(),
        is_host: kept.kind == Kind::Host,
        connected: kept.link == Link::Connected,
        reason: kept.reason.clone(),
        caps: kept.caps,
        model: kept.model().to_string(),
        serial: kept
            .identity
            .as_ref()
            .map_or_else(|| kept.serial.clone(), |i| i.serial.clone()),
        ..HostView::default()
    };
    if !v.is_host || !v.connected {
        return Ok(v);
    }
    let mut r = Reader {
        mgmt,
        name: kept.name.clone(),
        user: user.to_string(),
        caps: kept.caps,
    };
    match r.ask(&Q::Host(host::Request::Host(Empty)))? {
        Some(R::Host(host::Reply::Host(h))) => v.host = Some(h),
        None => {}
        Some(_) => return Err(odd()),
    }
    match r.ask(&Q::Host(host::Request::Hardware(Empty)))? {
        Some(R::Host(host::Reply::Hardware(h))) => v.hardware = Some(h),
        None => {}
        Some(_) => return Err(odd()),
    }
    match r.ask(&Q::Host(host::Request::Devices(Empty)))? {
        Some(R::Host(host::Reply::Devices(d))) => v.devices = d,
        None => {}
        Some(_) => return Err(odd()),
    }
    macro_rules! vres_list {
        ($req:expr, $variant:ident, $field:ident) => {
            match r.ask(&Q::Vres($req))? {
                Some(R::Vres(vres::Reply::$variant(x))) => v.$field = x,
                None => {}
                Some(_) => return Err(odd()),
            }
        };
    }
    vres_list!(vres::Request::Disks(Empty), Disks, disks);
    vres_list!(vres::Request::Pools(Empty), Pools, pools);
    vres_list!(
        vres::Request::Vdisks(vres::Vdisks {
            pool: String::new()
        }),
        Vdisks,
        vdisks
    );
    vres_list!(vres::Request::Images(Empty), Images, images);
    vres_list!(vres::Request::Volumes(Empty), Volumes, volumes);
    vres_list!(vres::Request::TapeDrives(Empty), TapeDrives, tape_drives);
    vres_list!(vres::Request::Switches(Empty), Switches, switches);
    let list = match r.ask(&Q::Partitions(partitions::Request::List(Empty)))? {
        Some(R::Partitions(partitions::Reply::List(l))) => l,
        None => Vec::new(),
        Some(_) => return Err(odd()),
    };
    for st in list {
        let definition = r
            .mgmt
            .partition(r.name.clone(), st.id, r.user.clone())
            .map_err(|e| ReadError::Down(e.to_string()))?
            .map_err(ReadError::Refused)?
            .definition;
        let settings = match r.ask(&Q::Partition(partition::Request::GetSettings(
            partition::GetSettings { id: st.id },
        )))? {
            Some(R::Partition(partition::Reply::GetSettings(s))) => Some(s),
            None => None,
            Some(_) => return Err(odd()),
        };
        let entries = match r.ask(&Q::Profiles(profiles::Request::List(profiles::List {
            id: st.id,
        })))? {
            Some(R::Profiles(profiles::Reply::List(e))) => e,
            None => Vec::new(),
            Some(_) => return Err(odd()),
        };
        let mut profs = Vec::new();
        for e in entries {
            match r.ask(&Q::Profiles(profiles::Request::Get(profiles::Get {
                id: st.id,
                name: e.name.clone(),
            })))? {
                Some(R::Profiles(profiles::Reply::Get(g))) => profs.push(g.profile),
                None => {}
                Some(_) => return Err(odd()),
            }
        }
        v.partitions.push(PartView {
            definition,
            settings,
            profiles: profs,
        });
    }
    v.partitions.sort_by_key(|p| p.definition.id);
    Ok(v)
}
