# QS/MC architecture

QS/MC (Quartz Systems/Management Console) is the appliance from which
an operator manages QS/VS hosts with their QS/400 partitions, and later
standalone QS/400 systems. It is its own small operating system on its
own machine or virtual machine, built the way QS/400 and QS/VS are: the
Linux kernel is the only inherited component and everything else is
Rust.

## Locked decisions

Later phases build on these. Changing one is a decision of its own,
recorded here.

| Area | Decision |
| --- | --- |
| Base | x86_64 only. The same Linux pin as QS/VS (6.18.y LTS), built `LLVM=1`, no modules, EFI stub, initramfs embedded; the bzImage is the UKI; two ESP slots A and B with the A/B record. |
| Userland | 100% Rust, static musl. Rust PID 1 `qsmcinit` over `base/supervise` with a fixed service table. No systemd, no shell, no SSH. |
| Shared code | QS/VS's `base/` crates and QS/400's crates, pinned as the two-pin rule below says. |
| Disk | A load source as `base/loadsource` lays it out: the ESP with both slots, the record partition, and the data file system `QSMCDATA` (xfs), on one disk or mirrored on two. |
| State | Typed TOML files under `/data/state/` through `base/state`; logs through `base/logd`. |
| Management surface | A web UI on port 443 (and 80 redirecting to it), served by QS/MC's own server from the prototype's pages as templates. A local console on the display and serial port for the basics, on `base/panels`. Nothing else. |
| What it manages | QS/VS hosts and their partitions; standalone QS/400 systems when QS/400 has its QSI listener. |
| QSI | QS/400's crates, used as a client: mutual TLS 1.3 with pinned certificates, pairing by the managed system's one-time password, events with replay, streams for consoles, uploads and downloads. |
| A partition's console | Frames of a cell grid from the managed side, drawn in the browser by one small script; keys go back as key events. This is the only script in the product. |
| Updates | The console reads a product stream's signed index, downloads, and brings an update to a host over QSI. Hosts fetch nothing. |
| Outbound traffic | DHCP, DNS, NTP, QSI to managed systems, HTTPS to the configured product stream, and the BMC of a machine it is asked to power on. Nothing else. |
| Identity | Release `V1R0M0` from `VERSION` (`1.0.0`). Fixes `QMnnnnn`. Message prefix `QMC`. Product identifier `qsmc`; environment prefix `QSMC_`; kernel command-line keys `qsmc.`; ESP vendor directory `QSMC`; ESP label `QSMC ESP`; record magic `QSMCREC`; data label `QSMCDATA`; seed label `QSMCSEED`; default host name `QSMC` and six hexadecimal digits. |
| Reference codes | A console posts its own, as `REF <code> <text>`, in the families whose second character is `3` (`C3`, `A3`, `B3`, `D3`), so a console's line never reads as a QS/400 system's or a QS/VS host's. No family of either ends in `3` (`docs/REFCODES.md`). `NEEDS-CODY`. |

A sentence on each:

- **Base.** A console boots the way a QS/VS host does, from the same
  kernel pin (`kernel/VERSION` and `kernel/SHA256` are QS/VS's at the
  pinned commit), as one signed-ready EFI image per slot, so an update
  is a file written to the other slot.
