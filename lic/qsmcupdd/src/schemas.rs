// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! The state files this release reads and the newest schema version of
//! each (`base/state`'s documents): what an update image's manifest
//! says, so a console can refuse a release that cannot read its state.
//! The base's documents by their types; the console's own by their
//! names, which a test holds to the types of the crates that write them.

use state::Document as _;

/// The console's own documents: the web server's, the scheduler's, the
/// managed-systems service's and the download service's.
pub const OWN: &[(&str, u32)] = &[
    ("webusers.toml", 1),
    ("web.toml", WEB_VERSION),
    ("trusted.toml", 1),
    ("backups.toml", 1),
    ("groups.toml", 1),
    ("schedules.toml", 1),
    ("systems.toml", 1),
    ("mgmt.toml", 1),
    ("updates.toml", 1),
];

/// `web.toml`'s version: 2 in the test release that changes it
/// (`docs/UPDATES.md`, suite 8), 1 in every other build.
#[cfg(qsmc_next_schema)]
const WEB_VERSION: u32 = 2;
#[cfg(not(qsmc_next_schema))]
const WEB_VERSION: u32 = 1;

/// Each state file and the newest version this release reads, sorted.
#[must_use]
pub fn this_release() -> Vec<(String, u32)> {
    let mut all: Vec<(String, u32)> = [
        (state::Host::FILE, state::Host::VERSION),
        (state::Identity::FILE, state::Identity::VERSION),
        (state::Inventory::FILE, state::Inventory::VERSION),
        (state::Network::FILE, state::Network::VERSION),
    ]
    .into_iter()
    .chain(OWN.iter().copied())
    .map(|(f, v)| (f.to_string(), v))
    .collect();
    all.sort();
    all
}

#[cfg(test)]
mod tests {
    use state::Document;

    fn of<D: Document>() -> (String, u32) {
        (D::FILE.to_string(), D::VERSION)
    }

    #[test]
    fn every_file_is_named_once() {
        let all = super::this_release();
        for (i, (f, v)) in all.iter().enumerate() {
            let toml = std::path::Path::new(f)
                .extension()
                .is_some_and(|x| x.eq_ignore_ascii_case("toml"));
            assert!(toml && *v >= 1, "{f}");
            assert!(all[..i].iter().all(|(o, _)| o != f), "{f} twice");
        }
    }

    /// The names and versions are those of the types that write them.
    #[test]
    fn the_console_s_own_are_their_types() {
        let types = [
            of::<webusers::WebUsers>(),
            of::<webusers::WebSettings>(),
            of::<webpages::certs_routes::Trusted>(),
            of::<webpages::backup::History>(),
            of::<webpages::groups::Groups>(),
            of::<schedd::model::Schedules>(),
            of::<mgmtd::keep::files::Systems>(),
            of::<mgmtd::keep::files::Limits>(),
            of::<upd::settings::Settings>(),
        ];
        let own: Vec<(String, u32)> = super::OWN
            .iter()
            .map(|(f, v)| ((*f).to_string(), *v))
            .collect();
        assert_eq!(own, types);
    }
}
