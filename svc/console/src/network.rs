// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! `Configure network`: the console's two interfaces by role. `open`,
//! the network operators reach the console on, is changed here with
//! `base/panels`' form, applied by the network service on trial and
//! put back if it does not come up and is not confirmed. `private`,
//! toward the systems the console manages, is shown and not changed:
//! it is set in the web interface's Network settings.

use std::fmt::Write as _;

use console::Screen;
use netd::control::NetStatus;
use panels::tools::NetworkPanel;
use panels::{Flow, Help, Named, Panel, kit, ui};
use state::{Interface, Network};

use crate::env::Env;
use crate::menu::{Outcome, Step};

const LABEL: usize = 17;

/// What a configuration of an interface says, in a few words.
pub fn described(i: &Interface) -> String {
    let mut out = match i.address {
        Some(address) => format!("static {address}"),
        None => "DHCP".to_string(),
    };
    if let Some(gateway) = i.gateway {
        let _ = write!(out, ", gateway {gateway}");
    }
    if let Some(vlan) = i.vlan {
        let _ = write!(out, ", VLAN {vlan}");
    }
    match &i.device {
        state::Device::Auto => out.push_str(" on the first link with carrier"),
        device => {
            let _ = write!(out, " on {device}");
        }
    }
    out
}

/// The lines above the options: each role, configured and as it is.
pub fn lines(network: &Network, live: &[NetStatus]) -> Vec<String> {
    let row = |label: &str, value: &str| format!("{} {value}", kit::dotted(label, LABEL));
    let line_of = |role: &str| {
        live.iter()
            .find(|s| s.role == role || (s.role.is_empty() && role == "open"))
            .map_or_else(|| "not up".to_string(), |s| s.line.clone())
    };
    let mut out = vec![
        row("Open, configured", &described(network.primary())),
        row("Open, now", &line_of("open")),
    ];
    match network.interfaces.iter().find(|i| i.role == "private") {
        Some(private) => {
            out.push(row("Private, config.", &described(private)));
            out.push(row("Private, now", &line_of("private")));
        }
        None => out.push(row("Private", "none configured")),
    }
    out.push(String::new());
    out.push("The private interface is set in the web interface's Network".into());
    out.push("settings (Phase 3) or by a seed volume; it is shown here only.".into());
    out
}

/// `Configure network`.
pub struct NetworkMenu;

impl Panel<Env, Outcome> for NetworkMenu {
    fn name(&self) -> &'static str {
        "console-network"
    }

    fn render(&self, env: &mut Env) -> Screen {
        let network = env.network();
        let live = env.interfaces();
        ui::menu_after(
            "Configure Network",
            &env.system(),
            &lines(&network, &live),
            &[(1, "Change the open interface")],
            "F1=Help   F3=Exit   F12=Cancel",
        )
    }

    fn enter(&mut self, fields: &[String], env: &mut Env) -> Step {
        match kit::number(fields.first()) {
            Some(1) => Flow::Push(Box::new(NetworkPanel::new(env))),
            Some(_) => Flow::Error("Selection not valid.".into()),
            None => Flow::Error("Type the number of an option.".into()),
        }
    }

    fn help(&self) -> Help {
        Help {
            title: "Configure Network",
            lines: &[
                "The console has two network roles. open is the network you",
                "reach the console on: it has the default route, the name",
                "servers and the time. private is optional and faces the hosts",
                "the console manages; it has no default route.",
                "",
                "1 changes the open interface: DHCP or a static address, its",
                "VLAN and name servers. The change is tried first and put back",
                "by itself if the interface does not come up within 30 seconds",
                "and you have not kept it. Nothing restarts.",
                "",
                "The private interface is shown here and changed in the web",
                "interface's Network settings.",
            ],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn both_roles_are_shown_and_private_is_read_only() {
        let network: Network = state::store::parse(
            "schema_version = 2\n[[interface]]\nrole = \"open\"\n\
             [[interface]]\nrole = \"private\"\ndevice = \"eth1\"\nmode = \"static\"\n\
             address = \"198.51.100.1/24\"\ncarries_route = false\n",
        )
        .unwrap();
        let live = [NetStatus {
            role: "open".into(),
            line: "10.0.2.15/24 on eth0, gateway 10.0.2.2".into(),
            ..NetStatus::default()
        }];
        let shown = lines(&network, &live);
        assert!(
            shown[0].ends_with("DHCP on the first link with carrier"),
            "{}",
            shown[0]
        );
        assert!(shown[1].ends_with("gateway 10.0.2.2"));
        assert!(
            shown[2].ends_with("static 198.51.100.1/24 on eth1"),
            "{}",
            shown[2]
        );
        assert!(shown[3].ends_with("not up"));
        assert!(shown.iter().any(|l| l.contains("web interface")));
        assert!(shown.iter().all(|l| l.len() <= 76));
        let one = lines(&Network::default(), &[]);
        assert!(one[2].ends_with("none configured"));
    }
}
