// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! The console's own dump (`docs/SERVICE.md`), asked of PID 1: by the
//! web interface or the local console over the control socket, or by
//! PID 1 itself when a service stops after its retries, when a pending
//! slot did not confirm, and after a kernel panic. One at a time;
//! written by a thread of its own, since this loop never waits on
//! `/data`. Its codes are posted through the control socket like any
//! service's.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use servicedump::Asked;

use super::Host;

/// A dump is being taken.
static TAKING: AtomicBool = AtomicBool::new(false);

/// The folder, root's alone: the web server and the local console,
/// root without capabilities, list, read and delete its dumps.
fn prepare(dir: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt as _;
    std::fs::create_dir_all(dir)?;
    std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))
}

/// The services' status as a person reads it.
fn status_text(host: &Host) -> String {
    let s = host.status_now();
    let mut out = String::new();
    for (k, v) in &s.facts {
        let _ = writeln!(out, "{k}: {v}");
    }
    out.push('\n');
    for svc in &s.services {
        let _ = writeln!(
            out,
            "{} {} process {} failures {}",
            svc.name, svc.state, svc.pid, svc.failures
        );
    }
    out
}

impl Host {
    /// Begin a dump asked for by `by`: its name, or why not (one is
    /// being taken, or there is no data file system to keep it on).
    pub fn take_dump(&mut self, by: &str) -> Result<String, String> {
        if !self.data_mounted {
            return Err("There is no data file system to keep a dump on.".into());
        }
        if TAKING.swap(true, Ordering::SeqCst) {
            return Err("A console dump is being taken: one at a time.".into());
        }
        let now = appliance::time::now_secs();
        let name = servicedump::store::name_at(&consdump::KIND, now);
        self.post(&refcode::DUMP_STARTED, &format!("{name}, by {by}"));
        let asked = Asked {
            by: by.to_string(),
            release: format!(
                "{} {} build {}",
                release::NAME,
                release::RELEASE,
                release::BUILD_ID
            ),
            host: format!("{} serial {}", self.facts.host, self.facts.serial),
            status: status_text(self),
        };
        let spawned = std::thread::Builder::new()
            .name("console-dump".into())
            .spawn(move || {
                let data = PathBuf::from(release::DATA_DIR);
                let store = consdump::store();
                let taken = prepare(store.dir())
                    .map_err(|e| format!("{}: {e}", store.dir().display()))
                    .and_then(|()| {
                        servicedump::take(
                            &store,
                            &consdump::paths(&data),
                            &mut consdump::Own { data: data.clone() },
                            &asked,
                            now,
                        )
                    });
                match taken {
                    Ok((info, gone)) => {
                        for old in gone {
                            refcode::post::remote::code(
                                &refcode::DUMP_REMOVED,
                                &format!("{old}, for the limits"),
                            );
                        }
                        refcode::post::remote::code(
                            &refcode::DUMP_WRITTEN,
                            &format!("{}, {} bytes", info.name, info.bytes),
                        );
                    }
                    Err(e) => refcode::post::remote::code(&refcode::DUMP_FAILED, &e),
                }
                TAKING.store(false, Ordering::SeqCst);
            });
        if spawned.is_err() {
            TAKING.store(false, Ordering::SeqCst);
            return Err("The dump's thread could not be started.".into());
        }
        Ok(name)
    }

    /// A dump PID 1 asks for itself, for `why`: said in the log when it
    /// cannot be taken.
    pub fn dump_for(&mut self, why: &str) {
        if let Err(e) = self.take_dump(&format!("PID 1: {why}")) {
            self.say(
                logd::Severity::Warning,
                &format!("no console dump for {why}: {e}"),
            );
        }
    }
}
