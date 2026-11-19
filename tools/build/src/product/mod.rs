// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! QS/MC as `buildkit` sees it: the names, the kernel's needs, the
//! userland, the initramfs, the command lines, the disk and the media.
//!
//! There are two kinds of image, as QS/VS has them. The release image is
//! what ships. The test image is the same with the test hooks added
//! (`qsmchook`, and PID 1 built under `cfg(test_hooks)`); it is built
//! under its own directory of the target root and its manifest names it
//! a test image.

mod initramfs;
pub mod kernel;
mod media;

use std::path::Path;

use anyhow::{Context, Result};
use buildkit::{Notices, Package, Product, UkiKind};
use loadsource::layout;
use release::LOAD_SOURCE;

/// The PID 1 package, `/init` in every initramfs.
pub const INIT: &str = "qsmcinit";

/// The service packages, `/qsmc/bin/<name>` in a slot's initramfs.
pub const SERVICES: [&str; 4] = ["qsmcstgd", "qsmclogd", "qsmcnetd", "qsmcwebd"];

/// The local console, `/qsmc/bin/qsmccons` in a slot's initramfs.
pub const CONSOLE: &str = "qsmccons";

/// The installer, `/qsmc/bin/qsmcinstall` in the media's initramfs.
pub const INSTALLER: &str = "qsmcinstall";

/// The test hooks' package: in a test image only.
pub const HOOK: &str = "qsmchook";

/// The environment variable that builds PID 1 with its test hooks.
pub const HOOKS_ENV: &str = "QSMC_TEST_HOOKS";

/// What PID 1 of a test image prints, and no other PID 1 contains.
pub const HOOKS_MARKER: &str = "TEST IMAGE:";

/// The test image's directory under the target root.
pub const TEST_DIR: &str = "test-image";

/// Which image is built.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Image {
    /// What ships: no test hooks.
    Release,
    /// The release image with the test hooks added. Never shipped.
    Test,
}

const fn package(name: &'static str) -> Package {
    Package {
        name,
        manifest: true,
    }
}

const RELEASE_PACKAGES: &[Package] = &[
    package(INIT),
    package(SERVICES[0]),
    package(SERVICES[1]),
    package(SERVICES[2]),
    package(SERVICES[3]),
    package(CONSOLE),
    package(INSTALLER),
];
const TEST_PACKAGES: &[Package] = &[
    package(INIT),
    package(SERVICES[0]),
    package(SERVICES[1]),
    package(SERVICES[2]),
    package(SERVICES[3]),
    package(CONSOLE),
    package(INSTALLER),
    package(HOOK),
];

/// Where the generated notices file lives in the tree.
pub const NOTICES_FILE: &str = "lic/release/NOTICES.txt";

/// QS/MC's image of kind `image`, from this crate's place in the
/// repository.
pub fn qsmc(image: Image) -> Result<Product> {
    let (name, packages, initramfs): (_, _, fn(&_, _) -> _) = match image {
        Image::Release => (release::NAME, RELEASE_PACKAGES, initramfs::entries),
        Image::Test => (release::TEST_NAME, TEST_PACKAGES, initramfs::test_entries),
    };
    let repo = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .context("build crate is not under tools/")?
        .to_path_buf();
    Ok(Product {
        id: release::ID,
        name,
        vendor: LOAD_SOURCE.vendor,
        env_prefix: release::ENV_PREFIX,
        key_prefix: release::KEY_PREFIX,
        repo,
        release: release::RELEASE.to_string(),
        build_id: release::BUILD_ID.to_string(),
        fragments: &[kernel::FRAGMENT],
        required_y: kernel::required_y(),
        required_n: kernel::required_n(),
        cmdline: kernel::CMDLINE,
        packages,
        init: INIT,
        kinds: &UkiKind::ALL,
        uki_cmdline,
        initramfs,
        disk_bytes: layout::DEFAULT_DISK_BYTES,
        disk_parts: || layout::parts(&LOAD_SOURCE),
        disk_guids: || layout::fixed_guids(&LOAD_SOURCE),
        esp_label: LOAD_SOURCE.esp_label,
        esp_volume_id: LOAD_SOURCE.esp_volume_id,
        media: Some(media::media),
        doctor: crate::host::doctor::rows,
        deny: &[],
        notices: Some(Notices {
            file: NOTICES_FILE,
            intro: notices_intro,
        }),
        secure_boot_db: || None,
    })
}

