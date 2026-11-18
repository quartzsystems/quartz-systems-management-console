// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! The rows QS/MC adds to `buildkit`'s doctor: the OVMF firmware, the
//! license headers, the two pins agreeing, and no shared crate twice.
//! `buildkit` adds KVM and the target directory itself.

use buildkit::Ctx;
use buildkit::host::doctor::{Row, Status};
use conformance::firmware;

fn row(name: &str, ok: bool, detail: String) -> Row {
    Row {
        name: name.to_string(),
        required: true,
        status: if ok { Status::Ok } else { Status::Missing },
        detail,
    }
}

fn ovmf() -> Row {
    let name = format!("OVMF ({})", firmware::CODE_ENV);
    let found = firmware::locate().and_then(|code| {
        // The slot B boot writes its boot entry into the variable store.
        let vars = firmware::locate_vars(&code)?;
        Ok(format!("{}, {}", code.display(), vars.display()))
    });
    match found {
        Ok(detail) => row(&name, true, detail),
        Err(e) => row(&name, false, e.to_string()),
    }
}

fn headers(ctx: &Ctx) -> Row {
    let name = "license headers";
    match super::header::check(&ctx.paths.repo) {
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

/// QS/MC's rows.
pub fn rows(ctx: &Ctx) -> Vec<Row> {
    vec![ovmf(), headers(ctx), pins(ctx), once(ctx)]
}
