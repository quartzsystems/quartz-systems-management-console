// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! Option 8, 4: an update installed from a removable volume, through the
//! console's update service (`base/update`): the image chosen, staged
//! and verified, shown beside what runs, installed into the other slot,
//! then a restart now or later. Option 8, 5: the code levels.

use console::Screen;
use panels::ui::{self, FormLine};
use panels::{Flow, Help, Named as _, Panel, PanelFlow, kit};
use update::proto::{Attempt, Calls as _, Staged, Status, Verification};

use super::{down, said, volume, when};
use crate::env::Env;
use crate::menu::Outcome;

type Step = PanelFlow<Env, Outcome>;

const TITLE: &str = "Install an Update from a Removable Volume";

/// `Install an Update from a Removable Volume`: the device.
pub struct FromVolume;

impl Panel<Env, Outcome> for FromVolume {
    fn name(&self) -> &'static str {
        "console-update-volume"
    }

    fn render(&self, env: &mut Env) -> Screen {
        let mut s = kit::form(
            TITLE,
            &env.system(),
            &[FormLine::new("Device", 12, "")],
            "F1=Help   F3=Exit   F12=Cancel",
        );
        kit::put(
            &mut s,
            ui::BODY_ROW + 3,
            "Type the device of the volume that holds the update image,",
        );
        kit::put(&mut s, ui::BODY_ROW + 4, "as sdb1, and press Enter.");
        s
    }

    fn enter(&mut self, fields: &[String], _env: &mut Env) -> Step {
        let device = fields.first().map_or("", |f| f.trim()).to_string();
        match volume::mount(&device, false) {
            Ok(m) => {
                let files = volume::files(m.path(), "upd");
                if files.is_empty() {
                    return Flow::Error(format!("{device} holds no update image (.upd)."));
                }
                Flow::Push(Box::new(Choose {
                    device,
                    mounted: m,
                    files,
                }))
            }
            Err(e) => Flow::Error(e),
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
                "Type the kernel's name of the volume (sdb1) and press Enter;",
                "it is mounted read-only and its update images listed. The",
                "image is verified against the keys built into this release",
                "before anything is written, and installed into the code slot",
                "that does not run. Nothing changes until you restart into it.",
            ],
        }
    }
}

/// The images on the volume.
struct Choose {
    device: String,
    mounted: volume::Mounted,
    files: Vec<String>,
}

impl Panel<Env, Outcome> for Choose {
    fn name(&self) -> &'static str {
        "console-update-choose"
    }

    fn render(&self, env: &mut Env) -> Screen {
        let rows: Vec<String> = self.files.iter().map(|f| kit::fit(f, 70)).collect();
        ui::list(
            "Choose an Update Image",
            &env.system(),
            &format!("1=Select   (images on {})", self.device),
            "Opt File",
            &rows,
            "F1=Help   F3=Exit   F12=Cancel",
        )
    }

    fn enter(&mut self, fields: &[String], env: &mut Env) -> Step {
        let Some(i) = fields.iter().position(|f| kit::number(Some(f)) == Some(1)) else {
            return Flow::Stay;
        };
        let Some(path) = self
            .files
            .get(i)
            .and_then(|f| volume::within(self.mounted.path(), f))
        else {
            return Flow::Error("Option not valid.".into());
        };
        // The service reads the whole image now; the volume may go after.
        match env.updd().stage_file(path.display().to_string(), false) {
            Ok(Ok(staged)) => Flow::Replace(Box::new(Chosen { staged })),
            Ok(Err(issued)) => Flow::Error(said(&issued)),
            Err(e) => Flow::Error(down("update service", &e)),
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
            title: "Choose an Update Image",
            lines: &[
                "Type 1 by the image to install and press Enter: it is copied",
                "to the console and verified there. F12 goes back.",
            ],
        }
    }
}

/// The running slot of `s`.
fn running(s: &Status) -> Option<&update::proto::Slot> {
    s.slots.iter().find(|x| x.running)
}

/// What was staged, beside what runs.
struct Chosen {
    staged: Staged,
}

