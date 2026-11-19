// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! The local console's own panels around `base/panels`' own: the main
//! menu, a choice of two or three, a place a later phase fills, and the
//! idle screen shown while nobody is signed on. The numbers of the menu
//! keep the places QS/VS's Service Tools gives the same things.

use console::Screen;
use panels::tools::{Confirm, Host as _, Log, LogPanel, TimePanel, UsersPanel};
use panels::ui;
use panels::{Flow, Help, Named, Panel, PanelFlow, kit};
use supervise::control::HostStatus;

use crate::env::Env;

/// How a session ends.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// The user signed off.
    SignOff,
}

/// What a panel of a session answers.
pub type Step = PanelFlow<Env, Outcome>;

/// The main menu's options.
pub const OPTIONS: [(u8, &str); 9] = [
    (1, "Display console status"),
    (2, "Work with disk units"),
    (3, "Configure network"),
    (4, "Work with the web interface"),
    (5, "Work with service tools user IDs"),
    (6, "Date and time"),
    (7, "Display logs"),
    (8, "Service functions"),
    (9, "Power off or restart"),
];

/// The main menu.
pub struct MainMenu;

impl Panel<Env, Outcome> for MainMenu {
    fn name(&self) -> &'static str {
        "console-main"
    }

    fn render(&self, env: &mut Env) -> Screen {
        ui::menu(
            release::CONSOLE_TITLE,
            &env.system(),
            &OPTIONS,
            "F1=Help   F3=Sign off   F12=Sign off",
        )
    }

    fn enter(&mut self, fields: &[String], env: &mut Env) -> Step {
        match kit::number(fields.first()) {
            Some(1) => Flow::Push(Box::new(crate::status::Status::default())),
            Some(2) => Flow::Push(Box::new(crate::disks::DiskUnits)),
            Some(3) => Flow::Push(Box::new(crate::network::NetworkMenu)),
            Some(4) => Flow::Push(Box::new(Later::WEB)),
            Some(5) => Flow::Push(Box::new(UsersPanel)),
            Some(6) => Flow::Push(Box::new(TimePanel::new(env))),
            Some(7) => Flow::Push(Box::new(Choice::LOGS)),
            Some(8) => Flow::Push(Box::new(Later::SERVICE)),
            Some(9) => Flow::Push(Box::new(Choice::POWER)),
            Some(_) => Flow::Error("Selection not valid.".into()),
            None => Flow::Error("Type the number of an option.".into()),
        }
    }

    fn function(&mut self, key: u8, _fields: &[String], _env: &mut Env) -> Step {
        match key {
            3 | 12 => Flow::Exit(Outcome::SignOff),
            _ => Flow::Stay,
        }
    }

    fn help(&self) -> Help {
        Help {
            title: release::CONSOLE_TITLE,
            lines: &[
                "This console looks after the management console itself. It",
                "works with no QS/VS host, partition or web user: those are",
                "managed from the web interface.",
                "",
                "Type the number of an option and press Enter. 1 shows what the",
                "console is running and how. 2 shows its load-source disks and",
                "protects or repairs them. 3 sets its open network. 4 is for the",
                "web interface, 8 for service functions: both arrive later. 5",
                "works with the users of this console. 6 sets the clock and the",
                "time servers. 7 shows the logs. 9 powers off or restarts.",
                "",
                "F3 or F12 signs you off. So do ten minutes without a key.",
            ],
        }
    }
}

/// A choice of a few things, each its own panel.
pub struct Choice {
    name: &'static str,
    title: &'static str,
    options: &'static [(u8, &'static str)],
    help: Help,
}

impl Choice {
    /// `Display logs`.
    pub const LOGS: Choice = Choice {
        name: "console-logs",
        title: "Display Logs",
        options: &[(1, "Reference code history"), (2, "System log")],
        help: Help {
            title: "Display Logs",
            lines: &[
                "1 shows the reference code history: every code the console has",
                "posted, kept across IPLs, newest first.",
                "",
                "2 shows the system log: what PID 1 and the services say,",
                "newest first. Its oldest lines go when it is full.",
            ],
        },
    };

