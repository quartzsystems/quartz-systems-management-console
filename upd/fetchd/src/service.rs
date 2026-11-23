// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! The service: what each stream offers, checked on request or by the
//! settings; files downloaded into a folder of its own, resumed when
//! cut, and moved where others may read them only once they match the
//! index; the settings kept through `base/state`.
//!
//! On disk, under the downloads folder (`/data/updates`):
//!
//! - `<stream>/index`: the bytes that last verified, so a restart knows
//!   them without the server;
//! - `<stream>/<file>`: a file that matched its index, readable by all;
//! - `<stream>/notes/<file>`: a release's notes, as the server sent them;
//! - `.partial/<stream>/<file>`: a download not yet whole, its own.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use msg::Issued;
use refcode::RefCode;

use crate::proto::{self, Config as Settings_, Entry, File, FileState, Kept, Known, Status};
use crate::settings::{Check, Settings, valid_server};
use crate::streams::{self, Stream, file_name};
use crate::verify;

/// The most a release's notes may be.
pub const MAX_NOTES: u64 = 256 * 1024;
/// What downloads may take of the data file system, by default.
pub const LIMIT: u64 = 4 << 30;

/// How a request reaches the update server.
pub trait Get: Send + Sync {
    /// The body at `url`, at most `limit` bytes.
    fn get(&self, url: &str, limit: u64) -> Result<Vec<u8>, String>;
    /// The body at `url` into the file at `path`, resumed from its
    /// length, at most `limit` bytes.
    fn fetch(&self, url: &str, path: &Path, limit: u64) -> Result<(), String>;
}

/// How the service posts a code with its detail.
pub type Post = Box<dyn Fn(&'static RefCode, &str) + Send + Sync>;

/// What the service is given.
pub struct Config {
    /// Its own folder: `updates.toml`.
    pub dir: PathBuf,
    /// The downloads.
    pub files: PathBuf,
    /// The image's folder of the streams' keys.
    pub keys: PathBuf,
    /// What downloads may take.
    pub limit: u64,
    /// The release that runs, which the console's own stream is read for.
    pub running: vrm::Release,
    /// Now, Unix seconds.
    pub clock: Box<dyn Fn() -> u64 + Send + Sync>,
    /// The console's offset from UTC now, seconds: the automatic check
    /// is at 04:00 its time.
    pub offset: Box<dyn Fn() -> i64 + Send + Sync>,
    /// Post a code with its detail.
    pub post: Post,
    /// A line for the log.
    pub log: Box<dyn Fn(&str) + Send + Sync>,
    /// The update server.
    pub get: Box<dyn Get>,
}

struct Inner {
    config: Config,
    store: state::Store,
    settings: Settings,
    known: BTreeMap<Stream, Known>,
    /// Downloads under way or stopped, by stream and name.
    going: BTreeMap<(Stream, String), File>,
}

struct Shared {
    /// The update server: asked with no lock held.
    get: Box<dyn Get>,
    inner: Mutex<Inner>,
}

/// The getter left in a configuration once the service took its own.
struct Taken;

impl Get for Taken {
    fn get(&self, _: &str, _: u64) -> Result<Vec<u8>, String> {
        Err("no update server".into())
    }
    fn fetch(&self, _: &str, _: &Path, _: u64) -> Result<(), String> {
        Err("no update server".into())
    }
}

/// The service, shared by the socket's threads and the timer.
#[derive(Clone)]
pub struct Service(Arc<Shared>);

fn base(d: &qmcmsg::Def, values: &[&str]) -> Issued {
    qmcmsg::base_of(d).with(values)
}

/// Write `bytes` to `path` whole, readable by all.
fn publish(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    // Durable: a power loss leaves the old index or the new, whole.
    state::atomic_write(path, bytes)?;
    readable(path);
    Ok(())
}

/// Move a verified download into place durably: its bytes on the disk
/// before the rename, the rename before the answer.
fn settle(from: &Path, to: &Path) -> std::io::Result<()> {
    fs::File::open(from)?.sync_all()?;
    fs::rename(from, to)?;
    if let Some(dir) = to.parent() {
        fs::File::open(dir)?.sync_all()?;
    }
    Ok(())
}

/// Make `path` readable by every service.
fn readable(path: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        let mode = if path.is_dir() { 0o755 } else { 0o644 };
        let _ = fs::set_permissions(path, fs::Permissions::from_mode(mode));
    }
    #[cfg(not(unix))]
    let _ = path;
}

