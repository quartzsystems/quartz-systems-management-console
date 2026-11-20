// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! QS/MC build orchestrator: `cargo run -p build -- <command>`.
//!
//! The pipeline is QS/400's `buildkit`, taken as a library; `product`
//! describes QS/MC to it, with the load source of QS/VS's
//! `base/loadsource`. What is QS/MC's own is here: the userland with or
//! without the test hooks, the A/B record on the disk image, the
//! manifest held to its image, a load source written onto image files
//! (`image`), and the host checks and the QEMU suites (`host`).
//!
//! `--test-image` builds the test image: the same sources with the test
//! hooks, under `test-image/` in the target root, a target of its own.

mod host;
mod image;
mod product;

use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result};
use buildkit::{Ctx, kernel, notices};
use clap::{Parser, Subcommand};
use product::Image;
use tracing_subscriber::EnvFilter;

#[derive(Parser)]
#[command(name = "build", about = "QS/MC build orchestrator", version)]
struct Cli {
    /// Stream tool output instead of writing it to target/logs.
    #[arg(short, long, global = true)]
    verbose: bool,
    /// Build the test image: the release image's sources with the test
    /// hooks, under `test-image/` in the target root. Never shipped.
    #[arg(long, global = true)]
    test_image: bool,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Download, verify and extract the pinned kernel tarball.
    Fetch,
    /// Configure and build the kernel.
    Kernel,
    /// Build PID 1 and the services for the musl target; verify they are
    /// static and stripped, and the test hooks only where they belong.
    Userland,
    /// Write the deterministic newc cpio.
    Initramfs,
    /// Rebuild the kernel with the initramfs embedded: the slot A, slot B
    /// and media UKIs.
    Uki,
    /// Write the GPT disk image: two-slot ESP, A/B record, QSMCDATA.
    Disk {
        /// Image size in bytes (default 1 GiB).
        #[arg(long)]
        size: Option<u64>,
    },
    /// Write a load source onto image files with `base/loadsource`'s
    /// writer, from the built slot UKIs: one file for one disk, two for a
    /// mirrored pair.
    Loadsource {
        /// The image files: one, or two for a mirrored pair.
        #[arg(required = true, num_args = 1..=2)]
        out: Vec<PathBuf>,
        /// Each disk's size in bytes (default 1 GiB).
        #[arg(long)]
        size: Option<u64>,
    },
    /// Write the UEFI-bootable installation ISO.
    Iso,
    /// Write and sign the artifact manifest.
    Manifest,
    /// Run the whole pipeline.
    All,
    /// Boot under QEMU: `--media disk` (default) runs Phase 0's steps on
    /// the release image, then the base system's suites on the test
    /// image; `--media iso` boots the release ISO.
    Test {
        /// What to boot.
        #[arg(long, default_value = "disk")]
        media: conformance::Media,
        /// Only this step or suite; `list` lists them.
        #[arg(long)]
        suite: Option<String>,
    },
    /// Build the fuzzer with coverage counters and run its targets
    /// (`tools/fuzz`): a backup's reader and the certificate import.
    Fuzz {
        /// Only this target.
        #[arg(long)]
        target: Option<String>,
        /// Seconds each.
        #[arg(long, default_value_t = 60)]
        secs: u64,
    },
    /// Check host prerequisites, the license headers and the two pins.
    Doctor,
    /// Run cargo-deny plus the C-linkage check of the compiled graph.
    Deny,
    /// Write `NOTICES.txt` (every crate of `Cargo.lock`, its license and
    /// the license texts) into `lic/release/`.
    Notices {
        /// Fail when the file in the tree is stale instead of writing it.
        #[arg(long)]
        check: bool,
    },
}

/// The context of one image kind. A test image is built in its own
/// directory of the target root, so nothing of it is ever among the
/// release artifacts.
fn context(image: Image, verbose: bool) -> Result<Ctx> {
    let mut ctx = Ctx::new(product::qsmc(image)?, verbose)?;
    if image == Image::Test {
        let release = ctx.paths.target.clone();
        ctx.paths.target = release.join(product::TEST_DIR);
        // The kernel tarball the release build fetched serves both.
        let pin = kernel::fetch::read_pin(&ctx.paths)?;
        let tarball = format!("linux-{}.tar.xz", pin.version);
        let (from, to) = (release.join("src").join(&tarball), ctx.paths.src());
        if from.is_file() && !to.join(&tarball).is_file() {
            fs::create_dir_all(&to)?;
            fs::copy(&from, to.join(&tarball))
                .with_context(|| format!("cannot copy {}", from.display()))?;
        }
    }
    Ok(ctx)
}

/// The pipeline in order. The userland, disk and manifest steps are
/// QS/MC's own.
fn all(ctx: &Ctx, image: Image) -> Result<()> {
    tracing::info!("building {} {}", ctx.product.name, ctx.product.full());
    kernel::fetch::fetch(ctx)?;
    kernel::kernel(ctx)?;
    image::userland::userland(ctx, image)?;
    buildkit::image::initramfs::initramfs(ctx)?;
    kernel::uki(ctx, false)?;
    image::disk::disk(ctx, None)?;
    buildkit::image::iso::iso(ctx)?;
    image::manifest::manifest(ctx, image)?;
    Ok(())
}

fn main() -> Result<()> {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .without_time()
        .with_target(false)
        .init();
    let cli = Cli::parse();
    let image = if cli.test_image {
        Image::Test
    } else {
        Image::Release
    };
    let ctx = context(image, cli.verbose)?;
    if !cfg!(target_os = "linux")
        && !matches!(
            cli.command,
            Command::Doctor | Command::Deny | Command::Notices { .. }
        )
    {
        tracing::warn!("the pipeline needs a Linux host; run this from WSL");
    }
    match cli.command {
        Command::Fetch => {
            kernel::fetch::fetch(&ctx)?;
        }
        Command::Kernel => kernel::kernel(&ctx)?,
        Command::Userland => {
            image::userland::userland(&ctx, image)?;
        }
        Command::Initramfs => {
            buildkit::image::initramfs::initramfs(&ctx)?;
        }
        Command::Uki => {
            kernel::uki(&ctx, false)?;
        }
        Command::Disk { size } => {
            image::disk::disk(&ctx, size)?;
        }
        Command::Loadsource { out, size } => image::writer::write(&ctx, &out, size)?,
        Command::Iso => {
            buildkit::image::iso::iso(&ctx)?;
        }
        Command::Manifest => {
            image::manifest::manifest(&ctx, image)?;
        }
        Command::All => all(&ctx, image)?,
        Command::Test { media, suite } => {
            let release = context(Image::Release, cli.verbose)?;
            let test = context(Image::Test, cli.verbose)?;
            host::testrun::test(&release, &test, media, suite.as_deref())?;
        }
        Command::Fuzz { target, secs } => host::fuzz::fuzz(&ctx, target.as_deref(), secs)?,
        Command::Doctor => buildkit::host::doctor::doctor(&ctx)?,
        Command::Deny => buildkit::host::deny::deny(&ctx)?,
        Command::Notices { check } => {
            notices::notices(&ctx, check)?;
        }
    }
    Ok(())
}