    /// `Power off or restart`.
    pub const POWER: Choice = Choice {
        name: "console-power",
        title: "Power Off or Restart",
        options: &[(1, "Power off"), (2, "Restart")],
        help: Help {
            title: "Power Off or Restart",
            lines: &[
                "1 shuts the console down in order and powers it off.",
                "2 shuts it down in order and restarts it.",
                "",
                "Either asks you to confirm first. The QS/VS hosts and the",
                "partitions the console manages run on without it; they cannot",
                "be managed until it is back.",
            ],
        },
    };
}

impl Panel<Env, Outcome> for Choice {
    fn name(&self) -> &'static str {
        self.name
    }

    fn render(&self, env: &mut Env) -> Screen {
        ui::menu(
            self.title,
            &env.system(),
            self.options,
            "F1=Help   F3=Exit   F12=Cancel",
        )
    }

    fn enter(&mut self, fields: &[String], _env: &mut Env) -> Step {
        match (self.name, kit::number(fields.first())) {
            ("console-logs", Some(1)) => Flow::Push(Box::new(LogPanel::new(Log::History))),
            ("console-logs", Some(2)) => Flow::Push(Box::new(LogPanel::new(Log::System))),
            ("console-power", Some(1)) => Flow::Push(Box::new(power(false))),
            ("console-power", Some(2)) => Flow::Push(Box::new(power(true))),
            (_, Some(_)) => Flow::Error("Selection not valid.".into()),
            (_, None) => Flow::Error("Type the number of an option.".into()),
        }
    }

    fn help(&self) -> Help {
        self.help
    }
}

/// An option a later phase fills: what it will hold, and when.
pub struct Later {
    name: &'static str,
    title: &'static str,
    lines: &'static [&'static str],
    help: Help,
}

impl Later {
    /// Option 4.
    pub const WEB: Later = Later {
        name: "console-web",
        title: "Work with the Web Interface",
        lines: &[
            "The web interface is not available yet. It arrives in Phase 3.",
            "",
            "This option will then hold:",
            "  - resetting the password of a web interface Administrator",
            "  - the web server's certificate: showing it and making a new one",
        ],
        help: Help {
            title: "Work with the Web Interface",
            lines: &[
                "The web interface is how the console is used: hosts, partitions",
                "and the console's own settings. Its server is not in this",
                "release of the console yet; it arrives in Phase 3.",
                "",
                "When it is, this option resets an Administrator's password",
                "when nobody can sign on to the web interface, and shows or",
                "replaces the certificate the web server presents.",
                "",
                "Enter, F3 or F12 goes back.",
            ],
        },
    };

    /// Option 8.
    pub const SERVICE: Later = Later {
        name: "console-service",
        title: "Service Functions",
        lines: &[
            "Service functions are not available yet. They arrive in Phase 6.",
            "",
            "This option will then hold the console's own dumps and its code",
            "updates.",
        ],
        help: Help {
            title: "Service Functions",
            lines: &[
                "Service functions are what a service provider asks for: the",
                "console's dumps, and its code updates from the product stream",
                "or a removable volume. They arrive in Phase 6.",
                "",
                "Enter, F3 or F12 goes back.",
            ],
        },
    };
}

impl Panel<Env, Outcome> for Later {
    fn name(&self) -> &'static str {
        self.name
    }

    fn render(&self, env: &mut Env) -> Screen {
        kit::page(
            self.title,
            &env.system(),
            &kit::lines(self.lines),
            "Enter=Continue   F1=Help   F3=Exit   F12=Cancel",
        )
    }

    fn enter(&mut self, _fields: &[String], _env: &mut Env) -> Step {
        Flow::Pop
    }

    fn help(&self) -> Help {
        self.help
    }
}

