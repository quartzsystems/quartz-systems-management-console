// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! QS/MC's identity over QS/400's `vrm` crate: the release, the build
//! ID, the product text, the banner, the prefixes everything named
//! after the product starts with, and the [`Appliance`] description the
//! product-neutral base crates of QS/VS take.
//!
//! - [`names`]: what a QS/MC load source carries that is its own.
//! - [`fact`]: the keys of the control socket's status.
//! - [`processor`]: QS/MC's words for what the processor lacks.
//! - [`screen`]: the status screen's text, which PID 1 draws and the
//!   conformance harness reads.

#![forbid(unsafe_code)]

pub mod fact;
pub mod names;
pub mod processor;
pub mod screen;
pub mod trial;

pub use appliance::{Appliance, Event, LoadSourceNames};
pub use names::LOAD_SOURCE;
pub use vrm::Release;

/// The release `build.rs` captured: `VERSION` (`1.0.0` is `V1R0M0`), or
/// `QSMC_RELEASE`.
pub const RELEASE: Release = vrm::release_from_env!("QSMC_");

/// The build ID: `git describe`, or `QSMC_BUILD_ID`.
pub const BUILD_ID: &str = env!("QSMC_BUILD_ID");

/// The product's short name.
pub const NAME: &str = "QS/MC";

/// The product's name in a test image's manifest: it says it is one,
/// so a release image never takes it, nor it a release image's.
pub const TEST_NAME: &str = "QS/MC (test image)";

/// The product text.
pub const PRODUCT: &str = "Quartz Systems/Management Console";

/// The lower-case identifier: PID 1 is `qsmcinit`.
pub const ID: &str = "qsmc";

/// Prefix of a fix identifier, `QMnnnnn`.
pub const FIX_PREFIX: &str = "QM";

/// Prefix of a message identifier.
pub const MSG_PREFIX: &str = "QMC";

/// Prefix of every environment variable QS/MC reads.
pub const ENV_PREFIX: &str = "QSMC_";

/// Prefix of QS/MC's kernel command-line keys.
pub const KEY_PREFIX: &str = "qsmc.";

/// Where `QSMCDATA` is mounted.
pub const DATA_DIR: &str = "/data";

/// The run directory.
pub const RUN_DIR: &str = "/run";

/// PID 1's control socket.
pub const CONTROL_SOCKET: &str = "/run/qsmcinit.sock";

/// Where the service binaries are in a slot's initramfs.
pub const BIN_DIR: &str = "/qsmc/bin";

/// The installer, in the media's initramfs.
pub const INSTALLER: &str = "/qsmc/bin/qsmcinstall";

/// The local console's binary, in a slot's initramfs.
pub const CONSOLE: &str = "/qsmc/bin/qsmccons";

/// The web server's binary, in a slot's initramfs.
pub const WEBD: &str = "/qsmc/bin/qsmcwebd";

/// PID 1's socket that hands the web server its listening sockets.
pub const WEB_SOCKET: &str = "/run/qsmcweb.sock";

/// The web server's own socket: the local console's requests.
pub const WEBD_SOCKET: &str = "/run/qsmcwebd.sock";

/// The catalogue of the QS/VS release this console is built with, in a
/// slot's initramfs: a host's codes and messages explained.
pub const CATALOGUE: &str = "/qsmc/catalogue.json";

/// The managed-systems service's binary, in a slot's initramfs.
pub const MGMTD: &str = "/qsmc/bin/qsmcmgmtd";

/// The scheduler's binary, in a slot's initramfs.
pub const SCHEDD: &str = "/qsmc/bin/qsmcschedd";

/// The scheduler's socket: the web server's requests.
pub const SCHEDD_SOCKET: &str = "/run/qsmcschedd.sock";

/// The managed-systems service's socket: the web server's requests.
pub const MGMTD_SOCKET: &str = "/run/qsmcmgmtd.sock";

/// The managed-systems service's socket for consoles: one connection a
/// browser's terminal window.
pub const MGMTD_CONSOLES: &str = "/run/qsmcmgmtd-consoles.sock";

/// What the managed-systems service keeps beside the state files: the
/// reference code history of each system.
pub const MGMT_DIR: &str = "/data/mgmt";

