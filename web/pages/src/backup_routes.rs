// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! Serving Backup and restore (`docs/BACKUP.md`). A backup is made of
//! `/data/state` and `/data/web`, sealed under the passphrase typed
//! (`qmcbackup`) and downloaded. A backup uploaded is opened with its
//! passphrase and its payload kept as `/data/restore/uploaded`; after
//! the confirmation it becomes `pending` and the console restarts, and
//! PID 1 applies it before anything reads state.

use std::fs;
use std::path::Path;

use qmcbackup::payload::{TOPS, valid_path};
use qmcbackup::{Backup, Entry, Manifest};
use qsweb::frame::Page;
use qsweb::http::Response;
use qsweb::msg::{Kind, Message};
use release::restore::{DIR, PENDING, UPLOADED};

use crate::app::routes::Rq;
use crate::backup::{
    BackupTab, History, Holds, KEPT, Made, Row, backup_page, history, history_page, restore_page,
    size,
};
use crate::user_routes::outcome;

fn when(rq: &Rq<'_>, at: u64) -> String {
    let s = rq.app.settings();
    let zone = rq.app.host().zone();
    let t = i64::try_from(at).unwrap_or(0);
    format!(
        "{} {}",
        crate::fmt::date(t, zone, &s.date_format),
        crate::fmt::time(t, zone, &s.time_format, false)
    )
}

fn rows(rq: &Rq<'_>, most: usize) -> Vec<Row> {
    history(rq.app)
        .made
        .iter()
        .rev()
        .take(most)
        .map(|m| Row {
            date: when(rq, m.at),
            destination: m.destination.clone(),
            size: size(m.size),
            status: (m.status.clone(), Some(m.status == "Complete")),
        })
        .collect()
}

fn show(rq: &Rq<'_>, mut v: BackupTab, messages: Vec<Message>) -> Response {
    v.last = rows(rq, 3);
    v.messages = messages;
    rq.page(backup_page(&rq.ctx, &v))
}

fn fresh() -> BackupTab {
    BackupTab {
        data: true,
        download: true,
        ..BackupTab::default()
    }
}

fn error(def: &qmcmsg::Def, data: &[&str]) -> Message {
    qmcmsg::message(Kind::Error, def, data)
}

/// Every file under `data`'s [`TOPS`], as a backup holds them.
pub fn collect(data: &Path) -> Result<Vec<Entry>, String> {
    fn walk(data: &Path, rel: &str, out: &mut Vec<Entry>) -> Result<(), String> {
        let dir = data.join(rel);
        let mut names: Vec<_> = match fs::read_dir(&dir) {
            Ok(d) => d.filter_map(Result::ok).collect(),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(e) => return Err(format!("{}: {e}", dir.display())),
        };
        names.sort_by_key(fs::DirEntry::file_name);
        for e in names {
            let name = e.file_name().to_string_lossy().into_owned();
            let path = format!("{rel}/{name}");
            // A write in progress, or a name no backup holds.
            if name.starts_with('.')
                || Path::new(&name).extension().is_some_and(|x| x == "new")
                || !valid_path(&path)
            {
                continue;
            }
            let meta = e.metadata().map_err(|e| format!("{path}: {e}"))?;
            if meta.is_dir() {
                walk(data, &path, out)?;
            } else if meta.is_file() {
                #[cfg(unix)]
                let mode = std::os::unix::fs::PermissionsExt::mode(&meta.permissions());
                #[cfg(not(unix))]
                let mode = 0o600;
                out.push(Entry {
                    data: fs::read(e.path()).map_err(|e| format!("{path}: {e}"))?,
                    path,
                    mode,
                });
            }
        }
        Ok(())
    }
    let mut out = Vec::new();
    for top in TOPS {
        walk(data, top, &mut out)?;
    }
    Ok(out)
}

