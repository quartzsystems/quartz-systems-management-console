// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! Option 8, 1: the console's own problems, one shown whole with its
//! resources and its analysis log, one closed.

use console::Screen;
use panels::ui;
use panels::{Flow, Help, Named as _, Panel, PanelFlow, kit};
use problem::proto::Calls as _;
use problem::{Filter, Status, Summary};

use super::{down, said, when};
use crate::env::Env;
use crate::menu::Outcome;

type Step = PanelFlow<Env, Outcome>;

const TITLE: &str = "Work with Problems";
/// The rows a list shows at a time.
const ROWS: usize = kit::BODY_ROWS - 4;

/// Every problem of the console's log, newest first.
fn all(env: &mut Env, open_only: bool) -> Result<Vec<Summary>, String> {
    let filter = Filter {
        status: open_only.then_some(Status::Open),
        ..Filter::default()
    };
    let mut out = Vec::new();
    let mut before = None;
    loop {
        let page = env
            .problems()
            .list(filter.clone(), before, 500)
            .map_err(|e| down("problem service", &e))?;
        out.extend(page.problems);
        match page.more {
            Some(m) => before = Some(m),
            None => return Ok(out),
        }
    }
}

/// `Open`, `Cleared` or `Closed`.
fn state(s: &Summary) -> &'static str {
    match (s.status, s.holds) {
        (Status::Closed, _) => "Closed",
        (Status::Open, true) => "Open",
        (Status::Open, false) => "Cleared",
    }
}

fn row(env: &Env, s: &Summary) -> String {
    format!(
        "{:>6}  {:<7}  {}  {:<8}  {}",
        s.number,
        state(s),
        kit::fit(&when(env, s.last_seen), 16),
        s.refcode.code,
        kit::fit(&s.description, 30)
    )
}

/// `Work with Problems`.
#[derive(Default)]
pub struct Problems {
    top: usize,
    every: bool,
}

impl Panel<Env, Outcome> for Problems {
    fn name(&self) -> &'static str {
        "console-problems"
    }

    fn render(&self, env: &mut Env) -> Screen {
        let list = match all(env, !self.every) {
            Ok(l) => l,
            Err(e) => {
                return kit::page(TITLE, &env.system(), &[e], "F1=Help   F3=Exit   F12=Cancel");
            }
        };
        let rows: Vec<String> = list.iter().map(|s| row(env, s)).collect();
        let (shown, foot) = kit::window(&rows, self.top, ROWS);
        let mut s = ui::list(
            TITLE,
            &env.system(),
            "4=Close  5=Display",
            "Opt Number  Status   Last seen         Code      Description",
            &shown,
            if self.every {
                "F1=Help  F3=Exit  F5=Refresh  F11=Open only  F12=Cancel"
            } else {
                "F1=Help  F3=Exit  F5=Refresh  F11=Show all  F12=Cancel"
            },
        );
        kit::put(&mut s, ui::BODY_ROW + 4 + ROWS, &foot);
        if rows.is_empty() {
            kit::put(
                &mut s,
                ui::BODY_ROW + 3,
                if self.every {
                    "(The console's problem log is empty.)"
                } else {
                    "(No problem is open. F11 shows the closed ones too.)"
                },
            );
        }
        s
    }

    fn enter(&mut self, fields: &[String], env: &mut Env) -> Step {
        let list = match all(env, !self.every) {
            Ok(l) => l,
            Err(e) => return Flow::Error(e),
        };
        let shown: Vec<&Summary> = list.iter().skip(self.top).take(ROWS).collect();
        let chosen: Vec<(&Summary, u32)> = shown
            .into_iter()
            .zip(fields)
            .filter_map(|(s, f)| Some((s, kit::number(Some(f))?)))
            .collect();
        let [(s, option)] = chosen.as_slice() else {
            return if chosen.is_empty() {
                Flow::Stay
            } else {
                Flow::Error("Type an option by one problem.".into())
            };
        };
        match option {
            4 if s.status == Status::Closed => {
                Flow::Error(format!("Problem {} is closed already.", s.number))
            }
            4 => Flow::Push(Box::new(Close {
                number: s.number,
                holds: s.holds,
            })),
            5 => Flow::Push(Box::new(Display {
                number: s.number,
                top: 0,
            })),
            _ => Flow::Error("Option not valid.".into()),
        }
    }

    fn function(&mut self, key: u8, _fields: &[String], env: &mut Env) -> Step {
        match key {
            3 => Flow::PopAll,
            12 => Flow::Pop,
            11 => {
                self.every = !self.every;
                self.top = 0;
                Flow::Stay
            }
            7 | 8 => {
                let len = all(env, !self.every).map_or(0, |l| l.len());
                self.top = kit::scroll(self.top, key, len, ROWS);
                Flow::Stay
            }
            _ => Flow::Stay,
        }
    }

    fn help(&self) -> Help {
        Help {
            title: TITLE,
            lines: &[
                "Each problem is a condition of the console a person should",
                "look at, opened from a reference code it posted: newest first,",
                "the open ones unless F11 shows all. Cleared means its condition",
                "no longer holds; a person still closes it.",
                "",
                "Type 5 by a problem to see it whole: what it is about, what to",
                "do, and its analysis log. Type 4 to close it. F7 and F8 page.",
                "The managed systems' problems are on the web interface.",
            ],
        }
    }
}

