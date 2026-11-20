// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! The requests on `release::MGMTD_SOCKET`, one a connection. Every
//! request that reaches a managed system names the signed-on user, for
//! the managed system's log; a password crosses here once, to pair, and
//! is never kept.

use ipc::wire_struct;
use qsi::host::refcodes::Posted;
use qsipart::console::Frame;

use crate::kept::{Kept, Refusal};

/// A reference code of the history, with what it is about.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    /// The managed system.
    pub system: String,
    /// The partition's ID; 0 for the system itself.
    pub partition: u32,
    /// The partition's name; empty for the system.
    pub partition_name: String,
    /// The code as posted.
    pub posted: Posted,
}
wire_struct!(Row {
    system,
    partition,
    partition_name,
    posted
});

/// Which codes of the history.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Filter {
    /// The managed system; every one when empty.
    pub system: String,
    /// The partition: `None` every one and the system, `Some(0)` the
    /// system alone, `Some(id)` that partition.
    pub partition: Option<u32>,
    /// Posted at or after, Unix milliseconds; 0 for no bound.
    pub from_ms: u64,
    /// Posted before, Unix milliseconds; 0 for no bound.
    pub to_ms: u64,
    /// Only this code; any when empty.
    pub code: String,
}
wire_struct!(Filter {
    system,
    partition,
    from_ms,
    to_ms,
    code
});

/// A page of the history, newest first.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Page {
    /// The rows of the page.
    pub rows: Vec<Row>,
    /// How many rows the filter matches.
    pub total: u32,
}
wire_struct!(Page { rows, total });

/// A connection's test, a line a step with its time.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Tested {
    /// `TCP connect 1 ms`, `TLS handshake 9 ms`, ...
    pub steps: Vec<String>,
    /// Whether every step passed.
    pub ok: bool,
}
wire_struct!(Tested { steps, ok });

/// A control panel function carried out.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Panelled {
    /// What the function displays, the host's words.
    pub text: String,
    /// The partition's code before.
    pub before: String,
    /// The partition's code after.
    pub after: String,
}
wire_struct!(Panelled {
    text,
    before,
    after
});

/// How the service stands.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Status {
    /// Managed systems.
    pub systems: u32,
    /// Of them connected.
    pub connected: u32,
    /// The most it manages.
    pub max_systems: u32,
    /// Consoles open now.
    pub consoles: u32,
    /// The most open at once.
    pub max_consoles: u32,
    /// The console's QSI certificate's SHA-256.
    pub fingerprint: String,
}
wire_struct!(Status {
    systems,
    connected,
    max_systems,
    consoles,
    max_consoles,
    fingerprint
});

/// A file of a backup: its path under `/data` and its bytes.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct File {
    /// `state/systems.toml`.
    pub path: String,
    /// Its mode.
    pub mode: u32,
    /// Its bytes.
    pub bytes: Vec<u8>,
}
wire_struct!(File { path, mode, bytes });

ipc::rpc! {
    request Request, service Mgmt, client Calls, dispatch dispatch;
    /// Every managed system as kept, by name.
    0 => fn systems / Systems () -> Vec<Kept>;
    /// One, by name.
    1 => fn system / System (name: String) -> Option<Kept>;
    /// Pair with the system at `address`:`port` with the password its
    /// operator read there, and keep it.
    2 => fn add / Add (address: String, port: u16, password: String, user: String) -> Result<Kept, Refusal>;
    /// Test the connection to a system: each step with its time.
    3 => fn test / Test (name: String, user: String) -> Result<Tested, Refusal>;
    /// Pair a kept system again with a new password.
    4 => fn reset / Reset (name: String, password: String, user: String) -> Result<Kept, Refusal>;
    /// Forget a system and its pin.
    5 => fn remove / Remove (name: String, user: String) -> Result<(), Refusal>;
    /// Power a host off, or restart it.
    6 => fn power / Power (name: String, restart: bool, immediate: bool, delay: u32, user: String) -> Result<(), Refusal>;
    /// A partition's control panel function.
    7 => fn panel / Panel (name: String, id: u32, function: u8, user: String) -> Result<Panelled, Refusal>;
    /// A page of the reference code history.
    8 => fn history / History (filter: Filter, skip: u32, rows: u32) -> Page;
    /// A partition's screen: the latest whole one a console has, or
    /// one asked of the host.
    9 => fn screen / Screen (name: String, id: u32, user: String) -> Result<Option<Frame>, Refusal>;
    /// How the service stands.
    10 => fn status / Status () -> Status;
    /// The managed systems' files for a backup.
    11 => fn backup_files / BackupFiles () -> Vec<File>;
    /// Keep the console's own note about a system.
    12 => fn describe / Describe (name: String, description: String, user: String) -> Result<(), Refusal>;
}

/// The largest request or answer on the socket.
pub const MAX_FRAME: usize = 16 << 20;

/// How long a client waits for its answer: pairing and a managed
/// system's answer can take this long; nothing waits on the console.
pub const WAIT: std::time::Duration = std::time::Duration::from_secs(90);

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
/// of its own so one slow request holds up no other.
#[cfg(unix)]
pub fn serve<S: Mgmt + Clone + Send + 'static>(
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
