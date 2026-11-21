// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! QS/MC's installer: `base/install`'s screens, in its order, over QS/MC's
//! disks, media and state. PID 1 of the media UKI runs it in place of
//! the service table. It shows its screens on the display and the
//! serial port at once and takes keys from both. It checks the media
//! before anything else; with answers on a seed volume it installs
//! without asking. It ends with status 0 and PID 1 restarts the machine.

// What needs no kernel is built and tested on any host.
#![cfg_attr(not(target_os = "linux"), allow(dead_code))]

mod answers;
#[cfg(target_os = "linux")]
mod target;

#[cfg(target_os = "linux")]
mod on_console {
    use std::io::Write as _;
    use std::path::Path;

    use console::{Identity, Scale};
    use install::flow::{self, Env};
    use install::{Answers, DiskChoice, Outcome, Step, Target};
    use panels::forms::LinkChoice;
    use panels::screens::{Displays, Ended, Screens, SerialPort, run};
    use panels::{Runner, kit};

    use crate::answers;
    use crate::target::Qsmc;

    /// The serial port.
    const SERIAL: &str = "/dev/ttyS0";
    /// Where the media is mounted.
    const MEDIA: &str = "/run/media";
    /// How long the media's device may take to appear: a USB stick is
    /// found seconds after the kernel starts.
    const MEDIA_WAIT: std::time::Duration = std::time::Duration::from_secs(20);

    /// The installer on media it refused: it has nothing to install.
    struct Refused;

    impl Target for Refused {
        fn product(&self) -> &'static str {
            release::NAME
        }
        fn banner(&self) -> String {
            release::banner()
        }
        fn what(&self) -> &'static [&'static str] {
            &[]
        }
        fn disks(&mut self) -> Vec<DiskChoice> {
            Vec::new()
        }
        fn min_disk_bytes(&self) -> u64 {
            release::MIN_DISK_BYTES
        }
        fn links(&mut self) -> Vec<LinkChoice> {
            Vec::new()
        }
        fn default_host(&self) -> String {
            release::NAME.into()
        }
        fn now(&self) -> i64 {
            appliance::time::now_secs()
        }
        fn steps(&self) -> &'static [Step] {
            &[]
        }
        fn step(&mut self, _index: usize, _answers: &Answers) -> Result<(), String> {
            Err("the media was refused".into())
        }
        fn failed(&self) -> &'static str {
            refcode::INSTALL_FAILED.code
        }
        fn log(&mut self, line: &str) {
            say(line);
        }
    }

    /// One line of the installer's log: a reference code with its word.
    fn say(line: &str) {
        let coded = line
            .split(' ')
            .next()
            .is_some_and(|w| refcode::find(w).is_some());
        if coded {
            println!("{} {line}", refcode::WORD);
        } else {
            println!("{line}");
        }
    }

    fn post(code: &refcode::RefCode, detail: &str) {
        say(&format!("{} {}", code.code, code.text_with(detail)));
    }

    /// The disk a block device is, or is on.
    fn disk_of(device: &Path) -> String {
        loadsource::disks::disk_of(device).unwrap_or_else(|| {
            device
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default()
        })
    }

    pub fn main() {
        let serial = SerialPort::open(Path::new(SERIAL)).ok();
        let line = std::fs::read_to_string("/proc/cmdline").unwrap_or_default();
        let screens = Screens::open(
            Identity {
                banner: release::banner(),
                ready_code: String::new(),
            },
            Displays {
                framebuffer: true,
                scale: Scale::from_cmdline(&line, &format!("{}console.scale", release::KEY_PREFIX)),
                serial: serial.as_ref().and_then(|s| s.writer().ok()),
            },
        );
        say(&format!("installer on {}", screens.describe()));
        let keys = serial.as_ref().and_then(|s| s.keys().ok());
        if let Some(keys) = &keys {
            keys.attach(&screens);
        }
        Qsmc::links_up();
        screens.show(kit::page(
            &format!("Install {}", release::NAME),
            release::NAME,
            &kit::lines(&[&release::banner(), "", "Checking the installation media."]),
            "",
        ));

        let outcome = match loadsource::payload::open(
            &release::LOAD_SOURCE,
            &release::RELEASE.to_string(),
            Path::new(release::TRUSTED_KEYS),
            MEDIA,
            MEDIA_WAIT,
        ) {
            Err(reason) => {
                post(&refcode::INSTALL_MEDIA_REFUSED, &reason);
                let mut target = Refused;
                let mut env = Env::new(&mut target);
                run(&screens, &mut flow::refused(reason), &mut env, None)
            }
            Ok((device, payload)) => {
                let m = &payload.manifest;
                post(
                    &refcode::INSTALL_VERIFIED,
                    &format!("{} {} {}", m.product, m.release, m.build),
                );
                let found = answers::seed();
                let mut skip = vec![disk_of(&device)];
                skip.extend(found.iter().map(|(device, _)| disk_of(device)));
                let mut target = Qsmc::new(payload, skip, Box::new(say));
                let mut runner: Runner<Env<'_>, Outcome> =
                    match answers::given(found.map(|(_, seed)| seed), &mut target) {
                        Ok(None) => flow::attended(),
                        Ok(Some(given)) => {
                            say("unattended: the answers of a seed volume are used");
                            flow::unattended(given)
                        }
                        Err(reason) => {
                            post(&refcode::INSTALL_ANSWERS_INVALID, &reason);
                            flow::stopped(reason)
                        }
                    };
                let mut env = Env::new(&mut target);
                run(&screens, &mut runner, &mut env, None)
            }
        };
        if outcome == Ended::Outcome(Outcome::Installed) {
            post(&refcode::INSTALL_COMPLETE, "");
        } else {
            say("the installer was left; nothing more is written");
        }
        rustix::fs::sync();
        // The terminal's cursor below the screen, for what PID 1 prints.
        if let Some(mut out) = serial.as_ref().and_then(|s| s.writer().ok()) {
            let _ = out.write_all(b"\x1b[0m\x1b[24;1H\r\n");
        }
        std::process::exit(0);
    }
}

#[cfg(target_os = "linux")]
fn main() {
    on_console::main();
}

#[cfg(not(target_os = "linux"))]
fn main() {
    eprintln!("qsmcinstall is the installer of a QS/MC console and runs on Linux only");
}
