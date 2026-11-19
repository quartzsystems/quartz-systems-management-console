// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! The sign-on rules: five wrong passwords in a row disable an ID for
//! five minutes (as `base/users` does at the local console); sign-on
//! attempts from one address are limited to a number a minute. Both
//! are kept in memory: a restart of the web server forgets them.

use std::collections::{HashMap, VecDeque};
use std::net::IpAddr;

use crate::list::WebUsers;
use crate::role::Role;

/// Wrong passwords in a row before an ID is disabled for a while.
pub const MAX_FAILURES: u32 = users::MAX_FAILURES as u32;
/// Seconds an ID stays disabled.
pub const LOCK_SECS: u64 = users::LOCK_SECS;
/// Sign-on attempts one address may make in [`WINDOW_SECS`].
pub const PER_ADDRESS: usize = 20;
/// The window of [`PER_ADDRESS`].
pub const WINDOW_SECS: u64 = 60;

/// Why a sign-on was refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refused {
    /// The ID or the password is not correct (never which).
    NotCorrect,
    /// The ID is disabled for this many more seconds.
    Locked(u64),
    /// The ID is disabled by an Administrator.
    Disabled,
    /// Too many attempts from this address: try again in this many
    /// seconds.
    TooMany(u64),
}

/// What the web server remembers between sign-ons.
#[derive(Debug, Default)]
pub struct Rules {
    failures: HashMap<String, u32>,
    locked: HashMap<String, u64>,
    attempts: HashMap<IpAddr, VecDeque<u64>>,
}

impl Rules {
    /// Check `id` and `password` from `from` at `now`: its role when
    /// they are right.
    pub fn sign_on(
        &mut self,
        list: &WebUsers,
        id: &str,
        password: &str,
        from: IpAddr,
        now: u64,
    ) -> Result<(String, Role), Refused> {
        let seen = self.attempts.entry(from).or_default();
        while seen
            .front()
            .is_some_and(|t| now.saturating_sub(*t) >= WINDOW_SECS)
        {
            seen.pop_front();
        }
        if seen.len() >= PER_ADDRESS {
            let wait = seen
                .front()
                .map_or(WINDOW_SECS, |t| WINDOW_SECS - now.saturating_sub(*t));
            return Err(Refused::TooMany(wait));
        }
        seen.push_back(now);
        let key = id.trim().to_ascii_lowercase();
        if let Some(until) = self.locked.get(&key).copied() {
            if until > now {
                return Err(Refused::Locked(until - now));
            }
            self.locked.remove(&key);
        }
        let user = list.get(&key);
        // A hash is checked even for an ID that is not there, so the
        // time taken does not say which it was.
        let hash = user.map_or(DUMMY, |u| u.hash.as_str());
        let right = svcusers::verify_password(hash, password) && user.is_some();
        let Some(user) = user.filter(|_| right) else {
            if list.get(&key).is_some() {
                let n = self.failures.entry(key.clone()).or_default();
                *n += 1;
                if *n >= MAX_FAILURES {
                    self.failures.remove(&key);
                    self.locked.insert(key, now + LOCK_SECS);
                    return Err(Refused::Locked(LOCK_SECS));
                }
            }
            return Err(Refused::NotCorrect);
        };
        if !user.enabled {
            return Err(Refused::Disabled);
        }
        self.failures.remove(&key);
        Ok((user.id.clone(), user.role))
    }

    /// Forget `id`'s failures and lock: its password was reset.
    pub fn clear(&mut self, id: &str) {
        let key = id.trim().to_ascii_lowercase();
        self.failures.remove(&key);
        self.locked.remove(&key);
    }
}

/// A hash of no one's password, checked for an ID that is not there.
const DUMMY: &str = "$argon2id$v=19$m=19456,t=2,p=1$c2FsdHNhbHRzYWx0c2FsdA$Zm9vYmFyZm9vYmFyZm9vYmFyZm9vYmFyZm9vYmFyZm8";

#[cfg(test)]
mod tests {
    use super::*;

    fn list() -> WebUsers {
        WebUsers::first("qsadmin", &users::hash("Right-pass-1").unwrap(), 1).unwrap()
    }

    #[test]
    fn five_wrong_passwords_disable_an_id_for_five_minutes() {
        let l = list();
        let mut r = Rules::default();
        let ip: IpAddr = "192.0.2.5".parse().unwrap();
        for _ in 0..4 {
            assert_eq!(
                r.sign_on(&l, "qsadmin", "wrong", ip, 100),
                Err(Refused::NotCorrect)
            );
        }
        assert_eq!(
            r.sign_on(&l, "qsadmin", "wrong", ip, 100),
            Err(Refused::Locked(300))
        );
        assert_eq!(
            r.sign_on(&l, "qsadmin", "Right-pass-1", ip, 200),
            Err(Refused::Locked(200))
        );
        assert_eq!(
            r.sign_on(&l, "QSADMIN", "Right-pass-1", ip, 400),
            Ok(("qsadmin".into(), Role::Administrator))
        );
        assert_eq!(
            r.sign_on(&l, "nobody", "x", ip, 400),
            Err(Refused::NotCorrect)
        );
    }

    #[test]
    fn an_address_is_limited() {
        let l = list();
        let mut r = Rules::default();
        let ip: IpAddr = "192.0.2.6".parse().unwrap();
        for _ in 0..PER_ADDRESS {
            let _ = r.sign_on(&l, "nobody", "x", ip, 1000);
        }
        assert_eq!(
            r.sign_on(&l, "qsadmin", "Right-pass-1", ip, 1010),
            Err(Refused::TooMany(50))
        );
        let other: IpAddr = "192.0.2.7".parse().unwrap();
        assert!(
            r.sign_on(&l, "qsadmin", "Right-pass-1", other, 1010)
                .is_ok()
        );
        assert!(r.sign_on(&l, "qsadmin", "Right-pass-1", ip, 1060).is_ok());
    }

    #[test]
    fn a_disabled_user_cannot_sign_on() {
        let mut l = list();
        l.add("op", "", Role::Operator, "Secret-word-1", 2).unwrap();
        l.set_enabled("op", false).unwrap();
        let mut r = Rules::default();
        let ip: IpAddr = "192.0.2.8".parse().unwrap();
        assert_eq!(
            r.sign_on(&l, "op", "Secret-word-1", ip, 5),
            Err(Refused::Disabled)
        );
    }
}
