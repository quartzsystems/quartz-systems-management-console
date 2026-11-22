// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! A slot on trial and the state an older release wrote
//! (`docs/UPDATES.md`). A release reads an older schema by migrating it
//! in memory; the file is written in this release's form only once the
//! slot that runs is no longer on trial, so a trial that rolls back
//! never leaves the older release a form it cannot read. Each service
//! that owns state files asks [`after`] to [`migrate`] them.

use std::time::Duration;

use supervise::control::{Calls as _, Client};

/// How often a service on trial asks PID 1 whether it confirmed.
const LOOK: Duration = Duration::from_secs(5);

/// Whether this IPL is a pending slot's trial, by PID 1's record: the
/// slot that booted is the one pending, and was activated. `None` when
/// PID 1 does not answer.
pub fn on_trial() -> Option<bool> {
    let s = Client::new(crate::CONTROL_SOCKET).slots().ok()?;
    Some(!s.pending.is_empty() && s.pending == s.booted && s.activated)
}

/// Run `f` on a thread once the slot that runs is not on trial: at once,
/// or when it confirms. Never, when it does not.
pub fn after(f: impl FnOnce() + Send + 'static) {
    std::thread::spawn(move || {
        loop {
            match on_trial() {
                Some(false) => break,
                // On trial, or PID 1 busy: asked again.
                Some(true) | None => std::thread::sleep(LOOK),
            }
        }
        f();
    });
}

/// The schema version written in the file at `text`: its
/// `schema_version` line.
pub fn written_version(text: &str) -> Option<u32> {
    text.lines().find_map(|l| {
        l.strip_prefix(state::store::SCHEMA_KEY)?
            .trim_start()
            .strip_prefix('=')?
            .trim()
            .parse()
            .ok()
    })
}

/// Write `D`'s file in this release's form when an older release wrote
/// it: whether it was.
///
/// # Errors
/// It did not read or could not be written; it is as it was.
pub fn migrate<D: state::Document>(store: &state::Store) -> Result<bool, String> {
    let Ok(text) = std::fs::read_to_string(store.path::<D>()) else {
        return Ok(false);
    };
    if written_version(&text).is_none_or(|v| v >= D::VERSION) {
        return Ok(false);
    }
    let doc: D = store
        .load()
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("{} is gone", D::FILE))?;
    store.save(&doc).map_err(|e| e.to_string())?;
    Ok(true)
}

/// One file's migration: [`migrate`] of its type.
pub type Step = fn(&state::Store) -> Result<bool, String>;

/// Once the slot that runs is not on trial, [`migrate`] each of `files`
/// in the state directory `dir`, and `post` the names of those written
/// again (the service's code for it); `log` says what failed.
pub fn migrate_after(
    dir: std::path::PathBuf,
    files: &'static [(&'static str, Step)],
    post: fn(&str),
    log: fn(&str),
) {
    after(move || {
        let store = state::Store::at(&dir);
        let mut done = Vec::new();
        for (name, step) in files {
            match step(&store) {
                Ok(true) => done.push(*name),
                Ok(false) => {}
                Err(e) => log(&format!(
                    "{name} was not written in this release's form: {e}"
                )),
            }
        }
        if !done.is_empty() {
            post(&done.join(", "));
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(serde::Serialize, serde::Deserialize)]
    struct Two {
        name: String,
        #[serde(default)]
        added: u32,
    }

    #[allow(clippy::unnecessary_wraps, reason = "a migration step's form")]
    fn add(t: &mut toml::Table) -> Result<(), String> {
        t.insert("added".into(), 7.into());
        Ok(())
    }

    impl state::Document for Two {
        const FILE: &'static str = "two.toml";
        const VERSION: u32 = 2;
        const MIGRATIONS: &'static [state::Migration] = &[add];
        fn validate(&self) -> Result<(), String> {
            Ok(())
        }
    }

    #[test]
    fn a_file_an_older_release_wrote_is_written_again_once() {
        let d = std::env::temp_dir().join(format!("qsmc-trial-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        let store = state::Store::at(&d);
        assert_eq!(migrate::<Two>(&store), Ok(false));
        std::fs::write(d.join("two.toml"), "schema_version = 1\nname = \"x\"\n").unwrap();
        assert_eq!(migrate::<Two>(&store), Ok(true));
        let text = std::fs::read_to_string(d.join("two.toml")).unwrap();
        assert_eq!(written_version(&text), Some(2));
        assert!(text.contains("added = 7"));
        assert_eq!(migrate::<Two>(&store), Ok(false));
        std::fs::remove_dir_all(&d).unwrap();
    }
}
