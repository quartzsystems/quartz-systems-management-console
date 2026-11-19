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
  Boot signing (`pesign`), and QSI itself (`qsi`, `qsisess`,
  `qsitrust`, `qsipart`).
- QS/VS (`https://github.com/quartzsystems/quartz-systems-virtual-server`),
  under `base/`: what every Quartz appliance needs, product-neutral:
  the appliance description, supervision for PID 1, state files,
  logging, the management network and time, the load source with its
  mirror, the installer's flow, the shared panels, service tools users,
  the seed volume, the identity, the product stream's index.

## Phases

| Phase | Delivers | Record |
| --- | --- | --- |
| **0 (current)** | Toolchain and build | `docs/PHASE0.md` |
| 1 | PID 1 and base system: services, state, logging, the management network, time | |
| 2 | Installer, ISO and the local console | |
| 3 | The web server and Console Management pages | |
| 4 | QSI and managed systems, proven on a QS/VS host; the browser terminal | |
| 5 | Partitions, profiles and virtual resources | |
| 6 | Service management and updates | |
| 7 | System plans | |
| 8 | Release engineering and hardening | |

Work that is not done is listed in the current phase's record,
`docs/PHASE0.md`. Code and documents mark it `BLOCKED(phase0): reason`
(one line) or `NEEDS-CODY` (Cody's hardware or sign-off). No other
marker form.

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
  of every shared crate. Both are recorded in `docs/PHASE0.md`. Moving
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
- Anything not done is listed in `docs/PHASE0.md`.

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
  family uses (`docs/REFCODES.md`; `NEEDS-CODY`).
- **Command line.** Baked: `console=ttyS0,115200 console=tty0 panic=-1
  quiet`, then `qsmc.slot=A` or `qsmc.slot=B`; the media UKI carries
  `qsmc.install=1` instead.

## Layout

```
VERSION         1.0.0, the release (V1R0M0)
kernel/         VERSION, SHA256 (QS/VS's), config/qsmc.config
tools/          host tools
  build/          the orchestrator, a thin binary over buildkit: cargo run -p build -- <cmd>
    src/product/    QS/MC as buildkit's Product: names, kernel symbols, the media
    src/image/      the A/B record on the disk image and the image read back
    src/host/       doctor rows, the license header check, the two pins, `test`
  conformance/    the QEMU harness
    src/qemu/       launch, OVMF, serial capture, the boot entry in a variable store
    src/suite/      Phase 0's steps
lic/            PID 1, release, the product's binaries
  qsmcinit/       PID 1
    src/ipl/        the IPL in order: mounts, the command line
    src/console/    /dev/console and /dev/ttyS0
    src/cpu/        what the cryptography needs of the processor
    src/sys/        the panic path, reaping, the loop and the heartbeat
    src/product.rs  QS/MC as the base crates see it, with PID 1's poster and notes
  release/        V1R0M0 over QS/400's vrm, the banner, the prefixes, the Appliance, LOAD_SOURCE
  refcode/        the console's reference codes (docs/REFCODES.md) and the poster
docs/           ARCHITECTURE, PHASE0, REFCODES, OPEN_QUESTIONS
```

Category folders are made as phases need them. The full set: `tools/`
(host tools), `lic/` (PID 1, release, the product's binaries), `web/`
(the server, sessions, templates, the terminal relay, static files),
`mgmt/` (managed systems: the QSI client's use, pairing, events, what
is kept about each system), `svc/` (the local console). Phase 0 has
`tools/` and `lic/`.

## Commands

The pipeline runs on Linux (WSL Ubuntu works); keep the output on ext4
with `QSMC_TARGET_DIR`.

```
cargo run -p build -- doctor          host prerequisites, the license headers, the two pins, no shared crate twice
cargo run -p build -- all             fetch, kernel, userland, initramfs, uki, disk, iso, manifest
cargo run -p build -- test            slot A, slot B, and slot A on a processor lacking what the cryptography needs
cargo run -p build -- test --media iso   the installation ISO
cargo run -p build -- deny            cargo-deny and the C-linkage check
cargo run -p build -- notices         rewrite lic/release/NOTICES.txt (--check in CI)
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo clippy -p qsmcinit --all-targets --target x86_64-unknown-linux-musl -- -D warnings
cargo test --workspace
cargo test -p build -- --ignored host::pins   the pin checks against cargo's checkout of QS/VS
```

Single steps: `fetch`, `kernel`, `userland`, `initramfs`, `uki`, `disk`,
`iso`, `manifest`. `-v` streams tool output; logs are always under
`target/logs/`.

Environment: `QSMC_TARGET_DIR`, `QSMC_JOBS`, `QSMC_SIGNING_KEY`,
`QSMC_BUILD_ID`, `QSMC_RELEASE`, `QSMC_OVMF_CODE`, `QSMC_OVMF_VARS`,
`QSMC_QEMU`, `QSMC_NO_KVM`, `QSMC_BOOT_TIMEOUT` (`docs/PHASE0.md` says
what each does).

## Documents

- `docs/ARCHITECTURE.md`: the locked decisions, a sentence each, and the
  two-pin rule.
- `docs/PHASE0.md`: the toolchain: what exists, how to run it, both
  pinned commits, every base crate and whether it is linked, hashes,
  the acceptance table with evidence, what is not done.
- `docs/REFCODES.md`: the console's reference codes, their range and
  blocks.
- `docs/OPEN_QUESTIONS.md`: what needs a decision.
- QS/400's `docs/BUILDKIT.md` (in its repository): the pipeline and the
  `Product` description.
- QS/VS's `docs/ARCHITECTURE.md` (in its repository): the base crates,
  the A/B record and the slots.