- **Userland.** Every program on the image is Rust built for
  `x86_64-unknown-linux-musl`, static and stripped (`buildkit`'s check),
  and PID 1 supervises a table fixed at build time: nothing is added at
  run time and there is no way in but the two surfaces below.
- **Shared code.** What a console has in common with a host is not
  written here: `base/` is product-neutral and takes an
  `appliance::Appliance` from its caller, and QS/400's crates are the
  pipeline, the disk formats and QSI.
- **Disk.** The load source's partitions, record and mirror are
  `base/loadsource`'s and `base/stgd`'s under QS/MC's own types and
  magics (`release::LOAD_SOURCE`), so neither appliance takes the
  other's disk for its own.
- **State.** Every setting is a typed, versioned TOML file written
  atomically, and every log line is the log service's: the same as a
  host's, so service tooling reads both.
- **Management surface.** The web UI is the console's one remote
  surface and the local console covers what must work without the
  network (its address, its time, its users, shutting down).
- **What it manages.** A console holds no partition and runs no
  workload: everything it shows or does is on a managed system, asked
  over QSI.
- **QSI.** The console is a QSI client and nothing of the protocol is
  restated: QS/400's `qsi`, `qsisess` and `qsitrust` are the messages,
  the session and the trust rules a QS/VS host's server speaks.
- **A partition's console.** The managed side already has the screen
  as cells (`qsipart`); the browser draws them and sends keys, and no
  other script is ever served.
- **Updates.** The console is the one machine that reaches the product
  stream; a host is updated by what the console brings it, verified by
  the host's own keys.
- **Outbound traffic.** What a console may reach is a short, fixed list,
  so a network team can write its rule from this table.
- **Identity.** Every name that carries the product is one constant in
  `lic/release`, and what is on an installed disk is held by a test.
- **Reference codes.** The `3` families keep a console's codes apart
  from both other products' on a screen an operator reads side by side.

## The two-pin rule

- QS/VS's crates are git dependencies at one commit of its `main`.
- QS/400's crates are git dependencies at exactly the commit that QS/VS
  commit pins in its own `Cargo.toml`. Cargo then resolves QS/VS's
  `abrec`, `ipc`, `protect` and the rest to the same source as QS/MC's,
  and the graph holds one copy of every shared crate.
- Both commits are recorded in `docs/PHASE0.md`. Moving either is its
  own commit, and they move together: a new QS/VS commit brings the
  QS/400 commit it pins, and QS/400 is never moved alone.
- Never path dependencies, never copies.
- `cargo run -p build -- doctor` holds it: the QS/400 commit in
  `Cargo.toml` is read against the one in cargo's checkout of the
  pinned QS/VS commit, `Cargo.lock` has each repository at its pin
  alone, and `cargo tree -d` names no QS/VS or QS/400 crate.
- When a base crate cannot be used from outside QS/VS (it names QS/VS,
  or needs a QS/VS crate outside `base/`), the fix is made in QS/VS and
  the pins move; nothing is copied here.

## The base crates

QS/VS's `base/`, at the pinned commit. Each takes an `Appliance` from
its caller and depends on nothing of QS/VS outside `base/`.

| Crate | What it is | QS/MC's use |
| --- | --- | --- |
| `appliance` | the description an appliance hands the base crates; civil time; Ethernet links; the load source's names | `lic/release` (`LOAD_SOURCE`, `appliance()`), PID 1 |
| `supervise` | the service table, restart policy, the runner, the control protocol, signals from a descriptor | PID 1 (signals in Phase 0; the table in Phase 1) |
| `state` | typed TOML state files | Phase 1 |
| `logd` | the system log, the reference code history | Phase 1 |
| `netd` | the management interface and NTP | Phase 1 |
| `seed` | `seed.toml` on a labelled FAT volume | Phase 1 |
| `users` | service tools user IDs | Phase 2 |
| `panels` | the local console's panels | Phase 2 |
| `install` | the installer's screens and answers | Phase 2 |
| `identity` | the appliance's key pair and certificate | Phase 4 (QSI) |
| `loadsource` | the load source: layout, A/B record, mirrored set label, boot entries, the media's payload | the build tool (layout, record, payload volume id) and the harness (GPT); PID 1 from Phase 1 |
| `stgd` | the storage service for a mirrored load source | Phase 2 |
| `fdpass` | open files handed between processes | Phase 3 (the web server) |
| `stream` | the release stream's signed index | Phase 6 |

## The slots

As QS/VS's: the ESP holds slot A at `EFI/BOOT/BOOTX64.EFI` and slot B
at `EFI/QSMC/B.EFI`; a slot is chosen by firmware boot entry; the A/B
record (version 2 of `loadsource::record`, QS/400's `abrec` under the
magic `QSMCREC` and QS/MC's partition type) holds the default slot, the
pending slot, the attempts left and the release last confirmed in each.
Phase 0 writes the first record (slot A, sequence 1) into the built
image; PID 1 reads it from Phase 1.
