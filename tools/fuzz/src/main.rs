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

/// A host's replies to the managed-systems service's reads and its
/// events, of every family, picked and broken by the input.
fn mgmt_host(data: &[u8]) {
    mgmtd::system::fuzzing::host_family(data);
}

/// The servicing families' replies and events (problems, dumps, code
/// update), as the managed-systems service keeps them.
fn mgmt_servicing(data: &[u8]) {
    mgmtd::system::fuzzing::servicing(data);
}

/// A product stream's index or catalog, as the download service reads
/// it, for each of the three streams.
fn stream_index(data: &[u8]) {
    fetchd::verify::fuzz(data);
}

/// An update image of the console, as its update service reads it.
fn update_image(data: &[u8]) {
    update::fuzz_image(&qsmcupdd::FORMAT, data);
}

/// A managed system's bytes after the handshake, as the managed-systems
/// service takes them.
fn mgmt_peer(data: &[u8]) {
    mgmtd::system::fuzzing::peer(data);
}

/// An imported system plan: read strictly, written and read back the
/// same, validated and its steps made.
fn system_plan(data: &[u8]) {
    plan::fuzz(data);
}

/// A request as the web server takes it once `qsweb` has read it: its
/// framing, the redirect on 80, which never leaves the console, and its
/// form.
fn web_request(data: &[u8]) {
    let Ok(req) = qsweb::http::request::read_request(&mut &data[..], &mut Vec::new()) else {
        return;
    };
    let _ = webd::serve::framing_refused(&req);
    let to = webd::serve::https_location(&req, "QSMC123456", "10.0.0.1");
    let host = to
        .strip_prefix("https://")
        .and_then(|r| r.split('/').next())
        .unwrap_or("");
    let first = host.split('.').next().unwrap_or("");
    assert!(
        host == "10.0.0.1" || first.eq_ignore_ascii_case("QSMC123456"),
        "a redirect left the console: {to}"
    );
    let _ = req.form();
}

/// A terminal window's message from the browser: the keys it names, as
/// the socket sends them on.
fn terminal_input(data: &[u8]) {
    let Ok(text) = std::str::from_utf8(data) else {
        return;
    };
    if let Some(input) = termlook::exchange::Input::parse(text) {
        for k in input.console_keys() {
            let _ = qsi::host::console::In::Key(k).piece();
        }
    }
}

/// What the scheduler keeps: its schedules, the plans' index and a
/// deployment's job, as it reads them again at a start.
fn sched_store(data: &[u8]) {
    let Ok(text) = std::str::from_utf8(data) else {
        return;
    };
    let _ = state::store::parse::<schedd::model::Schedules>(text);
    let _ = state::store::parse::<plan::index::Index>(text);
    if let Some(j) = plan::job::read(text) {
        assert!(
            plan::job::read(&plan::job::write(&j)).is_some(),
            "a job read writes and reads"
        );
    }
}

/// A seed volume's `seed.toml`, and the console's rule for its
/// interface roles.
fn seed_answers(data: &[u8]) {
    let Ok(text) = std::str::from_utf8(data) else {
        return;
    };
    if let Ok(s) = seed::format::parse(text) {
        let _ = netd::manage::check_roles(&s.network, release::INTERFACE_ROLES);
    }
}