/// A folder of the service's own.
fn private(path: &Path) -> std::io::Result<()> {
    fs::create_dir_all(path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    }
    Ok(())
}

/// The bytes the files under `dir` take.
fn taken(dir: &Path) -> u64 {
    fs::read_dir(dir).map_or(0, |rd| {
        rd.flatten()
            .map(|e| match e.metadata() {
                Ok(m) if m.is_dir() => taken(&e.path()),
                Ok(m) => m.len(),
                Err(_) => 0,
            })
            .sum()
    })
}

impl Service {
    /// The service of `config`: its settings read, what last verified
    /// of each stream read again from its folder.
    pub fn start(mut config: Config) -> Self {
        let get = std::mem::replace(&mut config.get, Box::new(Taken));
        let store = state::Store::at(&config.dir);
        let settings = match store.load::<Settings>() {
            Ok(Some(s)) => s,
            Ok(None) => Settings::default(),
            Err(e) => {
                (config.log)(&format!(
                    "updates.toml does not read, its defaults are used: {e}"
                ));
                Settings::default()
            }
        };
        let mut inner = Inner {
            config,
            store,
            settings,
            known: BTreeMap::new(),
            going: BTreeMap::new(),
        };
        for s in Stream::ALL {
            let mut k = Known {
                stream: s,
                ..Known::default()
            };
            let keys = streams::keys(s, &inner.config.keys);
            if let Ok(bytes) = fs::read(inner.config.files.join(s.name()).join("index"))
                && let Ok(t) = verify::index(s, &bytes, &keys, 0)
            {
                k.sequence = t.sequence;
                k.generated = t.generated;
                k.entries = t.entries;
                k.levels = t.levels;
                k.checked = inner.settings.checked;
            }
            inner.known.insert(s, k);
        }
        // What a download left before a restart resumes when asked.
        let partial = inner.config.files.join(".partial");
        for s in Stream::ALL {
            let Ok(rd) = fs::read_dir(partial.join(s.name())) else {
                continue;
            };
            for e in rd.flatten() {
                let name = e.file_name().to_string_lossy().to_string();
                let have = e.metadata().map_or(0, |m| m.len());
                let bytes = inner.entry(s, &name).map_or(0, |e| e.update_bytes);
                inner.going.insert(
                    (s, name.clone()),
                    File {
                        stream: s,
                        name,
                        bytes,
                        have,
                        state: FileState::Stopped,
                        error: None,
                        time: 0,
                    },
                );
            }
        }
        Self(Arc::new(Shared {
            get,
            inner: Mutex::new(inner),
        }))
    }

    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.0.inner.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// How it stands.
    pub fn status(&self) -> Status {
        let i = self.lock();
        let mut files: Vec<File> = i.going.values().cloned().collect();
        for s in Stream::ALL {
            let Ok(rd) = fs::read_dir(i.config.files.join(s.name())) else {
                continue;
            };
            for e in rd.flatten() {
                let name = e.file_name().to_string_lossy().to_string();
                let Ok(m) = e.metadata() else { continue };
                if !m.is_file()
                    || name == "index"
                    || Path::new(&name)
                        .extension()
                        .is_some_and(|x| x.eq_ignore_ascii_case("new"))
                {
                    continue;
                }
                files.push(File {
                    stream: s,
                    name,
                    bytes: m.len(),
                    have: m.len(),
                    state: FileState::Verified,
                    error: None,
                    time: m
                        .modified()
                        .ok()
                        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                        .map_or(0, |d| d.as_secs()),
                });
            }
        }
        files.sort_by(|a, b| (a.stream, &a.name).cmp(&(b.stream, &b.name)));
        Status {
            config: Settings_ {
                server: i.settings.server.clone(),
                check: i.settings.check,
                download: i.settings.download,
            },
            checked: i.settings.checked,
            streams: i.known.values().cloned().collect(),
            files,
            limit: i.config.limit,
            used: taken(&i.config.files),
        }
    }

    /// Change the settings, for `by`.
    pub fn configure(&self, c: &Settings_, by: &str) -> Result<(), Vec<Issued>> {
        let server = c.server.trim().trim_end_matches('/').to_string();
        if !valid_server(&server) {
            return Err(vec![base(&qmcmsg::SERVER_NOT_VALID, &[&c.server])]);
        }
        let mut i = self.lock();
        let mut next = i.settings.clone();
        next.server = server;
        next.check = c.check;
        next.download = c.download;
        i.store.save(&next).map_err(|e| {
            vec![
                base(&qmcmsg::PRB_NOT_KEPT, &[]),
                Issued {
                    id: String::new(),
                    text: e.to_string(),
                    help: String::new(),
                },
            ]
        })?;
        (i.config.log)(&format!(
            "settings changed by {by}: server {}, check {}, download {}",
            next.server,
            next.check.words(),
            if next.download {
                "automatically"
            } else {
                "on request"
            }
        ));
        i.settings = next;
        Ok(())
    }

