// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! The channel's commands (`crate::proto::command`), each answered with
//! the lines of `crate::proto::word`.

use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

use netd::control::Calls as _;
use rustix::fs::OFlags;
use rustix::process::{Pid, Signal, kill_process};
use stgd::Calls as _;
use supervise::control::{Calls as _, Client};

use crate::proto::{LOAD_DIR, command, parse_status, status_lines, storage_lines, word};

/// The largest file `cat` sends.
const CAT_MAX: u64 = 1 << 20;

fn appliance() -> release::Appliance {
    release::appliance(|_| {}, |_, _| {})
}

fn ok() -> Vec<String> {
    vec![word::OK.to_string()]
}

fn error(why: impl std::fmt::Display) -> Vec<String> {
    vec![format!("{} {why}", word::ERROR)]
}

fn done<E: std::fmt::Display>(result: Result<(), E>) -> Vec<String> {
    result.map_or_else(error, |()| ok())
}

fn control() -> Client {
    Client::new(release::CONTROL_SOCKET)
}

fn status() -> Vec<String> {
    control()
        .status()
        .map_or_else(error, |status| status_lines(&status))
}

fn kill(service: &str) -> Vec<String> {
    // PID 1 itself is asked to shut down, as an operator's `kill` asks.
    if service == command::INIT {
        return done(kill_process(Pid::INIT, Signal::TERM));
    }
    let status = match parse_status(&status()) {
        Ok(status) => status,
        Err(e) => return error(e),
    };
    let pid = status
        .service(service)
        .and_then(|s| i32::try_from(s.pid).ok())
        .and_then(Pid::from_raw);
    match pid {
        Some(pid) => done(kill_process(pid, Signal::KILL)),
        None => error(format!("{service} has no process")),
    }
}

fn cat(path: &str) -> Vec<String> {
    match fs::metadata(path) {
        Ok(meta) if meta.len() > CAT_MAX => error(format!("{path}: {} bytes", meta.len())),
        // Files of /proc and /sys say they are empty: read them anyway.
        Ok(_) => fs::read(path).map_or_else(
            |e| error(format!("{path}: {e}")),
            |bytes| vec![format!("{} {}", word::HEX, hex::encode(bytes))],
        ),
        Err(e) => error(format!("{path}: {e}")),
    }
}

fn put(rest: &str) -> Vec<String> {
    let Some((path, contents)) = rest.split_once(' ') else {
        return error("put <path> <hex>");
    };
    match hex::decode(contents) {
        Ok(bytes) => done(state::atomic_write(Path::new(path), &bytes)),
        Err(e) => error(e),
    }
}

fn ls(dir: &str) -> Vec<String> {
    match fs::read_dir(dir) {
        Ok(entries) => {
            let mut names: Vec<String> = entries
                .flatten()
                .map(|e| format!("{} {}", word::NAME, e.file_name().to_string_lossy()))
                .collect();
            names.sort();
            names
        }
        Err(e) => error(format!("{dir}: {e}")),
    }
}

fn stat(path: &str) -> Vec<String> {
    match fs::metadata(path) {
        Ok(m) => vec![format!(
            "{} {:o} {} {} {}",
            word::OK,
            m.mode() & 0o7777,
            m.uid(),
            m.gid(),
            m.len()
        )],
        Err(e) => error(format!("{path}: {e}")),
    }
}

fn readlink(path: &str) -> Vec<String> {
    match fs::read_link(path) {
        Ok(t) => vec![format!("{} {}", word::OK, t.display())],
        Err(e) => error(format!("{path}: {e}")),
    }
}

/// The kernel log's lines that contain `text`, read from its start.
fn kmsg(text: &str) -> Vec<String> {
    let flags = i32::try_from(OFlags::NONBLOCK.bits()).unwrap_or(0);
    let opened = OpenOptions::new()
        .read(true)
        .custom_flags(flags)
        .open("/dev/kmsg");
    let mut log = match opened {
        Ok(log) => log,
        Err(e) => return error(e),
    };
    let mut lines = Vec::new();
    let mut buf = vec![0u8; 8192];
    loop {
        match log.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                let record = String::from_utf8_lossy(&buf[..n]);
                let said = record.split_once(';').map_or("", |(_, rest)| rest);
                let said = said.lines().next().unwrap_or("");
                if said.contains(text) {
                    lines.push(format!("{} {said}", word::LINE));
                }
            }
            // Records overwritten before they were read.
            Err(e) if e.kind() == io::ErrorKind::BrokenPipe => {}
            Err(_) => break,
        }
    }
    lines
}

