// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! Serving Network settings. The `open` interface's address, mask and
//! gateway are the ones the browser came in on: a change to them waits
//! for the network service to keep a change only when the console is
//! reached again (`docs/OPEN_QUESTIONS.md`, item 24). Its name
//! servers, the host name, the `private` interface and the addresses
//! the web interface answers are changed here; a network change goes
//! to the network service on trial.

use std::net::{IpAddr, Ipv4Addr};

use qsweb::http::Response;
use qsweb::msg::{Kind, Message};
use state::{Cidr, Device, Interface, Mode, Network};

use crate::app::routes::Rq;
use crate::network::{Link, Net, mask_of, page, prefix_of};

/// The role of the private interface.
const PRIVATE: &str = "private";

fn speed(mbps: Option<u32>) -> String {
    match mbps {
        Some(m) if m >= 1000 && m % 1000 == 0 => format!("{} GbE", m / 1000),
        Some(m) => format!("{m} Mb/s"),
        None => String::new(),
    }
}

fn link_of(rq: &Rq<'_>, role: &str) -> Link {
    let status = rq.app.console.interfaces();
    let Some(s) = status.iter().find(|s| s.role == role) else {
        return Link {
            name: "none".into(),
            state: "not configured".into(),
        };
    };
    let inventory = rq.app.store().load::<state::Inventory>().ok().flatten();
    let device = if s.device.is_empty() {
        &s.interface
    } else {
        &s.device
    };
    let found = inventory
        .as_ref()
        .and_then(|i| i.interfaces.iter().find(|n| &n.name == device));
    let name = found
        .map(|n| n.location.clone())
        .filter(|l| !l.is_empty())
        .map_or_else(|| s.interface.clone(), |l| format!("{l} {}", s.interface));
    let mut state = Vec::new();
    let sp = speed(found.and_then(|n| n.speed_mbps));
    if !sp.is_empty() {
        state.push(sp);
    }
    state.push(if s.carrier { "link up" } else { "link down" }.to_string());
    Link {
        name,
        state: state.join(", "),
    }
}

fn configured(network: &Network, role: &str) -> Option<Interface> {
    network.interfaces.iter().find(|i| i.role == role).cloned()
}

fn view(rq: &Rq<'_>, messages: Vec<Message>) -> Net {
    let app = rq.app;
    let network = app
        .store()
        .load::<Network>()
        .ok()
        .flatten()
        .unwrap_or_default();
    let open = network.interfaces.first().cloned().unwrap_or_default();
    let private = configured(&network, PRIVATE);
    let host = app.host();
    let addr = |i: &Interface| {
        i.address
            .filter(|_| i.mode == Mode::Static)
            .map_or((String::new(), String::new()), |c| {
                (c.addr.to_string(), mask_of(c.prefix))
            })
    };
    let (ip, mask) = addr(&open);
    let (pip, pmask) = private.as_ref().map(addr).unwrap_or_default();
    Net {
        open: link_of(rq, ""),
        ip,
        mask,
        gateway: open.gateway.map(|g| g.to_string()).unwrap_or_default(),
        dns: open
            .dns
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(", "),
        host: host.name.unwrap_or_else(|| app.console.name()),
        domain: String::new(),
        private: link_of(rq, PRIVATE),
        pip,
        pmask,
        range: "none".into(),
        dhcp: false,
        reached: "none".into(),
        allow: app.settings().allow.join(", "),
        ssh: false,
        messages,
    }
}

fn show(rq: &Rq<'_>, messages: Vec<Message>) -> Response {
    rq.page(page(&rq.ctx, &view(rq, messages)))
}

fn not_valid(text: &str) -> Message {
    qmcmsg::message(Kind::Error, &qmcmsg::NOT_VALID, &[text])
}

fn list(text: &str) -> Vec<String> {
    text.split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(String::from)
        .collect()
}

/// The `private` interface the form asks for: none when its address is
/// empty; on the first link that is not the `open` one's.
fn private_of(
    rq: &Rq<'_>,
    before: Option<Interface>,
    pip: &str,
    pmask: &str,
) -> Result<Option<Interface>, String> {
    if pip.is_empty() {
        return Ok(None);
    }
    let addr: Ipv4Addr = pip
        .parse()
        .map_err(|_| format!("`{pip}` is not an IPv4 address."))?;
    let prefix = prefix_of(pmask).ok_or_else(|| format!("`{pmask}` is not a subnet mask."))?;
    let mut i = before.unwrap_or_else(|| {
        // A new private interface: the first link the open one is not on.
        let open = rq.app.console.interfaces();
        let used: Vec<String> = open.iter().map(|s| s.device.clone()).collect();
        let free = rq
            .app
            .store()
            .load::<state::Inventory>()
            .ok()
            .flatten()
            .and_then(|inv| inv.interfaces.into_iter().find(|n| !used.contains(&n.name)))
            .map_or(Device::Auto, |n| Device::Name(n.name));
        Interface {
            role: PRIVATE.into(),
            device: free,
            carries_route: false,
            carries_dns: false,
            carries_time: false,
            ..Interface::default()
        }
    });
    i.mode = Mode::Static;
    i.address = Some(Cidr { addr, prefix });
    i.gateway = None;
    Ok(Some(i))
}

