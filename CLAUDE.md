# QS/MC (Quartz Systems/Management Console)

The appliance from which an operator manages QS/VS hosts with their
QS/400 partitions, and later standalone QS/400 systems. It is its own
small operating system on its own machine or virtual machine. Its only
management surface is a web UI, with a small local console on the
display and serial port for the basics. It is built the way QS/400 and
QS/VS are: the Linux kernel is the only inherited component and
everything else is Rust. It reaches what it manages over QSI, Quartz's
own protocol, on TCP 2300.

Two things already exist and are reused, never rewritten or copied:

- QS/400 (`https://github.com/quartzsystems/quartz-systems-400`): the
  build pipeline as a library (`buildkit`), release identity (`vrm`),
  disk and media images (`diskimg`, `optical`), the A/B record
  (`abrec`), the xfs formatter, disk protection (`protect`, `ublk`), the
  console and screen kit (`console`, `scrkit`), `ipc`, `netlink`, Secure
  Boot signing (`pesign`), QSI itself (`qsi`, `qsisess`, `qsitrust`,
  `qsipart`), and the web layer its web products share (`qsweb`: HTTP,
  sessions and CSRF, the frame under a product's `Brand`, the partials,
  golden comparison, WebSocket).
- QS/VS (`https://github.com/quartzsystems/quartz-systems-virtual-server`),
  under `base/`: what every Quartz appliance needs, product-neutral:
  the appliance description, supervision for PID 1, state files,
  logging, the management network and time, the load source with its
  mirror, the installer's flow, the shared panels, service tools users,
  the seed volume, the identity, the product stream's index.

## Phases

| Phase | Delivers | Record |
| --- | --- | --- |
| 0 | Toolchain and build | `docs/PHASE0.md` |
| 1 | PID 1 and base system: services, state, logging, the management network, time | `docs/PHASE1.md` |
| 2 | Installer, ISO and the local console | `docs/PHASE2.md` |
| 3 | The web server and Console Management pages | `docs/PHASE3.md` |
| **4 (current)** | QSI and managed systems, proven on a QS/VS host; the browser terminal | `docs/PHASE4.md` |
| 5 | Partitions, profiles and virtual resources | |
| 6 | Service management and updates | |
| 7 | System plans | |
| 8 | Release engineering and hardening | |