fn set_host(name: &str) -> Vec<String> {
    let store = state::Store::at(appliance().state_dir());
    let host: state::Host = match store.load() {
        Ok(found) => found.unwrap_or_default(),
        Err(e) => return error(e),
    };
    match control().set_host(name.to_string(), host.time_zone, host.ntp_servers) {
        Ok(Ok(())) => ok(),
        Ok(Err(e)) => error(e),
        Err(e) => error(e),
    }
}

fn net() -> netd::control::Client {
    netd::control::Client::new(appliance().net_socket())
}

fn set_clock(secs: &str) -> Vec<String> {
    let Ok(secs) = secs.trim().parse::<i64>() else {
        return error("setclock <seconds since the epoch>");
    };
    match net().set_clock(secs) {
        Ok(Ok(())) => ok(),
        Ok(Err(e)) => error(e),
        Err(e) => error(e),
    }
}

fn storage_client() -> stgd::Client {
    stgd::Client::of(&appliance())
}

fn keep(disk: &str) -> Vec<String> {
    match storage_client().keep(disk.trim().to_string()) {
        Ok(Ok(())) => ok(),
        Ok(Err(e)) => error(e),
        Err(e) => error(e),
    }
}

static LOAD: AtomicU64 = AtomicU64::new(0);

/// Write files of the load's size under `/data`, each synced and then
/// listed in `done` (synced too), until the load is 0.
fn load_writer() {
    let _ = fs::create_dir_all(LOAD_DIR);
    let mut n: u64 = 0;
    loop {
        let megabytes = LOAD.load(Ordering::Relaxed);
        if megabytes == 0 {
            return;
        }
        n += 1;
        let path = format!("{LOAD_DIR}/file-{n}");
        let chunk = vec![u8::try_from(n % 251).unwrap_or(0); 1 << 20];
        let written = File::create(&path).and_then(|mut f| {
            for _ in 0..megabytes {
                f.write_all(&chunk)?;
            }
            f.sync_all()
        });
        if written.is_ok() {
            let _ = OpenOptions::new()
                .create(true)
                .append(true)
                .open(format!("{LOAD_DIR}/done"))
                .and_then(|mut f| {
                    writeln!(f, "{n} {megabytes}")?;
                    f.sync_all()
                });
        }
        // Old files go, so the disk does not fill.
        if n > 4 {
            let _ = fs::remove_file(format!("{LOAD_DIR}/file-{}", n - 4));
        }
    }
}

fn load(megabytes: &str) -> Vec<String> {
    let Ok(megabytes) = megabytes.trim().parse::<u64>() else {
        return error("load <megabytes>");
    };
    let before = LOAD.swap(megabytes, Ordering::Relaxed);
    if before == 0 && megabytes > 0 {
        std::thread::spawn(load_writer);
    }
    ok()
}

fn sum(path: &str) -> Vec<String> {
    let summed = (|| -> io::Result<(u64, u64)> {
        let mut file = File::open(path)?;
        let mut piece = vec![0u8; 1 << 20];
        let (mut bytes, mut sum) = (0u64, 0u64);
        loop {
            let n = file.read(&mut piece)?;
            if n == 0 {
                return Ok((bytes, sum));
            }
            bytes += n as u64;
            sum += piece[..n].iter().map(|b| u64::from(*b)).sum::<u64>();
        }
    })();
    match summed {
        Ok((bytes, sum)) => vec![format!("{} {bytes} {sum}", word::OK)],
        Err(e) => error(format!("{path}: {e}")),
    }
}

/// The answer to one command line.
pub fn answer(line: &str) -> Vec<String> {
    let (verb, rest) = line.split_once(' ').unwrap_or((line, ""));
    match verb {
        command::PING => vec![format!("pong {rest}")],
        command::STATUS => status(),
        command::KILL => kill(rest),
        command::CAT => cat(rest),
        command::PUT => put(rest),
        command::RM => done(fs::remove_file(rest)),
        command::LS => ls(rest),
        command::STAT => stat(rest),
        command::READLINK => readlink(rest),
        command::KMSG => kmsg(rest),
        command::POWER_OFF => done(control().power_off()),
        command::RESTART => done(control().restart()),
        command::SET_HOST => set_host(rest),
        command::NET_RELOAD => match net().reload() {
            Ok(Ok(())) => ok(),
            Ok(Err(e)) => error(e),
            Err(e) => error(e),
        },
        command::SET_CLOCK => set_clock(rest),
        command::STORAGE => storage_client()
            .status()
            .map_or_else(error, |s| storage_lines(&s)),
        command::KEEP => keep(rest),
        command::LOAD => load(rest),
        command::SUM => sum(rest),
        other => error(format!("`{other}` is no command")),
    }
}