/// The confirmation of a power off or a restart: `base/panels`' own
/// confirmation with the console's words (its `power` speaks of a
/// host's partitions).
pub fn power(restart: bool) -> Confirm<Env, Outcome> {
    let (name, title, first) = if restart {
        (
            "console-restart",
            "Confirm Restart",
            "The console will shut down in order and restart.",
        )
    } else {
        (
            "console-power-off",
            "Confirm Power Off",
            "The console will shut down in order and power off.",
        )
    };
    Confirm::new(
        name,
        title,
        kit::lines(&[
            first,
            "",
            "Its services are stopped and its disks left clean. The hosts and",
            "partitions it manages run on; they cannot be managed until the",
            "console is back.",
        ]),
        Help {
            title: "Confirm power off or restart",
            lines: &[
                "F10 starts an orderly shutdown: the console's services end one",
                "after another, its data is written and its file system",
                "unmounted, and then the machine powers off or restarts.",
                "",
                "Nothing the console manages is stopped. F12 goes back and",
                "nothing happens.",
            ],
        },
        move |env: &mut Env| match env.power(restart) {
            Ok(()) => Flow::Reset(Box::new(Stopping { restart })),
            Err(e) => Flow::Unwind(1, Some((e, true))),
        },
    )
}

/// The console is shutting down: nothing more to do here.
struct Stopping {
    restart: bool,
}

impl Panel<Env, Outcome> for Stopping {
    fn name(&self) -> &'static str {
        "console-stopping"
    }

    fn render(&self, env: &mut Env) -> Screen {
        let what = if self.restart {
            "The console is restarting."
        } else {
            "The console is powering off."
        };
        kit::page(
            "Power Off or Restart",
            &env.system(),
            &kit::lines(&[what, "", "Its services are being stopped in order."]),
            "",
        )
    }

    fn enter(&mut self, _fields: &[String], _env: &mut Env) -> Step {
        Flow::Stay
    }

    fn function(&mut self, _key: u8, _fields: &[String], _env: &mut Env) -> Step {
        Flow::Stay
    }

    fn help(&self) -> Help {
        Help {
            title: "Power Off or Restart",
            lines: &["The console is stopping. This session ends with it."],
        }
    }
}

/// What the idle screen asks for.
pub const PRESS_ENTER: &str = "Press Enter to sign on.";

/// The idle screen: the status screen's rows, and how to sign on.
pub fn idle(status: &HostStatus) -> Screen {
    let mut lines = release::screen::lines(&release::banner(), status);
    // The banner is the first row; the title says the product.
    lines.remove(0);
    lines.truncate(kit::BODY_ROWS);
    let host = status.fact(release::fact::HOST).unwrap_or(release::NAME);
    let mut s = ui::text(release::CONSOLE_TITLE, host, &lines, None, PRESS_ENTER);
    kit::put(&mut s, 2, &release::banner());
    kit::enterable(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_menu_keeps_service_tools_places() {
        let names: Vec<u8> = OPTIONS.iter().map(|(n, _)| *n).collect();
        assert_eq!(names, [1, 2, 3, 4, 5, 6, 7, 8, 9]);
        assert_eq!(OPTIONS[1].1, "Work with disk units");
        assert_eq!(OPTIONS[4].1, "Work with service tools user IDs");
        assert_eq!(OPTIONS[8].1, "Power off or restart");
    }

    #[test]
    fn the_placeholders_name_their_phases() {
        assert!(Later::WEB.lines[0].contains("Phase 3"));
        assert!(Later::SERVICE.lines[0].contains("Phase 6"));
        for later in [&Later::WEB, &Later::SERVICE] {
            assert!(later.lines.iter().all(|l| l.len() <= 76));
            assert!(later.help.lines.iter().all(|l| l.len() <= 76));
        }
    }

    #[test]
    fn the_idle_screen_shows_the_status_rows_and_asks_for_enter() {
        let status = HostStatus {
            facts: vec![
                (release::fact::HOST.into(), "QSMC123456".into()),
                (release::fact::SLOT.into(), "A".into()),
            ],
            services: Vec::new(),
        };
        let text = idle(&status).dump();
        assert!(text.contains("QS/MC Local Console"));
        assert!(text.contains("System:  QSMC123456"));
        assert!(text.contains("Slot           A"));
        assert!(text.contains(PRESS_ENTER));
    }
}