Work that is not done is listed in the current phase's record,
`docs/PHASE4.md`. Code and documents mark it `BLOCKED(phase4): reason`
(one line) or `NEEDS-CODY` (Cody's hardware or sign-off); an earlier
phase's `BLOCKED(phase0)` to `BLOCKED(phase3)` stays as it was written.
No other marker form.

## Ground rules

- Every source file starts with exactly this header (the MIT license text
  is `LICENSE` in the repository root):
  ```
  // Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
  // licensed under the terms of the MIT license which can be found in the
  // root directory of this project.
  ```
  `cargo run -p build -- doctor` fails if any `.rs` file lacks it.
- Pure Rust, static musl, x86_64 only. No crate may compile or link C
  (`cc`, `cmake`, `bindgen`, `links =`). `deny.toml` enforces it, with
  QS/VS's rules; `cargo run -p build -- deny` checks it.
- Claude commits at each natural checkpoint as the repository's
  configured git user (Cody) only, with a one-line imperative message
  under 60 characters. No `Co-Authored-By`, no AI attribution of any
  kind. Never push, rewrite history, branch or tag. Carry on until the
  phase is finished.
- Crates live in category folders, never at the repository root. Source
  files are grouped into folders by concern, not left flat under `src/`.
  Comments are short.
- **The two pins.** QS/VS's crates are git dependencies at one commit of
  its `main`. QS/400's crates are git dependencies at one commit:
  exactly the commit that QS/VS commit pins, so the tree holds one copy
  of every shared crate. Both are recorded in `docs/PHASE4.md`, where
  they last moved (`docs/PHASE0.md` has the first). Moving
  either is its own commit, and they move together. Never path
  dependencies, never copies. `build doctor` fails when they disagree
  or `cargo tree -d` shows a QS/VS or QS/400 crate twice.
- Nothing is written here that QS/VS's `base/` or QS/400 already has. If
  a base crate turns out not to be usable from outside QS/VS (it names
  QS/VS, or needs a QS/VS crate outside `base/`), stop and report it:
  the fix belongs in QS/VS, not in a copy here.
- What is on an installed disk and is QS/MC's own (the partition types
  and names, the records' magics, the labels) is `release::LOAD_SOURCE`.
  None of it may change once a console is installed: a test holds each
  value.
- No IBM text, product names or marks anywhere. The names in use are
  QS/MC, QS/VS and QS/400.
- Anything not done is listed in the current phase's record (`docs/PHASE4.md`).

## Locked decisions

See `docs/ARCHITECTURE.md`. Short form:

- **Base.** x86_64 only. The same Linux pin as QS/VS (6.18.y LTS), built
  `LLVM=1`, no modules, EFI stub, initramfs embedded. The bzImage is the
  UKI. Two ESP slots, A and B, with the A/B record.
- **Userland.** 100% Rust, static musl. Rust PID 1 `qsmcinit` over
  `base/supervise` with a fixed service table. No systemd, no shell, no
  SSH.
- **Shared code.** QS/VS's `base/` crates and QS/400's crates, pinned as
  above.
- **Disk.** A load source as `base/loadsource` lays it out: the ESP with
  both slots, the record partition, and `QSMCDATA` (xfs), on one disk
  or mirrored on two.
- **State.** Typed TOML files under `/data/state/` through `base/state`;
  logs through `base/logd`.
- **Management surface.** A web UI on port 443 (80 redirects to it),
  served by QS/MC's own server from the prototype's pages as templates.
  A local console on the display and serial port for the basics, on
  `base/panels`. Nothing else.
- **What it manages.** QS/VS hosts and their partitions; standalone
  QS/400 systems once QS/400 has its QSI listener.
- **QSI.** QS/400's crates, used as a client: mutual TLS 1.3 with pinned
  certificates, pairing by the managed system's one-time password,
  events with replay, streams for consoles, uploads and downloads.
- **Managed systems.** `mgmtd` (`qsmcmgmtd`, at 40), its own user for
  good, alone holds the QSI key and every session; the web server asks
  it on local sockets and holds neither (`docs/MGMT.md`).
- **A partition's console.** Frames of a cell grid from the managed
  side, drawn in the browser by one small script; keys go back as key
  events. The only script in the product.
- **Updates.** The console reads a product stream's signed index,
  downloads, and brings an update to a host over QSI. Hosts fetch
  nothing.
- **Outbound traffic.** DHCP, DNS, NTP, QSI to managed systems, HTTPS to
  the configured product stream, and the BMC of a machine it is asked to
  power on. Nothing else.
- **Identity.** Release `V1R0M0` from `VERSION` (`1.0.0`). Fixes
  `QMnnnnn`. Message prefix `QMC`. Product identifier `qsmc`;
  environment prefix `QSMC_`; kernel command-line keys `qsmc.`; ESP
  vendor directory `QSMC`; ESP label `QSMC ESP`; record magic
  `QSMCREC`; data label `QSMCDATA`; seed label `QSMCSEED`; default host
  name `QSMC` and six hexadecimal digits.
- **Reference codes.** `REF <code> <text>`, in the families whose second
  character is `3` (`C3`, `A3`, `B3`, `D3`), which no QS/400 or QS/VS
  family uses (`docs/REFCODES.md`); confirmed in Phase 1.
- **Network roles.** `open`, first: operators reach the console on it,
  with the default route, the name servers and the time; `private`,
  optional, toward the systems it manages, with no default route. The
  installer and the local console configure `open`; `private` is set
  from a seed and, from Phase 3, the web interface.
- **Installation.** The ISO's media UKI runs `qsmcinstall` over
  `base/install`; the payload is `base/loadsource::payload`, signed by
  the build's key and checked before any write. The smallest load-source
  disk is 16 GiB; protection is none on one disk or mirrored on two. The
  user made there is the first service tools user, and Phase 3's first
  Administrator from the same hash.
- **Local console.** `qsmccons` over `base/panels`, a session on the
  display and one on the serial port, idle sign-off after ten minutes;
  it changes nothing itself and manages nothing but the console.
- **Command line.** Baked: `console=ttyS0,115200 console=tty0 panic=-1
  quiet`, then `qsmc.slot=A` or `qsmc.slot=B`; the media UKI carries
  `qsmc.install=1` instead.

## Layout

```
VERSION         1.0.0, the release (V1R0M0)
kernel/         VERSION, SHA256 (QS/VS's), config/qsmc.config
tools/          host tools
  sim/            the simulated managed systems (docs/SIM.md): sim, binary qsmcsim; screens/ the recorded console screen
  build/          the orchestrator, a thin binary over buildkit: cargo run -p build -- <cmd>
    src/product/    QS/MC as buildkit's Product, release or test image: names, kernel, initramfs, media
    src/image/      the userland and its hook check, the disk's record, the manifest, the load-source writer
    src/host/       doctor rows, the license header check, the two pins, `test`, `fuzz`
  conformance/    the QEMU harness
    src/qemu/       base/qemu (as `driver`) with QS/MC's variables; a suite's machine, its forwards and its session's cursor; the test hooks' channel; golden screens
    src/image/      a load source on image files (base/loadsource's writer) and what the suites read of a disk
    src/web/        a browser of the web interface, pinned to the certificate it expects
    src/suite/      Phase 0's steps; base/: Phase 1's suites; install/: Phase 2's, the installer and the local console; web/: Phase 3's
    golden/         every installer and local console screen, as text
  fuzz/           qsmcfuzz over QS/400's fuzzer: a backup's reader, the certificate import, what mgmtd does with a peer's messages
lic/            PID 1, release, the product's binaries
  qsmcinit/       PID 1
    src/ipl/        the IPL in order: mounts, the record, storage and /data (the panel for the unit to keep), a restore, clock, seed, state and inventory, identity; the media's hand-off to the installer
    src/host/       the running console: the loop, the table, the control socket, the web server's listeners, option 4's requests while it is down, shutdown, the load source's line, the serial port lent to a session
    src/slot/       the slot rule: a slot on trial, and a restore, confirm when the web server answers
    src/console/    /dev/console and /dev/ttyS0, the status screen
    src/sys/        the panic path, reaping, the power button
    src/hooks.rs    a test image's hooks (cfg(test_hooks) only)
  qsmcinstall/    the installer over base/install: QS/MC's Target, the media's check, the answers of a seed
  qsmcstgd/       base/stgd run as QS/MC
  qsmclogd/       base/logd run as QS/MC
  qsmcnetd/       base/netd run as QS/MC
  qsmchook/       the test hooks: the channel, flaky, the churn, a failing web server (test image only)
  qsmcwebd/       the web server's binary: web/webd with web/pages, and its socket for the local console
  qsmcmgmtd/      the managed-systems service's binary: its sockets bound, its own user for good, mgmt/mgmtd
  release/        V1R0M0 over QS/400's vrm, the banner, the prefixes, the Appliance and its interface roles, LOAD_SOURCE, status facts and screen, the processor's words
  refcode/        the console's reference codes (docs/REFCODES.md), each base event's, the poster
web/            the web interface (docs/WEB.md)
  webd/           qsweb under rustls: the listeners PID 1 hands over, TLS and the certificate, the limits and headers, the self-test
  users/          web users, roles and the matrix, sign-on rules, web.toml, the web server's socket protocol (docs/WEBUSERS.md)
  msgs/           the QMC message catalogue (docs/MESSAGES.md)
  pages/          every page from its view, a module and its routes each; the App: sessions, the task of each route, audit;
                  systems/ the managed systems' pages, refcodes/ explanations and the history, terminal/ the window, its socket, the control panel
  backup/         the backup file: the payload PID 1 restores, sealed under a passphrase (docs/BACKUP.md)
mgmt/           managed systems (docs/MGMT.md)
  proto/          mgmtproto: what is kept of a system, the requests on mgmtd's socket, a terminal window's connection
  mgmtd/          the service: keep/ systems.toml, mgmt.toml, the history; peer/ a QSI connection, pairing; system/ a thread a system, its consoles shared; service/ the requests, the console socket
svc/            the local console
  console/        qsmccons over base/panels: the sessions, the main menu, status, network, disk units, the web interface (option 4), the one panel for the unit to keep
docs/           ARCHITECTURE, PHASE0-4, INSTALL, LOCALCONSOLE, REFCODES, SEED, WEB, WEBUSERS, BACKUP, MESSAGES, MGMT, TERMINAL, SIM, OPEN_QUESTIONS
design/         qsmc-prototype/: the web UI's approved design, every page a golden rendering (its README is the handoff)
```

Category folders are made as phases need them. The full set: `tools/`
(host tools), `lic/` (PID 1, release, the product's binaries), `web/`
(the server, sessions, templates, the terminal relay, static files),
`mgmt/` (managed systems: the QSI client's use, pairing, events, what
is kept about each system), `svc/` (the local console). Phases 0 and 1
made `tools/` and `lic/`, Phase 2 `svc/`, Phase 3 `web/`, Phase 4
`mgmt/`.

## Commands

The pipeline runs on Linux (WSL Ubuntu works); keep the output on ext4
with `QSMC_TARGET_DIR`.

```
cargo run -p build -- doctor          host prerequisites, the license headers, the two pins, no shared crate twice
cargo run -p build -- all             fetch, kernel, userland, initramfs, uki, disk, iso, manifest
cargo run -p build -- --test-image all   the same with the test hooks, under test-image/ in the target
cargo run -p build -- test            Phase 0's steps on the release image, then Phases 1 to 4's suites on the test image
cargo run -p build -- test --suite mgmt-host   suite 10 on a real QS/VS host: QSMC_QSVS_DISK names its installed test disk
cargo run -p sim --bin qsmcsim        the prototype's managed systems on the network, told what to do on standard input
cargo run -p build -- fuzz            the fuzzer with coverage counters, every target 60 s (--target NAME, --secs N)
cargo run -p build -- test --suite <name>   one step or suite; `list` lists them
cargo run -p build -- test --media iso   the installation ISO
cargo run -p build -- loadsource a.img [b.img]   a load source (two files: mirrored) from the built UKIs
cargo run -p build -- deny            cargo-deny and the C-linkage check
cargo run -p build -- notices         rewrite lic/release/NOTICES.txt (--check in CI)
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo clippy -p qsmcinit -p qsmcstgd -p qsmclogd -p qsmcnetd -p qsmccons -p qsmcinstall -p qsmcwebd -p qsmcmgmtd --all-targets --target x86_64-unknown-linux-musl -- -D warnings
QSMC_TEST_HOOKS=1 cargo clippy -p qsmcinit -p qsmchook -p qsmccons -p webpages --all-targets --target x86_64-unknown-linux-musl -- -D warnings
cargo test --workspace
cargo test -p build -- --ignored host::pins   the pin checks against cargo's checkout of QS/VS
```

Single steps: `fetch`, `kernel`, `userland`, `initramfs`, `uki`, `disk`,
`iso`, `manifest`. `-v` streams tool output; logs are always under
`target/logs/`.

Environment: `QSMC_TARGET_DIR`, `QSMC_JOBS`, `QSMC_SIGNING_KEY`,
`QSMC_BUILD_ID`, `QSMC_RELEASE`, `QSMC_OVMF_CODE`, `QSMC_OVMF_VARS`,
`QSMC_QEMU`, `QSMC_NO_KVM`, `QSMC_BOOT_TIMEOUT` (`docs/PHASE0.md` says
what each does), `QSMC_TEST_HOOKS`, which the userland step sets itself
(`docs/PHASE1.md`), and `QSMC_GOLDEN=update`, which writes the golden
screens instead of comparing (`docs/PHASE2.md`).

A test image's hooks are switched on for one boot by an SMBIOS OEM
string `qsmc.hook=<name>` (`churn`, `flaky-fails`, `refuse-confirm`,
`short-idle`, `webd-fails`); a test image also serves the pattern
reference under `/qsmc/dev/`;
the harness reaches the control socket through the channel on the
guest's second serial port.

## Documents

- `docs/ARCHITECTURE.md`: the locked decisions, a sentence each, and the
  two-pin rule.
- `docs/PHASE0.md`: the toolchain: what exists, how to run it, both
  pinned commits, every base crate and whether it is linked, hashes,
  the acceptance table with evidence, what is not done.
- `docs/PHASE1.md`: the base system: what exists, how to run it, every
  base crate and how it is used, kernel additions, where a crate
  differed from the handoff, hashes, the acceptance table with
  evidence, what is not done.
- `docs/PHASE2.md`: the installer and the local console: the new pins
  and what they changed, what was deleted in favour of `base/`, kernel
  additions, hashes, the acceptance table with evidence, what is not
  done.
- `docs/PHASE3.md`: the web server and Console Management pages: the
  new pins and what they changed, the acceptance table with evidence,
  what is not done.
- `docs/PHASE4.md`: QSI and managed systems: the new pins, what exists,
  the pages live, the acceptance table with evidence, the joint gate
  with QS/VS, what is not done.
- `docs/MGMT.md`: the managed-systems service: what it keeps, sessions,
  pairing as the console does it, its sockets, what each failure shows.
- `docs/TERMINAL.md`: the terminal window: the exchange, viewers, roles,
  the picture, its script and policy.
- `docs/SIM.md`: the simulated managed systems and what they are told.
- `docs/WEB.md`: the web server: its listeners, TLS, limits and
  headers, static files, how a page is added against its golden file
  and every golden correction, time, Certificates.
- `docs/WEBUSERS.md`: web users, roles, the matrix, sign-on rules, audit.
- `docs/BACKUP.md`: what a backup holds, its format and encryption, and
  how a restore is applied and taken back.
- `docs/MESSAGES.md`: every `QMC` message (a test holds it to the
  catalogue).
- `design/qsmc-prototype/`: the web UI's design. `README.md` is the
  handoff (build order, routes, fixture data), `STYLE.md` the style
  rules; each page is the golden its served page must match.
- `docs/INSTALL.md`: an operator's guide to installing, attended and
  unattended, and what a console needs of its machine.
- `docs/LOCALCONSOLE.md`: every panel of the local console.
- `docs/REFCODES.md`: the console's reference codes, their range and
  blocks, and what to do for each `A3` and `B3` code.
- `docs/SEED.md`: what a QS/MC seed holds over the base format.
- `docs/OPEN_QUESTIONS.md`: what needs a decision.
- QS/400's `docs/BUILDKIT.md` (in its repository): the pipeline and the
  `Product` description.
- QS/VS's `docs/ARCHITECTURE.md` (in its repository): the base crates,
  the A/B record and the slots.
