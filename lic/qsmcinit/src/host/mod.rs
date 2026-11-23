// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! The running console: one loop over the signals, the control socket,
//! the power button and the services' pipes. Nothing here blocks for
//! longer than a client of the control socket is given.

mod control;
mod dump;
mod shutdown;
pub mod storage;
pub mod table;
pub mod web;
pub mod webreq;

use std::fs::File;
use std::os::fd::AsFd;
use std::os::unix::net::UnixListener;
use std::time::{Duration, Instant};

use appliance::time::{Zone, now_secs};
use logd::client::Sender;
use logd::{Record, Severity};
use refcode::RefCode;
use refcode::post::{Posting, SOURCE, posting};
use release::{fact, screen};
use rustix::event::{PollFd, PollFlags, Timespec, poll};
use supervise::run::{Occurrence, Supervisor, Token};
use supervise::signal::{CHLD, INT, Signals, TERM};

use crate::console::Console;
use crate::slot::{Slots, rule};
use crate::sys::{button, reap};
pub use shutdown::Kind;

/// Seconds between heartbeats on the serial port.
const HEARTBEAT: Duration = Duration::from_secs(5);
/// The longest the loop sleeps: the log queue is flushed and the
/// display's clock looked at this often.
const TICK: Duration = Duration::from_secs(1);
/// How soon records the log service could not take are offered again.
const RETRY: Duration = Duration::from_millis(20);
/// What each service is given to end before it is killed.
const STOP_GRACE: Duration = Duration::from_secs(5);
/// How a service its seccomp filter ended is said to have ended
/// (`sys::reap`): `SIGSYS`.
const SIGSYS_ENDED: &str = "signal 31";
/// How long a slot on trial has to confirm before the console restarts
/// into the other slot: its services never became ready.
const TRIAL_WAIT: Duration = Duration::from_mins(15);

/// How long a slot on trial has; a test image's `short-trial` hook
/// gives it 90 seconds.
fn trial_wait() -> Duration {
    #[cfg(test_hooks)]
    if crate::hooks::has(crate::hooks::SHORT_TRIAL) {
        return Duration::from_secs(90);
    }
    TRIAL_WAIT
}

/// What a descriptor that became readable is.
#[derive(Clone, Copy)]
enum Ready {
    Signal,
    Control,
    Web,
    Button(usize),
    Service(Token),
}

/// The facts the status screen and the control socket show, besides
/// the ones read when asked.
pub struct Facts {
    /// The slot this IPL came from: `A`, `B`, `media` or `unknown`.
    pub slot: String,
    /// The host name.
    pub host: String,
    /// The serial number, empty when there is none.
    pub serial: String,
    /// The display zone.
    pub zone: Zone,
    /// The `open` interface's line, as the network service last noted it.
    pub management: String,
    /// The `private` interface's line; none until it is noted.
    pub private: String,
    /// The machine, from the inventory.
    pub machine: String,
    /// What is mounted at `/data`: the device, or `memory`.
    pub data: String,
    /// What the processor has of what the cryptography needs.
    pub processor: String,
}

