// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! Console version (`console-version.html`): the release, its build
//! and fixes, the kernel, the slot, and the machine from the inventory.

use qsweb::frame::{Ctx, Page};
use qsweb::html::esc;
use qsweb::msg::Message;
use qsweb::partials::{Button, Prop, PropGroup, link, mono, props};

/// What the page shows.
#[derive(Clone, Debug, Default)]
pub struct Version {
    /// `V1R0M0`.
    pub release: String,
    /// The build's identifier.
    pub build: String,
    /// When it was built, formatted; empty when not recorded.
    pub built: String,
    /// The fixes installed, comma-separated; `None` when there are none.
    pub fixes: String,
    /// The kernel.
    pub kernel: String,
    /// The web server.
    pub web: String,
    /// `41 days 3:12 (since 09/19/26 04:02)`.
    pub uptime: String,
    /// The machine.
    pub platform: String,
    /// Its processors.
    pub processors: String,
    /// Its memory.
    pub memory: String,
    /// Its disk.
    pub disk: String,
    /// Its interfaces with their roles.
    pub interfaces: String,
    /// The serial console.
    pub serial: String,
    /// The message rows.
    pub messages: Vec<Message>,
}

/// The page.
pub fn page(ctx: &Ctx, v: &Version) -> Page {
    let build = if v.built.is_empty() {
        mono(&v.build)
    } else {
        format!("{} ({})", mono(&v.build), esc(&v.built))
    };
    let qsmc = PropGroup {
        title: Some("QS/MC".into()),
        rows: vec![
            Prop::new("Version", mono(&v.release)),
            Prop::new("Build", build),
            Prop::new(
                "Installed fixes",
                format!(
                    "{} &nbsp; {}",
                    mono(&v.fixes),
                    link(&ctx.url("/updates/code"), "Code updates")
                ),
            ),
            Prop::new("Kernel", mono(&v.kernel)),
            Prop::new("Web UI", mono(&v.web)),
            Prop::new("Uptime", esc(&v.uptime)),
        ],
    };
    let hardware = PropGroup {
        title: Some("Hardware".into()),
        rows: vec![
            Prop::new("Platform", esc(&v.platform)),
            Prop::new("Processors", esc(&v.processors)),
            Prop::new("Memory", esc(&v.memory)),
            Prop::new("Disk", esc(&v.disk)),
            Prop::new("Interfaces", esc(&v.interfaces)),
            Prop::new("Serial console", v.serial.clone()),
        ],
    };
    let body = props(ctx, &[qsmc, hardware], &[Button::new("refresh", "Refresh")]);
    Page {
        title: "Console version".into(),
        crumb: vec!["Console Management".into(), "Console version".into()],
        h1: "Console version".into(),
        messages: v.messages.clone(),
        body,
        help: "version".into(),
        nav: Some(("Console Management", Some("/console/version"))),
        ..Page::default()
    }
}

/// Words the inventory's manufacturer or product has on a virtual
/// machine.
const VIRTUAL: [&str; 8] = [
    "QEMU",
    "KVM",
    "VMware",
    "VirtualBox",
    "innotek",
    "Xen",
    "Virtual Machine",
    "Bochs",
];

/// The Platform line: the machine, or that it is a virtual one and on
/// what, with its serial number.
pub fn platform(manufacturer: &str, product: &str, serial: &str) -> String {
    let what = [manufacturer, product]
        .iter()
        .filter(|s| !s.is_empty())
        .copied()
        .collect::<Vec<_>>()
        .join(" ");
    let what = if what.is_empty() {
        "not known".to_string()
    } else {
        what
    };
    let virt = VIRTUAL.iter().any(|w| {
        manufacturer.to_lowercase().contains(&w.to_lowercase())
            || product.to_lowercase().contains(&w.to_lowercase())
    });
    let serial = if serial.is_empty() {
        String::new()
    } else {
        format!(", serial {serial}")
    };
    if virt {
        format!("Virtual machine ({what}){serial}")
    } else {
        format!("{what}{serial}")
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_virtual_machine_says_so() {
        assert_eq!(
            super::platform("QEMU", "Standard PC (Q35 + ICH9, 2009)", ""),
            "Virtual machine (QEMU Standard PC (Q35 + ICH9, 2009))"
        );
        assert_eq!(
            super::platform("Supermicro", "SYS-5019C", "S123"),
            "Supermicro SYS-5019C, serial S123"
        );
    }
}
