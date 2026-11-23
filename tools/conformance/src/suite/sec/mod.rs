// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! Phase 8's suites: release engineering and hardening on the final
//! image (`docs/SECURITY.md`).

mod baselines;
mod confined;
mod imports;
mod limits;
mod machine;
mod power;
mod secrets;
mod secureboot;
mod soak;
mod stream;
mod systems;
mod web;

use super::base::SuiteFn;

/// The suites.
pub const SUITES: &[(&str, SuiteFn)] = &[
    ("sec-confined", confined::confined),
    ("sec-machine", machine::machine),
    ("sec-web", web::web),
    ("sec-systems", systems::systems),
    ("sec-secrets", secrets::secrets),
    ("sec-limits", limits::limits),
    ("sec-rotation", super::serv::console::rotation),
    ("sec-baselines", baselines::baselines),
    ("sec-power", power::power),
    ("sec-imports", imports::imports),
];

/// The suites run only by name: after `build release`, with its
/// certificate.
pub const NAMED: &[(&str, SuiteFn)] = &[
    ("sec-secure-boot", secureboot::secure_boot),
    ("sec-stream", stream::stream),
    ("sec-soak", soak::soak),
];
