// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! Suite 4, and the answers half of suite 12: an unattended
//! installation mirrored on two disks, with a static `open` address and
//! a `private` interface in the seed; answers that cannot be used stop
//! on a screen that says why.

use std::time::{Duration, Instant};

use release::fact;

use super::{
    DISK_BYTES, INSTALL_WITHIN, Media, TITLE, answers, blank_disks, boot_installed, golden,
    install_unattended, installed_machine, installing, is_installed, titled_with, with_seed,
};
use crate::qemu::session::Disk;
use crate::suite::base::{Run, channel, power_off};

/// The host of suite 4.
const HOST: &str = "mc-02";
/// The `open` interface's static address and gateway on QEMU's network.
const OPEN: &str = "10.0.2.20/24";
const GATEWAY: &str = "10.0.2.2";
/// The `private` interface's address, on the second NIC.
const PRIVATE: &str = "198.51.100.1/24";
/// The second NIC's MAC.
pub const PRIVATE_MAC: &str = "52:54:00:12:34:57";

/// The network tables of suite 4's seed: both roles.
pub fn both_roles() -> String {
    format!(
        "[[network.interface]]\nrole = \"open\"\nmode = \"static\"\naddress = \"{OPEN}\"\n\
         gateway = \"{GATEWAY}\"\ndns = [\"10.0.2.3\"]\n\n\
         [[network.interface]]\nrole = \"private\"\ndevice = \"{PRIVATE_MAC}\"\n\
         mode = \"static\"\naddress = \"{PRIVATE}\"\ncarries_route = false\n\
         carries_dns = false\ncarries_time = false\n"
    )
}

/// The interface of the default route, from `/proc/net/route`.
fn default_routes(table: &str) -> Vec<String> {
    table
        .lines()
        .skip(1)
        .filter_map(|l| {
            let words: Vec<&str> = l.split_whitespace().collect();
            (words.get(1) == Some(&"00000000")).then(|| words[0].to_string())
        })
        .collect()
}

/// Every disk holds both slots and a valid A/B record.
fn both_slots_and_record(disks: &[Disk]) -> Result<(), String> {
    use loadsource::layout::SLOT_A_PATH;
    use release::names::SLOT_B_PATH;
    for disk in disks {
        let esp = crate::image::disks::esp_of(&disk.path)?;
        for path in [SLOT_A_PATH, SLOT_B_PATH] {
            let uki = diskimg::fat::read_file(&esp, path)
                .map_err(|e| format!("{}: {path}: {e}", disk.path.display()))?;
            if uki.len() < 1 << 20 {
                return Err(format!(
                    "{}: {path} is {} bytes",
                    disk.path.display(),
                    uki.len()
                ));
            }
        }
        crate::image::disks::record_of(&disk.path)?;
    }
    Ok(())
}

