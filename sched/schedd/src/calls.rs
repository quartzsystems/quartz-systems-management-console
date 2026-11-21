// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! The scheduler's socket (`release::SCHEDD_SOCKET`): the web server's
//! requests, one a connection, an `ipc` frame each way.

use crate::model::{Run, Schedule, Schedules};

ipc::rpc! {
    request Request, service Sched, client Calls, dispatch dispatch;
    /// Every operation and the runs kept.
    0 => fn list / List () -> Schedules;
    /// Schedule `s` for `user`, its owner: its number.
    1 => fn add / Add (s: Schedule, user: String) -> Result<u32, String>;
    /// Change operation `s.id` as `user`, its owner from now on.
    2 => fn change / Change (s: Schedule, user: String) -> Result<(), String>;
    /// Remove operation `id`.
    3 => fn remove / Remove (id: u32, user: String) -> Result<(), String>;
    /// Run operation `id` now, as its owner, its role checked.
    4 => fn run_now / RunNow (id: u32, user: String) -> Result<Run, String>;
}

/// The most a frame carries.
pub const MAX_FRAME: usize = 4 << 20;
/// How long a call may take: a run now waits for the managed system.
pub const WAIT: std::time::Duration = std::time::Duration::from_secs(120);

/// A client of the socket.
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

/// Answer requests on `listener` for ever, each connection a thread.
#[cfg(unix)]
pub fn serve<S: Sched + Clone + Send + 'static>(
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
