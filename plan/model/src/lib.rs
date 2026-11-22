// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! System plans (`docs/PLANS.md`): a saved configuration of a QS/VS
//! host and its partitions, built only from what QSI's host family reads
//! and writes. It holds configuration and nothing else: no disk's
//! contents, no image, no dump, no password or key.
//!
//! - [`doc`]: the document, its version, its strict reader and writer.
//! - [`view`]: a host as it is read now through `mgmtd`.
//! - [`capture`]: a plan of a host.
//! - [`compare`]: what differs between two plans.
//! - [`validate`]: every item of a plan against a host, OK, Warning or
//!   Error, by the rules table.
//! - [`deploy`]: the steps a validated plan and a choice of partitions
//!   give, and the request of each.
//! - [`index`], [`job`]: what the scheduler keeps of plans and of a
//!   deployment, as its socket carries them.

#![forbid(unsafe_code)]

pub mod capture;
pub mod compare;
pub mod deploy;
pub mod doc;
pub mod index;
pub mod job;
pub mod validate;
pub mod view;

pub use doc::Plan;
pub use validate::{Item, Status};
pub use view::HostView;
