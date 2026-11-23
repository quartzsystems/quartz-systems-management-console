// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! Secure Boot (`docs/RELEASING.md`, `docs/INSTALL.md`), run by name
//! after `build release`: the release's UKIs, signed with the db key,
//! start under OVMF's Secure Boot build once the db certificate
//! (`QSMC_SB_CERT`) is enrolled beside the firmware makers' keys, and the
//! console says Secure Boot is on; without the certificate the firmware
//! refuses them, and it refuses the test image's, which are not signed.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use crate::qemu::session::{Disk, Machine, Session};
use crate::suite::base::{Run, posted};

/// OVMF's Secure Boot build, and its variables with the firmware
/// makers' keys enrolled and Secure Boot on.
const CODE: &str = "/usr/share/OVMF/OVMF_CODE_4M.secboot.fd";
const VARS: &str = "/usr/share/OVMF/OVMF_VARS_4M.ms.fd";
/// The db certificate a release's UKIs are signed for.
const CERT_ENV: &str = "QSMC_SB_CERT";
/// How long a refused image is given to show its banner anyway.
const REFUSED_WAIT: Duration = Duration::from_secs(90);

/// The release image's file a pointer of `images` names.
fn pointed(images: &Path, pointer: &str) -> Result<PathBuf, String> {
    let p = images.join(pointer);
    let text = fs::read_to_string(&p).map_err(|e| format!("{}: {e}", p.display()))?;
    Ok(PathBuf::from(text.trim()))
}

/// `disk` booted under the Secure Boot firmware with `vars`.
fn boot(run: &Run<'_>, disk: &Path, vars: &Path) -> Result<Session, String> {
    let launch = crate::qemu::Launch {
        firmware: CODE.into(),
        secure_boot: true,
        ..run.launch.clone()
    };
    let m = Machine {
        disks: vec![Disk {
            path: disk.to_path_buf(),
            serial: crate::image::disks::serial(0),
            snapshot: true,
        }],
        vars: vars.to_path_buf(),
        ..run.machine()?
    };
    Session::spawn(&launch, &m, run.work)
}

/// `disk` with `vars` never shows its banner: the firmware refused it.
fn refused(run: &Run<'_>, disk: &Path, vars: &Path, what: &str) -> Result<(), String> {
    let mut s = boot(run, disk, vars)?;
    let shown = s.seen(release::NAME, Instant::now() + REFUSED_WAIT);
    s.kill();
    match shown {
        Ok(()) => Err(format!("{what} started under Secure Boot")),
        Err(_) => Ok(()),
    }
}

/// The suite.
pub fn secure_boot(run: &Run<'_>) -> Result<String, String> {
    let cert_path = std::env::var_os(CERT_ENV).ok_or_else(|| {
        format!("{CERT_ENV} is not set: the db certificate `build release` signed with")
    })?;
    let cert = fs::read(&cert_path).map_err(|e| format!("{CERT_ENV}: {e}"))?;
    let target =
        std::env::var_os("QSMC_TARGET_DIR").map_or_else(|| PathBuf::from("target"), PathBuf::from);
    let images = target.join("images");
    let uki = fs::read(pointed(&images, "uki-a.path")?).map_err(|e| e.to_string())?;
    pesign::verify(&uki, &cert).map_err(|e| {
        format!("the release's slot A UKI is not signed with {CERT_ENV} ({e}): build it with `build release`")
    })?;
    let disk = pointed(&images, "disk.path")?;
    let ms = fs::read(VARS).map_err(|e| format!("{VARS}: {e}"))?;
    let enrolled = run.work.join("sb-enrolled.fd");
    let list = pesign::keys::esl::list(&cert);
    fs::write(
        &enrolled,
        pesign::keys::ovmf::add_db(&ms, &list).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    // Enrolled: it starts, says Secure Boot is on, and its base is ready.
    let mut s = boot(run, &disk, &enrolled)?;
    let deadline = Instant::now() + run.launch.timeout + Duration::from_secs(30);
    s.seen("Secure Boot on", deadline)?;
    s.seen(&posted(&refcode::BASE_READY, ""), deadline)?;
    s.kill();
    // Not enrolled: refused. The test image, not signed: refused.
    let plain = run.work.join("sb-plain.fd");
    fs::write(&plain, &ms).map_err(|e| e.to_string())?;
    refused(
        run,
        &disk,
        &plain,
        "the release, its certificate not enrolled,",
    )?;
    let again = run.work.join("sb-enrolled-again.fd");
    fs::copy(&enrolled, &again).map_err(|e| e.to_string())?;
    refused(run, &run.image.disk, &again, "the test image, not signed,")?;
    Ok(format!(
        "the release's UKIs verify against {} and start under Secure Boot with it enrolled \
         (`Secure Boot on`, base ready); refused without it; the test image refused",
        Path::new(&cert_path).display()
    ))
}
