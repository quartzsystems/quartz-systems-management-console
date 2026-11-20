// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! The console's pages as `webd` serves them: sign-on and its rules,
//! the session table, the task each route needs, and the pages. Every
//! change made through it is audited ([`App::audit`]).

#[cfg(target_os = "linux")]
pub mod live;
pub mod routes;
pub mod settings;

use std::net::IpAddr;
use std::path::Path;
use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use qsweb::http::{Peer, Request, Response};
use qsweb::session::Sessions;
use state::Store;
use webusers::{Role, Rules, WebUsers};

pub use settings::WebSettings;

/// What the pages need of the console besides their own files.
pub trait Console: Send + Sync {
    /// The console's name.
    fn name(&self) -> String;
    /// Seconds since the console started: what sessions, the lock-out
    /// and a trial are timed by, as setting the clock does not move it.
    fn uptime(&self) -> u64;
    /// Unix seconds now.
    fn now(&self) -> u64;
    /// A fact of PID 1's status (`release::fact`), empty when it has none.
    fn fact(&self, key: &str) -> String;

    /// The network service's interfaces, in `network.toml`'s order.
    fn interfaces(&self) -> Vec<netd::control::NetStatus> {
        Vec::new()
    }

    /// Put `network` (a `network.toml`) on trial through the network
    /// service.
    fn apply_network(&self, _network: &str) -> Result<(), String> {
        Err("There is no network service.".into())
    }

    /// Put `network` on trial, kept only when [`Console::confirm_network`]
    /// is called within `seconds`.
    fn apply_network_confirmed(&self, _network: &str, _seconds: u32) -> Result<(), String> {
        Err("There is no network service.".into())
    }

    /// Keep the change on trial.
    fn confirm_network(&self) -> Result<(), String> {
        Err("There is no network service.".into())
    }

    /// Set the host name, display zone and time servers through PID 1.
    fn set_host(&self, _name: &str, _zone: &str, _ntp: &[String]) -> Result<(), String> {
        Err("PID 1 did not answer.".into())
    }

    /// Set the clock (and the RTC) through the network service.
    fn set_clock(&self, _secs: i64) -> Result<(), String> {
        Err("There is no network service.".into())
    }

    /// Power off or restart through PID 1, for `by`.
    fn power_down(&self, _restart: bool, _by: &str) -> Result<(), String> {
        Err("PID 1 did not answer.".into())
    }
}

/// The web server's pages and what they keep.
pub struct App {
    /// The session table.
    pub sessions: Sessions,
    rules: Mutex<Rules>,
    store: Store,
    users: Mutex<WebUsers>,
    settings: Mutex<WebSettings>,
    /// The console.
    pub console: Box<dyn Console>,
    /// A change of the `open` address on trial: the address it is
    /// kept at (empty: any, with DHCP) and until when (uptime).
    pub pending: Mutex<Option<(String, u64)>>,
    /// The web certificate, once the server has started.
    pub certs: std::sync::OnceLock<std::sync::Arc<webd::tls::Certs>>,
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Session values: the role at sign-on, when it began, where from.
pub const ROLE: &str = "role";
/// See [`ROLE`]. The uptime the session began at.
pub const SINCE: &str = "since";
/// See [`ROLE`].
pub const FROM: &str = "from";

impl App {
    /// The pages over the state in `state_dir`. The users' list is made
    /// at the first start, its first Administrator from the local
    /// console's first service tools user (`users_db`).
    pub fn open(
        state_dir: &Path,
        users_db: &Path,
        console: Box<dyn Console>,
    ) -> Result<Self, String> {
        let store = Store::at(state_dir);
        let users = if let Some(u) = store.load::<WebUsers>().map_err(|e| e.to_string())? {
            u
        } else {
            let first = std::fs::read(users_db)
                .ok()
                .and_then(|b| webusers::list::first_service_user(&b));
            if let Some((id, hash)) = first {
                let first = WebUsers::first(&id, &hash, console.now())?;
                store.save(&first).map_err(|e| e.to_string())?;
                eprintln!("the first web Administrator was made from service tools user {id}");
                first
            } else {
                // Nobody can sign on until the local console makes an
                // Administrator (option 4); the server runs meanwhile.
                eprintln!(
                    "no service tools user in {}: no web user until one is made at the local console",
                    users_db.display()
                );
                WebUsers::default()
            }
        };
        let settings = store
            .load::<WebSettings>()
            .map_err(|e| e.to_string())?
            .unwrap_or_default();
        let sessions = Sessions::new(Duration::from_secs(u64::from(settings.idle_minutes) * 60));
        Ok(Self {
            sessions,
            rules: Mutex::new(Rules::default()),
            store,
            users: Mutex::new(users),
            settings: Mutex::new(settings),
            console,
            certs: std::sync::OnceLock::new(),
            pending: Mutex::new(None),
        })
    }

    /// The users' list.
    pub fn users(&self) -> MutexGuard<'_, WebUsers> {
        lock(&self.users)
    }

    /// Change the users' list: written before it is taken into use.
    pub fn change_users(
        &self,
        f: impl FnOnce(&mut WebUsers) -> Result<(), String>,
    ) -> Result<(), String> {
        let mut users = lock(&self.users);
        let mut next = users.clone();
        f(&mut next)?;
        self.store.save(&next).map_err(|e| e.to_string())?;
        *users = next;
        Ok(())
    }

    /// `host.toml`: the name, the display zone, the time servers.
    pub fn host(&self) -> state::Host {
        self.store
            .load::<state::Host>()
            .ok()
            .flatten()
            .unwrap_or_default()
    }

    /// The state directory's store.
    pub fn store(&self) -> &Store {
        &self.store
    }

    /// The settings.
    pub fn settings(&self) -> WebSettings {
        lock(&self.settings).clone()
    }

    /// Change the settings: written before they are taken into use.
    pub fn change_settings(&self, next: WebSettings) -> Result<(), String> {
        use state::Document as _;
        next.validate()?;
        self.store.save(&next).map_err(|e| e.to_string())?;
        self.sessions
            .set_idle(Duration::from_secs(u64::from(next.idle_minutes) * 60));
        *lock(&self.settings) = next;
        Ok(())
    }

    /// Check a sign-on.
    pub fn sign_on(
        &self,
        id: &str,
        password: &str,
        from: IpAddr,
    ) -> Result<(String, Role), webusers::Refused> {
        let now = self.console.uptime();
        let users = lock(&self.users);
        lock(&self.rules).sign_on(&users, id, password, from, now)
    }

    /// Forget an ID's failures: its password was set.
    pub fn clear_failures(&self, id: &str) {
        lock(&self.rules).clear(id);
    }

    /// Record a change: who, from where, what, and how it ended. Never
    /// a password, a passphrase or a key.
    pub fn audit(&self, user: &str, from: &str, what: &str, outcome: &str) {
        // Standard error reaches the log through PID 1's pipe.
        eprintln!("audit: user {user} from {from}: {what}: {outcome}");
    }
}

impl webd::serve::Pages for App {
    fn page(&self, req: &Request, peer: &Peer) -> Response {
        routes::route(self, req, peer)
    }

    fn started(&self, certs: std::sync::Arc<webd::tls::Certs>) {
        let _ = self.certs.set(certs);
    }

    fn admits(&self, ip: IpAddr) -> bool {
        settings::admits(&lock(&self.settings).allow, ip)
    }
}
