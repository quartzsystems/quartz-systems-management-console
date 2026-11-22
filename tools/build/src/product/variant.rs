// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! The test image's variants (`docs/UPDATES.md`, suites 8 to 13): the
//! updates a console of the test image installs. Each is the test image
//! built again under `test-image/<variant>/` with what makes it differ
//! set in the environment, as far as its update image, signed with the
//! test image's own key so the test image trusts it. The kernel's
//! source and QS/VS's catalogue are the test image's, not made again.
//!
//! - `next`: `QSMC_RELEASE=V1R0M1 QSMC_NEXT_SCHEMA=1`: V1R0M1, whose
//!   `web.toml` is of a newer schema.
//! - `broken`: `QSMC_RELEASE=V1R0M1 QSMC_TEST_WEBD_BROKEN=1`: one whose
//!   web server cannot start, which a console rolls back from.

use std::fs;
use std::path::Path;

use anyhow::{Context, Result, bail};

/// A variant.
#[derive(Clone, Copy, Debug, PartialEq, Eq, clap::ValueEnum)]
pub enum Variant {
    /// V1R0M1 with a newer state schema.
    Next,
    /// V1R0M1 whose web server cannot start.
    Broken,
}

/// Whether `name` is set to `1`.
fn on(name: &str) -> bool {
    std::env::var(name).is_ok_and(|v| v == "1")
}

impl Variant {
    /// Its directory under the test image's.
    #[must_use]
    pub const fn dir(self) -> &'static str {
        match self {
            Variant::Next => "next",
            Variant::Broken => "broken",
        }
    }

    /// What its environment must say, and what it must not.
    fn check(self) -> Result<()> {
        let v: Vec<&str> = include_str!("../../../../VERSION")
            .trim()
            .split('.')
            .collect();
        let here: vrm::Release = match v.as_slice() {
            [a, b, c] => format!("V{a}R{b}M{c}"),
            _ => String::new(),
        }
        .parse()
        .map_err(|e| anyhow::anyhow!("VERSION: {e:?}"))?;
        if release::RELEASE <= here {
            bail!(
                "the {} variant is a newer release: set QSMC_RELEASE past {here} (V1R0M1)",
                self.dir()
            );
        }
        let (want, not) = match self {
            Variant::Next => ("QSMC_NEXT_SCHEMA", "QSMC_TEST_WEBD_BROKEN"),
            Variant::Broken => ("QSMC_TEST_WEBD_BROKEN", "QSMC_NEXT_SCHEMA"),
        };
        if !on(want) || on(not) {
            bail!(
                "the {} variant is built with {want}=1 and without {not}",
                self.dir()
            );
        }
        Ok(())
    }

    /// Make `to` ready to build into: the environment checked, the test
    /// image's key, catalogue and kernel source (`test`) taken.
    ///
    /// # Errors
    /// The environment is not the variant's, or the test image is not
    /// built.
    pub fn prepare(self, test: &Path, to: &Path) -> Result<()> {
        self.check()?;
        let key = test.join("keys").join("dev.ed25519");
        if !key.is_file() {
            bail!(
                "{} is missing: build the test image first (`--test-image all`)",
                key.display()
            );
        }
        fs::create_dir_all(to.join("keys"))?;
        fs::copy(&key, to.join("keys").join("dev.ed25519"))?;
        let cat = test.join("qsvs-catalogue");
        if cat.is_dir() {
            fs::create_dir_all(to.join("qsvs-catalogue"))?;
            for e in fs::read_dir(&cat)?.flatten() {
                let name = e.file_name();
                if name.to_string_lossy().ends_with(".json") {
                    fs::copy(e.path(), to.join("qsvs-catalogue").join(&name))?;
                }
            }
        }
        let src = to.join("src");
        if !src.exists() {
            #[cfg(unix)]
            std::os::unix::fs::symlink(test.join("src"), &src)
                .with_context(|| format!("cannot link {}", src.display()))?;
        }
        Ok(())
    }
}