/// The console.
// Each flag is a fact of its own, not a state of one machine.
#[allow(clippy::struct_excessive_bools)]
pub struct Host {
    console: Console,
    log: Sender,
    signals: Signals,
    /// The current reference code and its text.
    reference: String,
    /// The last time synchronization, as [`fact::SYNCED`] says it.
    synced: String,
    /// The outcome of the last web interface request, as
    /// [`fact::WEB_REQUEST`] says it.
    web_request: String,
    /// Set once the IPL reaches the storage service.
    sup: Option<Supervisor>,
    listener: Option<UnixListener>,
    /// The web server's listeners, handed over.
    web: web::Web,
    buttons: Vec<File>,
    /// The load source as the storage service last said.
    load_source: Option<storage::Watched>,
    /// The record and the slot rule; `None` on installation media.
    pub slots: Option<Slots>,
    /// What the screen shows.
    pub facts: Facts,
    /// Whether `/data` is the data file system.
    pub data_mounted: bool,
    ready: bool,
    /// The IPL is still at its steps: no status screen, no heartbeat.
    pub quiet: bool,
    /// `/data` is on the storage service's device.
    pub served: bool,
    /// The storage service is gone for good under a mounted `/data`.
    storage_lost: bool,
    shutdown: Option<Kind>,
    /// The shutdown has unmounted `/data` and is ending the storage
    /// service.
    unmounted: bool,
    heartbeat_at: Instant,
    /// A slot on trial: when it is given up if not confirmed.
    trial_until: Option<Instant>,
    /// The screen as last drawn on the serial port, without its date.
    drawn: Vec<String>,
    /// The date last drawn on the display.
    drawn_date: String,
    /// The serial port is the local console's: PID 1 writes nothing
    /// there until it is given back.
    serial_lent: bool,
    /// Why a dump is to be taken once every service is ready: a slot
    /// that did not confirm, a kernel panic.
    pub dump_when_ready: Vec<String>,
}

impl Host {
    /// A console that can post: the console is open, the signals are
    /// taken, and records wait for the log service.
    pub fn new(console: Console, signals: Signals, log_socket: &std::path::Path) -> Self {
        Self {
            console,
            log: Sender::new(log_socket),
            signals,
            reference: String::new(),
            synced: String::new(),
            web_request: String::new(),
            sup: None,
            listener: None,
            web: web::Web::default(),
            buttons: Vec::new(),
            load_source: None,
            slots: None,
            facts: Facts {
                slot: "unknown".into(),
                host: String::new(),
                serial: String::new(),
                zone: Zone::UTC,
                management: "none: the network service has not started".into(),
                private: "none configured".into(),
                machine: "not read yet".into(),
                data: "memory".into(),
                processor: String::new(),
            },
            data_mounted: false,
            ready: false,
            quiet: false,
            served: false,
            storage_lost: false,
            shutdown: None,
            unmounted: false,
            heartbeat_at: Instant::now() + HEARTBEAT,
            trial_until: None,
            drawn: Vec::new(),
            drawn_date: String::new(),
            serial_lent: false,
            dump_when_ready: Vec::new(),
        }
    }

    /// A line on the display, and on the serial port while it is PID 1's.
    fn show(&mut self, line: &str) {
        if self.serial_lent {
            self.console.display(line);
        } else {
            self.console.line(line);
        }
    }

    /// Lend the serial port to the local console, or take it back: the
    /// status screen is drawn again when it is back.
    pub fn lend_serial(&mut self, lent: bool) {
        self.serial_lent = lent;
        self.drawn.clear();
    }

    fn take(&mut self, posted: Posting) {
        self.show(&posted.line);
        let reference = posted
            .line
            .strip_prefix(refcode::WORD)
            .unwrap_or(&posted.line)
            .trim_start();
        self.reference = reference.to_string();
        let synced = format!("{} ", refcode::TIME_SYNCHRONIZED.code);
        if let Some(rest) = reference.strip_prefix(&synced) {
            let server = rest
                .strip_prefix(refcode::TIME_SYNCHRONIZED.text)
                .map_or(rest, |r| r.trim_start_matches(':').trim());
            self.synced = format!("{} {server}", now_secs());
        }
        self.log.send(posted.record);
    }

    /// Post a reference code: the console line and the history record.
    pub fn post(&mut self, code: &RefCode, detail: &str) {
        self.take(posting(code, detail));
    }

    /// Post a code of PID 1's own with what it is about.
    pub fn post_about(&mut self, code: &RefCode, detail: &str, about: &[refcode::post::About]) {
        self.take(refcode::post::posting_about(code, detail, about));
    }

