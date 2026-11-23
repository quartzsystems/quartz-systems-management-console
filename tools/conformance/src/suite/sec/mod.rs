// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! Phase 8's suites: release engineering and hardening on the final
//! image (`docs/SECURITY.md`).

mod confined;
mod machine;
mod secrets;
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
    ("sec-rotation", super::serv::console::rotation),
];