    /// Check every stream now, for `by`.
    pub fn check(&self, by: &str) -> Vec<Known> {
        let (server, keys_dir) = {
            let i = self.lock();
            (i.settings.server.clone(), i.config.keys.clone())
        };
        // The three indexes at once: a slow stream holds the check for
        // its own timeout, not the others' as well (open question 44).
        let gots: Vec<(String, Result<Vec<u8>, String>)> = std::thread::scope(|scope| {
            let each: Vec<_> = Stream::ALL
                .iter()
                .map(|s| {
                    let url = s.index_url(&server);
                    let at = url.clone();
                    (
                        url,
                        scope.spawn(move || self.lock_free_get(&at, verify::MAX_INDEX)),
                    )
                })
                .collect();
            each.into_iter()
                .map(|(url, h)| (url, h.join().unwrap_or_else(|_| Err("no answer".into()))))
                .collect()
        });
        let mut said = Vec::new();
        for (s, (url, got)) in Stream::ALL.into_iter().zip(gots) {
            let mut i = self.lock();
            let now = (i.config.clock)();
            let seen = s.seen(&i.settings.seen);
            let keys = streams::keys(s, &keys_dir);
            let outcome = got
                .map_err(|why| {
                    (i.config.post)(&refcode::STREAM_NOT_REACHED, &format!("{url}: {why}"));
                    base(&qmcmsg::STREAM_NOT_REACHED, &[&server, &why])
                })
                .and_then(|bytes| {
                    let t = verify::index(s, &bytes, &keys, seen).inspect_err(|m| {
                        (i.config.post)(
                            &refcode::STREAM_REFUSED,
                            &format!("{}: {}", s.name(), m.text),
                        );
                    })?;
                    let dir = i.config.files.join(s.name());
                    let _ = fs::create_dir_all(&dir);
                    readable(&dir);
                    if let Err(e) = publish(&dir.join("index"), &bytes) {
                        (i.config.log)(&format!("{}: the index was not kept: {e}", s.name()));
                    }
                    Ok(t)
                });
            let k = i.known.entry(s).or_default();
            k.stream = s;
            k.checked = now;
            match outcome {
                Ok(t) => {
                    k.sequence = t.sequence;
                    k.generated = t.generated;
                    k.entries = t.entries;
                    k.levels = t.levels;
                    k.error = None;
                    said.push(format!("{} {}", s.name(), t.sequence));
                    let seq = k.sequence;
                    s.see(&mut i.settings.seen, seq);
                }
                Err(m) => k.error = Some(m),
            }
        }
        let wanted = {
            let mut i = self.lock();
            i.settings.checked = (i.config.clock)();
            let next = i.settings.clone();
            if let Err(e) = i.store.save(&next) {
                (i.config.log)(&format!("updates.toml was not written: {e}"));
            }
            if !said.is_empty() {
                (i.config.post)(
                    &refcode::STREAMS_CHECKED,
                    &format!("by {by}: {}", said.join(", ")),
                );
            }
            if i.settings.download {
                i.wanted()
            } else {
                Vec::new()
            }
        };
        self.notes(&server);
        for (s, name) in wanted {
            let _ = self.download(s, &name, "automatic download");
        }
        self.lock().known.values().cloned().collect()
    }

    /// A request to the server, made with no lock held.
    fn lock_free_get(&self, url: &str, limit: u64) -> Result<Vec<u8>, String> {
        self.0.get.get(url, limit)
    }

    /// Each release's notes the console has not, fetched once.
    fn notes(&self, server: &str) {
        let wanted: Vec<(Stream, String, PathBuf)> = {
            let i = self.lock();
            i.known
                .values()
                .flat_map(|k| k.entries.iter().map(move |e| (k.stream, e.notes.clone())))
                .filter(|(_, n)| !n.is_empty() && file_name(n))
                .map(|(s, n)| {
                    let p = i.config.files.join(s.name()).join("notes").join(&n);
                    (s, n, p)
                })
                .filter(|(_, _, p)| !p.exists())
                .collect()
        };
        for (s, n, p) in wanted {
            let got = self.lock_free_get(&s.file_url(server, &n), MAX_NOTES);
            if let (Ok(bytes), Some(dir)) = (got, p.parent()) {
                let _ = fs::create_dir_all(dir);
                readable(dir);
                let _ = publish(&p, &bytes);
            }
        }
    }

