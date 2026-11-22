// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! `fetchd`'s socket, `release::FETCHD_SOCKET`, root's: the web server
//! and the local console ask it what each stream offers, to check now,
//! to download or remove a file, and change its settings. One request a
//! connection; every change names the signed-on user, for the log.

use ipc::{wire_enum, wire_struct};
use msg::Issued;

use crate::settings::Check;
use crate::streams::Stream;

/// A release a stream's index offers, as the pages read it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Entry {
    /// `V1R0M1`.
    pub release: String,
    /// Its build.
    pub build: String,
    /// The fixes it names.
    pub fixes: Vec<String>,
    /// The least release it updates from.
    pub updates_from: String,
    /// When it was published, Unix seconds.
    pub published: u64,
    /// Its date, `YYYY-MM-DD`; empty when the index has none.
    pub released: String,
    /// Its release notes' file beside the index; empty for none.
    pub notes: String,
    /// The update image's file.
    pub update: String,
    /// Its size.
    pub update_bytes: u64,
    /// Its SHA-256, hexadecimal.
    pub update_sha256: String,
}
wire_struct!(Entry {
    release,
    build,
    fixes,
    updates_from,
    published,
    released,
    notes,
    update,
    update_bytes,
    update_sha256
});

impl Entry {
    /// The entry as `base/stream` reads it, for `stream::newest_for`.
    pub fn of(e: &stream::Entry) -> Self {
        Self {
            release: e.release.clone(),
            build: e.build.clone(),
            fixes: e.fixes.clone(),
            updates_from: e.updates_from.clone(),
            published: e.published,
            released: e.released.clone(),
            notes: e.notes.clone(),
            update: e.update.clone(),
            update_bytes: e.update_bytes,
            update_sha256: e.update_sha256.clone(),
        }
    }

    /// Its release, when it reads as one.
    pub fn parsed(&self) -> Option<vrm::Release> {
        self.release.parse().ok()
    }
}

/// The newest entry past `running` that updates from it, as
/// `stream::newest_for` chooses.
pub fn newest_for(entries: &[Entry], running: vrm::Release) -> Option<&Entry> {
    entries
        .iter()
        .filter(|e| e.parsed().is_some_and(|r| r > running))
        .filter(|e| {
            e.updates_from
                .parse::<vrm::Release>()
                .is_ok_and(|f| f <= running)
        })
        .max_by_key(|e| e.parsed())
}

/// A release's latest cumulative level in QS/400's catalog.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Level {
    /// `V1R1M0`.
    pub release: String,
    /// The package's name, `Cyyddd110`.
    pub name: String,
    /// Its level.
    pub level: u32,
    /// The date its name carries, `YYYY-MM-DD`; empty when it has none.
    pub date: String,
}
wire_struct!(Level {
    release,
    name,
    level,
    date
});

/// What is known of a stream: what verified last, and how the last
/// check ended.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Known {
    /// Which.
    pub stream: Stream,
    /// The sequence of what verified; 0 when nothing has.
    pub sequence: u64,
    /// When it was made, Unix seconds.
    pub generated: u64,
    /// When it was last checked, Unix seconds; 0 for never.
    pub checked: u64,
    /// The releases an index offers.
    pub entries: Vec<Entry>,
    /// The levels a catalog offers.
    pub levels: Vec<Level>,
    /// Why the last check did not take it; none when it did.
    pub error: Option<Issued>,
}
wire_struct!(Known {
    stream,
    sequence,
    generated,
    checked,
    entries,
    levels,
    error
});

/// How a file of a stream stands.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum FileState {
    /// Being downloaded.
    #[default]
    Downloading,
    /// Whole, and matched its index: others may read it.
    Verified,
    /// The last download did not end whole; it resumes from `have`.
    Stopped,
}
wire_enum!(FileState {
    1 => Downloading,
    2 => Verified,
    3 => Stopped,
});

/// A file of a stream on the console.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct File {
    /// Its stream.
    pub stream: Stream,
    /// Its name, as the index names it.
    pub name: String,
    /// Its size, as the index names it.
    pub bytes: u64,
    /// The bytes the console has.
    pub have: u64,
    /// How it stands.
    pub state: FileState,
    /// Why the last download stopped; none when it did not.
    pub error: Option<Issued>,
    /// When it was verified or last stopped, Unix seconds.
    pub time: u64,
}
wire_struct!(File {
    stream,
    name,
    bytes,
    have,
    state,
    error,
    time
});

