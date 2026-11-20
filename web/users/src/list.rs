// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! `webusers.toml`: the web users and the role matrix, written only by
//! the web server through `base/state`. A user is an ID, a description,
//! a role, an Argon2id password hash, whether it is enabled, and when
//! and from where it last signed on. No password is kept anywhere.
//!
//! The list is apart from the local console's service tools users
//! (`base/users`); only the first Administrator is made from the first
//! of them, once ([`WebUsers::first`]).

use serde::{Deserialize, Serialize};

use crate::role::{Matrix, Role};

/// The longest user ID.
pub const ID_MAX: usize = 32;

/// One web user.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct User {
    /// The ID, lower case.
    pub id: String,
    /// What it is for.
    #[serde(default)]
    pub description: String,
    /// Its role.
    pub role: Role,
    /// The Argon2id hash of its password, PHC form.
    pub hash: String,
    /// Whether it may sign on.
    pub enabled: bool,
    /// When it last signed on, Unix seconds; 0 never.
    #[serde(default)]
    pub last_sign_on: u64,
    /// The address it last signed on from.
    #[serde(default)]
    pub last_from: String,
    /// When its password was last set, Unix seconds.
    #[serde(default)]
    pub changed: u64,
}

/// `webusers.toml`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WebUsers {
    /// The users, in the order they were made.
    #[serde(default, rename = "user")]
    pub users: Vec<User>,
    /// The role matrix.
    #[serde(default)]
    pub matrix: Matrix,
}

impl state::Document for WebUsers {
    const FILE: &'static str = "webusers.toml";
    const VERSION: u32 = 1;
    const MIGRATIONS: &'static [state::Migration] = &[];

    fn validate(&self) -> Result<(), String> {
        for (i, u) in self.users.iter().enumerate() {
            check_id(&u.id)?;
            if self.users[..i].iter().any(|o| o.id == u.id) {
                return Err(format!("user ID {} is listed twice", u.id));
            }
            if !users::valid_hash(&u.hash) {
                return Err(format!("user ID {} has no Argon2id hash", u.id));
            }
        }
        if !self.users.is_empty() && self.enabled_administrators() == 0 {
            return Err("no enabled Administrator".into());
        }
        Ok(())
    }
}

/// A user ID as kept: lower case, 1 to 32 letters, digits, `.`, `_`
/// and `-`, a letter first.
pub fn check_id(id: &str) -> Result<String, String> {
    let id = id.trim().to_ascii_lowercase();
    let ok = !id.is_empty()
        && id.len() <= ID_MAX
        && id.starts_with(|c: char| c.is_ascii_alphabetic())
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'_' || b == b'-');
    if ok {
        Ok(id)
    } else {
        Err(format!(
            "A user ID is 1 to {ID_MAX} letters, digits, periods, underscores and hyphens, starting with a letter."
        ))
    }
}

/// Why a change to the list was refused.
pub const LAST_ADMIN: &str =
    "The last enabled Administrator cannot be removed, disabled or given another role.";

impl WebUsers {
    /// The list with one user: the first Administrator, from the local
    /// console's first service tools user's ID and hash.
    pub fn first(id: &str, hash: &str, now: u64) -> Result<Self, String> {
        let id = check_id(id)?;
        if !users::valid_hash(hash) {
            return Err("The service tools user's password hash is not an Argon2id hash.".into());
        }
        Ok(Self {
            users: vec![User {
                id,
                description: "First Administrator, from the installation".into(),
                role: Role::Administrator,
                hash: hash.into(),
                enabled: true,
                last_sign_on: 0,
                last_from: String::new(),
                changed: now,
            }],
            matrix: Matrix::default(),
        })
    }

    /// The user `id`.
    pub fn get(&self, id: &str) -> Option<&User> {
        let id = id.trim().to_ascii_lowercase();
        self.users.iter().find(|u| u.id == id)
    }

    fn get_mut(&mut self, id: &str) -> Result<&mut User, String> {
        let id = id.trim().to_ascii_lowercase();
        self.users
            .iter_mut()
            .find(|u| u.id == id)
            .ok_or_else(|| format!("There is no user ID {id}."))
    }

    /// How many enabled Administrators there are.
    pub fn enabled_administrators(&self) -> usize {
        self.users
            .iter()
            .filter(|u| u.enabled && u.role == Role::Administrator)
            .count()
    }

    /// Whether `id` is the last enabled Administrator.
    pub fn is_last_admin(&self, id: &str) -> bool {
        self.get(id)
            .is_some_and(|u| u.enabled && u.role == Role::Administrator)
            && self.enabled_administrators() == 1
    }

    /// Add a user with `password`, hashed here.
    pub fn add(
        &mut self,
        id: &str,
        description: &str,
        role: Role,
        password: &str,
        now: u64,
    ) -> Result<(), String> {
        let id = check_id(id)?;
        if self.get(&id).is_some() {
            return Err(format!("User ID {id} exists already."));
        }
        users::check_password(&id, password)?;
        let hash = users::hash(password).map_err(|e| format!("No random bytes: {e}"))?;
        self.users.push(User {
            id,
            description: description.trim().into(),
            role,
            hash,
            enabled: true,
            last_sign_on: 0,
            last_from: String::new(),
            changed: now,
        });
        Ok(())
    }

