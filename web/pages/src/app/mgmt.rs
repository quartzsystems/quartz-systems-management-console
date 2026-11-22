// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! The way to the managed-systems service. The web server holds no QSI
//! key and no session: every managed system it shows or acts on comes
//! through `mgmtd`'s socket (`mgmtproto`). In tests the same calls go
//! to a service in the same process ([`Local`]).

use mgmtproto::calls::Calls;
use mgmtproto::{Mgmt, Request};

/// A client when there is no service: every call fails, so pages show
/// the systems as not reached.
#[derive(Debug, Default)]
pub struct Nothing;

impl schedd::Calls for Nothing {
    fn call(&mut self, _req: &schedd::Request) -> Result<Vec<u8>, ipc::WireError> {
        Err(
            std::io::Error::new(std::io::ErrorKind::NotFound, "the scheduler is not running")
                .into(),
        )
    }
}

impl problem::proto::Calls for Nothing {
    fn call(&mut self, _req: &problem::proto::Request) -> Result<Vec<u8>, ipc::WireError> {
        Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "the problem service is not running",
        )
        .into())
    }
}

impl update::proto::Calls for Nothing {
    fn call(&mut self, _req: &update::proto::Request) -> Result<Vec<u8>, ipc::WireError> {
        Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "the update service is not running",
        )
        .into())
    }
}

impl fetchd::proto::Calls for Nothing {
    fn call(&mut self, _req: &fetchd::proto::Request) -> Result<Vec<u8>, ipc::WireError> {
        Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "the download service is not running",
        )
        .into())
    }
}

impl Calls for Nothing {
    fn call(&mut self, _req: &Request) -> Result<Vec<u8>, ipc::WireError> {
        Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "the managed-systems service is not running",
        )
        .into())
    }
}

/// The calls answered by `M` in this process, as the socket would.
#[derive(Debug, Clone)]
pub struct Local<M>(pub M);

impl<M: Mgmt> Calls for Local<M> {
    fn call(&mut self, req: &Request) -> Result<Vec<u8>, ipc::WireError> {
        Ok(mgmtproto::dispatch(&mut self.0, req.clone()))
    }
}
