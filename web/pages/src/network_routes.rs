// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! Serving Network settings. The `open` interface's address, mask and
//! gateway are the ones the browser came in on: a change to them is
//! said first, then put on the network service's confirmed trial and
//! kept only when someone signs on at the new address in time
//! (`app::routes`). Its name
//! servers, the host name, the `private` interface and the addresses
//! the web interface answers are changed here; a network change goes
//! to the network service on trial.

use std::fmt::Write as _;
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
    // The open interface's address is the one the browser came in on:
    // the page says what happens first, then it goes on a confirmed
    // trial.
    let own =
        field("ip") != before.ip || field("mask") != before.mask || field("gw") != before.gateway;
    if own {
        if let Err(e) = set_open(&mut next, &field("ip"), &field("mask"), &field("gw")) {
            return show(rq, vec![not_valid(&e)]);
        }
        if f.get("confirmed").is_none() {
            return confirm_own(rq, &f, &field("ip"));
        }
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
        let done = if own {
            app.console.apply_network_confirmed(&text, WINDOW)
        } else {
            app.console.apply_network(&text)
        };
        rq.audit(
            if own {
                "change the open interface's address, kept only when reached again"
            } else {
                "change the network (name servers, private interface)"
            },
            crate::user_routes::outcome(&done),
        );
        match done {
            Ok(()) if own => {
                let at = field("ip");
                *app.pending
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner) =
                    Some((at.clone(), app.console.now() + u64::from(WINDOW)));
                // Nothing here is reached once the address moves: the
                // page says where to go.
                return show(
                    rq,
                    vec![qmcmsg::message(
                        Kind::Info,
                        &qmcmsg::NETWORK_TRYING,
                        &[&WINDOW.to_string()],
                    )],
                );
            }
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

/// Seconds a change of the `open` address waits to be confirmed.
pub const WINDOW: u32 = 120;

/// Put the form's address, mask and gateway on the `open` interface:
/// an empty address is DHCP.
fn set_open(next: &mut Network, ip: &str, mask: &str, gw: &str) -> Result<(), String> {
    let open = next
        .interfaces
        .first_mut()
        .ok_or("There is no open interface.")?;
    if ip.is_empty() {
        open.mode = Mode::Dhcp;
        open.address = None;
        open.gateway = None;
        return Ok(());
    }
    let addr: Ipv4Addr = ip
        .parse()
        .map_err(|_| format!("`{ip}` is not an IPv4 address."))?;
    let prefix = prefix_of(mask).ok_or_else(|| format!("`{mask}` is not a subnet mask."))?;
    open.mode = Mode::Static;
    open.address = Some(Cidr { addr, prefix });
    open.gateway = if gw.is_empty() {
        None
    } else {
        Some(
            gw.parse()
                .map_err(|_| format!("`{gw}` is not an IPv4 address."))?,
        )
    };
    Ok(())
}

/// What a change of the `open` address does, said before it is made:
/// the form again, carried in hidden fields, with OK.
fn confirm_own(rq: &Rq<'_>, f: &qsweb::http::Form, ip: &str) -> Response {
    use qsweb::html::esc;
    let fields = ["ip", "mask", "gw", "dns", "h", "d", "pip", "pmask", "allow"];
    let mut hidden = String::new();
    for k in fields {
        let _ = write!(
            hidden,
            "<input type=\"hidden\" name=\"{k}\" value=\"{}\">",
            esc(f.get(k).unwrap_or(""))
        );
    }
    hidden.push_str("<input type=\"hidden\" name=\"confirmed\" value=\"yes\">");
    let at = if ip.is_empty() {
        "the address the DHCP server gives".to_string()
    } else {
        format!("https://{ip}{}", rq.ctx.url("/signon"))
    };
    let note = format!(
        "<p class=\"small grey\">The change is made at once and this page stops answering. Sign on at {} within {WINDOW} seconds to keep it; otherwise it is taken back and the console answers at its old address again.</p>",
        esc(&at)
    );
    let body = qsweb::pages::confirm_body(
        &rq.ctx,
        "You are changing the address of the interface your browser came in on. Press OK to make the change on trial, or Cancel to return without changing it.",
        &note,
        &hidden,
    );
    rq.page(qsweb::frame::Page {
        title: "Confirm Network Change".into(),
        crumb: vec![
            "Console Management".into(),
            "Network settings".into(),
            "Confirm Network Change".into(),
        ],
        h1: "Confirm Network Change".into(),
        body,
        help: "network".into(),
        nav: Some(("Console Management", Some("/console/network"))),
        ..qsweb::frame::Page::default()
    })
}