/// Suite 4.
pub fn mirrored(run: &Run<'_>) -> Result<String, String> {
    let disks = blank_disks(run, "unattended", 2, DISK_BYTES)?;
    let seed = answers(
        &["QSMCHD0", "QSMCHD1"],
        &format!("name = \"{HOST}\"\nntp_servers = []"),
        &both_roles(),
    )?;
    let said = install_unattended(run, &disks, &seed, Media::Cdrom)?;
    // With valid answers no screen but the progress was shown.
    for screen in [
        "Type 1 by the load-source disk",
        "F10 erases the disks named",
    ] {
        if said.contains(screen) {
            return Err(format!("an unattended installation showed `{screen}`"));
        }
    }
    both_slots_and_record(&disks)?;
    let mut machine = installed_machine(run, &disks);
    machine.second_nic = Some(PRIVATE_MAC.into());
    // The first IPL writes the second unit from the first.
    let mut guest = boot_installed(run, &machine, run.launch.timeout * 2)?;
    let status = is_installed(&mut guest, HOST)?;
    let data = status.fact(fact::DATA).unwrap_or_default().to_string();
    if !data.starts_with("/dev/ublkb") {
        return Err(format!("/data is on {data}, not the protected set"));
    }
    let deadline = Instant::now() + Duration::from_secs(300);
    let storage = channel(&mut guest)?.storage_until("both units in step", deadline, |s| {
        s.protection == stgd::MIRRORED && s.whole() && s.work.is_empty()
    })?;
    let deadline = run.deadline();
    let status = channel(&mut guest)?.until("both interfaces up", deadline, |s| {
        s.fact(fact::MANAGEMENT)
            .is_some_and(|m| m.starts_with(OPEN))
            && s.fact(fact::PRIVATE)
                .is_some_and(|p| p.starts_with(PRIVATE))
    })?;
    let routes = default_routes(&channel(&mut guest)?.text("/proc/net/route")?);
    let open = status
        .fact(fact::MANAGEMENT)
        .unwrap_or_default()
        .to_string();
    let private = status.fact(fact::PRIVATE).unwrap_or_default().to_string();
    let open_dev = open.split(" on ").nth(1).and_then(|r| r.split(',').next());
    if routes.len() != 1 || open_dev != Some(routes[0].as_str()) {
        return Err(format!(
            "the default routes are {routes:?}; only open's ({open}) may carry one"
        ));
    }
    let network: state::Network =
        state::store::parse(&channel(&mut guest)?.text("/data/state/network.toml")?)
            .map_err(|e| e.to_string())?;
    let roles: Vec<&str> = network.interfaces.iter().map(|i| i.role.as_str()).collect();
    if roles != ["open", "private"] {
        return Err(format!("network.toml has the roles {roles:?}"));
    }
    power_off(&mut guest)?;
    let units: Vec<String> = storage
        .units
        .iter()
        .map(|u| format!("unit {} {} {}", u.number, u.location, u.state))
        .collect();
    Ok(format!(
        "only the progress showed; both disks hold slot A, slot B and a valid record; \
         /data on {data}, {}; open `{open}`, private `{private}`; the one default route is \
         on {}",
        units.join(", "),
        routes[0]
    ))
}

/// The answers half of suite 12: each file stops on the screen that
/// says what is wrong, and nothing is written.
pub fn invalid(run: &Run<'_>) -> Result<String, String> {
    let bad = [
        (
            answers(&["NOSUCHDISK"], "name = \"mc-03\"", "")?,
            "there is no disk NOSUCHDISK",
        ),
        (
            answers(
                &["QSMCHD0"],
                "name = \"mc-03\"",
                "[[network.interface]]\nrole = \"public\"\n",
            )?,
            "is not a role of this appliance",
        ),
        (
            answers(&["QSMCHD0"], "name = \"mc-03\"", "")?
                .replace("user_id = \"admin\"", "user_id = \"no such id\""),
            "user_id",
        ),
    ];
    let mut seen = Vec::new();
    for (i, (seed, why)) in bad.iter().enumerate() {
        let disks = blank_disks(run, &format!("answers-{i}"), 1, DISK_BYTES)?;
        let mut machine = installing(run, &disks, Media::Cdrom)?;
        with_seed(run, &mut machine, seed)?;
        let mut guest = run.boot(&machine)?;
        let deadline = Instant::now() + INSTALL_WITHIN;
        let shown = titled_with(&mut guest, TITLE, "cannot be used", deadline)?;
        let flat = shown.split_whitespace().collect::<Vec<_>>().join(" ");
        if !flat.contains(why) {
            return Err(format!("the screen does not say `{why}`:\n{shown}"));
        }
        if i == 0 {
            golden("install-answers-invalid", &shown)?;
        }
        guest.kill();
        if crate::image::disks::has_table(&disks[0].path)? {
            return Err("a disk was written after answers were refused".into());
        }
        seen.push(format!("`{why}`"));
    }
    Ok(format!(
        "{} answers files each stopped on the screen with what is wrong ({}); no disk was \
         written",
        seen.len(),
        seen.join(", ")
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_route_is_read_from_the_kernel_s_table() {
        let table = "Iface\tDestination\tGateway\tFlags\n\
                     eth0\t00000000\t0202000A\t0003\n\
                     eth0\t0002000A\t00000000\t0001\n\
                     eth1\t00643364\t00000000\t0001\n";
        assert_eq!(default_routes(table), ["eth0"]);
    }

    #[test]
    fn the_seed_names_both_roles() {
        let seed = seed::format::parse(&answers(&["QSMCHD0"], "", &both_roles()).unwrap()).unwrap();
        let roles: Vec<&str> = seed
            .network
            .interfaces
            .iter()
            .map(|i| i.role.as_str())
            .collect();
        assert_eq!(roles, ["open", "private"]);
        assert!(!seed.network.interfaces[1].carries_route);
    }
}
