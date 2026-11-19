// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! The control socket's requests, answered by the console: status,
//! power off and restart, and what services tell PID 1. What a console
//! does not do yet (a service dump, a code update's slots) is refused
//! with the reason.

use std::os::unix::net::UnixListener;
use std::time::{Duration, Instant};

use logd::Severity;
use release::fact;
use supervise::control::{About, Control, HostState, HostStatus, MAX_POWER_DELAY, Power, serve};

use super::{Host, Kind};

/// How long a request waits for a state file to be written: less than
/// a client of the control socket waits for its answer.
const WRITE_WAIT: Duration = Duration::from_millis(1500);
/// Why a code update's request is refused.
const NO_UPDATE: &str = "the console has no code update yet";

impl Control for Host {
    fn status(&mut self) -> HostStatus {
        self.status_now()
    }

    fn power_off(&mut self) {
        self.begin_shutdown(Kind::PowerOff, "the control socket");
    }

    fn restart(&mut self) {
        self.begin_shutdown(Kind::Restart, "the control socket");
    }

    fn post(&mut self, code: String, text: String) {
        Control::post_about(self, code, text, Vec::new());
    }

    fn post_about(&mut self, code: String, text: String, about: Vec<About>) {
        match refcode::post::posting_of(&code, &text, &about) {
            Some(posted) => self.take(posted),
            None => self.say(
                Severity::Warning,
                &format!("a service posted {code}, which is no code of the table: {text}"),
            ),
        }
    }

    fn note(&mut self, key: String, value: String) {
        if key == fact::MANAGEMENT {
            self.facts.management = value;
            self.web_follow();
        } else if key == fact::PRIVATE {
            self.facts.private = value;
        } else if key == fact::SERIAL_CONSOLE {
            self.lend_serial(value == fact::SERIAL_SESSION);
        } else {
            self.say(Severity::Info, &format!("noted {key}: {value}"));
        }
    }

    fn set_host(
        &mut self,
        name: String,
        time_zone: String,
        ntp_servers: Vec<String>,
    ) -> Result<(), String> {
        let done = self.host_settings(name, time_zone, ntp_servers);
        match &done {
            Ok(()) => self.say(
                Severity::Info,
                &format!("host settings changed: host {}", self.facts.host),
            ),
            Err(e) => self.say(Severity::Warning, &format!("host settings refused: {e}")),
        }
        done
    }

    fn state(&mut self) -> HostState {
        let (code, text) = self
            .reference
            .split_once(' ')
            .map_or((self.reference.as_str(), ""), |(c, t)| (c, t.trim()));
        HostState {
            power: if self.shutdown.is_some() {
                Power::Ending
            } else if self.ready {
                Power::Running
            } else {
                Power::Starting
            },
            code: code.to_string(),
            text: text.to_string(),
            release: release::RELEASE.to_string(),
            slot: match self.facts.slot.as_str() {
                s @ ("A" | "B") => s.to_string(),
                _ => String::new(),
            },
            power_in: None,
        }
    }

    fn power_down(
        &mut self,
        restart: bool,
        _immediate: bool,
        delay: u32,
        by: String,
    ) -> Result<(), String> {
        if delay > MAX_POWER_DELAY {
            return Err(format!("a delay of {delay} s; at most {MAX_POWER_DELAY}"));
        }
        if delay != 0 {
            return Err("the console powers down at once or not at all".into());
        }
        if self.shutdown.is_some() {
            return Err("the console is shutting down already".into());
        }
        let kind = if restart {
            Kind::Restart
        } else {
            Kind::PowerOff
        };
        let by: String = by.chars().filter(|c| !c.is_control()).take(128).collect();
        self.begin_shutdown(kind, &by);
        Ok(())
    }

    fn take_dump(&mut self, _by: String) -> Result<String, String> {
        Err("the console takes no service dump yet".into())
    }

    fn slots(&mut self) -> supervise::control::SlotState {
        self.slots
            .as_ref()
            .map(crate::slot::Slots::state)
            .unwrap_or_default()
    }

    fn install_slot(&mut self, _slot: String) -> Result<(), String> {
        Err(NO_UPDATE.into())
    }

    fn cancel_slot(&mut self) -> Result<(), String> {
        Err(NO_UPDATE.into())
    }

    fn activate_slot(&mut self, _by: String) -> Result<(), String> {
        Err(NO_UPDATE.into())
    }
}

impl Host {
    /// Write `host.toml` and take the name and the zone into use.
    fn host_settings(
        &mut self,
        name: String,
        time_zone: String,
        ntp_servers: Vec<String>,
    ) -> Result<(), String> {
        use state::Document as _;
        let host = state::Host {
            name: Some(name.clone()),
            time_zone,
            ntp_servers,
        };
        host.validate()?;
        // Written by another thread and waited for a moment only:
        // `/data` may be on the storage service's device, and this
        // loop is what starts that service again when it ends.
        let (done, written) = std::sync::mpsc::channel();
        let to_write = host.clone();
        std::thread::spawn(move || {
            let store = state::Store::at(crate::product::QSMC.state_dir());
            let _ = done.send(store.save(&to_write).map_err(|e| e.to_string()));
        });
        written
            .recv_timeout(WRITE_WAIT)
            .map_err(|_| "the data file system does not answer".to_string())??;
        let _ = rustix::system::sethostname(name.as_bytes());
        self.facts.host = name;
        self.facts.zone = host.zone();
        Ok(())
    }

    /// Accept one connection and answer its request.
    pub(super) fn serve_control(&mut self) {
        let Some(Ok((stream, _))) = self.listener.as_ref().map(UnixListener::accept) else {
            return;
        };
        let started = Instant::now();
        if let Err(e) = serve(stream, self) {
            self.say(Severity::Warning, &format!("control socket: {e}"));
        }
        let took = started.elapsed();
        if took > Duration::from_secs(1) {
            self.say(
                Severity::Warning,
                &format!("a control request took {} ms", took.as_millis()),
            );
        }
    }
}