impl Panel<Env, Outcome> for Chosen {
    fn name(&self) -> &'static str {
        "console-update-chosen"
    }

    fn render(&self, env: &mut Env) -> Screen {
        let line = |label: &str, value: String| format!("{} {value}", kit::dotted(label, 22));
        let st = env.updd().status().unwrap_or_default();
        let now = running(&st);
        let mut lines = vec![line("Image", self.staged.name.clone())];
        match (&self.staged.verification, &self.staged.image) {
            (Verification::Verified, Some(i)) => {
                lines.push(line("Product", i.product.clone()));
                lines.push(line("Release", format!("{} build {}", i.release, i.build)));
                lines.push(line(
                    "Fixes",
                    if i.fixes.is_empty() {
                        "none named".into()
                    } else {
                        i.fixes.join(", ")
                    },
                ));
            }
            _ => lines.push(line("Verified", "no".into())),
        }
        lines.push(String::new());
        lines.push(line(
            "Runs now",
            now.map_or_else(
                || release::RELEASE.to_string(),
                |s| format!("{} build {} in slot {}", s.release, s.build, s.name),
            ),
        ));
        for e in &self.staged.error {
            lines.push(kit::fit(&format!("{} {}", e.id, e.text), 76));
        }
        lines.extend([
            String::new(),
            "F10 installs it into the slot that does not run, on every".into(),
            "load-source disk. Nothing that runs changes until a restart.".into(),
        ]);
        kit::page(
            "Confirm Install Update",
            &env.system(),
            &lines,
            "F1=Help   F10=Install   F12=Cancel",
        )
    }

    fn enter(&mut self, _fields: &[String], _env: &mut Env) -> Step {
        Flow::Stay
    }

    fn function(&mut self, key: u8, _fields: &[String], env: &mut Env) -> Step {
        match key {
            10 if self.staged.verification != Verification::Verified => Flow::Error(format!(
                "{} did not verify: it is not installed.",
                self.staged.name
            )),
            10 => match env.updd().install() {
                Ok(Ok(slot)) => {
                    println!(
                        "update {} installing into slot {slot}, by {}",
                        self.staged.name, env.user
                    );
                    Flow::Replace(Box::new(Installing { slot }))
                }
                Ok(Err(issued)) => Flow::Error(said(&issued)),
                Err(e) => Flow::Error(down("update service", &e)),
            },
            12 | 3 => {
                let _ = env.updd().discard();
                if key == 3 { Flow::PopAll } else { Flow::Pop }
            }
            _ => Flow::Stay,
        }
    }

    fn help(&self) -> Help {
        Help {
            title: "Confirm Install Update",
            lines: &[
                "The image was verified: signed by a key this release trusts,",
                "whole, for this product, and able to read this console's state.",
                "F10 installs it into the other code slot. F12 discards it.",
            ],
        }
    }
}

/// The install, watched; then a restart now or later.
struct Installing {
    slot: String,
}

impl Panel<Env, Outcome> for Installing {
    fn name(&self) -> &'static str {
        "console-update-installing"
    }

    fn live(&self) -> bool {
        true
    }

    fn render(&self, env: &mut Env) -> Screen {
        let st = env.updd().status().unwrap_or_default();
        let pending = st
            .slots
            .iter()
            .any(|s| s.name == self.slot && s.last == Attempt::Pending);
        let lines = match (&st.installing, pending) {
            (Some(p), _) => vec![format!("Writing slot {}: {}%.", p.slot, p.percent)],
            (None, true) => vec![
                format!("Slot {} is installed and pending.", self.slot),
                String::new(),
                "F10 restarts the console into it now: it confirms when its web".into(),
                "server answers, or the console comes back on the slot it runs".into(),
                "now, with its state as it was. F12 leaves it pending: option 5".into(),
                "restarts into it later.".into(),
            ],
            (None, false) => {
                vec!["The install ended without a pending slot: option 5 says why.".into()]
            }
        };
        kit::page(
            "Installing Update",
            &env.system(),
            &lines,
            "F1=Help   F10=Restart now   F12=Restart later",
        )
    }

    fn enter(&mut self, _fields: &[String], _env: &mut Env) -> Step {
        Flow::Stay
    }

    fn function(&mut self, key: u8, _fields: &[String], env: &mut Env) -> Step {
        match key {
            10 => activate(env),
            12 | 3 => Flow::PopAll,
            _ => Flow::Stay,
        }
    }

    fn help(&self) -> Help {
        Help {
            title: "Installing Update",
            lines: &[
                "The slot is written on every load-source disk and read back.",
                "When it is pending, F10 restarts into it now, F12 later.",
            ],
        }
    }
}