    /// One line on the console that is no reference code, logged too.
    pub fn line(&mut self, line: &str) {
        self.show(line);
        self.say(Severity::Info, line);
    }

    /// A record of PID 1's own in the log.
    pub fn say(&mut self, severity: Severity, message: &str) {
        self.log.send(Record::new(SOURCE, severity, message));
    }

    /// Start the service table and take the control socket: from here
    /// the loop runs.
    pub fn start(&mut self, mut sup: Supervisor, listener: Option<UnixListener>) {
        let started = sup.advance(Instant::now());
        self.sup = Some(sup);
        self.listener = listener;
        // Installation media has no web server.
        if self.slots.is_some() {
            match web::Web::open() {
                Ok(w) => self.web = w,
                Err(e) => self.line(&format!("the web server's socket: {e}")),
            }
        }
        self.buttons = button::open();
        self.load_source = Some(storage::watch(&crate::product::QSMC));
        if self
            .slots
            .as_ref()
            .is_some_and(|s| s.ipl == loadsource::record::Ipl::Trial)
        {
            self.trial_until = Some(Instant::now() + trial_wait());
        }
        self.occurred(started);
    }

    fn occurred(&mut self, occurrences: Vec<Occurrence>) {
        for occurrence in occurrences {
            match occurrence {
                Occurrence::Started { service, pid } => {
                    self.say(Severity::Info, &format!("{service} started, process {pid}"));
                }
                Occurrence::Ready { service } => {
                    self.say(Severity::Info, &format!("{service} is ready"));
                    if Self::rule().due(Some(service)) {
                        self.confirm_slot();
                    }
                }
                Occurrence::AllReady => self.base_ready(),
                Occurrence::Failed {
                    service,
                    how,
                    restart_in,
                } => {
                    let next = restart_in.map_or_else(
                        || "not started again".to_string(),
                        |d| format!("started again in {} s", d.as_secs()),
                    );
                    self.say(Severity::Warning, &format!("{service}: {how}; {next}"));
                    // Its seccomp filter ended it (docs/SECURITY.md).
                    if how == SIGSYS_ENDED {
                        self.post_about(
                            &refcode::SERVICE_CONFINED,
                            service,
                            &[refcode::event::service(service)],
                        );
                        #[cfg(test_hooks)]
                        crate::hooks::tripped(service);
                    }
                    // A console that ended has no session any more.
                    if service == table::CONSOLE.name && self.serial_lent {
                        self.lend_serial(false);
                    }
                }
                Occurrence::GaveUp { service } if service == stgd::NAME && self.served => {
                    // The kernel holds every request to /data for a
                    // server that will not come: nothing can go on.
                    self.storage_lost = true;
                    self.post(&refcode::STORAGE_LOST, "the data file system has no server");
                }
                Occurrence::GaveUp { service } => {
                    self.post_about(
                        &refcode::SERVICE_STOPPED,
                        service,
                        &[refcode::event::service(service)],
                    );
                    self.dump_for(&format!("{service} stopped after its retries"));
                }
                Occurrence::Output {
                    service,
                    error,
                    line,
                } => {
                    let severity = if error {
                        Severity::Warning
                    } else {
                        Severity::Info
                    };
                    // The learning mode's reports, on the serial log too.
                    #[cfg(test_hooks)]
                    if line.contains(": confine: ") {
                        self.line(&line);
                    }
                    self.log.send(Record::new(service, severity, &line));
                }
                Occurrence::Killed { service } => {
                    self.line(&format!(
                        "{service} did not end when asked to and was killed"
                    ));
                }
                // The loop sees the end itself.
                Occurrence::Ended => {}
            }
        }
    }

    /// When a slot on trial confirms, by this processor's table.
    fn rule() -> crate::slot::Rule {
        rule(table::chosen(), table::WEBD.name)
    }

    /// The addresses the web server listens on: the `open` interface's.
    fn web_addresses(&self) -> Vec<std::net::Ipv4Addr> {
        web::address(&self.facts.management).into_iter().collect()
    }

