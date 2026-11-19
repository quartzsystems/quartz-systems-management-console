// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! The answers of an unattended installation: the `[install]` table of
//! a seed file on a `QSMCSEED` volume, in the base format, with its
//! host, network and time tables. `base/install` holds them against the
//! machine; QS/MC adds that the network names only its own interface
//! roles, `open` and `private`.

use install::{Answers, Target};

/// Where the seed volume is mounted while it is read.
#[cfg(target_os = "linux")]
const SEED_MOUNT: &str = "/run/seed";

/// The seed volume: its device, and its file or why it cannot be read.
#[cfg(target_os = "linux")]
pub fn seed() -> Option<(std::path::PathBuf, Result<seed::Seed, String>)> {
    let device = seed::volume::find(release::SEED_LABEL)?;
    let parsed = seed::volume::read(&device, std::path::Path::new(SEED_MOUNT))
        .and_then(|text| seed::format::parse(&text));
    Some((device, parsed))
}

/// What a seed volume says: no answers when there is none or it has no
/// `[install]` table; why they cannot be used when they cannot.
pub fn given(
    found: Option<Result<seed::Seed, String>>,
    target: &mut dyn Target,
) -> Result<Option<Answers>, String> {
    let file = seed::format::FILE;
    let seed = match found {
        None => return Ok(None),
        Some(Err(reason)) => return Err(reason),
        Some(Ok(seed)) => seed,
    };
    if seed.install.is_none() {
        return Ok(None);
    }
    netd::manage::check_roles(&seed.network, release::INTERFACE_ROLES)
        .map_err(|e| format!("{file}: [network]: {e}"))?;
    install::answers::from_seed(&seed, target).map_err(|e| format!("{file}: [install]: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use install::{DiskChoice, Step};
    use panels::forms::LinkChoice;

    const HASH: &str = "$argon2id$v=19$m=19456,t=2,p=1$c2FsdHNhbHQ$aGFzaGhhc2hoYXNo";

    /// A machine with one disk of 64 GiB and one of 8 GiB.
    struct Machine;

    impl Target for Machine {
        fn product(&self) -> &'static str {
            release::NAME
        }
        fn banner(&self) -> String {
            release::banner()
        }
        fn what(&self) -> &'static [&'static str] {
            &[]
        }
        fn disks(&mut self) -> Vec<DiskChoice> {
            let disk = |name: &str, serial: &str, bytes| DiskChoice {
                name: name.into(),
                location: format!("P0000-B00-D0{}-F0-U0", name.len()),
                model: "virtio".into(),
                serial: serial.into(),
                bytes,
            };
            vec![
                disk("vda", "QSMCHD0", 64 << 30),
                disk("vdbb", "QSMCHD1", 8 << 30),
            ]
        }
        fn min_disk_bytes(&self) -> u64 {
            release::MIN_DISK_BYTES
        }
        fn links(&mut self) -> Vec<LinkChoice> {
            Vec::new()
        }
        fn default_host(&self) -> String {
            "QSMC123456".into()
        }
        fn now(&self) -> i64 {
            0
        }
        fn steps(&self) -> &'static [Step] {
            &[]
        }
        fn step(&mut self, _index: usize, _answers: &Answers) -> Result<(), String> {
            Ok(())
        }
        fn failed(&self) -> &'static str {
            ""
        }
        fn log(&mut self, _line: &str) {}
    }

    fn seed(disks: &str, network: &str) -> Result<seed::Seed, String> {
        seed::format::parse(&format!(
            "schema_version = 2\n[host]\nname = \"mc-01\"\n{network}\n\
             [install]\ndisks = {disks}\nuser_id = \"admin\"\npassword_hash = \"{HASH}\"\n"
        ))
    }

    const BOTH: &str = "[[network.interface]]\nrole = \"open\"\nmode = \"static\"\n\
                        address = \"192.0.2.10/24\"\ngateway = \"192.0.2.1\"\n\
                        [[network.interface]]\nrole = \"private\"\ndevice = \"eth1\"\n\
                        mode = \"static\"\naddress = \"198.51.100.1/24\"\n\
                        carries_route = false\ncarries_dns = false\ncarries_time = false\n";

    #[test]
    fn answers_with_both_roles_are_taken() {
        let answers = given(Some(seed("[\"QSMCHD0\"]", BOTH)), &mut Machine)
            .unwrap()
            .unwrap();
        assert_eq!(answers.host, "mc-01");
        assert_eq!(answers.network.interfaces.len(), 2);
        assert_eq!(answers.network.interfaces[1].role, "private");
        assert!(!answers.mirrored());
    }

    #[test]
    fn no_volume_or_no_install_table_asks_the_operator() {
        assert_eq!(given(None, &mut Machine), Ok(None));
        let plain = seed::format::parse("schema_version = 2\n");
        assert_eq!(given(Some(plain), &mut Machine), Ok(None));
    }

    #[test]
    fn answers_that_do_not_fit_say_why() {
        let small = given(Some(seed("[\"QSMCHD1\"]", BOTH)), &mut Machine).unwrap_err();
        assert!(small.starts_with("seed.toml: [install]: "), "{small}");
        assert!(small.contains("a load source needs"), "{small}");
        let role = "[[network.interface]]\nrole = \"public\"\n";
        let wrong = given(Some(seed("[\"QSMCHD0\"]", role)), &mut Machine).unwrap_err();
        assert!(wrong.starts_with("seed.toml: [network]: "), "{wrong}");
        let unread = given(Some(Err("seed.toml: bad".into())), &mut Machine).unwrap_err();
        assert_eq!(unread, "seed.toml: bad");
    }
}
