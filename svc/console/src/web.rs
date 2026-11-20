// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! `Work with the web interface` (option 4): where the web interface
//! answers, its certificate, whether its server is up; and the three
//! things a console needs when nobody can sign on there: an
//! Administrator's password, a new certificate, the default listening
//! rule. Each is asked of the web server, or of PID 1 when the web
//! server is down (`webusers::control`).

use console::Screen;
use panels::tools::Confirm;
use panels::ui::{self, FormLine};
use panels::{Flow, Help, Named, Panel, kit};
use webusers::control::WebStatus;

use crate::env::Env;
use crate::menu::{Outcome, Step};

const LABEL: usize = 17;

fn row(label: &str, value: &str) -> String {
    if label.is_empty() {
        // A value's second line, under the first.
        return format!("{}{value}", " ".repeat(LABEL + 2));
    }
    format!("{} {value}", kit::dotted(label, LABEL))
}

/// `AA:BB:...` in rows of `per` bytes, for a fingerprint too long for a
/// line.
fn halves(colons: &str, per: usize) -> Vec<String> {
    let bytes: Vec<&str> = colons.split(':').collect();
    bytes.chunks(per.max(1)).map(|c| c.join(":")).collect()
}

/// The lines above the options.
pub fn lines(
    web: Option<&WebStatus>,
    administrators: &[String],
    date: impl Fn(u64) -> String,
) -> Vec<String> {
    let Some(w) = web else {
        return vec![
            row("Web server", "not running"),
            row(
                "Administrators",
                &if administrators.is_empty() {
                    "none enabled".to_string()
                } else {
                    administrators.join(", ")
                },
            ),
            String::new(),
            "While the web server is down, the changes below are made on its".into(),
            "files and take effect when it starts.".into(),
        ];
    };
    let answers = w
        .listening
        .iter()
        .find(|l| l.ends_with(":443"))
        .map_or_else(
            || "no address yet".to_string(),
            |l| {
                format!(
                    "https://{}/qsmc/ (443; 80 redirects)",
                    l.trim_end_matches(":443")
                )
            },
        );
    let mut out = vec![
        row("Web server", "running"),
        row("Answers on", &answers),
        row(
            "Answers from",
            &if w.allow.is_empty() {
                "every address".to_string()
            } else {
                w.allow.join(", ")
            },
        ),
        row(
            "Certificate",
            &w.subject.chars().take(56).collect::<String>(),
        ),
        row(
            "Valid",
            &format!("{} to {}", date(w.not_before), date(w.not_after)),
        ),
    ];
    for (i, part) in halves(&w.sha256, 16).iter().enumerate() {
        out.push(row(if i == 0 { "SHA-256" } else { "" }, part));
    }
    for (i, part) in halves(&w.sha1, 10).iter().enumerate() {
        out.push(row(if i == 0 { "SHA-1" } else { "" }, part));
    }
    out.push(row(
        "Administrators",
        &if w.administrators.is_empty() {
            "none enabled".to_string()
        } else {
            w.administrators.join(", ")
        },
    ));
    out
}

/// The menu's options.
pub const OPTIONS: [(u8, &str); 3] = [
    (1, "Reset or make an Administrator"),
    (2, "Make a new self-signed certificate"),
    (3, "Restore the default listening rule"),
];

/// `Work with the web interface`.
pub struct WebMenu;

impl Panel<Env, Outcome> for WebMenu {
    fn name(&self) -> &'static str {
        "console-web"
    }

    fn render(&self, env: &mut Env) -> Screen {
        let web = env.web();
        let administrators = env.administrators();
        let zone = env.zone();
        let date = |t: u64| {
            zone.display(i64::try_from(t).unwrap_or(0))
                .split(' ')
                .next()
                .unwrap_or("")
                .to_string()
        };
        ui::menu_after(
            "Work with the Web Interface",
            &env.system(),
            &lines(web.as_ref(), &administrators, date),
            &OPTIONS,
            "F1=Help   F3=Exit   F5=Refresh   F12=Cancel",
        )
    }

    fn enter(&mut self, fields: &[String], env: &mut Env) -> Step {
        match kit::number(fields.first()) {
            Some(1) => {
                let id = env
                    .web()
                    .map_or_else(|| env.administrators(), |w| w.administrators)
                    .first()
                    .cloned()
                    .unwrap_or_default();
                Flow::Push(Box::new(Reset { id }))
            }
            Some(2) => Flow::Push(Box::new(certificate())),
            Some(3) => Flow::Push(Box::new(listening())),
            Some(_) => Flow::Error("Selection not valid.".into()),
            None => Flow::Error("Type the number of an option.".into()),
        }
    }

    fn function(&mut self, key: u8, _fields: &[String], _env: &mut Env) -> Step {
        match key {
            // F5 draws the panel again, with the status as it is now.
            3 | 12 => Flow::Pop,
            _ => Flow::Stay,
        }
    }

    fn help(&self) -> Help {
        Help {
            title: "Work with the Web Interface",
            lines: &[
                "The web interface is how the console is used. This shows where",
                "it answers, the certificate it presents with its fingerprints,",
                "and its enabled Administrators. F5 shows them again.",
                "",
                "1 gives an Administrator a new password, or makes one when no",
                "Administrator is enabled. 2 replaces the certificate with a new",
                "self-signed one. 3 makes the web interface answer every address",
                "again, on the open interface only.",
                "",
                "If the web server is down, the change is made on its files and",
                "takes effect when it starts.",
            ],
        }
    }
}