/// `Display Problem`.
struct Display {
    number: u64,
    top: usize,
}

/// The lines of one problem.
fn detail(env: &Env, p: &problem::Problem, severity: problem::Severity) -> Vec<String> {
    let s = &p.summary;
    let line = |label: &str, value: String| format!("{} {value}", kit::dotted(label, 20));
    let mut out = vec![
        line("Problem", s.number.to_string()),
        line(
            "Status",
            match (s.status, s.holds) {
                (Status::Closed, _) => {
                    format!("Closed by {} at {}", s.closed_by, when(env, s.closed_at))
                }
                (Status::Open, true) => "Open; its condition holds".into(),
                (Status::Open, false) => "Open; its condition has cleared".into(),
            },
        ),
        line("Severity", severity.word().into()),
        line("Reference code", s.refcode.code.clone()),
        line("First seen", when(env, s.first_seen)),
        line("Last seen", when(env, s.last_seen)),
        line("Seen", format!("{} times", s.count)),
        String::new(),
    ];
    out.extend(textwrap(&s.description));
    if let Some(c) = refcode::find(&s.refcode.code) {
        out.push(String::new());
        out.extend(textwrap(&format!("What to do: {}.", c.action)));
    }
    out.push(String::new());
    out.push("What it is about:".into());
    if p.resources.is_empty() {
        out.push("  The console itself.".into());
    }
    for r in &p.resources {
        let mut l = format!("  {} {}", r.kind, r.name);
        if !r.serial.is_empty() {
            l.push_str(", serial ");
            l.push_str(&r.serial);
        }
        out.push(kit::fit(&l, 76));
    }
    out.push(String::new());
    out.push("Analysis log:".into());
    for l in &p.log {
        out.push(kit::fit(
            &format!("  {}  {}", when(env, l.time), l.text),
            76,
        ));
    }
    out
}

/// `text` in lines of at most 76.
fn textwrap(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut line = String::new();
    for w in text.split_whitespace() {
        if !line.is_empty() && line.len() + 1 + w.len() > 76 {
            out.push(std::mem::take(&mut line));
        }
        if !line.is_empty() {
            line.push(' ');
        }
        line.push_str(w);
    }
    if !line.is_empty() {
        out.push(line);
    }
    out
}

