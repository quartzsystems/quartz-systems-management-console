// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! The update server over `httpc`: one request a connection, no
//! redirect followed, `https://` checked against the root authorities it
//! is given by name, `http://` for a local mirror or a test (what it
//! sends is still verified by its signature).

use std::path::Path;

use crate::service::Get;

/// Seconds to connect and to wait for each read.
const TIMEOUT: u32 = 30;

/// The server as `httpc` reaches it.
pub struct Http {
    /// The authorities an `https://` server's certificate is checked by.
    pub roots: Vec<Vec<u8>>,
    /// The `User-Agent`: `QSMC/V1R0M0 build`.
    pub agent: String,
}

impl Http {
    fn request(&self, url: &str) -> httpc::Request {
        httpc::Request {
            method: "GET".into(),
            url: url.into(),
            headers: Vec::new(),
            body: Vec::new(),
            timeout: TIMEOUT,
            redirects: 0,
            trust: httpc::Trust::Roots(self.roots.clone()),
            agent: self.agent.clone(),
        }
    }
}

impl Get for Http {
    fn get(&self, url: &str, limit: u64) -> Result<Vec<u8>, String> {
        let mut out = Vec::new();
        httpc::download(&self.request(url), 0, limit, &mut out).map_err(|e| e.to_string())?;
        Ok(out)
    }

    fn fetch(&self, url: &str, path: &Path, limit: u64) -> Result<(), String> {
        httpc::download_file(&self.request(url), path, limit)
            .map(|_| ())
            .map_err(|e| e.to_string())
    }
}