/// The settings an operator changes.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Config {
    /// The update server.
    pub server: String,
    /// When it is checked without being asked.
    pub check: Check,
    /// New levels found are downloaded without being asked.
    pub download: bool,
}
wire_struct!(Config {
    server,
    check,
    download
});

/// How the service stands.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Status {
    /// The settings.
    pub config: Config,
    /// When the streams were last checked; 0 for never.
    pub checked: u64,
    /// Each stream, in [`Stream::ALL`]'s order.
    pub streams: Vec<Known>,
    /// The files on the console and those being downloaded.
    pub files: Vec<File>,
    /// What downloads may take of the data file system.
    pub limit: u64,
    /// What they take now.
    pub used: u64,
}
wire_struct!(Status {
    config,
    checked,
    streams,
    files,
    limit,
    used
});

/// A file a backup holds: its path under `/data` and its bytes.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Kept {
    /// `fetch/updates.toml`.
    pub path: String,
    /// Its bytes.
    pub bytes: Vec<u8>,
}
wire_struct!(Kept { path, bytes });

ipc::rpc! {
    request Request, service Fetch, client Calls, dispatch dispatch;
    /// How the service stands.
    0 => fn status / Status () -> Status;
    /// Change the settings, for `by`.
    1 => fn configure / Configure (config: Config, by: String) -> Result<(), Vec<Issued>>;
    /// Check every stream now, for `by`: each as it ended.
    2 => fn check / Check (by: String) -> Vec<Known>;
    /// Download a file a stream's index names, for `by`; it goes on in
    /// the background.
    3 => fn download / Download (stream: Stream, name: String, by: String) -> Result<(), Vec<Issued>>;
    /// Remove a file of a stream, for `by`.
    4 => fn remove / Remove (stream: Stream, name: String, by: String) -> Result<(), Vec<Issued>>;
    /// The settings' file, for a backup.
    5 => fn backup / Backup () -> Vec<Kept>;
}

/// The largest request or answer on the socket.
pub const MAX_FRAME: usize = 4 << 20;

/// How long a client waits: a check of three streams over a slow link.
pub const WAIT: std::time::Duration = std::time::Duration::from_secs(120);

/// A client of the service's socket.
#[cfg(unix)]
#[derive(Clone, Debug)]
pub struct Client {
    path: std::path::PathBuf,
}

#[cfg(unix)]
impl Client {
    /// A client of the socket at `path`.
    pub fn new(path: impl Into<std::path::PathBuf>) -> Self {
        Self { path: path.into() }
    }
}

#[cfg(unix)]
impl Calls for Client {
    fn call(&mut self, req: &Request) -> Result<Vec<u8>, ipc::WireError> {
        let mut s = std::os::unix::net::UnixStream::connect(&self.path)?;
        s.set_read_timeout(Some(WAIT))?;
        s.set_write_timeout(Some(WAIT))?;
        ipc::frame::write_frame_max(&mut s, &ipc::encode(req), MAX_FRAME)?;
        ipc::frame::read_frame_max(&mut s, MAX_FRAME)
    }
}

/// Serve `service` on `listener` for ever, each connection on a thread
/// of its own.
#[cfg(unix)]
pub fn serve<S: Fetch + Clone + Send + 'static>(
    listener: &std::os::unix::net::UnixListener,
    service: &S,
) {
    for mut stream in listener.incoming().flatten() {
        let mut svc = service.clone();
        std::thread::spawn(move || {
            let _ = stream.set_read_timeout(Some(WAIT));
            let _ = stream.set_write_timeout(Some(WAIT));
            let Ok(req) = ipc::frame::read_frame_max(&mut stream, MAX_FRAME) else {
                return;
            };
            if let Ok(r) = ipc::decode::<Request>(&req) {
                let _ = ipc::frame::write_frame_max(&mut stream, &dispatch(&mut svc, r), MAX_FRAME);
            }
        });
    }
}
