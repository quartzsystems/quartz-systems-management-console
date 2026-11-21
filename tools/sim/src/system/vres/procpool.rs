// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! Shared processor pools as QS/VS's partition manager keeps them
//! (`docs/RESOURCES.md`): a named pool has cores and a maximum; the
//! default pool, first, is not made, changed or deleted.

use qsi::Empty;
use qsi::any::AnyEvent;
use qsi::host::host::{self, ProcPool};
use qsi::host::model::ProcessorMode;
use qsi::host::vres::{ProcPoolSpec, Reply, Request};

use crate::system::System;
use crate::system::refuse::{Refusal, name_ok, no, no_such};

/// What the processor pools' requests come to.
pub fn answer(s: &mut System, r: &Request) -> Result<Reply, Refusal> {
    Ok(match r {
        Request::ProcPoolCreate(p) => {
            make(s, p, true)?;
            Reply::ProcPoolCreate(Empty)
        }
        Request::ProcPoolChange(p) => {
            make(s, p, false)?;
            Reply::ProcPoolChange(Empty)
        }
        Request::ProcPoolDelete(d) => {
            in_use(s, &d.name, false)?;
            if let Some(h) = &mut s.host {
                h.pools.retain(|p| p.name != d.name);
            }
            resources(s);
            Reply::ProcPoolDelete(Empty)
        }
        _ => return Err(no_such("operation")),
    })
}

fn resources(s: &mut System) {
    if let Some(h) = s.host.clone() {
        s.push(&AnyEvent::Host(host::Event::Resources(h)));
    }
}

/// Pool `name` may be changed or deleted (or, `new`, made): QS/VS's
/// message when not.
fn in_use(s: &System, name: &str, new: bool) -> Result<(), Refusal> {
    let pools = s.host.as_ref().map_or(&[][..], |h| h.pools.as_slice());
    if pools.first().is_some_and(|d| d.name == name) {
        return Err(no(
            "QVS2209",
            "The default shared processor pool cannot be created, changed or deleted.",
        ));
    }
    let there = pools.iter().any(|p| p.name == name);
    if new && there {
        return Err(no(
            "QVS2202",
            format!("Shared processor pool {name} already exists."),
        ));
    }
    if !new && !there {
        return Err(no(
            "QVS2105",
            format!("Shared processor pool {name} does not exist."),
        ));
    }
    let running = s
        .parts
        .iter()
        .filter_map(|p| p.holds.as_ref())
        .filter(|c| c.mode == ProcessorMode::Shared && c.pool == name)
        .count();
    if running > 0 {
        return Err(no(
            "QVS2208",
            format!("Shared processor pool {name} is in use by {running} running partitions."),
        ));
    }
    Ok(())
}

fn make(s: &mut System, p: &ProcPoolSpec, new: bool) -> Result<(), Refusal> {
    in_use(s, &p.name, new)?;
    if !name_ok(&p.name) {
        return Err(no(
            "QVS1110",
            format!("Shared processor pool name {} is not valid.", p.name),
        ));
    }
    let Some(h) = &s.host else {
        return Err(no("QSI0301", "The partition manager did not answer."));
    };
    if p.cores.is_empty() {
        return Err(no(
            "QVS2206",
            format!("Shared processor pool {} has no core.", p.name),
        ));
    }
    let dedicated: Vec<u32> = s
        .parts
        .iter()
        .filter_map(|x| x.holds.as_ref())
        .filter(|c| c.mode == ProcessorMode::Dedicated)
        .flat_map(|c| c.cores.clone())
        .collect();
    for (i, c) in p.cores.iter().enumerate() {
        if *c >= h.cores || h.reserved_cores.contains(c) {
            return Err(no(
                "QVS2203",
                format!("Core {c} cannot be in shared processor pool {}.", p.name),
            ));
        }
        let twice = p.cores[..i].contains(c);
        let other = h
            .pools
            .iter()
            .skip(1)
            .any(|x| x.name != p.name && x.cores.contains(c));
        if twice || other {
            return Err(no(
                "QVS2204",
                format!(
                    "Core {c} of shared processor pool {} is listed twice or is in another pool.",
                    p.name
                ),
            ));
        }
        if dedicated.contains(c) {
            return Err(no(
                "QVS2205",
                format!(
                    "Core {c} cannot be in shared processor pool {}: a running partition has it as a dedicated processor.",
                    p.name
                ),
            ));
        }
    }
    let worth = u32::try_from(p.cores.len()).unwrap_or(0) * 100;
    let maximum = p.maximum_units.unwrap_or(worth);
    if maximum == 0 || maximum > worth {
        return Err(no(
            "QVS2207",
            format!(
                "The maximum of {} processing units is not valid for shared processor pool {}.",
                crate::system::refuse::units(maximum),
                p.name
            ),
        ));
    }
    let pool = ProcPool {
        name: p.name.clone(),
        cores: p.cores.clone(),
        maximum,
        entitled: 0,
        available: maximum,
        partitions: 0,
    };
    if let Some(h) = &mut s.host {
        match h.pools.iter_mut().find(|x| x.name == p.name) {
            Some(x) => *x = pool,
            None => h.pools.push(pool),
        }
    }
    resources(s);
    Ok(())
}