    /// Download `name` of `stream`, for `by`: begun on a thread of its
    /// own, or why not.
    pub fn download(&self, stream: Stream, name: &str, by: &str) -> Result<(), Vec<Issued>> {
        let (url, partial, done, entry) = {
            let mut i = self.lock();
            let Some(entry) = i.entry(stream, name).cloned() else {
                return Err(vec![base(&qmcmsg::NOT_IN_STREAM, &[name, stream.name()])]);
            };
            let done = i.config.files.join(stream.name()).join(name);
            if done.exists()
                && verify::file(&done, entry.update_bytes, &entry.update_sha256).is_ok()
            {
                return Ok(());
            }
            let key = (stream, name.to_string());
            if i.going
                .get(&key)
                .is_some_and(|f| f.state == FileState::Downloading)
            {
                return Ok(());
            }
            let partial = i
                .config
                .files
                .join(".partial")
                .join(stream.name())
                .join(name);
            let have = fs::metadata(&partial).map_or(0, |m| m.len());
            let used = taken(&i.config.files);
            if used.saturating_sub(have) + entry.update_bytes > i.config.limit {
                return Err(vec![base(
                    &qmcmsg::FILE_TOO_LARGE,
                    &[
                        name,
                        &entry.update_bytes.to_string(),
                        &i.config.limit.to_string(),
                    ],
                )]);
            }
            let url = stream.file_url(&i.settings.server, name);
            let time = (i.config.clock)();
            i.going.insert(
                key,
                File {
                    stream,
                    name: name.to_string(),
                    bytes: entry.update_bytes,
                    have,
                    state: FileState::Downloading,
                    error: None,
                    time,
                },
            );
            (i.config.log)(&format!(
                "download of {} {name} begun by {by}",
                stream.name()
            ));
            (url, partial, done, entry)
        };
        let svc = self.clone();
        let name = name.to_string();
        std::thread::spawn(move || svc.run_download(stream, &name, &url, &partial, &done, &entry));
        Ok(())
    }

    fn run_download(
        &self,
        stream: Stream,
        name: &str,
        url: &str,
        partial: &Path,
        done: &Path,
        e: &Entry,
    ) {
        let key = (stream, name.to_string());
        let fetched = partial
            .parent()
            .map_or(Ok(()), |d| private(d).map_err(|e| e.to_string()))
            .and_then(|()| self.0.get.fetch(url, partial, e.update_bytes));
        let mut i = self.lock();
        let now = (i.config.clock)();
        let have = fs::metadata(partial).map_or(0, |m| m.len());
        if let Err(why) = fetched {
            (i.config.post)(&refcode::STREAM_NOT_REACHED, &format!("{url}: {why}"));
            let server = i.settings.server.clone();
            if let Some(f) = i.going.get_mut(&key) {
                f.state = FileState::Stopped;
                f.have = have;
                f.time = now;
                f.error = Some(base(&qmcmsg::STREAM_NOT_REACHED, &[&server, &why]));
            }
            return;
        }
        match verify::file(partial, e.update_bytes, &e.update_sha256) {
            Ok(()) => {
                let dir = done.parent().map(Path::to_path_buf).unwrap_or_default();
                let _ = fs::create_dir_all(&dir);
                readable(&dir);
                readable(partial);
                match settle(partial, done) {
                    Ok(()) => {
                        i.going.remove(&key);
                        (i.config.post)(&refcode::DOWNLOADED, &format!("{} {name}", stream.name()));
                    }
                    Err(err) => {
                        if let Some(f) = i.going.get_mut(&key) {
                            f.state = FileState::Stopped;
                            f.error = Some(base(&qmcmsg::PRB_NOT_KEPT, &[]));
                        }
                        (i.config.log)(&format!("{name} was not moved into place: {err}"));
                    }
                }
            }
            Err(why) => {
                let _ = fs::remove_file(partial);
                (i.config.post)(
                    &refcode::DOWNLOAD_BAD,
                    &format!("{} {name}: {why}", stream.name()),
                );
                if let Some(f) = i.going.get_mut(&key) {
                    f.state = FileState::Stopped;
                    f.have = 0;
                    f.time = now;
                    f.error = Some(base(&qmcmsg::FILE_BAD, &[name, &why]));
                }
            }
        }
    }

