// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! The two pins (docs/PHASE0.md). QS/VS's crates are at one commit;
//! QS/400's at one commit too, and that commit is the one the pinned
//! QS/VS commit's own `Cargo.toml` names, read from cargo's checkout of
//! it. Then the graph holds one copy of every shared crate, which
//! `cargo tree -d` shows.

use std::collections::BTreeSet;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail, ensure};

/// QS/VS's repository, as `Cargo.toml` names it.
pub const QSVS_GIT: &str = "https://github.com/quartzsystems/quartz-systems-virtual-server";
/// QS/400's repository, as `Cargo.toml` names it.
pub const QS400_GIT: &str = "https://github.com/quartzsystems/quartz-systems-400";

/// The quoted value after `key = ` on `line`.
fn value<'a>(line: &'a str, key: &str) -> Option<&'a str> {
    let at = line.find(&format!("{key} = \""))? + key.len() + 4;
    let rest = &line[at..];
    Some(&rest[..rest.find('"')?])
}

/// Every revision `manifest` (a `Cargo.toml`'s text) pins `url` at.
pub fn revs(manifest: &str, url: &str) -> BTreeSet<String> {
    manifest
        .lines()
        .filter(|l| !l.trim_start().starts_with('#'))
        .filter(|l| value(l, "git") == Some(url))
        .filter_map(|l| value(l, "rev").map(str::to_string))
        .collect()
}

/// The one revision `manifest` pins `url` at.
pub fn pin(manifest: &str, url: &str) -> Result<String> {
    let all = revs(manifest, url);
    match all.len() {
        1 => Ok(all.into_iter().next().unwrap_or_default()),
        0 => bail!("nothing is pinned from {url}"),
        _ => bail!("{url} is pinned at {} commits: {all:?}", all.len()),
    }
}

/// Every revision `lock` (a `Cargo.lock`'s text) has `url` at.
pub fn locked(lock: &str, url: &str) -> BTreeSet<String> {
    let prefix = format!("source = \"git+{url}?rev=");
    lock.lines()
        .filter_map(|l| l.strip_prefix(&prefix))
        .filter_map(|l| l.split(['#', '"']).next())
        .map(str::to_string)
        .collect()
}

fn cargo(repo: &Path) -> Command {
    let mut cmd = Command::new(env::var_os("CARGO").unwrap_or_else(|| "cargo".into()));
    cmd.current_dir(repo);
    cmd
}

