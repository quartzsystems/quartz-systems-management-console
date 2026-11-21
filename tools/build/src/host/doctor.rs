// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! The rows QS/MC adds to `buildkit`'s doctor: the OVMF firmware, the
//! license headers, the two pins agreeing, and no shared crate twice.
//! `buildkit` adds KVM and the target directory itself.

use buildkit::Ctx;
use buildkit::host::doctor::{Row, Status};
use conformance::qemu::{CODE_ENV, VARS_ENV};

fn row(name: &str, ok: bool, detail: String) -> Row {
    Row {
        name: name.to_string(),
        required: true,
        status: if ok { Status::Ok } else { Status::Missing },
        detail,
    }
}

fn ovmf() -> Row {
    let name = format!("OVMF ({CODE_ENV})");
    let found = driver::firmware::locate(CODE_ENV).and_then(|code| {
        // The slot B boot writes its boot entry into the variable store.
        let vars = driver::firmware::locate_vars(&code, VARS_ENV)?;
        Ok(format!("{}, {}", code.display(), vars.display()))
    });
    match found {
        Ok(detail) => row(&name, true, detail),
        Err(e) => row(&name, false, e.to_string()),
    }
}

fn headers(ctx: &Ctx) -> Row {
    let name = "license headers";
    match buildaid::header::check(&ctx.paths.repo) {
        Ok((count, bad)) if bad.is_empty() => row(
            name,
            true,
            format!("{count} .rs files start with the header"),
        ),
        Ok((_, bad)) => row(name, false, format!("no header: {}", bad.join(", "))),
        Err(e) => row(name, false, format!("{e:#}")),
    }
}

fn pins(ctx: &Ctx) -> Row {
    let name = "the two pins agree";
    match super::pins::agree(&ctx.paths.repo) {
        Ok(detail) => row(name, true, detail),
        Err(e) => row(name, false, format!("{e:#}")),
    }
}

fn once(ctx: &Ctx) -> Row {
    let name = "no shared crate twice";
    match super::pins::duplicates(&ctx.paths.repo) {
        Ok(twice) if twice.is_empty() => row(
            name,
            true,
            "cargo tree -d names no QS/VS or QS/400 crate".to_string(),
        ),
        Ok(twice) => row(name, false, format!("twice: {}", twice.join(", "))),
        Err(e) => row(name, false, format!("{e:#}")),
    }
}

/// The QS/VS test disk suite 10 runs a real host from: not required,
/// and how to make one when it is missing.
fn qsvs_disk(ctx: &Ctx) -> Row {
    let name = format!("QS/VS test disk ({})", conformance::suite::mgmt::host::DISK);
    let mut r = match std::env::var(conformance::suite::mgmt::host::DISK) {
        Ok(p) if std::path::Path::new(&p).is_file() => row(&name, true, p),
        _ => {
            let checkout = super::pins::qsvs_checkout(&ctx.paths.repo).map_or_else(
                |_| "cargo's checkout of QS/VS at the pin".into(),
                |c| c.display().to_string(),
            );
            row(
                &name,
                false,
                format!(
                    "for `build test --suite mgmt-host` only: in {checkout}, `cargo run -p build -- --test-image all` \
                     and `cargo run -p build -- test --suite qsi-pairing`, which keeps the installed disk as \
                     `conformance/installed-single-0.img` in its target; name it"
                ),
            )
        }
    };
    r.required = false;
    r
}

/// QS/400's disk image, for suite 10's partition.
fn qs400_image() -> Row {
    let name = format!("QS/400 image ({})", conformance::suite::mgmt::host::QS400);
    let mut r = match std::env::var(conformance::suite::mgmt::host::QS400) {
        Ok(p) if std::path::Path::new(&p).is_file() => row(&name, true, p),
        _ => row(
            &name,
            false,
            "for `build test --suite mgmt-host` only: QS/400's `cargo run -p build -- all` at the commit \
             QS/VS pins writes `images/qs400-*.img`; name it"
                .into(),
        ),
    };
    r.required = false;
    r
}

/// QS/MC's rows.
pub fn rows(ctx: &Ctx) -> Vec<Row> {
    vec![
        ovmf(),
        headers(ctx),
        pins(ctx),
        once(ctx),
        qsvs_disk(ctx),
        qs400_image(),
    ]
}