/// Option 1: an Administrator's new password, or a new Administrator.
struct Reset {
    id: String,
}

impl Panel<Env, Outcome> for Reset {
    fn name(&self) -> &'static str {
        "console-web-reset"
    }

    fn render(&self, env: &mut Env) -> Screen {
        let mut s = kit::form(
            "Reset or Make an Administrator",
            &env.system(),
            &[
                FormLine::new("Web user ID", 32, self.id.clone()),
                FormLine::hidden("New password", 32),
                FormLine::hidden("New password again", 32),
            ],
            "F1=Help   F3=Exit   F12=Cancel",
        );
        let notes = [
            "A user ID that is there becomes an enabled Administrator with",
            "this password; one that is not is made.",
        ];
        for (i, note) in notes.iter().enumerate() {
            kit::put(&mut s, ui::BODY_ROW + 4 + i, note);
        }
        for (i, rule) in users::RULES.iter().enumerate().skip(1) {
            kit::put(&mut s, ui::BODY_ROW + 6 + i, rule);
        }
        s
    }

    fn enter(&mut self, fields: &[String], env: &mut Env) -> Step {
        let id = fields.first().map_or("", |f| f.trim()).to_lowercase();
        let password = zeroize::Zeroizing::new(fields.get(1).cloned().unwrap_or_default());
        let again = zeroize::Zeroizing::new(fields.get(2).cloned().unwrap_or_default());
        self.id.clone_from(&id);
        if id.is_empty() {
            return Flow::Error("Type a web user ID.".into());
        }
        if *password != *again {
            return Flow::Error("The two passwords are not the same.".into());
        }
        if let Err(e) = users::check_password(&id, &password) {
            return Flow::Error(e);
        }
        let hash = match users::hash(&password) {
            Ok(h) => h,
            Err(e) => return Flow::Error(format!("The password was not hashed: {e}.")),
        };
        match env.reset_admin(&id, &hash) {
            Ok(()) => Flow::Unwind(
                1,
                Some((
                    format!("{id} can sign on to the web interface as an Administrator."),
                    false,
                )),
            ),
            Err(e) => Flow::Error(e),
        }
    }

    fn help(&self) -> Help {
        Help {
            title: "Reset or Make an Administrator",
            lines: &[
                "Type a web user ID and its new password twice. The password is",
                "not shown as you type, and only its hash leaves this panel.",
                "",
                "The user ID becomes an enabled Administrator: its old password",
                "stops working, and a lock from wrong passwords ends. A user ID",
                "the web interface does not have is made. Its sessions are not",
                "ended. The change is in the web interface's audit.",
            ],
        }
    }
}

fn certificate() -> Confirm<Env, Outcome> {
    Confirm::new(
        "console-web-certificate",
        "Confirm New Certificate",
        kit::lines(&[
            "The web server's certificate will be replaced with a new",
            "self-signed one for the console's name and open address, valid",
            "for a year. Browsers warn about it until it is accepted again.",
        ]),
        Help {
            title: "Confirm New Certificate",
            lines: &[
                "F10 makes the new certificate. New connections are served with",
                "it; the sessions open keep the one they have. Compare its",
                "fingerprint, shown after, with the one a browser shows.",
                "",
                "An imported certificate is replaced too. F12 goes back and",
                "nothing happens.",
            ],
        },
        |env: &mut Env| match env.new_certificate() {
            Ok(Some(sha256)) => {
                let short = sha256.get(..23).unwrap_or(&sha256).to_string();
                Flow::Unwind(
                    1,
                    Some((format!("New certificate in use: SHA-256 {short}..."), false)),
                )
            }
            Ok(None) => Flow::Unwind(
                1,
                Some((
                    "The web server makes a new certificate when it starts.".into(),
                    false,
                )),
            ),
            Err(e) => Flow::Unwind(1, Some((e, true))),
        },
    )
}

fn listening() -> Confirm<Env, Outcome> {
    Confirm::new(
        "console-web-listening",
        "Confirm Default Listening",
        kit::lines(&[
            "The web interface will answer every address again, on the open",
            "interface only: the Allow web UI from list is emptied.",
        ]),
        Help {
            title: "Confirm Default Listening",
            lines: &[
                "Use this when the Allow web UI from list of Network settings",
                "keeps you out. F10 empties it; F12 goes back and nothing",
                "happens.",
            ],
        },
        |env: &mut Env| match env.default_listening() {
            Ok(()) => Flow::Unwind(
                1,
                Some(("The web interface answers every address.".into(), false)),
            ),
            Err(e) => Flow::Unwind(1, Some((e, true))),
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_status_fits_and_says_how_the_server_is() {
        let w = WebStatus {
            listening: vec!["10.0.2.15:443".into(), "10.0.2.15:80".into()],
            subject: "CN=QSMC123456".into(),
            sha256: ["AB"; 32].join(":"),
            sha1: ["CD"; 20].join(":"),
            not_before: 0,
            not_after: 1,
            allow: Vec::new(),
            listen_private: false,
            administrators: vec!["qsadmin".into()],
        };
        let shown = lines(Some(&w), &[], |t| t.to_string());
        assert!(shown.iter().any(|l| l.contains("https://10.0.2.15/qsmc/")));
        assert!(shown.iter().all(|l| l.len() <= 76), "{shown:?}");
        let down = lines(None, &["qsadmin".into()], |t| t.to_string());
        assert!(down[0].contains("not running") && down[1].contains("qsadmin"));
        assert_eq!(halves(&w.sha256, 16).len(), 2);
    }
}