const PLAN: &[u8] = b"schema_version = 1\nname = \"seed\"\ncreated = 1\nby = \"qsadmin\"\nsystem = \"QSHQVS1\"\n\n[host]\nname = \"QSHQVS1\"\nserial = \"10-9F11A2\"\ncores = 8\nmemory_mib = 131072\n\n[[pool]]\nname = \"POOL1\"\nprotection = \"raid6\"\nbytes = 1073741824\n\n[[pool.unit]]\nlocation = \"U9Q10.V1.9F11A2-P1-C2-T1-L0\"\ndisk = \"sdc\"\nbytes = 1073741824\n\n[[switch]]\nname = \"ETHSW0\"\nvlans = [10, 20]\nmtu = 1500\nuplink = [\"U9Q10.V1.9F11A2-P1-C1-T1\"]\n\n[[procpool]]\nname = \"TESTPOOL\"\ncores = [6, 7]\n\n[[vdisk]]\npool = \"POOL1\"\nname = \"VD01\"\nmib = 100\nthick = true\n\n[[volume]]\nname = \"VTAP01\"\nmib = 100\n\n[[image]]\nname = \"A.iso\"\n\n[[partition]]\nid = 1\nname = \"QSDEV01\"\nserial = \"K2M4101\"\nkeylock = \"normal\"\ndefault_profile = \"default\"\n\n[[partition.profile]]\nname = \"default\"\nscsi = [{ slot = 3, luns = [{ lun = 0, backing = \"POOL1/VD01\" }] }]\nethernet = [{ slot = 2, switch = \"ETHSW0\", vlan = 10, mac = \"02:51:53:00:01:02\", required = true }]\noptical = [{ slot = 5, media = \"A.iso\" }]\ntape = [{ slot = 6, media = \"VTAP01\" }]\nphysical_io = [{ location = \"U9Q10.V1.9F11A2-P1-C3\" }]\n\n[partition.profile.processors]\nmode = \"shared\"\nminimum = 1\ndesired = 1\nmaximum = 2\nminimum_units = 10\ndesired_units = 50\nmaximum_units = 100\nsharing = \"uncapped\"\nweight = 128\npool = \"TESTPOOL\"\n\n[partition.profile.memory]\nminimum_mib = 1024\ndesired_mib = 2048\nmaximum_mib = 4096\n\n[partition.profile.boot]\nsource = \"A\"\nipl_type = \"unattended\"\nconsole = \"partition\"\n";

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
        name: "mgmt-host",
        what: "a host's replies to every read and its events, each family's, as the managed-systems service keeps them",
        run: mgmt_host,
        seeds: &[&[2; 64], &[3, 0, 2, 7, 2, 9, 3, 1, 2, 4, 2, 2, 2, 5]],
        dict: &[b"\x00\x01", b"\x00\x18", b"\x00\x13"],
        max_len: 16 * 1024,
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
    Target {
        name: "mgmt-servicing",
        what: "a system's replies and events of problems, dumps and code update, picked and broken by the input",
        run: mgmt_servicing,
        seeds: &[b"", &[1u8; 64], &[2u8; 64]],
        dict: &[b"\x00\x03", b"\x00\x04", b"\x00\x16", b"QSI1001"],
        max_len: 64 * 1024,
    },
    Target {
        name: "stream-index",
        what: "a product stream's signed index or catalog, as the download service reads it",
        run: stream_index,
        seeds: &[
            b"format = 1\nstream = \"qsmc\"\nsequence = 1\ngenerated = 1\n\n[[release]]\nrelease = \"V1R0M1\"\n# signature 00\n",
            b"format = 1\nsequence = 42\ngenerated = 1\n\n[[group]]\nname = \"C26290110\"\nkind = \"cumulative\"\n# signature 00\n",
        ],
        dict: &[b"# signature ", b"[[release]]", b"[[group]]", b"sequence = ", b"stream = ", b"update_sha256", b"updates_from"],
        max_len: 64 * 1024,
    },
    Target {
        name: "system-plan",
        what: "an imported system plan: its strict reader, writer, validation and steps",
        run: system_plan,
        seeds: &[PLAN],
        dict: &[
            b"schema_version = ",
            b"[[partition]]",
            b"[[partition.profile]]",
            b"[[pool]]",
            b"[[pool.unit]]",
            b"[[switch]]",
            b"[[vdisk]]",
            b"[partition.profile.processors]",
            b"mode = \"dedicated\"",
            b"backing = ",
            b"media = \"@",
            b"protection = ",
        ],
        max_len: 64 * 1024,
    },
    Target {
        name: "qsmc-update-image",
        what: "an update image of the console, its container, manifest and verification",
        run: update_image,
        seeds: &[b"QSMCUPDT\x01\x00\x00\x00", b"QSMC-UPDATE 1\nproduct QS/MC\nrelease V1R0M1\n"],
        dict: &[b"QSMCUPDT", b"QSMC-UPDATE 1", b"product ", b"release ", b"state ", b"file ", b"SLOTA.EFI", b"SLOTB.EFI"],
        max_len: 64 * 1024,
    },
    Target {
        name: "web-request",
        what: "a request after qsweb read it: its framing, the redirect on 80, its form",
        run: web_request,
        seeds: &[b"POST /qsmc/signon HTTP/1.1
Host: QSMC123456
Content-Length: 9

user=a&b=c"],
        dict: &[b"
", b"Host: ", b"Content-Length: ", b"Transfer-Encoding: chunked", b"%2e%2e/", b"&", b"="],
        max_len: 16 * 1024,
    },
    Target {
        name: "terminal-input",
        what: "a terminal window's messages from the browser, as keys",
        run: terminal_input,
        seeds: &[b"{\"k\":\"Enter\"}", b"{\"t\":\"abc\"}"],
        dict: &[b"{\"k\":\"", b"{\"t\":\"", b"\"}", b"F24", b"Enter"],
        max_len: 4096,
    },
    Target {
        name: "sched-store",
        what: "the scheduler's schedules, the plans' index and a deployment's job",
        run: sched_store,
        seeds: &[b"schema_version = 1
next_id = 1
"],
        dict: &[b"schema_version = 1
", b"[[schedules]]
", b"[[runs]]
", b"[[step]]
", b"id = ", b"\""],
        max_len: 64 * 1024,
    },
    Target {
        name: "seed-answers",
        what: "a seed volume's seed.toml and the interface roles",
        run: seed_answers,
        seeds: &[b"[network]
interfaces = []
"],
        dict: &[b"[network]
", b"[install]
", b"role = \"open\"
", b"[[network.interfaces]]
"],
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