/// The user and group the managed-systems service runs as: it alone
/// reads the console's QSI key.
pub const MGMT_ID: u32 = 23_990;

/// The download service's binary.
pub const FETCHD: &str = "/qsmc/bin/qsmcfetchd";

/// The download service's socket, root's.
pub const FETCHD_SOCKET: &str = "/run/qsmcfetchd.sock";

/// The download service's own folder: `updates.toml`.
pub const FETCH_DIR: &str = "/data/fetch";

/// What the download service downloads, readable by every service once
/// it matched its stream's index.
pub const UPDATES_DIR: &str = "/data/updates";

/// The user and group the download service runs as: no key of the
/// console's is its; the control socket's group (the managed-systems
/// service's) is its only other, for posting its codes.
pub const FETCH_ID: u32 = 23_991;

/// The problem service's binary.
pub const PROBD: &str = "/qsmc/bin/qsmcprobd";

/// The update service's binary.
pub const UPDD: &str = "/qsmc/bin/qsmcupdd";

/// System plans' folder under [`DATA_DIR`]: the scheduler's alone
/// (`docs/PLANS.md`).
pub const PLANS: &str = "plans";

/// The console's own dumps, PID 1's, under `/data`.
pub const DUMPS_DIR: &str = "/data/dumps/console";

/// The web server's certificate and key, under `/data`.
pub const WEB_DIR: &str = "/data/web";

/// The web certificate's file under [`WEB_DIR`], the chain with the
/// leaf first.
pub const WEB_CERT_FILE: &str = "certificate.pem";
/// Its key's file, PKCS #8, readable by its owner only.
pub const WEB_KEY_FILE: &str = "key.pem";

/// A restore's folder under `/data` (`docs/BACKUP.md`): the web server
/// leaves the backup's payload there as [`restore::PENDING`]; PID 1
/// applies it at the next IPL and keeps the state before it as
/// [`restore::PREVIOUS`] while [`restore::TRIAL`] is there.
pub mod restore {
    /// The folder's name under `/data`.
    pub const DIR: &str = "restore";
    /// The payload waiting for the IPL.
    pub const PENDING: &str = "pending";
    /// A payload uploaded and not yet asked for: the confirmation's.
    pub const UPLOADED: &str = "uploaded";
    /// The restore is applied and not yet confirmed.
    pub const TRIAL: &str = "trial";
    /// The state before it.
    pub const PREVIOUS: &str = "previous";
}

/// The local console's argument for its one panel: the unit to keep of
/// a mirrored load source whose units ran apart.
pub const KEEP_UNIT: &str = "keep-unit";

/// The trusted signers' public keys in the media's initramfs, 32 bytes
/// each: what the installer checks the payload's manifest against.
pub const TRUSTED_KEYS: &str = "/qsmc/keys/system.pub";

/// The local console's title.
pub const CONSOLE_TITLE: &str = "QS/MC Local Console";

/// The smallest load-source disk QS/MC installs on: 16 GiB. A console
/// wants 64 GiB or more (`docs/INSTALL.md`).
pub const MIN_DISK_BYTES: u64 = 16 << 30;

/// The key of a test image's hook switch, after [`KEY_PREFIX`]: an
/// SMBIOS OEM string `qsmc.hook=<name>`. Only a test image reads it.
pub const HOOK_KEY: &str = "hook";

/// What a default host name starts with; six hexadecimal digits follow.
pub const HOST_PREFIX: &str = "QSMC";

/// The FAT volume label of a seed volume.
pub const SEED_LABEL: &str = "QSMCSEED";

/// The roles of the console's network interfaces, as `network.toml`
/// writes them. `open` is first: the network operators reach the
/// console on, with the default route, the name servers and time.
/// `private`, optional, faces the systems it manages and carries none
/// of them. Written into state on an installed console: never changed.
pub const INTERFACE_ROLES: &[&str] = &["open", "private"];

/// The port QSI's managed systems listen on; QS/MC connects to it.
pub const QSI_PORT: u16 = 2300;

/// The web UI's port.
pub const HTTPS_PORT: u16 = 443;

