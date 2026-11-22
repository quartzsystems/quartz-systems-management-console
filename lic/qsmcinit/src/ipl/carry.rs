// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! The console's folders beside `/data/state` across an update:
//! `mgmt/` (the managed systems and their pins), `web/` (the web
//! certificate) and `fetch/` (the download service's settings).
//! `update::state_copy` keeps `/data/state` alone; these are kept by the
//! same rule, so a slot that rolls back has everything as it was: the
//! pending slot's first IPL copies each, the slot that confirms drops the
//! copies, the slot that comes back puts them back. Each copy is made
//! beside it and renamed whole, so a power loss leaves the folder or its
//! copy whole and the next IPL finishes what was begun.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use update::state_copy::{Done, Ipl};

/// The folders kept, under the data file system.
pub const FOLDERS: &[&str] = &["mgmt", "web", "fetch"];

/// Where the copies are, under the data file system.
pub const COPIES: &str = "update/carry";

fn copy_tree(from: &Path, to: &Path) -> io::Result<()> {
    fs::create_dir_all(to)?;
    for e in fs::read_dir(from)? {
        let e = e?;
        let target = to.join(e.file_name());
        if e.file_type()?.is_dir() {
            copy_tree(&e.path(), &target)?;
        } else {
            fs::copy(e.path(), &target)?;
            fs::File::open(&target)?.sync_all()?;
        }
    }
    fs::File::open(to)?.sync_all()
}

fn sync_dir(dir: &Path) {
    if let Ok(d) = fs::File::open(dir) {
        let _ = d.sync_all();
    }
}

/// One folder's step, as `state_copy::before_state` takes the state's.
fn one(data: &Path, folder: &str, ipl: Ipl) -> io::Result<Done> {
    let live = data.join(folder);
    let copies = data.join(COPIES);
    let copy = copies.join(folder);
    let part: PathBuf = copies.join(format!("{folder}.part"));
    if part.exists() {
        fs::remove_dir_all(&part)?;
    }
    // A put back cut short between its renames: the copy is the folder.
    if !live.exists() && copy.exists() {
        fs::rename(&copy, &live)?;
        sync_dir(data);
        return Ok(Done::Restored);
    }
    match ipl {
        Ipl::Trial if copy.exists() => Ok(Done::Kept),
        Ipl::Trial => {
            fs::create_dir_all(&copies)?;
            if live.exists() {
                copy_tree(&live, &part)?;
            } else {
                fs::create_dir_all(&part)?;
            }
            fs::rename(&part, &copy)?;
            sync_dir(&copies);
            Ok(Done::Copied)
        }
        Ipl::FellBack if copy.exists() => {
            let trial = copies.join(format!("{folder}.trial"));
            if trial.exists() {
                fs::remove_dir_all(&trial)?;
            }
            if live.exists() {
                fs::rename(&live, &trial)?;
            }
            fs::rename(&copy, &live)?;
            sync_dir(data);
            let _ = fs::remove_dir_all(&trial);
            Ok(Done::Restored)
        }
        Ipl::Normal if copy.exists() => {
            fs::remove_dir_all(&copy)?;
            Ok(Done::Dropped)
        }
        Ipl::FellBack | Ipl::Normal => Ok(Done::Nothing),
    }
}

/// Each folder's step for an IPL that is `ipl`: what was done to each.
///
/// # Errors
/// A copy or a rename failed; that folder is as it was.
pub fn before_state(data: &Path, ipl: Ipl) -> Vec<(&'static str, io::Result<Done>)> {
    FOLDERS.iter().map(|f| (*f, one(data, f, ipl))).collect()
}

/// The pending slot confirmed: the copies are dropped.
///
/// # Errors
/// They could not be removed; the next IPL drops them.
pub fn confirmed(data: &Path) -> io::Result<()> {
    match fs::remove_dir_all(data.join(COPIES)) {
        Err(e) if e.kind() != io::ErrorKind::NotFound => Err(e),
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("qsmc-carry-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(d.join("mgmt/refcodes")).unwrap();
        fs::write(d.join("mgmt/systems.toml"), "old").unwrap();
        fs::write(d.join("mgmt/refcodes/QSHQVS1"), "codes").unwrap();
        fs::create_dir_all(d.join("web")).unwrap();
        fs::write(d.join("web/certificate.pem"), "cert").unwrap();
        d
    }

    #[test]
    fn a_trial_that_rolls_back_leaves_each_folder_as_it_was() {
        let d = scratch("back");
        let done = before_state(&d, Ipl::Trial);
        assert!(done.iter().all(|(_, r)| matches!(r, Ok(Done::Copied))));
        // The trial's release writes, makes and removes.
        fs::write(d.join("mgmt/systems.toml"), "new").unwrap();
        fs::create_dir_all(d.join("fetch")).unwrap();
        fs::write(d.join("fetch/updates.toml"), "made").unwrap();
        fs::remove_file(d.join("web/certificate.pem")).unwrap();
        // A second IPL of the same trial keeps the first copy.
        assert!(matches!(before_state(&d, Ipl::Trial)[0].1, Ok(Done::Kept)));
        before_state(&d, Ipl::FellBack);
        assert_eq!(
            fs::read_to_string(d.join("mgmt/systems.toml")).unwrap(),
            "old"
        );
        assert_eq!(
            fs::read_to_string(d.join("mgmt/refcodes/QSHQVS1")).unwrap(),
            "codes"
        );
        assert_eq!(
            fs::read_to_string(d.join("web/certificate.pem")).unwrap(),
            "cert"
        );
        assert!(!d.join("fetch/updates.toml").exists());
        assert!(!d.join(COPIES).join("mgmt").exists());
        fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn a_trial_that_confirms_keeps_what_it_wrote() {
        let d = scratch("confirm");
        before_state(&d, Ipl::Trial);
        fs::write(d.join("mgmt/systems.toml"), "new").unwrap();
        confirmed(&d).unwrap();
        assert!(matches!(
            before_state(&d, Ipl::Normal)[0].1,
            Ok(Done::Nothing)
        ));
        assert_eq!(
            fs::read_to_string(d.join("mgmt/systems.toml")).unwrap(),
            "new"
        );
        fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn a_power_loss_at_any_point_is_finished_by_the_next_ipl() {
        let d = scratch("cut");
        // Cut while copying: the part is no copy.
        fs::create_dir_all(d.join(COPIES).join("mgmt.part")).unwrap();
        assert!(matches!(
            before_state(&d, Ipl::Normal)[0].1,
            Ok(Done::Nothing)
        ));
        assert!(!d.join(COPIES).join("mgmt.part").exists());
        // Cut between the put back's renames: the copy is the folder.
        before_state(&d, Ipl::Trial);
        fs::rename(d.join("mgmt"), d.join(COPIES).join("mgmt.trial")).unwrap();
        assert!(matches!(
            before_state(&d, Ipl::Normal)[0].1,
            Ok(Done::Restored)
        ));
        assert_eq!(
            fs::read_to_string(d.join("mgmt/systems.toml")).unwrap(),
            "old"
        );
        // A copy left from an update that ended is dropped.
        before_state(&d, Ipl::Trial);
        assert!(matches!(
            before_state(&d, Ipl::Normal)[0].1,
            Ok(Done::Dropped)
        ));
        fs::remove_dir_all(&d).unwrap();
    }
}