fn run(cmd: &mut Command) -> Result<String> {
    let out = cmd
        .output()
        .with_context(|| format!("cannot run {cmd:?}"))?;
    ensure!(
        out.status.success(),
        "{cmd:?} failed: {}",
        String::from_utf8_lossy(&out.stderr).trim()
    );
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// The root of cargo's checkout of QS/VS: the workspace a QS/VS package
/// of the graph sits in.
fn qsvs_checkout(repo: &Path) -> Result<PathBuf> {
    let json = run(cargo(repo).args(["metadata", "--format-version", "1", "--locked"]))?;
    let meta: serde_json::Value = serde_json::from_str(&json)?;
    let source = format!("git+{QSVS_GIT}");
    let manifest = meta["packages"]
        .as_array()
        .context("no packages in cargo metadata")?
        .iter()
        .find(|p| p["source"].as_str().is_some_and(|s| s.starts_with(&source)))
        .and_then(|p| p["manifest_path"].as_str())
        .context("no QS/VS crate in the graph")?;
    Path::new(manifest)
        .ancestors()
        .skip(1)
        .find(|d| fs::read_to_string(d.join("Cargo.toml")).is_ok_and(|t| t.contains("[workspace]")))
        .map(Path::to_path_buf)
        .context("the QS/VS crate is in no workspace")
}

/// The pins agree: one QS/VS commit, one QS/400 commit, the QS/400 one
/// the pinned QS/VS's, and `Cargo.lock` has each crate at its pin.
pub fn agree(repo: &Path) -> Result<String> {
    let ours = fs::read_to_string(repo.join("Cargo.toml"))?;
    let qsvs = pin(&ours, QSVS_GIT)?;
    let qs400 = pin(&ours, QS400_GIT)?;
    let checkout = qsvs_checkout(repo)?;
    let theirs = fs::read_to_string(checkout.join("Cargo.toml"))
        .with_context(|| checkout.display().to_string())?;
    let theirs = pin(&theirs, QS400_GIT).context("QS/VS's Cargo.toml")?;
    ensure!(
        qs400 == theirs,
        "QS/400 is pinned at {qs400}, the pinned QS/VS ({qsvs}) pins {theirs}"
    );
    let lock = fs::read_to_string(repo.join("Cargo.lock"))?;
    for (url, rev) in [(QSVS_GIT, &qsvs), (QS400_GIT, &qs400)] {
        let found = locked(&lock, url);
        ensure!(
            found.len() == 1 && found.contains(rev),
            "Cargo.lock has {url} at {found:?}, the pin is {rev}"
        );
    }
    Ok(format!(
        "QS/VS {}, QS/400 {} (QS/VS's own, from {})",
        &qsvs[..9],
        &qs400[..9],
        checkout.display()
    ))
}

/// The QS/VS and QS/400 crates `cargo tree -d` names: any is there
/// twice.
pub fn duplicates(repo: &Path) -> Result<Vec<String>> {
    let out = run(cargo(repo).args([
        "tree",
        "-d",
        "--workspace",
        "--locked",
        "-e",
        "normal,build,dev",
        "--target",
        "all",
        "--prefix",
        "depth",
        "--format",
        "{p}",
    ]))?;
    Ok(shared_twice(&out))
}

/// The duplicated packages of `cargo tree -d --prefix depth` output that
/// come from QS/VS or QS/400.
pub fn shared_twice(tree: &str) -> Vec<String> {
    let mut found: Vec<String> = tree
        .lines()
        .filter_map(|l| l.strip_prefix('0'))
        .filter(|l| l.contains(QSVS_GIT) || l.contains(QS400_GIT))
        .map(|l| l.split_whitespace().next().unwrap_or("").to_string())
        .collect();
    found.sort();
    found.dedup();
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    const MANIFEST: &str = r#"
[workspace.dependencies]
# appliance = { git = "https://github.com/quartzsystems/quartz-systems-virtual-server", rev = "0000" }
appliance = { git = "https://github.com/quartzsystems/quartz-systems-virtual-server", rev = "aaaa" }
buildkit = { git = "https://github.com/quartzsystems/quartz-systems-400", rev = "bbbb" }
vrm = { git = "https://github.com/quartzsystems/quartz-systems-400", rev = "bbbb", default-features = false }
"#;

    #[test]
    fn the_pins_are_read() {
        assert_eq!(pin(MANIFEST, QSVS_GIT).unwrap(), "aaaa");
        assert_eq!(pin(MANIFEST, QS400_GIT).unwrap(), "bbbb");
        let two = format!("{MANIFEST}x = {{ git = \"{QS400_GIT}\", rev = \"cccc\" }}\n");
        assert!(pin(&two, QS400_GIT).is_err());
        assert!(pin("", QS400_GIT).is_err());
    }

    #[test]
    fn the_lock_is_read() {
        let lock =
            format!("[[package]]\nname = \"vrm\"\nsource = \"git+{QS400_GIT}?rev=bbbb#bbbb\"\n");
        assert_eq!(
            locked(&lock, QS400_GIT).into_iter().collect::<Vec<_>>(),
            ["bbbb"]
        );
        assert!(locked(&lock, QSVS_GIT).is_empty());
    }

    #[test]
    fn only_a_shared_crate_at_depth_0_counts() {
        let tree = format!(
            "0hashbrown v0.15.5\n1indexmap v2.12.0\n0abrec v0.1.0 ({QS400_GIT}?rev=bbbb#bbbb)\n\
             1loadsource v0.1.0 ({QSVS_GIT}?rev=aaaa#aaaa)\n0abrec v0.1.0 ({QS400_GIT}?rev=cccc#cccc)\n"
        );
        assert_eq!(shared_twice(&tree), ["abrec"]);
        assert!(shared_twice("0hashbrown v0.15.5\n0hashbrown v0.16.0\n").is_empty());
    }

    /// This repository's own pins: what CI's pin check runs.
    #[test]
    fn this_repository_pins_each_once() {
        let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let manifest = fs::read_to_string(repo.join("Cargo.toml")).unwrap();
        let qsvs = pin(&manifest, QSVS_GIT).unwrap();
        let qs400 = pin(&manifest, QS400_GIT).unwrap();
        assert_eq!((qsvs.len(), qs400.len()), (40, 40));
        let lock = fs::read_to_string(repo.join("Cargo.lock")).unwrap();
        assert_eq!(
            locked(&lock, QSVS_GIT).into_iter().collect::<Vec<_>>(),
            [qsvs]
        );
        assert_eq!(
            locked(&lock, QS400_GIT).into_iter().collect::<Vec<_>>(),
            [qs400]
        );
    }
}
