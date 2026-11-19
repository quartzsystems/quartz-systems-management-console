// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! QS/MC's local console. The display and the serial port each have
//! their own session. The display shows the console's status until
//! Enter is pressed. The serial port is PID 1's, with the status screen
//! as lines, until Enter is pressed there; then this service has it for
//! the session and gives it back. Enter leads to sign-on with a service
//! tools user ID, sign-on to the main menu. A session that sees no key
//! for ten minutes is signed off.
//!
//! It manages nothing but the console itself: no host, partition or web
//! user is worked with here.

// What needs no kernel is built and tested on any host.
#![cfg_attr(not(target_os = "linux"), allow(dead_code))]

mod disks;
mod env;
mod keep;
mod menu;
mod network;
mod status;

#[cfg(target_os = "linux")]
mod on_console {
    use std::io::Write as _;
    use std::path::Path;
    use std::time::Duration;

    use console::{Identity, Input, Scale};
    use panels::screens::{Displays, Ended, Screens, SerialKeys, SerialPort, run};
    use panels::tools::SignOn;
    use panels::{Named, Runner, kit};
    use release::fact;
    use rustix::event::{PollFd, PollFlags, poll};
    use supervise::signal::{Signals, TERM};

    use crate::env::{Env, SharedUsers, open_users};
    use crate::menu::{self, MainMenu, Outcome};

    /// The serial port.
    pub const SERIAL: &str = "/dev/ttyS0";
    /// A session with no key for this long is signed off.
    const IDLE: Duration = Duration::from_secs(600);
    /// A sign-on screen with no key for this long is left.
    const SIGN_ON_IDLE: Duration = Duration::from_secs(60);

    /// The idle time of a session. A test image's is made short by a
    /// hook, so a suite need not wait ten minutes.
    fn idle() -> Duration {
        #[cfg(test_hooks)]
        {
            let wanted = format!("{}{}=short-idle", release::KEY_PREFIX, release::HOOK_KEY);
            if inventory::oem_strings(&inventory::Roots::default()).contains(&wanted) {
                return Duration::from_secs(15);
            }
        }
        IDLE
    }

    fn identity() -> Identity {
        Identity {
            banner: release::banner(),
            ready_code: String::new(),
        }
    }

    fn scale() -> Scale {
        let line = std::fs::read_to_string("/proc/cmdline").unwrap_or_default();
        Scale::from_cmdline(&line, &format!("{}console.scale", release::KEY_PREFIX))
    }

    /// A console on the display and the serial port at once, with the
    /// port's keys: the one-panel form's.
    pub fn both(serial: &Path) -> (Screens, Option<SerialPort>) {
        let port = SerialPort::open(serial).ok();
        let screens = Screens::open(
            identity(),
            Displays {
                framebuffer: true,
                scale: scale(),
                serial: port.as_ref().and_then(|s| s.writer().ok()),
            },
        );
        if let Some(keys) = port.as_ref().and_then(|p| p.keys().ok()) {
            keys.attach(&screens);
            // The reader runs for the life of the process.
            std::mem::forget(keys);
        }
        (screens, port)
    }

    /// Sign on, then the main menu, until the user signs off or goes
    /// quiet.
    fn session(screens: &Screens, env: &mut Env) {
        screens.drain();
        env.status();
        let mut sign_on: Runner<Env, String> = Runner::new(Box::new(SignOn));
        env.user = match run(screens, &mut sign_on, env, Some(SIGN_ON_IDLE)) {
            Ended::Outcome(user) => user,
            _ => return,
        };
        let mut main: Runner<Env, Outcome> = Runner::new(Box::new(MainMenu));
        let how = match run(screens, &mut main, env, Some(idle())) {
            Ended::Idle => "signed off after no key for the idle time",
            _ => "signed off",
        };
        println!("service tools user {} {how} at the {}", env.user, env.place);
        env.user.clear();
    }

    /// The display: the status until Enter, then a session, for ever.
    fn display(screens: &Screens, users: SharedUsers) {
        let mut env = Env::new("display", users);
        loop {
            let status = env.status();
            screens.show(menu::idle(&status));
            match screens.input(Duration::from_secs(1)) {
                Ok(Some(Input::Enter(_))) => session(screens, &mut env),
                Ok(_) => {}
                Err(_) => return,
            }
        }
    }

    /// The serial port: PID 1's until Enter, then a session, for ever.
    fn serial(port: &SerialPort, keys: &SerialKeys, users: SharedUsers) {
        let mut env = Env::new("serial port", users);
        loop {
            if !keys.wait_enter(Duration::from_secs(3600)) {
                continue;
            }
            let Ok(writer) = port.writer() else { continue };
            env.note(fact::SERIAL_CONSOLE, fact::SERIAL_SESSION);
            let screens = Screens::open(
                identity(),
                Displays {
                    framebuffer: false,
                    scale: Scale::Auto,
                    serial: Some(writer),
                },
            );
            keys.attach(&screens);
            session(&screens, &mut env);
            keys.detach();
            // The last screen is drawn before the terminal is cleared.
            screens.show(kit::page(release::CONSOLE_TITLE, &env.system(), &[], ""));
            std::thread::sleep(Duration::from_millis(200));
            drop(screens);
            if let Ok(mut out) = port.writer() {
                let _ = out.write_all(b"\x1b[0m\x1b[2J\x1b[H");
            }
            env.note(fact::SERIAL_CONSOLE, fact::SERIAL_IDLE);
        }
    }

    pub fn main() {
        // PID 1 runs the console so when the load source's units ran
        // apart: one panel, no session.
        if std::env::args().nth(1).as_deref() == Some(release::KEEP_UNIT) {
            crate::keep::main();
        }
        let mut signals = match Signals::take(&[TERM]) {
            Ok(signals) => signals,
            Err(e) => {
                eprintln!("the signal descriptor: {e}");
                std::process::exit(1);
            }
        };
        let users = open_users();
        let shown = Screens::open(
            identity(),
            Displays {
                framebuffer: true,
                scale: scale(),
                serial: None,
            },
        );
        println!("display: {}", shown.describe());
        let last = shown.shower();
        if shown.has_framebuffer() {
            let for_display = users.clone();
            std::thread::spawn(move || display(&shown, for_display));
        }
        match SerialPort::open(Path::new(SERIAL)).and_then(|port| Ok((port.keys()?, port))) {
            Ok((keys, port)) => {
                std::thread::spawn(move || serial(&port, &keys, users));
            }
            Err(e) => eprintln!("{SERIAL}: no session there: {e}"),
        }
        if let Err(e) = supervise::ready::signal() {
            eprintln!("the readiness pipe: {e}");
        }
        // PID 1 ends this service first at a shutdown: the display says
        // so, since nothing else draws on it afterwards.
        loop {
            let mut fds = [PollFd::from_borrowed_fd(signals.fd(), PollFlags::IN)];
            if poll(&mut fds, None).is_ok() && signals.read().contains(&TERM) {
                last.show(kit::page(
                    release::CONSOLE_TITLE,
                    release::NAME,
                    &kit::lines(&[&release::banner(), "", "The console is shutting down."]),
                    "",
                ));
                std::thread::sleep(Duration::from_millis(150));
                std::process::exit(0);
            }
        }
    }
}

#[cfg(target_os = "linux")]
fn main() {
    on_console::main();
}

#[cfg(not(target_os = "linux"))]
fn main() {
    eprintln!("qsmccons is the local console of a QS/MC console and runs on Linux only");
}