    /// The `open` address may have changed: the listeners follow it.
    pub fn web_follow(&mut self) {
        let addresses = self.web_addresses();
        let done = self.web.follow(&addresses);
        self.web_said(done);
    }

    fn web_said(&mut self, done: Result<(), String>) {
        if let Err(e) = done {
            self.say(Severity::Warning, &format!("web listeners: {e}"));
        }
    }

    /// Every service is ready: the base is.
    fn base_ready(&mut self) {
        self.ready = true;
        let up = std::fs::read_to_string("/proc/uptime")
            .ok()
            .and_then(|t| t.split_ascii_whitespace().next().map(str::to_string))
            .unwrap_or_else(|| "?".into());
        self.post(
            &refcode::BASE_READY,
            &format!("{up} s after the kernel started"),
        );
        if Self::rule().due(None) {
            self.confirm_slot();
        }
        let why = std::mem::take(&mut self.dump_when_ready);
        if !why.is_empty() {
            self.dump_for(&why.join(", "));
        }
        // A button that appeared after the IPL looked.
        if self.buttons.is_empty() {
            self.buttons = button::open();
        }
    }

    /// The console is up by the slot rule: a slot on trial confirms, and
    /// so does a restore.
    fn confirm_slot(&mut self) {
        #[cfg(test_hooks)]
        if crate::hooks::has(crate::hooks::REFUSE_CONFIRM) {
            self.line("TEST HOOK: the slot is not confirmed");
            return;
        }
        // A restore on trial is kept by the same rule as a slot.
        if self.data_mounted
            && crate::ipl::restore::confirm(std::path::Path::new(release::DATA_DIR))
        {
            self.post(
                &refcode::RESTORE_CONFIRMED,
                "the state before it is deleted",
            );
        }
        if let Some(mut slots) = self.slots.take() {
            let trial = slots.on_trial();
            let mut posted = Vec::new();
            slots.confirm(|code, detail| posted.push(posting(code, &detail)));
            let confirmed = trial && !slots.on_trial();
            let booted = loadsource::record::letter(slots.booted).to_string();
            if slots.ipl == loadsource::record::Ipl::Normal {
                self.trial_until = None;
            }
            self.slots = Some(slots);
            for p in posted {
                self.take(p);
            }
            if confirmed {
                self.updated(&booted);
            }
        }
    }

    /// A pending slot confirmed: said, and the copies of state kept for
    /// its trial dropped by a thread, since the loop never waits on
    /// `/data`.
    fn updated(&mut self, slot: &str) {
        self.post_about(
            &refcode::UPDATE_CONFIRMED,
            &format!(
                "{} build {} in slot {slot}",
                release::RELEASE,
                release::BUILD_ID
            ),
            &[refcode::event::slot(slot)],
        );
        std::thread::spawn(|| {
            let data = std::path::Path::new(release::DATA_DIR);
            if let Err(e) = ::update::state_copy::confirmed(data) {
                eprintln!("the copy of state was not dropped: {e}");
            }
            if let Err(e) = crate::ipl::carry::confirmed(data) {
                eprintln!("the copies of the console's folders were not dropped: {e}");
            }
        });
    }