    /// Give `id` a new password.
    pub fn set_password(&mut self, id: &str, password: &str, now: u64) -> Result<(), String> {
        let id = check_id(id)?;
        users::check_password(&id, password)?;
        let hash = users::hash(password).map_err(|e| format!("No random bytes: {e}"))?;
        let u = self.get_mut(&id)?;
        u.hash = hash;
        u.changed = now;
        Ok(())
    }

    /// Change `id`'s description and role.
    pub fn set_properties(
        &mut self,
        id: &str,
        description: &str,
        role: Role,
    ) -> Result<(), String> {
        if role != Role::Administrator && self.is_last_admin(id) {
            return Err(LAST_ADMIN.into());
        }
        let u = self.get_mut(id)?;
        u.description = description.trim().into();
        u.role = role;
        Ok(())
    }

    /// Enable or disable `id`.
    pub fn set_enabled(&mut self, id: &str, enabled: bool) -> Result<(), String> {
        if !enabled && self.is_last_admin(id) {
            return Err(LAST_ADMIN.into());
        }
        self.get_mut(id)?.enabled = enabled;
        Ok(())
    }

    /// Remove `id`.
    pub fn remove(&mut self, id: &str) -> Result<(), String> {
        if self.is_last_admin(id) {
            return Err(LAST_ADMIN.into());
        }
        let id = id.trim().to_ascii_lowercase();
        let before = self.users.len();
        self.users.retain(|u| u.id != id);
        if self.users.len() == before {
            return Err(format!("There is no user ID {id}."));
        }
        Ok(())
    }

    /// `id` signed on at `now` from `from`.
    pub fn signed_on(&mut self, id: &str, now: u64, from: &str) {
        if let Ok(u) = self.get_mut(id) {
            u.last_sign_on = now;
            from.clone_into(&mut u.last_from);
        }
    }

    /// Make or reset an Administrator for the local console: `id` with
    /// the password of `hash` (the local console checks the rules and
    /// hashes), enabled and Administrator, made when it is not there.
    pub fn reset_admin(&mut self, id: &str, hash: &str, now: u64) -> Result<(), String> {
        let id = check_id(id)?;
        if !users::valid_hash(hash) {
            return Err("The password hash is not an Argon2id hash.".into());
        }
        if self.get(&id).is_none() {
            self.users.push(User {
                id,
                description: "Made at the local console".into(),
                role: Role::Administrator,
                hash: hash.into(),
                enabled: true,
                last_sign_on: 0,
                last_from: String::new(),
                changed: now,
            });
            return Ok(());
        }
        let u = self.get_mut(&id)?;
        hash.clone_into(&mut u.hash);
        u.changed = now;
        u.enabled = true;
        u.role = Role::Administrator;
        Ok(())
    }
}

/// The first service tools user's ID and hash, from the local console's
/// users' file.
pub fn first_service_user(bytes: &[u8]) -> Option<(String, String)> {
    let db = svcusers::UserDb::decode(bytes)?;
    let u = db.list().first()?;
    Some((u.name.clone(), u.hash.clone()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use state::Document as _;

    fn hash() -> String {
        users::hash("Passw0rd!long").unwrap()
    }

    #[test]
    fn the_first_administrator_comes_from_the_service_tools_user() {
        let h = hash();
        let bytes = users::first("QSADMIN", &h, 1).unwrap();
        let (id, got) = first_service_user(&bytes).unwrap();
        assert_eq!((id.as_str(), got.as_str()), ("QSADMIN", h.as_str()));
        let list = WebUsers::first(&id, &got, 1).unwrap();
        let u = list.get("QSAdmin").unwrap();
        assert_eq!(
            (u.id.as_str(), u.role, u.enabled),
            ("qsadmin", Role::Administrator, true)
        );
        assert!(svcusers::verify_password(&u.hash, "Passw0rd!long"));
        assert_eq!(list.validate(), Ok(()));
    }

    #[test]
    fn the_last_enabled_administrator_is_kept() {
        let mut list = WebUsers::first("qsadmin", &hash(), 1).unwrap();
        assert_eq!(list.remove("qsadmin"), Err(LAST_ADMIN.into()));
        assert_eq!(list.set_enabled("qsadmin", false), Err(LAST_ADMIN.into()));
        assert_eq!(
            list.set_properties("qsadmin", "", Role::Operator),
            Err(LAST_ADMIN.into())
        );
        list.add("second", "", Role::Administrator, "An0ther-long-one", 2)
            .unwrap();
        assert_eq!(list.set_enabled("qsadmin", false), Ok(()));
        assert_eq!(list.set_enabled("second", false), Err(LAST_ADMIN.into()));
        assert_eq!(list.remove("second"), Err(LAST_ADMIN.into()));
        list.set_enabled("qsadmin", true).unwrap();
        assert_eq!(list.remove("second"), Ok(()));
    }

    #[test]
    fn ids_and_passwords_follow_the_rules() {
        let mut list = WebUsers::first("qsadmin", &hash(), 1).unwrap();
        assert!(check_id("9lives").is_err());
        assert!(check_id(&"a".repeat(33)).is_err());
        assert_eq!(check_id(" Operator1 "), Ok("operator1".into()));
        assert!(list.add("op", "", Role::Operator, "short", 2).is_err());
        assert!(
            list.add("QSADMIN", "", Role::Operator, "Long-enough-1", 2)
                .is_err()
        );
        assert!(list.reset_admin("rescue", "not a hash", 3).is_err());
        list.reset_admin("rescue", &hash(), 3).unwrap();
        assert!(
            list.get("rescue")
                .is_some_and(|u| u.role == Role::Administrator)
        );
    }
}
