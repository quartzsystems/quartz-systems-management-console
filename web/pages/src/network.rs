// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! Network settings (`console-network.html`): the `open` network
//! (operators, the web interface), the `private` one (the managed
//! systems), and the rest. An empty IP address is DHCP; the VLAN and
//! listening on `private` are kept as `network.toml` and `web.toml`
//! have them (`docs/OPEN_QUESTIONS.md`, item 25).

use std::net::Ipv4Addr;

use qsweb::frame::{Ctx, Page};
use qsweb::html::esc;
use qsweb::msg::Message;
use qsweb::partials::{Button, Prop, PropGroup, checkbox, input, props};

/// One interface's line: what it is, and how it stands.
#[derive(Clone, Debug, Default)]
pub struct Link {
    /// Its location code or name.
    pub name: String,
    /// `1 GbE, link up`.
    pub state: String,
}

/// What Network settings shows.
#[derive(Clone, Debug, Default)]
pub struct Net {
    /// The `open` interface.
    pub open: Link,
    /// Its address, empty with DHCP.
    pub ip: String,
    /// Its mask, dotted.
    pub mask: String,
    /// Its gateway.
    pub gateway: String,
    /// Its name servers, comma-separated.
    pub dns: String,
    /// The host name.
    pub host: String,
    /// The domain.
    pub domain: String,
    /// The `private` interface.
    pub private: Link,
    /// Its address.
    pub pip: String,
    /// Its mask.
    pub pmask: String,
    /// The DHCP server's range, as the page says it.
    pub range: String,
    /// Whether the DHCP server is on.
    pub dhcp: bool,
    /// The managed systems reached on `private`.
    pub reached: String,
    /// The addresses the web interface answers.
    pub allow: String,
    /// Whether SSH is on.
    pub ssh: bool,
    /// The message rows.
    pub messages: Vec<Message>,
}

fn link(l: &Link) -> String {
    format!(
        "<span class=\"mono\">{}</span> ({})",
        esc(&l.name),
        esc(&l.state)
    )
}

/// The page.
pub fn page(ctx: &Ctx, v: &Net) -> Page {
    let open = PropGroup {
        title: Some("Public network (operators, web UI)".into()),
        rows: vec![
            Prop::new("Interface", link(&v.open)),
            Prop::new("IP address", input("ip", &v.ip, 16)),
            Prop::new("Subnet mask", input("mask", &v.mask, 16)),
            Prop::new("Gateway", input("gw", &v.gateway, 16)),
            Prop::new("DNS servers", input("dns", &v.dns, 30)),
            Prop::new("Host name", input("h", &v.host, 20)),
            Prop::new("Domain", input("d", &v.domain, 20)),
            Prop::new("Web UI ports", "443 (HTTPS), 80 redirects".into()),
        ],
    };
    let private = PropGroup {
        title: Some("Private management network (managed systems' service interfaces)".into()),
        rows: vec![
            Prop::new("Interface", link(&v.private)),
            Prop::new("IP address", input("pip", &v.pip, 16)),
            Prop::new("Subnet mask", input("pmask", &v.pmask, 16)),
            Prop::new(
                "DHCP for service interfaces",
                format!(
                    "{} Serve {} to managed systems",
                    checkbox("dhcp", v.dhcp),
                    esc(&v.range)
                ),
            ),
            Prop::new("Managed systems reached here", esc(&v.reached)),
        ],
    };
    let other = PropGroup {
        title: Some("Other".into()),
        rows: vec![
            Prop::new("Allow web UI from", input("allow", &v.allow, 40)),
            Prop::new(
                "SSH to console (serial login equivalent)",
                format!("{} Enable on port 22", checkbox("ssh", v.ssh)),
            ),
        ],
    };
    let body = props(
        ctx,
        &[open, private, other],
        &[
            Button::new("apply", "Apply"),
            Button::new("ok", "OK"),
            Button::new("cancel", "Cancel"),
        ],
    );
    Page {
        title: "Network settings".into(),
        crumb: vec!["Console Management".into(), "Network settings".into()],
        h1: "Network settings".into(),
        messages: v.messages.clone(),
        body,
        help: "network".into(),
        nav: Some(("Console Management", Some("/console/network"))),
        ..Page::default()
    }
}

/// The dotted mask of a prefix length.
pub fn mask_of(prefix: u8) -> String {
    let bits = if prefix == 0 {
        0
    } else {
        u32::MAX << (32 - u32::from(prefix.min(32)))
    };
    Ipv4Addr::from(bits).to_string()
}

/// The prefix length of a dotted mask, when it is one.
pub fn prefix_of(mask: &str) -> Option<u8> {
    let bits = u32::from(mask.trim().parse::<Ipv4Addr>().ok()?);
    let ones = bits.leading_ones();
    (bits.checked_shl(ones).unwrap_or(0) == 0 && ones > 0)
        .then(|| u8::try_from(ones).ok())
        .flatten()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn masks_and_prefixes() {
        assert_eq!(mask_of(24), "255.255.255.0");
        assert_eq!(mask_of(32), "255.255.255.255");
        assert_eq!(prefix_of("255.255.255.0"), Some(24));
        assert_eq!(prefix_of("255.255.0.255"), None);
        assert_eq!(prefix_of("0.0.0.0"), None);
    }
}