    /// Remove `name` of `stream`, whole or partial, for `by`.
    pub fn remove(&self, stream: Stream, name: &str, by: &str) -> Result<(), Vec<Issued>> {
        if !file_name(name) {
            return Err(vec![base(&qmcmsg::NOT_IN_STREAM, &[name, stream.name()])]);
        }
        let mut i = self.lock();
        let key = (stream, name.to_string());
        if i.going
            .get(&key)
            .is_some_and(|f| f.state == FileState::Downloading)
        {
            return Err(vec![base(&qmcmsg::UPD_BUSY, &["being downloaded"])]);
        }
        let done = i.config.files.join(stream.name()).join(name);
        let partial = i
            .config
            .files
            .join(".partial")
            .join(stream.name())
            .join(name);
        let gone = fs::remove_file(&done).is_ok()
            | fs::remove_file(&partial).is_ok()
            | i.going.remove(&key).is_some();
        if !gone {
            return Err(vec![base(&qmcmsg::NOT_IN_STREAM, &[name, stream.name()])]);
        }
        (i.config.post)(
            &refcode::DOWNLOAD_REMOVED,
            &format!("{} {name}, by {by}", stream.name()),
        );
        Ok(())
    }

    /// The settings' file, for a backup.
    pub fn backup(&self) -> Vec<Kept> {
        let i = self.lock();
        let path = i.store.path::<Settings>();
        fs::read(&path).map_or_else(
            |_| Vec::new(),
            |bytes| {
                vec![Kept {
                    path: format!(
                        "{}/{}",
                        crate::BACKUP_DIR,
                        <Settings as state::Document>::FILE
                    ),
                    bytes,
                }]
            },
        )
    }

    /// Whether the automatic check is due at `now`: past 04:00 of the
    /// console's day (a Sunday's for weekly), and not checked since.
    pub fn due(&self, now: u64) -> bool {
        let i = self.lock();
        let offset = (i.config.offset)();
        due(i.settings.check, i.settings.checked, now, offset)
    }
}

/// Whether a check of `check`, last at `checked`, is due at `now`, the
/// console `offset` seconds from UTC.
pub fn due(check: Check, checked: u64, now: u64, offset: i64) -> bool {
    let local = |t: u64| i64::try_from(t).unwrap_or(0) + offset;
    let day = local(now).div_euclid(86_400);
    let at = |d: i64| d * 86_400 + 4 * 3600;
    let last = match check {
        Check::Never => return false,
        Check::Daily => {
            if local(now) >= at(day) {
                at(day)
            } else {
                at(day - 1)
            }
        }
        Check::Weekly => {
            // 1970-01-01 was a Thursday: Sunday is day 3 of the week.
            let sunday = day - (day + 4).rem_euclid(7);
            if local(now) >= at(sunday) {
                at(sunday)
            } else {
                at(sunday - 7)
            }
        }
    };
    local(checked) < last
}

impl Inner {
    /// The entry of `stream` whose update image is `name`.
    fn entry(&self, stream: Stream, name: &str) -> Option<&Entry> {
        self.known
            .get(&stream)?
            .entries
            .iter()
            .find(|e| e.update == name)
    }

    /// What the automatic download takes: the console's own next level,
    /// and the newest host release.
    fn wanted(&self) -> Vec<(Stream, String)> {
        let mut out = Vec::new();
        if let Some(k) = self.known.get(&Stream::Qsmc)
            && let Some(e) = proto::newest_for(&k.entries, self.config.running)
        {
            out.push((Stream::Qsmc, e.update.clone()));
        }
        if let Some(k) = self.known.get(&Stream::Qsvs)
            && let Some(e) = k.entries.iter().max_by_key(|e| e.parsed())
        {
            out.push((Stream::Qsvs, e.update.clone()));
        }
        out
    }
}

impl proto::Fetch for Service {
    fn status(&mut self) -> Status {
        Service::status(self)
    }
    fn configure(&mut self, config: Settings_, by: String) -> Result<(), Vec<Issued>> {
        Service::configure(self, &config, &by)
    }
    fn check(&mut self, by: String) -> Vec<Known> {
        Service::check(self, &by)
    }
    fn download(&mut self, stream: Stream, name: String, by: String) -> Result<(), Vec<Issued>> {
        Service::download(self, stream, &name, &by)
    }
    fn remove(&mut self, stream: Stream, name: String, by: String) -> Result<(), Vec<Issued>> {
        Service::remove(self, stream, &name, &by)
    }
    fn backup(&mut self) -> Vec<Kept> {
        Service::backup(self)
    }
}