impl Panel<Env, Outcome> for Display {
    fn name(&self) -> &'static str {
        "console-problem"
    }

    fn render(&self, env: &mut Env) -> Screen {
        let lines = match env.problems().get_rated(self.number) {
            Ok(Ok(p)) => detail(env, &p.problem, p.severity),
            Ok(Err(issued)) => vec![said(&issued)],
            Err(e) => vec![down("problem service", &e)],
        };
        let (shown, foot) = kit::window(&lines, self.top, kit::BODY_ROWS - 1);
        let mut s = kit::page(
            "Display Problem",
            &env.system(),
            &shown,
            "F1=Help   F3=Exit   F4=Close problem   F12=Cancel",
        );
        kit::put(&mut s, ui::BODY_ROW + kit::BODY_ROWS - 1, &foot);
        s
    }

    fn enter(&mut self, _fields: &[String], _env: &mut Env) -> Step {
        Flow::Stay
    }

    fn function(&mut self, key: u8, _fields: &[String], env: &mut Env) -> Step {
        match key {
            3 => Flow::PopAll,
            12 => Flow::Pop,
            7 | 8 => {
                let len = match env.problems().get_rated(self.number) {
                    Ok(Ok(p)) => detail(env, &p.problem, p.severity).len(),
                    _ => 0,
                };
                self.top = kit::scroll(self.top, key, len, kit::BODY_ROWS - 1);
                Flow::Stay
            }
            4 => match env.problems().get(self.number) {
                Ok(Ok(p)) if p.summary.status == Status::Closed => {
                    Flow::Error(format!("Problem {} is closed already.", self.number))
                }
                Ok(Ok(p)) => Flow::Push(Box::new(Close {
                    number: self.number,
                    holds: p.summary.holds,
                })),
                Ok(Err(issued)) => Flow::Error(said(&issued)),
                Err(e) => Flow::Error(down("problem service", &e)),
            },
            _ => Flow::Stay,
        }
    }

    fn help(&self) -> Help {
        Help {
            title: "Display Problem",
            lines: &[
                "One problem whole: its status and severity, its reference",
                "code and what to do for it, what it is about, and its analysis",
                "log, oldest first. F7 and F8 page; F4 closes it.",
            ],
        }
    }
}

/// `Confirm Close Problem`.
struct Close {
    number: u64,
    holds: bool,
}

impl Panel<Env, Outcome> for Close {
    fn name(&self) -> &'static str {
        "console-problem-close"
    }

    fn render(&self, env: &mut Env) -> Screen {
        let mut lines = vec![format!(
            "Problem {} is closed, with your user ID.",
            self.number
        )];
        if self.holds {
            lines.extend([
                String::new(),
                "Its condition still holds: it is closed only because you".into(),
                "say so. Seen again, the condition opens a new problem.".into(),
            ]);
        }
        lines.extend([
            String::new(),
            "Press F10 to close it, or F12 to return.".into(),
        ]);
        kit::page(
            "Confirm Close Problem",
            &env.system(),
            &lines,
            "F1=Help   F10=Close problem   F12=Cancel",
        )
    }

    fn enter(&mut self, _fields: &[String], _env: &mut Env) -> Step {
        Flow::Stay
    }

    fn function(&mut self, key: u8, _fields: &[String], env: &mut Env) -> Step {
        match key {
            10 => {
                let by = format!("service tools user {}", env.user);
                match env.problems().close(self.number, self.holds, by) {
                    Ok(Ok(_)) => {
                        println!("problem {} closed by {}", self.number, env.user);
                        Flow::Unwind(1, Some((format!("Problem {} closed.", self.number), false)))
                    }
                    Ok(Err(issued)) => Flow::Error(said(&issued)),
                    Err(e) => Flow::Error(down("problem service", &e)),
                }
            }
            3 => Flow::PopAll,
            12 => Flow::Pop,
            _ => Flow::Stay,
        }
    }

    fn help(&self) -> Help {
        Help {
            title: "Confirm Close Problem",
            lines: &[
                "F10 closes the problem in the console's log, with your user",
                "ID. A closed problem stays closed; its condition seen again",
                "opens a new one. F12 goes back and nothing changes.",
            ],
        }
    }
}