#[allow(clippy::too_many_lines, reason = "one form, field by field")]
fn post(rq: &Rq<'_>) -> Response {
    let f = rq.req.form();
    let app = rq.app;
    if f.get("cancel").is_some() {
        return Response::redirect(&rq.ctx.url("/console"));
    }
    let field = |k: &str| f.get(k).unwrap_or("").trim().to_string();
    // What the design offers and this release does not do.
    let mut later = Vec::new();
    if f.get("ssh").is_some() {
        later.push("SSH to the console");
    }
    if f.get("dhcp").is_some() {
        later.push("A DHCP server on the private network");
    }
    if !field("d").is_empty() {
        later.push("A DNS domain for the console");
    }
    if !later.is_empty() {
        let messages = later
            .into_iter()
            .map(|w| qmcmsg::message(Kind::Error, &qmcmsg::NOT_IN_V1, &[w]))
            .collect();
        return show(rq, messages);
    }
    let before = view(rq, Vec::new());
    let network = app
        .store()
        .load::<Network>()
        .ok()
        .flatten()
        .unwrap_or_default();
    let mut next = network.clone();
    let mut said = Vec::new();
    // The open interface: its address waits; its name servers do not.
    if field("ip") != before.ip || field("mask") != before.mask || field("gw") != before.gateway {
        rq.audit(
            "change the open interface's address",
            "refused: waits for a confirmed trial",
        );
        // BLOCKED(phase3): a change to the open address needs netd's confirmed trial (OPEN_QUESTIONS item 24)
        return show(
            rq,
            vec![qmcmsg::message(
                Kind::Error,
                &qmcmsg::NETWORK_OWN_ADDRESS,
                &[],
            )],
        );
    }
    let dns: Result<Vec<IpAddr>, String> = list(&field("dns"))
        .iter()
        .map(|d| d.parse().map_err(|_| format!("`{d}` is not an address.")))
        .collect();
    let dns = match dns {
        Ok(d) => d,
        Err(e) => return show(rq, vec![not_valid(&e)]),
    };
    if let Some(open) = next.interfaces.first_mut() {
        open.dns = dns;
    }
    let private = match private_of(
        rq,
        configured(&network, PRIVATE),
        &field("pip"),
        &field("pmask"),
    ) {
        Ok(p) => p,
        Err(e) => return show(rq, vec![not_valid(&e)]),
    };
    next.interfaces.retain(|i| i.role != PRIVATE);
    next.interfaces.extend(private);
    // The addresses the web interface answers.
    let allow = list(&field("allow"));
    let mut settings = app.settings();
    if allow != settings.allow {
        settings.allow = allow;
        let done = app.change_settings(settings);
        rq.audit(
            "change Allow web UI from",
            crate::user_routes::outcome(&done),
        );
        if let Err(e) = done {
            return show(rq, vec![not_valid(&e)]);
        }
        said.push("the addresses the web interface answers");
    }
    // The host name.
    let name = field("h");
    if name != before.host {
        let host = app.host();
        let done = app
            .console
            .set_host(&name, &host.time_zone, &host.ntp_servers);
        rq.audit(
            &format!("change the host name to {name}"),
            crate::user_routes::outcome(&done),
        );
        if let Err(e) = done {
            return show(rq, vec![not_valid(&e)]);
        }
        said.push("the host name");
    }
    // The network, on trial.
    let mut messages = Vec::new();
    if next != network {
        let text = match toml_of(&next) {
            Ok(t) => t,
            Err(e) => return show(rq, vec![not_valid(&e)]),
        };
        let done = app.console.apply_network(&text);
        rq.audit(
            "change the network (name servers, private interface)",
            crate::user_routes::outcome(&done),
        );
        match done {
            Ok(()) => messages.push(qmcmsg::message(
                Kind::Info,
                &qmcmsg::NETWORK_TRYING,
                &["30"],
            )),
            Err(e) => {
                return show(
                    rq,
                    vec![qmcmsg::message(
                        Kind::Error,
                        &qmcmsg::NETWORK_RESTORED,
                        &[&e],
                    )],
                );
            }
        }
    } else if !said.is_empty() {
        messages.push(qmcmsg::message(Kind::Done, &qmcmsg::NETWORK_SAVED, &[]));
    }
    if f.get("ok").is_some() && messages.iter().all(|m| m.kind != Kind::Error) {
        if let Some(m) = messages.pop() {
            return rq.done("/console", m);
        }
        return Response::redirect(&rq.ctx.url("/console"));
    }
    show(rq, messages)
}

/// `network.toml`'s text of `n`, checked as the network service will.
fn toml_of(n: &Network) -> Result<String, String> {
    use state::Document as _;
    n.validate()?;
    let dir = std::env::temp_dir().join(format!("qsmc-net-{}", std::process::id()));
    let store = state::Store::at(&dir);
    store.save(n).map_err(|e| e.to_string())?;
    let text = std::fs::read_to_string(store.path::<Network>()).map_err(|e| e.to_string());
    let _ = std::fs::remove_dir_all(&dir);
    text
}

/// `/console/network`.
pub fn network(rq: &Rq<'_>) -> Response {
    if rq.post() {
        return post(rq);
    }
    show(rq, Vec::new())
}