fn make(rq: &Rq<'_>, f: &qsweb::http::Form) -> Response {
    let app = rq.app;
    let v = BackupTab {
        data: f.get("d").is_some(),
        history: f.get("h").is_some(),
        media: f.get("m").is_some(),
        download: f.get("w") != Some("remote"),
        host: f.get("host").unwrap_or("").into(),
        path: f.get("path").unwrap_or("").into(),
        user: f.get("u").unwrap_or("").into(),
        ..BackupTab::default()
    };
    let mut refused = Vec::new();
    if !v.data {
        refused.push(error(
            &qmcmsg::NOT_VALID,
            &["A backup always holds the console data."],
        ));
    }
    if v.history {
        refused.push(error(
            &qmcmsg::NOT_IN_V1,
            &["Reference code and event history in a backup"],
        ));
    }
    if v.media {
        refused.push(error(
            &qmcmsg::NOT_IN_V1,
            &["The virtual optical media library"],
        ));
    }
    if !v.download {
        refused.push(error(&qmcmsg::REMOTE_NOT_IN_V1, &[]));
    }
    let passphrase = zeroize::Zeroizing::new(f.get("pp").unwrap_or("").to_string());
    if !qmcbackup::seal::passphrase_ok(&passphrase) {
        refused.push(error(&qmcmsg::PASSPHRASE_RULES, &[]));
    }
    if !refused.is_empty() {
        return show(rq, v, refused);
    }
    let now = app.console.now();
    let name = app.console.name();
    let made = collect(&app.data_dir()).and_then(|entries| {
        let b = Backup {
            manifest: Manifest {
                console: name.clone(),
                release: release::RELEASE.to_string(),
                build: release::BUILD_ID.to_string(),
                made: now,
            },
            entries,
        };
        qmcbackup::seal::seal(&b, &passphrase).map_err(|e| e.to_string())
    });
    let at = i64::try_from(now).unwrap_or(0);
    let file = format!(
        "{}-{}.{}",
        name.to_lowercase(),
        crate::fmt::date(at, app.host().zone(), "YYYY-MM-DD").replace('-', ""),
        qmcbackup::seal::EXTENSION
    );
    let bytes = match made {
        Ok(b) => b,
        Err(e) => {
            rq.audit("make a backup", &e);
            return show(rq, fresh(), vec![error(&qmcmsg::NOT_VALID, &[&e])]);
        }
    };
    let mut h = history(app);
    h.made.push(Made {
        at: now,
        destination: format!("Download {file}"),
        size: bytes.len() as u64,
        status: "Complete".into(),
        user: rq.user().into(),
    });
    let over = h.made.len().saturating_sub(KEPT);
    h.made.drain(..over);
    let kept: Result<(), String> = app.store().save::<History>(&h).map_err(|e| e.to_string());
    rq.audit(&format!("make a backup, {file}"), outcome(&kept));
    Response::download(&file, "application/octet-stream", bytes)
}

fn restore_dir(rq: &Rq<'_>) -> std::path::PathBuf {
    rq.app.data_dir().join(DIR)
}

fn holds(rq: &Rq<'_>, b: &Backup) -> Holds {
    Holds {
        console: b.manifest.console.clone(),
        release: b.manifest.release.clone(),
        build: b.manifest.build.clone(),
        made: when(rq, b.manifest.made),
        files: b.entries.len(),
    }
}

/// The payload uploaded and waiting for the confirmation.
fn uploaded(rq: &Rq<'_>) -> Option<Backup> {
    let bytes = fs::read(restore_dir(rq).join(UPLOADED)).ok()?;
    Backup::decode(&bytes).ok()
}

fn open_upload(rq: &Rq<'_>) -> Response {
    let parts = rq.req.multipart().unwrap_or_default();
    let field = |n: &str| parts.iter().find(|p| p.name == n);
    let data = field("f").map(|p| p.data.as_slice()).unwrap_or_default();
    let passphrase = zeroize::Zeroizing::new(
        field("pp")
            .map(|p| String::from_utf8_lossy(&p.data).into_owned())
            .unwrap_or_default(),
    );
    let refuse = |m: Message, why: &str| {
        rq.audit("open a backup to restore", why);
        rq.page(restore_page(&rq.ctx, None, vec![m]))
    };
    let b = match qmcbackup::seal::open(data, &passphrase) {
        Ok(b) => b,
        Err(qmcbackup::Error::Passphrase) => {
            return refuse(error(&qmcmsg::BACKUP_PASSPHRASE, &[]), "wrong passphrase");
        }
        Err(e) => return refuse(error(&qmcmsg::BACKUP_DAMAGED, &[]), &e.to_string()),
    };
    match b.manifest.release.parse::<release::Release>() {
        Ok(theirs) if theirs <= release::RELEASE => {}
        Ok(theirs) => {
            let (t, o) = (theirs.to_string(), release::RELEASE.to_string());
            return refuse(error(&qmcmsg::BACKUP_NEWER, &[&t, &o]), "newer release");
        }
        Err(_) => return refuse(error(&qmcmsg::BACKUP_DAMAGED, &[]), "no release"),
    }
    let dir = restore_dir(rq);
    let kept = b.encode().map_err(|e| e.to_string()).and_then(|bytes| {
        fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        write_secret(&dir.join(UPLOADED), &bytes)
    });
    rq.audit(
        &format!(
            "open the backup of {} made {}",
            b.manifest.console,
            when(rq, b.manifest.made)
        ),
        outcome(&kept),
    );
    if let Err(e) = kept {
        return refuse(error(&qmcmsg::NOT_VALID, &[&e]), &e);
    }
    rq.page(restore_page(&rq.ctx, Some(&holds(rq, &b)), Vec::new()))
}

