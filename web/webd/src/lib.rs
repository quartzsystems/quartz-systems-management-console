// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! QS/MC's web server (`docs/WEB.md`), on QS/400's `qsweb`: its HTTP,
//! sessions, frame and partials; `x509kit` for the certificate; rustls
//! with the graviola provider for TLS. Nothing of them is here.
//!
//! - [`product`]: QS/MC as `qsweb` sees it: the brand, the tree.
//! - [`tls`]: the web certificate and the TLS configuration.
//! - [`serve`]: the answers: headers, the stylesheet, the redirect.
//! - [`listen`]: the listeners PID 1 hands over, the limits.
//! - [`selftest`]: the first answer over TLS, asked of itself.
//! - [`run`]: the service.

pub mod product;
pub mod serve;
pub mod tls;

#[cfg(target_os = "linux")]
pub mod listen;
#[cfg(target_os = "linux")]
pub mod run;
#[cfg(target_os = "linux")]
pub mod selftest;
