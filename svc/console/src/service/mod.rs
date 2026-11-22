// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! Option 8, Service functions (`docs/LOCALCONSOLE.md`): the console's
//! own problems, a console dump, its dumps copied to a removable volume
//! or deleted, an update installed from a removable volume, its code
//! levels. Everything is the console's own: the systems it manages are
//! serviced from the web interface.

pub mod dumps;
pub mod problems;
pub mod update;
pub mod volume;

use console::Screen;
use panels::ui;
use panels::{Flow, Help, Named as _, Panel, PanelFlow, kit};

use crate::env::Env;
use crate::menu::Outcome;

type Step = PanelFlow<Env, Outcome>;

/// The menu's title.
pub const TITLE: &str = "Service Functions";

/// A service that did not answer, said as a person reads it.
pub(crate) fn down(who: &str, e: &dyn std::fmt::Display) -> String {
    format!("The {who} does not answer: {e}.")
}

/// The first message of a refusal, its text.
pub(crate) fn said(issued: &[msg::Issued]) -> String {
    issued.first().map_or_else(
        || "It was refused.".to_string(),
        |i| format!("{} {}", i.id, i.text),
    )
}

/// A time as the display zone shows it; `-` for none.
pub(crate) fn when(env: &Env, secs: u64) -> String {
    if secs == 0 {
        "-".into()
    } else {
        env.zone().display(i64::try_from(secs).unwrap_or(i64::MAX))
    }
}

/// Option 8's menu.
pub struct ServiceMenu;

impl Panel<Env, Outcome> for ServiceMenu {
    fn name(&self) -> &'static str {
        "console-service"
    }

    fn render(&self, env: &mut Env) -> Screen {
        ui::menu(
            TITLE,
            &env.system(),
            &[
                (1, "Work with problems"),
                (2, "Take a console dump"),
                (3, "Work with dumps"),
                (4, "Install an update from a removable volume"),
                (5, "Display code levels"),
            ],
            "F1=Help   F3=Exit   F12=Cancel",
        )
    }

    fn enter(&mut self, fields: &[String], _env: &mut Env) -> Step {
        match kit::number(fields.first()) {
            Some(1) => Flow::Push(Box::new(problems::Problems::default())),
            Some(2) => Flow::Push(Box::new(TakeDump)),
            Some(3) => Flow::Push(Box::new(dumps::Dumps::default())),
            Some(4) => Flow::Push(Box::new(update::FromVolume)),
            Some(5) => Flow::Push(Box::new(update::Levels)),
            Some(_) => Flow::Error("Selection not valid.".into()),
            None => Flow::Error("Type the number of an option.".into()),
        }
    }

    fn function(&mut self, key: u8, _fields: &[String], _env: &mut Env) -> Step {
        match key {
            3 => Flow::PopAll,
            12 => Flow::Pop,
            _ => Flow::Stay,
        }
    }

    fn help(&self) -> Help {
        Help {
            title: TITLE,
            lines: &[
                "Service functions are the console's own: what a service",
                "provider asks for when the console itself has a problem.",
                "",
                "1 lists the console's problems, shows one whole and closes it.",
                "2 takes a console dump: logs, problems, state and the services'",
                "status, every secret left out. 3 copies a dump to a removable",
                "volume, or deletes it. 4 installs an update from a removable",
                "volume into the other code slot, then restarts now or later.",
                "5 shows both code slots and how the last update ended.",
                "",
                "The systems the console manages are serviced from the web",
                "interface, not here.",
            ],
        }
    }
}

/// Option 8, 2: a console dump, asked of PID 1.
pub struct TakeDump;

impl Panel<Env, Outcome> for TakeDump {
    fn name(&self) -> &'static str {
        "console-dump-take"
    }

    fn render(&self, env: &mut Env) -> Screen {
        kit::page(
            "Take a Console Dump",
            &env.system(),
            &kit::lines(&[
                "A console dump holds the logs, the problem log, the state",
                "and each service's status, every secret left out: the",
                "console's keys, the users' password hashes and the managed",
                "systems' pins. It is written in the background; option 3",
                "lists it.",
                "",
                "Press F10 to take it, or F12 to return without it.",
            ]),
            "F1=Help   F10=Take dump   F12=Cancel",
        )
    }

    fn enter(&mut self, _fields: &[String], _env: &mut Env) -> Step {
        Flow::Stay
    }

    fn function(&mut self, key: u8, _fields: &[String], env: &mut Env) -> Step {
        match key {
            10 => match env.take_dump() {
                Ok(name) => {
                    println!("console dump {name} taken by {}", env.user);
                    Flow::Unwind(1, Some((format!("Console dump {name} begun."), false)))
                }
                Err(e) => Flow::Error(crate::env::sentence(&e)),
            },
            3 => Flow::PopAll,
            12 => Flow::Pop,
            _ => Flow::Stay,
        }
    }

    fn help(&self) -> Help {
        Help {
            title: "Take a Console Dump",
            lines: &[
                "F10 asks PID 1 for a dump of the console's service data. One",
                "is taken at a time; the oldest is put out when four are kept",
                "or they take 1 GiB together.",
                "",
                "F12 goes back and nothing is taken.",
            ],
        }
    }
}