/// The slots differ only in `qsmc.slot=`; the media UKI carries
/// `qsmc.install=1` instead of a slot. There are no development words.
fn uki_cmdline(p: &Product, kind: UkiKind, _dev: bool) -> String {
    let base = p.cmdline;
    match kind {
        UkiKind::SlotA => format!("{base} {}=A", p.key("slot")),
        UkiKind::SlotB => format!("{base} {}=B", p.key("slot")),
        UkiKind::Media => format!("{base} {}=1", p.key("install")),
    }
}

/// The opening lines of `NOTICES.txt`.
fn notices_intro(release: &str, kernel_version: &str, crates: usize) -> String {
    format!(
        "QS/MC {release} third-party notices\n\
         \nQS/MC is licensed under the MIT license (the LICENSE file of its source).\n\
         It contains the work of others, listed here with their licenses:\n\n\
         1. The Linux kernel {kernel_version}, under GPL-2.0 with the Linux syscall note.\n\
         \x20  No firmware is built into the kernel (CONFIG_EXTRA_FIRMWARE is empty).\n\
         2. musl libc, under MIT: Copyright (c) 2005-2020 Rich Felker, et al.\n\
         \x20  musl's COPYRIGHT file names its authors and the few files it holds\n\
         \x20  under other permissive terms.\n\
         3. The Rust standard library (core, alloc, std and their dependencies),\n\
         \x20  under MIT OR Apache-2.0, taken under MIT; LLVM's libunwind and\n\
         \x20  compiler-rt builtins, under Apache-2.0 WITH LLVM-exception.\n\
         4. The {crates} Rust crates below, each under the license named. The\n\
         \x20  crates of QS/VS and of QS/400 (Quartz Systems, MIT) are among them:\n\
         \x20  QS/MC takes QS/VS's base crates and QS/400's build pipeline and\n\
         \x20  disk formats as libraries.\n"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_command_lines_are_the_baked_ones() {
        let p = qsmc(Image::Release).unwrap();
        let base = "console=ttyS0,115200 console=tty0 panic=-1 quiet";
        let line = |kind| (p.uki_cmdline)(&p, kind, true);
        assert_eq!(line(UkiKind::SlotA), format!("{base} qsmc.slot=A"));
        assert_eq!(line(UkiKind::SlotB), format!("{base} qsmc.slot=B"));
        assert_eq!(line(UkiKind::Media), format!("{base} qsmc.install=1"));
        assert_eq!(
            (p.uki_cmdline)(&p, UkiKind::SlotA, false),
            line(UkiKind::SlotA)
        );
    }

    #[test]
    fn the_names_are_qs_mc_s() {
        let p = qsmc(Image::Release).unwrap();
        assert_eq!((p.id, p.name, p.vendor), ("qsmc", "QS/MC", "QSMC"));
        assert_eq!((p.env_prefix, p.key_prefix), ("QSMC_", "qsmc."));
        assert_eq!(buildkit::image::disk::slot_b(&p), LOAD_SOURCE.slot_b_path);
        assert_eq!(buildkit::image::disk::SLOT_A, layout::SLOT_A_PATH);
        let spec = p.fat_spec(diskimg::fat::DISK_ESP).unwrap();
        assert_eq!(&spec.label, b"QSMC ESP   ");
        assert_eq!(spec.volume_id, 0x5153_4d43);
        assert!(p.repo.join("VERSION").is_file());
        let names: Vec<&str> = p.packages.iter().map(|p| p.name).collect();
        assert_eq!(
            names,
            [
                "qsmcinit",
                "qsmcstgd",
                "qsmclogd",
                "qsmcnetd",
                "qsmcwebd",
                "qsmccons",
                "qsmcinstall"
            ]
        );
        assert_eq!(format!("/qsmc/bin/{CONSOLE}"), release::CONSOLE);
        assert_eq!(format!("/qsmc/bin/{INSTALLER}"), release::INSTALLER);
        let test = qsmc(Image::Test).unwrap();
        assert_eq!(test.name, "QS/MC (test image)");
        assert_eq!(test.packages.last().map(|p| p.name), Some(HOOK));
        assert_eq!((test.cmdline, test.fragments), (p.cmdline, p.fragments));
        assert_eq!(p.fragments, ["qsmc.config"]);
    }
}
