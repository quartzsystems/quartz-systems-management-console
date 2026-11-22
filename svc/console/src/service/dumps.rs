// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! Option 8, 3: the console's own dumps, copied to a removable volume or
//! deleted.

use console::Screen;
use panels::ui::{self, FormLine};
use panels::{Flow, Help, Named as _, Panel, PanelFlow, kit};
use servicedump::{Info, Store};

use super::{volume, when};
use crate::env::Env;
use crate::menu::Outcome;

type Step = PanelFlow<Env, Outcome>;

const TITLE: &str = "Work with Dumps";
const ROWS: usize = kit::BODY_ROWS - 4;

fn store() -> Store {
    consdump::store()
}

/// `Work with Dumps`.
#[derive(Default)]
pub struct Dumps {
    top: usize,
}

impl Panel<Env, Outcome> for Dumps {
    fn name(&self) -> &'static str {
        "console-dumps"
    }

    fn render(&self, env: &mut Env) -> Screen {
        let all = store().list();
        let rows: Vec<String> = all
            .iter()
            .rev()
            .map(|d| {
                format!(
                    "{}  {}  {:>10}  {}",
                    kit::fit(&d.name, 30),
                    kit::fit(&when(env, d.time), 16),
                    kit::size(d.bytes),
                    kit::fit(&d.by, 14)
                )
            })
            .collect();
        let (shown, foot) = kit::window(&rows, self.top, ROWS);
        let mut s = ui::list(
            TITLE,
            &env.system(),
            "3=Copy to a removable volume  4=Delete",
            "Opt Name                            Taken                   Size  By",
            &shown,
            "F1=Help  F3=Exit  F5=Refresh  F12=Cancel",
        );
        kit::put(&mut s, ui::BODY_ROW + 4 + ROWS, &foot);
        if rows.is_empty() {
            kit::put(
                &mut s,
                ui::BODY_ROW + 3,
                "(No console dump is kept. Option 2 takes one.)",
            );
        }
        s
    }

    fn enter(&mut self, fields: &[String], _env: &mut Env) -> Step {
        let all: Vec<Info> = store().list().into_iter().rev().collect();
        let chosen: Vec<(&Info, u32)> = all
            .iter()
            .skip(self.top)
            .take(ROWS)
            .zip(fields)
            .filter_map(|(d, f)| Some((d, kit::number(Some(f))?)))
            .collect();
        let [(d, option)] = chosen.as_slice() else {
            return if chosen.is_empty() {
                Flow::Stay
            } else {
                Flow::Error("Type an option by one dump.".into())
            };
        };
        match option {
            3 => Flow::Push(Box::new(Copy {
                name: d.name.clone(),
            })),
            4 => Flow::Push(Box::new(Delete {
                name: d.name.clone(),
            })),
            _ => Flow::Error("Option not valid.".into()),
        }
    }

    fn function(&mut self, key: u8, _fields: &[String], _env: &mut Env) -> Step {
        match key {
            3 => Flow::PopAll,
            12 => Flow::Pop,
            7 | 8 => {
                self.top = kit::scroll(self.top, key, store().list().len(), ROWS);
                Flow::Stay
            }
            _ => Flow::Stay,
        }
    }

    fn help(&self) -> Help {
        Help {
            title: TITLE,
            lines: &[
                "The console's dumps, newest first: each a ZIP archive a",
                "standard tool opens. Type 3 by one to copy it to a removable",
                "volume, 4 to delete it. The managed systems' dumps are on the",
                "web interface's Manage dumps.",
            ],
        }
    }
}

/// `Copy Dump to a Removable Volume`.
struct Copy {
    name: String,
}

impl Panel<Env, Outcome> for Copy {
    fn name(&self) -> &'static str {
        "console-dump-copy"
    }

    fn render(&self, env: &mut Env) -> Screen {
        let mut s = kit::form(
            "Copy Dump to a Removable Volume",
            &env.system(),
            &[FormLine::new("Device", 12, "")],
            "F1=Help   F3=Exit   F12=Cancel",
        );
        kit::put(&mut s, ui::BODY_ROW + 3, &format!("Dump: {}", self.name));
        kit::put(
            &mut s,
            ui::BODY_ROW + 4,
            "Type the volume's device, as sdb1, and press Enter.",
        );
        s
    }

    fn enter(&mut self, fields: &[String], env: &mut Env) -> Step {
        let device = fields.first().map_or("", |f| f.trim()).to_string();
        let Some(from) = store().path(&self.name) else {
            return Flow::Error(format!("Dump {} is no longer kept.", self.name));
        };
        let mounted = match volume::mount(&device, true) {
            Ok(m) => m,
            Err(e) => return Flow::Error(e),
        };
        let to = mounted.path().join(&self.name);
        match std::fs::copy(&from, &to) {
            Ok(n) => {
                drop(mounted);
                println!("dump {} copied to {device} by {}", self.name, env.user);
                Flow::Unwind(
                    1,
                    Some((
                        format!("Dump {} copied to {device}, {}.", self.name, kit::size(n)),
                        false,
                    )),
                )
            }
            Err(e) => Flow::Error(format!("The dump was not copied: {e}.")),
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
            title: "Copy Dump to a Removable Volume",
            lines: &[
                "Type the kernel's name of the volume (sdb1, or sdb for a",
                "volume without partitions) and press Enter: the dump is copied",
                "to its top folder and the volume is unmounted. A load-source",
                "disk is never written here. F12 goes back.",
            ],
        }
    }
}

/// `Confirm Delete Dump`.
struct Delete {
    name: String,
}

impl Panel<Env, Outcome> for Delete {
    fn name(&self) -> &'static str {
        "console-dump-delete"
    }

    fn render(&self, env: &mut Env) -> Screen {
        kit::page(
            "Confirm Delete Dump",
            &env.system(),
            &[
                format!("Dump {} is deleted and cannot be had again.", self.name),
                String::new(),
                "Press F10 to delete it, or F12 to return.".into(),
            ],
            "F1=Help   F10=Delete   F12=Cancel",
        )
    }

    fn enter(&mut self, _fields: &[String], _env: &mut Env) -> Step {
        Flow::Stay
    }

    fn function(&mut self, key: u8, _fields: &[String], env: &mut Env) -> Step {
        match key {
            10 => match store().delete(&self.name) {
                Ok(()) => {
                    env.post(
                        &refcode::DUMP_REMOVED,
                        &format!("{}, by {}", self.name, env.user),
                    );
                    Flow::Unwind(1, Some((format!("Dump {} deleted.", self.name), false)))
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
            title: "Confirm Delete Dump",
            lines: &["F10 deletes the dump. F12 goes back and nothing changes."],
        }
    }
}