/// The port that redirects to [`HTTPS_PORT`].
pub const HTTP_PORT: u16 = 80;

/// QS/MC as the base crates see it. The caller says how an event is
/// posted and a fact noted.
pub const fn appliance(post: fn(&Event<'_>), note: fn(&str, &str)) -> Appliance {
    Appliance {
        id: ID,
        name: NAME,
        banner,
        data_dir: DATA_DIR,
        run_dir: RUN_DIR,
        host_prefix: HOST_PREFIX,
        seed_label: SEED_LABEL,
        interface_roles: INTERFACE_ROLES,
        load_source: &LOAD_SOURCE,
        post,
        note,
    }
}

/// The banner for a release and a build ID: `QS/MC V1R0M0 a1b2c3d`.
pub fn banner_of(release: Release, build_id: &str) -> String {
    format!("{NAME} {release} {build_id}")
}

/// This build's banner.
pub fn banner() -> String {
    banner_of(RELEASE, BUILD_ID)
}

/// A fix identifier, `QM00001`.
pub fn fix_id(number: u32) -> String {
    format!("{FIX_PREFIX}{number:05}")
}

/// A default host name: the prefix and six hexadecimal digits.
pub fn default_host_name(digits: u32) -> String {
    format!("{HOST_PREFIX}{:06X}", digits & 0x00FF_FFFF)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_banner_names_the_release_and_the_build() {
        let r = Release { v: 1, r: 0, m: 0 };
        assert_eq!(banner_of(r, "abc1234"), "QS/MC V1R0M0 abc1234");
        assert!(banner().starts_with("QS/MC V"));
        assert!(banner().ends_with(BUILD_ID));
    }

    #[test]
    fn the_release_is_version_s() {
        let version = include_str!("../../../VERSION").trim();
        if option_env!("QSMC_RELEASE").is_none() {
            assert_eq!(version, "1.0.0");
            assert_eq!(RELEASE.to_string(), "V1R0M0");
        }
    }

    #[test]
    fn the_prefixes_are_qs_mc_s() {
        assert_eq!((ID, ENV_PREFIX, KEY_PREFIX), ("qsmc", "QSMC_", "qsmc."));
        assert_eq!((FIX_PREFIX, MSG_PREFIX), ("QM", "QMC"));
        assert_eq!(fix_id(1), "QM00001");
        assert_eq!(fix_id(12345), "QM12345");
        assert_eq!(default_host_name(0x00ab_12cd), "QSMCAB12CD");
        assert_eq!(default_host_name(0xffab_12cd), "QSMCAB12CD");
        assert_eq!(PRODUCT, "Quartz Systems/Management Console");
        assert_eq!(MIN_DISK_BYTES, 17_179_869_184);
        for path in [INSTALLER, CONSOLE, TRUSTED_KEYS] {
            assert!(path.starts_with("/qsmc/"), "{path}");
        }
        assert!(INSTALLER.starts_with(BIN_DIR) && CONSOLE.starts_with(BIN_DIR));
    }

    #[test]
    fn the_appliance_description_is_qs_mc() {
        let a = appliance(|_| {}, |_, _| {});
        assert_eq!((a.id, a.name), ("qsmc", "QS/MC"));
        assert_eq!(a.control_socket().to_str(), Some(CONTROL_SOCKET));
        assert_eq!(a.log_socket().to_str(), Some("/run/logd.sock"));
        assert_eq!(a.storage_file("sock").to_str(), Some("/run/qsmcstgd.sock"));
        assert_eq!(a.state_dir().to_str(), Some("/data/state"));
        assert_eq!(a.log_dir().to_str(), Some("/data/log"));
        assert_eq!((a.banner)(), banner());
        assert_eq!(a.host_prefix, "QSMC");
        // A FAT label is eleven characters at most.
        assert!(a.seed_label.len() <= 11);
        assert_eq!(a.load_source, &LOAD_SOURCE);
        assert_eq!(a.interface_roles, ["open", "private"]);
        assert_eq!(a.interface_note(""), appliance::NOTE_MANAGEMENT);
        assert_eq!(a.interface_note("private"), "private");
    }
}
