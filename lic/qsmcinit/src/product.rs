// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! QS/MC as the base crates see it from PID 1. They report to PID 1
//! through what they return, so nothing is posted or noted through
//! this; a service's `Appliance` asks PID 1 instead
//! (`refcode::post::remote`).

use appliance::Appliance;

/// QS/MC, as PID 1 hands it to the base crates.
pub static QSMC: Appliance = release::appliance(|_| {}, |_, _| {});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_description_is_qs_mc_s() {
        assert_eq!((QSMC.id, QSMC.name), ("qsmc", "QS/MC"));
        assert_eq!((QSMC.banner)(), release::banner());
        assert_eq!(
            QSMC.control_socket().to_str(),
            Some(release::CONTROL_SOCKET)
        );
        assert_eq!(QSMC.load_source, &release::LOAD_SOURCE);
    }
}