/// Restart into the pending slot, as the signed-on user.
fn activate(env: &mut Env) -> Step {
    let by = format!("service tools user {}", env.user);
    match env.updd().activate(false, 0, by) {
        Ok(Ok(())) => Flow::Message("The console is restarting into the pending slot.".into()),
        Ok(Err(issued)) => Flow::Error(said(&issued)),
        Err(e) => Flow::Error(down("update service", &e)),
    }
}

/// Option 8, 5: `Display Code Levels`.
pub struct Levels;

impl Panel<Env, Outcome> for Levels {
    fn name(&self) -> &'static str {
        "console-levels"
    }

    fn render(&self, env: &mut Env) -> Screen {
        let line = |label: &str, value: String| format!("{} {value}", kit::dotted(label, 22));
        let mut lines = Vec::new();
        match env.updd().status() {
            Ok(st) => {
                for s in &st.slots {
                    let mut v = if s.release.is_empty() {
                        "empty".to_string()
                    } else {
                        format!("{} build {}", s.release, s.build)
                    };
                    if s.running {
                        v.push_str(", running");
                    }
                    if s.default {
                        v.push_str(", default");
                    }
                    lines.push(line(&format!("Slot {}", s.name), v));
                    let last = match s.last {
                        Attempt::Never => "never activated".to_string(),
                        Attempt::Pending => "pending: a restart activates it".into(),
                        Attempt::Confirmed => format!("confirmed {}", when(env, s.time)),
                        Attempt::RolledBack => {
                            format!("rolled back {}: {}", when(env, s.time), s.reason)
                        }
                    };
                    lines.push(line("  Last activation", kit::fit(&last, 50)));
                    if !s.fixes.is_empty() {
                        lines.push(line("  Fixes", kit::fit(&s.fixes.join(", "), 50)));
                    }
                }
                if let Some(s) = &st.staged {
                    lines.push(String::new());
                    lines.push(line("Staged", format!("{} ({:?})", s.name, s.verification)));
                }
                if st.slots.iter().any(|s| s.last == Attempt::Pending) {
                    lines.push(String::new());
                    lines.push("F10 restarts into the pending slot; F9 cancels it.".into());
                }
            }
            Err(e) => lines.push(down("update service", &e)),
        }
        kit::page(
            "Display Code Levels",
            &env.system(),
            &lines,
            "F1=Help   F3=Exit   F9=Cancel pending   F10=Restart into pending   F12=Cancel",
        )
    }

    fn enter(&mut self, _fields: &[String], _env: &mut Env) -> Step {
        Flow::Stay
    }

    fn function(&mut self, key: u8, _fields: &[String], env: &mut Env) -> Step {
        match key {
            10 => activate(env),
            9 => match env.updd().cancel() {
                Ok(Ok(())) => Flow::Message("The pending slot is pending no more.".into()),
                Ok(Err(issued)) => Flow::Error(said(&issued)),
                Err(e) => Flow::Error(down("update service", &e)),
            },
            3 => Flow::PopAll,
            12 => Flow::Pop,
            _ => Flow::Stay,
        }
    }

    fn help(&self) -> Help {
        Help {
            title: "Display Code Levels",
            lines: &[
                "Both code slots: what each holds, which runs, which is the",
                "default, and how its last activation ended, with why it",
                "rolled back when it did. A pending slot is activated by F10,",
                "a restart into it, or cancelled by F9.",
            ],
        }
    }
}
