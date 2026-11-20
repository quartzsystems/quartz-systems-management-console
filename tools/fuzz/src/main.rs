// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! `qsmcfuzz list | run TARGET [SECS] | all [SECS]`: QS/MC's readers of
//! bytes from outside, run under QS/400's fuzzer (`fuzz::engine`), whose
//! coverage counters `build fuzz` switches on. Corpora are under
//! `tools/fuzz/corpus/<target>/`, findings under `tools/fuzz/crashes/`.

use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Duration;

use fuzz::engine::{self, Options};
use fuzz::targets::Target;

/// A backup file: the header is read as an upload's is, before any key
/// is derived; the body is then taken as a payload.
fn backup_file(data: &[u8]) {
    if let Ok(h) = qmcbackup::seal::header(data) {
        let _ = qmcbackup::Backup::decode(h.body);
    }
}

/// A backup's payload, as PID 1 reads it at a restore.
fn backup_payload(data: &[u8]) {
    if let Ok(b) = qmcbackup::Backup::decode(data) {
        // What decodes encodes again, and reads back the same.
        let again = b.encode().expect("a payload read encodes");
        assert_eq!(qmcbackup::Backup::decode(&again).as_ref(), Ok(&b));
    }
}

/// A certificate import: PEM or PKCS #12, then the check of its key.
fn cert_import(data: &[u8]) {
    if let Ok(b) = webd::tls::read_bundle(data, "fuzz") {
        let _ = webd::tls::checked(b);
    }
}

/// A managed system's bytes after the handshake, as the managed-systems
/// service takes them.
fn mgmt_peer(data: &[u8]) {
    mgmtd::system::fuzzing::peer(data);
}

const PAYLOAD: &[u8] =
    b"QSMCPAY1console QSHQMC1\nrelease V1R0M0\nbuild 1\nmade 1\n\n\x00\x0fstate/host.toml\x00\x00\x01\xa4\x00\x00\x00\x03abc";

/// QS/MC's targets.
pub static TARGETS: &[Target] = &[
    Target {
        name: "backup-file",
        what: "a backup file's header and body, as an upload is read",
        run: backup_file,
        seeds: &[b"QSMCBAK1"],
        dict: &[b"QSMCBAK1", b"QSMCPAY1", b"\x00\x01\x00\x00"],
        max_len: 64 * 1024,
    },
    Target {
        name: "backup-payload",
        what: "a backup's payload, as PID 1 reads it at a restore",
        run: backup_payload,
        seeds: &[PAYLOAD],
        dict: &[
            b"QSMCPAY1",
            b"console ",
            b"release ",
            b"made ",
            b"state/",
            b"web/",
            b"\n\n",
        ],
        max_len: 64 * 1024,
    },
    Target {
        name: "mgmt-peer",
        what: "a managed system's frames after the handshake: replies, events, streams",
        run: mgmt_peer,
        seeds: &[b"QI\x10\x12\x00\x00\x00\x10\x00\x00"],
        dict: &[
            b"QI",
            b"\x10\x11",
            b"\x10\x12",
            b"\x10\x13",
            b"\x10\x20",
            b"\x10\x21",
            b"\x10\x22",
            b"\x10\x23",
            b"\x00\x01",
            b"\x00\x15",
            b"\x00\x17",
            b"\x04\x01",
        ],
        max_len: 64 * 1024,
    },
    Target {
        name: "cert-import",
        what: "a certificate chain and key, PEM or PKCS #12, and its check",
        run: cert_import,
        seeds: &[b"-----BEGIN CERTIFICATE-----\nMA==\n-----END CERTIFICATE-----\n"],
        dict: &[
            b"-----BEGIN CERTIFICATE-----\n",
            b"-----END CERTIFICATE-----\n",
            b"-----BEGIN PRIVATE KEY-----\n",
            b"-----END PRIVATE KEY-----\n",
            b"\x30\x82",
        ],
        max_len: 16 * 1024,
    },
];

fn dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn run(t: &Target, secs: u64) -> std::io::Result<bool> {
    let o = Options {
        time: Duration::from_secs(secs),
        corpus: dir().join("corpus").join(t.name),
        crashes: dir().join("crashes").join(t.name),
        hang: Duration::from_secs(10),
        seed: std::env::var("QSMC_FUZZ_SEED")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(0),
    };
    let r = engine::fuzz(t, &o)?;
    println!(
        "{:<15} {:>10} execs  {:>6} in corpus  {:>7} edges  {} finding(s)",
        t.name,
        r.execs,
        r.corpus,
        r.covered,
        r.crashes.len()
    );
    for (place, file) in &r.crashes {
        println!(
            "                {place}\n                -> {}",
            file.display()
        );
    }
    Ok(r.crashes.is_empty())
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let secs = |i: usize| args.get(i).and_then(|s| s.parse().ok()).unwrap_or(60);
    let chosen: Vec<&Target> = match args.first().map(String::as_str) {
        Some("run") => {
            let wanted = args.get(1).map(String::as_str);
            let Some(t) = TARGETS.iter().find(|t| Some(t.name) == wanted) else {
                eprintln!("run TARGET [SECS]: no target {wanted:?}");
                return ExitCode::FAILURE;
            };
            vec![t]
        }
        Some("all") => TARGETS.iter().collect(),
        _ => {
            println!(
                "{} targets; coverage {}",
                TARGETS.len(),
                if fuzz::cov::instrumented() {
                    "on"
                } else {
                    "off (build with `build fuzz`)"
                }
            );
            for t in TARGETS {
                println!("{:<15} {}", t.name, t.what);
            }
            return ExitCode::SUCCESS;
        }
    };
    let time = secs(if args.first().map(String::as_str) == Some("run") {
        2
    } else {
        1
    });
    let mut clean = true;
    for t in chosen {
        match run(t, time) {
            Ok(ok) => clean &= ok,
            Err(e) => {
                eprintln!("{}: {e}", t.name);
                clean = false;
            }
        }
    }
    if clean {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}