    /// The status: the facts in the screen's order, then the services.
    pub fn status_now(&self) -> supervise::control::HostStatus {
        let now = now_secs();
        let record = self
            .slots
            .as_ref()
            .map_or_else(|| "no A/B record".to_string(), Slots::summary);
        let image = if cfg!(test_hooks) { "test" } else { "release" };
        let load_source = self.load_source.as_ref().map_or_else(
            || "not known: the storage service has not started".to_string(),
            storage::Watched::get,
        );
        let facts = [
            (fact::PRODUCT, release::NAME.to_string()),
            (fact::RELEASE, release::RELEASE.to_string()),
            (fact::BUILD, release::BUILD_ID.to_string()),
            (fact::IMAGE, image.to_string()),
            (fact::SIGNATURE, signature()),
            (fact::SLOT, self.facts.slot.clone()),
            (fact::RECORD, record),
            (fact::HOST, self.facts.host.clone()),
            (fact::MANAGEMENT, self.facts.management.clone()),
            (fact::PRIVATE, self.facts.private.clone()),
            (fact::TIME, now.to_string()),
            (fact::DATE, self.facts.zone.display(now)),
            (fact::LOAD_SOURCE, load_source),
            (fact::REFERENCE, self.reference.clone()),
            (fact::SYNCED, self.synced.clone()),
            (fact::WEB_REQUEST, self.web_request.clone()),
            (fact::DATA, self.facts.data.clone()),
            (
                fact::READY,
                if self.ready { "yes" } else { "no" }.to_string(),
            ),
            (fact::PROCESSOR, self.facts.processor.clone()),
            (fact::MACHINE, self.facts.machine.clone()),
        ];
        supervise::control::HostStatus {
            facts: facts.into_iter().map(|(k, v)| (k.to_string(), v)).collect(),
            services: self
                .sup
                .as_ref()
                .map(Supervisor::status)
                .unwrap_or_default(),
        }
    }

    /// Draw the screen where it changed: on the serial port when
    /// anything but the date did, on the display for the date too.
    fn draw(&mut self) {
        let status = self.status_now();
        let date = status.fact(fact::DATE).unwrap_or("").to_string();
        let lines = screen::lines(&release::banner(), &status);
        let undated: Vec<String> = lines
            .iter()
            .filter(|l| !l.ends_with(&date) || date.is_empty())
            .cloned()
            .collect();
        let changed = undated != self.drawn;
        if changed && !self.serial_lent {
            self.console.serial_screen(&lines);
            self.drawn = undated;
        }
        if changed || date != self.drawn_date {
            self.console.display_screen(&lines);
            self.drawn_date = date;
        }
    }

    /// Wait for something to do, at most until `deadline`.
    fn wait(&mut self, deadline: Instant) -> Vec<Ready> {
        let left = deadline.saturating_duration_since(Instant::now());
        let timeout = Timespec {
            tv_sec: i64::try_from(left.as_secs()).unwrap_or(1),
            tv_nsec: i64::from(left.subsec_nanos()),
        };
        let mut what = vec![Ready::Signal];
        let mut fds = vec![PollFd::from_borrowed_fd(self.signals.fd(), PollFlags::IN)];
        if let Some(listener) = &self.listener {
            what.push(Ready::Control);
            fds.push(PollFd::from_borrowed_fd(listener.as_fd(), PollFlags::IN));
        }
        if let Some(fd) = self.web.fd() {
            what.push(Ready::Web);
            fds.push(PollFd::from_borrowed_fd(fd, PollFlags::IN));
        }
        for (i, device) in self.buttons.iter().enumerate() {
            what.push(Ready::Button(i));
            fds.push(PollFd::from_borrowed_fd(device.as_fd(), PollFlags::IN));
        }
        if let Some(sup) = &self.sup {
            for (token, fd) in sup.fds() {
                what.push(Ready::Service(token));
                fds.push(PollFd::from_borrowed_fd(fd, PollFlags::IN));
            }
        }
        if poll(&mut fds, Some(&timeout)).is_err() {
            return Vec::new();
        }
        what.into_iter()
            .zip(&fds)
            .filter(|(_, fd)| !fd.revents().is_empty())
            .map(|(ready, _)| ready)
            .collect()
    }

