// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! The web server's socket ([`release::WEBD_SOCKET`]): what the local
//! console's option 4 asks of it. A password comes as its hash, never
//! as itself. When the web server is down, the same three changes go to
//! PID 1 as notes ([`NOTE_RESET`], [`NOTE_CERTIFICATE`],
//! [`NOTE_LISTENING`]), which PID 1 makes on the files and says the
//! outcome of in its fact [`release::fact::WEB_REQUEST`].

#[cfg(unix)]
use std::path::{Path, PathBuf};

/// What the web server says of itself.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct WebStatus {
    /// `address:port` of each listener.
    pub listening: Vec<String>,
    /// The certificate's subject.
    pub subject: String,
    /// Its SHA-256 fingerprint, `AA:BB:...`.
    pub sha256: String,
    /// Its SHA-1 fingerprint.
    pub sha1: String,
    /// Valid from, Unix seconds.
    pub not_before: u64,
    /// Valid to, Unix seconds.
    pub not_after: u64,
    /// The addresses it answers; all when empty.
    pub allow: Vec<String>,
    /// Whether it answers on `private` too.
    pub listen_private: bool,
    /// The enabled Administrators.
    pub administrators: Vec<String>,
}
ipc::wire_struct!(WebStatus {
    listening,
    subject,
    sha256,
    sha1,
    not_before,
    not_after,
    allow,
    listen_private,
    administrators
});

ipc::rpc! {
    request Request, service Web, client Calls, dispatch dispatch;
    /// How the web server stands.
    0 => fn status / Status () -> WebStatus;
    /// Give Administrator `id` the password of `hash` and enable it, or
    /// make it an Administrator when there is no enabled one; for `by`.
    1 => fn reset_admin / ResetAdmin (id: String, hash: String, by: String) -> Result<(), String>;
    /// Make a new self-signed certificate: its SHA-256.
    2 => fn new_certificate / NewCertificate (by: String) -> Result<String, String>;
    /// Answer every address again, on `open` only.
    3 => fn default_listening / DefaultListening (by: String) -> Result<(), String>;
}

/// PID 1's note: reset or make an Administrator, `<id> <hash>`.
pub const NOTE_RESET: &str = "web.reset-admin";
/// PID 1's note: the certificate goes; the web server makes a new one
/// when it starts.
pub const NOTE_CERTIFICATE: &str = "web.new-certificate";
/// PID 1's note: the default listening rule.
pub const NOTE_LISTENING: &str = "web.default-listening";

/// A client of the web server's socket.
#[cfg(unix)]
#[derive(Clone, Debug)]
pub struct Client {
    path: PathBuf,
}

#[cfg(unix)]
impl Client {
    /// A client of the socket at `path`.
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    /// The socket's path.
    pub fn path(&self) -> &Path {
        &self.path
    }
}

#[cfg(unix)]
impl Calls for Client {
    fn call(&mut self, req: &Request) -> Result<Vec<u8>, ipc::WireError> {
        supervise::control::exchange(&self.path, &ipc::encode(req))
    }
}

/// Serve `service` on `listener` for ever, one request a connection.
#[cfg(unix)]
pub fn serve(listener: &std::os::unix::net::UnixListener, service: &mut impl Web) {
    for stream in listener.incoming().flatten() {
        let _ = supervise::control::answer(stream, |request| {
            Ok(dispatch(service, ipc::decode::<Request>(request)?))
        });
    }
}