fn write_secret(path: &Path, bytes: &[u8]) -> Result<(), String> {
    use std::io::Write;
    let mut options = fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut f = options.open(path).map_err(|e| e.to_string())?;
    f.write_all(bytes)
        .and_then(|()| f.sync_all())
        .map_err(|e| e.to_string())
}

/// `/console/backup`, its three tabs.
pub fn backup(rq: &Rq<'_>) -> Response {
    let tab = rq.req.query().get("tab").unwrap_or("").to_string();
    if rq.post() {
        if rq.req.multipart().is_some() {
            return open_upload(rq);
        }
        let f = rq.req.form();
        if f.get("schedule").is_some() {
            return show(
                rq,
                fresh(),
                vec![error(&qmcmsg::NOT_IN_V1, &["Scheduled backups"])],
            );
        }
        return make(rq, &f);
    }
    match tab.as_str() {
        "restore" => {
            let h = uploaded(rq).map(|b| holds(rq, &b));
            rq.page(restore_page(&rq.ctx, h.as_ref(), Vec::new()))
        }
        "history" => rq.page(history_page(&rq.ctx, &rows(rq, KEPT), Vec::new())),
        _ => show(rq, fresh(), Vec::new()),
    }
}

/// `/confirm/restore`: the backup uploaded, then OK restores it at a
/// restart, now.
pub fn confirm(rq: &Rq<'_>) -> Response {
    let app = rq.app;
    let back = "/console/backup?tab=restore";
    let Some(b) = uploaded(rq) else {
        return rq.done(
            back,
            error(
                &qmcmsg::NOT_VALID,
                &["Open a backup with its passphrase first."],
            ),
        );
    };
    let f = rq.req.form();
    if rq.post() && f.get("ok").is_some() {
        let dir = restore_dir(rq);
        let staged = fs::rename(dir.join(UPLOADED), dir.join(PENDING)).map_err(|e| e.to_string());
        rq.audit(
            &format!(
                "restore the backup of {} made {} at the restart",
                b.manifest.console,
                when(rq, b.manifest.made)
            ),
            outcome(&staged),
        );
        if let Err(e) = staged {
            return rq.done(back, error(&qmcmsg::NOT_VALID, &[&e]));
        }
        let pending = qmcmsg::message(Kind::Info, &qmcmsg::RESTORE_PENDING, &[&b.manifest.console]);
        let restarted = app.console.power_down(
            true,
            &format!("{} from the web interface: a restore", rq.user()),
        );
        rq.audit("restart the console for a restore", outcome(&restarted));
        return match restarted {
            Ok(()) => rq.done(
                back,
                qmcmsg::message(Kind::Info, &qmcmsg::RESTART_STARTED, &[]),
            ),
            Err(_) => rq.done(back, pending),
        };
    }
    if rq.post() {
        return Response::redirect(&rq.ctx.url(back));
    }
    let list = qsweb::partials::List {
        cols: ["Console", "Release", "Made", "Files"]
            .iter()
            .map(|l| qsweb::partials::Col::plain(l))
            .collect(),
        rows: vec![vec![
            qsweb::partials::Cell::mono(&b.manifest.console),
            qsweb::partials::Cell::text(&b.manifest.release),
            qsweb::partials::Cell::text(&when(rq, b.manifest.made)),
            qsweb::partials::Cell::num(&b.entries.len().to_string()),
        ]],
        empty: String::new(),
        keep: Vec::new(),
    };
    let table = format!(
        "{}<p class=\"small grey\">Every session ends. The state before the restore is kept until the restored console starts its web server; if it does not, the next restart takes the restore back.</p>",
        qsweb::partials::list_table(&list)
    );
    let body = qsweb::pages::confirm_body(
        &rq.ctx,
        "Restoring replaces the console's state, users, network settings, identity and web certificate with the backup's, and restarts the console now. Press OK to restore the backup listed below, or Cancel to return without restoring.",
        &table,
        "",
    );
    rq.page(Page {
        title: "Confirm Restore".into(),
        crumb: vec![
            "Console Management".into(),
            "Backup and restore".into(),
            "Confirm Restore".into(),
        ],
        h1: "Confirm Restore".into(),
        body,
        help: "backup".into(),
        nav: Some(("Console Management", Some("/console/backup"))),
        ..Page::default()
    })
}