    fn signalled(&mut self, now: Instant) {
        for signal in self.signals.read() {
            match signal {
                CHLD => {
                    for (pid, how) in reap::ended() {
                        if let Some(sup) = &mut self.sup {
                            let (_, occurrences) = sup.exited(pid, &how, now);
                            self.occurred(occurrences);
                        }
                    }
                }
                TERM => self.begin_shutdown(Kind::PowerOff, "SIGTERM"),
                // Ctrl-Alt-Del, which the kernel hands to PID 1.
                INT => self.line("Ctrl-Alt-Del ignored"),
                _ => {}
            }
        }
    }

    /// One turn of the loop.
    fn turn(&mut self) {
        // Records the log service has not taken yet are offered again soon.
        let tick = if self.log.waiting() > 0 { RETRY } else { TICK };
        let mut deadline = (Instant::now() + tick).min(self.heartbeat_at);
        if let Some(at) = self.sup.as_ref().and_then(Supervisor::deadline) {
            deadline = deadline.min(at);
        }
        if let Some(at) = self.trial_until {
            deadline = deadline.min(at);
        }
        let ready = self.wait(deadline);
        let now = Instant::now();
        if self.trial_until.is_some_and(|at| now >= at) && self.shutdown.is_none() {
            self.trial_until = None;
            self.line("the slot on trial did not confirm in time: the other slot is started");
            self.begin_shutdown(Kind::Restart, "the trial's deadline");
        }
        for what in ready {
            match what {
                Ready::Signal => self.signalled(now),
                Ready::Control => self.serve_control(),
                Ready::Web => {
                    let addresses = self.web_addresses();
                    let done = self.web.accept(&addresses);
                    self.web_said(done);
                }
                Ready::Button(i) => {
                    if self.buttons.get(i).is_some_and(button::pressed) {
                        self.begin_shutdown(Kind::PowerOff, "the power button");
                    }
                }
                Ready::Service(token) => {
                    if let Some(sup) = &mut self.sup {
                        let occurrences = sup.readable(token, now);
                        self.occurred(occurrences);
                    }
                }
            }
        }
        if let Some(sup) = &mut self.sup {
            let occurrences = sup.advance(now);
            self.occurred(occurrences);
        }
        if now >= self.heartbeat_at {
            if !self.quiet && !self.serial_lent {
                self.console.serial(&refcode::HEARTBEAT.line());
            }
            self.heartbeat_at += HEARTBEAT;
            // A set of listeners that could not be bound is tried again.
            if self.web.retry {
                self.web_follow();
            }
        }
        self.log.flush();
        if self.shutdown.is_some() {
            self.advance_shutdown();
        } else if !self.quiet {
            self.draw();
        }
    }

    /// The services up to the storage service have settled: each is
    /// ready or will not be started again.
    pub fn storage_settled(&self) -> bool {
        self.sup
            .as_ref()
            .is_none_or(|sup| sup.settled_up_to(table::STGD.order))
    }

    /// The storage service waits on the operator: it is not late.
    pub fn storage_waits(&mut self) {
        if let Some(sup) = &mut self.sup {
            sup.patient(table::STGD.name, Instant::now());
        }
    }

    /// Start the services that waited for `/data`.
    pub fn release_services(&mut self) {
        if let Some(sup) = &mut self.sup {
            let started = sup.release(Instant::now());
            self.occurred(started);
        }
    }

    /// One turn of the loop, for an IPL step that waits on a service.
    pub fn pump(&mut self) {
        self.turn();
    }

    /// Run for ever: PID 1 must not exit.
    pub fn run(mut self) -> ! {
        loop {
            self.turn();
        }
    }
}

/// The signature fact: the key that signed the image, and whether the
/// firmware checks it (`docs/RELEASING.md`).
fn signature() -> String {
    let key = if release::DEVELOPMENT_KEY {
        "development key"
    } else {
        "release key"
    };
    let boot = match loadsource::boot::secure_boot(std::path::Path::new(loadsource::boot::EFIVARS))
    {
        Some(true) => "on",
        Some(false) => "off",
        None => "unknown",
    };
    format!("{key}, Secure Boot {boot}")
}
