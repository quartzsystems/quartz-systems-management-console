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
| Outbound traffic | DHCP, DNS, NTP, QSI to managed systems, HTTPS to the configured product stream, and the BMC of a machine it is asked to power on. Nothing else. The product stream is reached by one unprivileged process alone, `fetchd`, holding no key of the console's. |
| Identity | Release `V1R0M0` from `VERSION` (`1.0.0`). Fixes `QMnnnnn`. Message prefix `QMC`. Product identifier `qsmc`; environment prefix `QSMC_`; kernel command-line keys `qsmc.`; ESP vendor directory `QSMC`; ESP label `QSMC ESP`; record magic `QSMCREC`; data label `QSMCDATA`; seed label `QSMCSEED`; default host name `QSMC` and six hexadecimal digits. |
| Reference codes | A console posts its own, as `REF <code> <text>`, in the families whose second character is `3` (`C3`, `A3`, `B3`, `D3`), so a console's line never reads as a QS/400 system's or a QS/VS host's. No family of either ends in `3` (`docs/REFCODES.md`). Confirmed in Phase 1. |
| Network roles | Two, as `network.toml` names them: `open`, first, the network operators reach the console on, with the default route, the name servers and the time; `private`, optional, toward the systems it manages, with no default route. Decided in Phase 2. |
| Installation | The ISO's media UKI runs the installer (`qsmcinstall`, over `base/install`) in place of the services. It installs from the payload beside it, both slots and a manifest signed by the build's key, checked before any write. The smallest load-source disk is 16 GiB; none or mirrored protection. The user it makes is the first service tools user. Decided in Phase 2. |
| Local console | `qsmccons` over `base/panels`, on the display and the serial port, a session each, idle sign-off after ten minutes. It changes nothing itself: `netd`, PID 1 and the storage service do. It manages nothing but the console. Decided in Phase 2. |
| Web interface | `webd` (`qsmcwebd`): QS/400's `qsweb` under rustls with graviola, TLS 1.2 and 1.3, handed 443 and 80 on `open` by PID 1; no script but the terminal window's. Its own users in four roles with a role matrix, the first Administrator from the installation's user. An ECDSA P-256 self-signed certificate made at first start, not the QSI identity. Backups always under a passphrase, through the browser only, restored by PID 1 at an IPL. English only. Every message a `QMC` one. Decided in Phase 3. |
| Managed systems | `mgmtd` (`qsmcmgmtd`), at 40 in PID 1's table: its own user (23990) for good, the only reader of the console's QSI key, a QSI session with every managed system over QS/400's `qsi`, `qsisess` and `qsitrust`, kept open and resumed with its events. The web server asks it on two local sockets and holds no key and no session. A list never waits on a managed system. Pairing by the system's one-time password, never kept. A partition's console shared among windows by `mgmtd`, the host's one viewer; drawn by QS/400's `termlook` and its script, the only script served, on the terminal window only. Decided in Phase 4. |

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
- **Managed systems.** The key managed systems trust is held by one
  unprivileged service apart from the listener that faces browsers, so
  a fault in a page reaches neither the key nor a session.
- **QSI.** The console is a QSI client and nothing of the protocol is
  restated: QS/400's `qsi`, `qsisess` and `qsitrust` are the messages,
  the session and the trust rules a QS/VS host's server speaks.
- **A partition's console.** The managed side already has the screen
  as cells (`qsipart`); the browser draws them and sends keys, and no
  other script is ever served.
- **Updates.** The console is the one machine that reaches the product
  stream; a host is updated by what the console brings it, verified by
  the host's own keys. The console updates itself as a host does, with
  `base/update`, into its other slot, confirming when its web server
  answers and otherwise back on the old slot with its state put back
  (`docs/UPDATES.md`).
- **Outbound traffic.** What a console may reach is a short, fixed list,
  so a network team can write its rule from this table. Beyond DHCP,
  DNS, NTP and QSI, the one exception is `fetchd`'s traffic to the
  configured server: one small service, the only process that connects
  to it, unprivileged and holding no key, so what talks to the internet
  can do nothing else.
- **Identity.** Every name that carries the product is one constant in
  `lic/release`, and what is on an installed disk is held by a test.
- **Reference codes.** The `3` families keep a console's codes apart
  from both other products' on a screen an operator reads side by side.
- **Network roles.** The network the console is reached on and the one
  it reaches hosts on are kept apart, and only the first carries what
  leaves the machine by default.
- **Installation.** A console is installed the way a host is, by the
  base installer's screens or a seed's answers, and installs only what
  its own build signed.
- **Local console.** What must work without the network (its address,
  its disks, its users, its clock, shutting down) is at the machine.
- **What pages read.** A list is what `mgmtd` keeps, fed by events; it
  never waits on a managed system. An action is a live request, and a
  refusal shows the managed system's own message. A page sends any of
  QSI's requests through one call (`ask`) that `mgmtd` checks against
  what the pages send and what the system's hello offers.
- **A partition's fields.** Each is its definition's, its default
  profile's or its settings'; a profile is put back with the revision it
  was read at, and a stale put shows the page again with the entries
  kept (`docs/PARTITIONS.md`).
- **Uploads.** An image streams from the browser through the web server
  and `mgmtd` to the host, paced by the host's stream credit; nothing of
  it is held whole on the console.
- **Scheduled operations.** A service of its own runs them in their
  owners' names, their roles checked when they run, and never runs a
  missed one late (`docs/SCHED.md`).
- **System plans.** A plan is one versioned text document of a host's
  configuration, built only from what QSI's host family reads and
  writes, owned and written by the scheduler alone in `/data/plans/`;
  a deployment validates every item against the host as it is, makes
  only what is missing, never deletes, changes or takes what it did not
  make, and keeps its job on disk so a restart loses none of it
  (`docs/PLANS.md`).

## The two-pin rule

- QS/VS's crates are git dependencies at one commit of its `main`.
- QS/400's crates are git dependencies at exactly the commit that QS/VS
  commit pins in its own `Cargo.toml`. Cargo then resolves QS/VS's
  `abrec`, `ipc`, `protect` and the rest to the same source as QS/MC's,
  and the graph holds one copy of every shared crate.
- Both commits are recorded in the record of the phase that last moved
  them (`docs/PHASE4.md`: QS/VS `fd245e6`, QS/400 `cb58b01`; the first
  were `docs/PHASE0.md`'s). Moving either is its own commit, and they
  move together: a new QS/VS commit brings the QS/400 commit it pins,
  and QS/400 is never moved alone.
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
| `supervise` | the service table, restart policy, the runner, the control protocol, signals from a descriptor | PID 1: the table, the control socket, signals; the services: readiness; the harness: the status types |
| `state` | typed TOML state files | PID 1 (`host.toml`, `network.toml`, `identity.toml`), `netd`, the test hooks |
| `logd` | the system log, the reference code history | `qsmclogd`; PID 1 and `refcode` send its records |
| `netd` | the network interfaces, each with its role, and NTP | `qsmcnetd`, with QS/MC's roles `open` and `private`; the console's network panel; the installer's clock |
| `seed` | `seed.toml` on a labelled FAT volume | PID 1 (`QSMCSEED`); the installer's answers |
| `users` | service tools user IDs | the installer (the first user), the local console (sign-on, the users panel), the harness (an answers file's hash) |
| `panels` | panels with help, the console on display and serial, the common forms and panels, the unit to keep | the installer (through `install`), the local console |
| `install` | the installer's screens and answers | `qsmcinstall` |
| `processor` | what QSI's cryptography needs of the processor | PID 1's processor line, the console's status |
| `inventory` | the hardware inventory with location codes | PID 1 (`inventory.toml`, the Machine row), `qsmcstgd`, the installer, the console, the test hooks' OEM strings |
| `qemu` | a harness's QEMU driver | the harness, as `driver` |
| `identity` | the appliance's key pair and certificate | PID 1, at the first IPL with a `/data`; QSI presents it from Phase 4 |
| `loadsource` | the load source: layout, A/B record, mirrored set label, boot entries, the media's payload | PID 1 (the disks, the record, the boot entries), the build tool and the harness (the writer) |
| `stgd` | the storage service: the load source, mirrored through ublk | `qsmcstgd`, first in the table |
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

**The slot rule.** At each IPL PID 1 reads the record on every
load-source disk (the copy with the highest sequence wins) and writes it
back to every disk when this IPL changes it. A slot booted while it is
pending is on trial: its one attempt is spent. When the console is up
by the rule, the slot on trial is confirmed: it becomes the default in
the record, its release is noted there, and `BootOrder` is rewritten
with its entries first (`loadsource::boot::make_default`, an entry made
for a slot that has none). A slot on trial that never gets there was
only `BootNext`: the next IPL is on the old default, which clears the
pending slot and posts `A3001035`; PID 1 also restarts into the other
slot if the trial has not confirmed in 15 minutes.

The rule is `slot::rule` in `lic/qsmcinit/src/slot/mod.rs`, of the
service table in use. In Phase 1 it was `Rule::BaseReady`: confirmed
when every base service has said it is ready. From Phase 3 it is
`Rule::ServiceReady` of the web server, which is ready only once it has
answered a request over TLS; on a processor without what the
cryptography needs there is no web server and the rule is
`Rule::BaseReady` again. A restore on trial (`docs/BACKUP.md`) is kept
by the same rule. Nothing writes the other slot or sets a pending slot
yet; that is the update phase.

## The IPL

PID 1 (`lic/qsmcinit/src/ipl/mod.rs`), each step posting its code:

1. The kernel's file systems (proc, sysfs, devtmpfs, `/run`, efivarfs)
   and cgroup2: `C3001010`. The banner, the slot from the command line,
   the processor's line (`A3001015` when it lacks something).
2. The load-source disks found (`loadsource::disks`) and the A/B record
   read: `C3001030`.
3. The control socket, then the service table started with everything
   after the storage service held back; the loop turned until the
   storage service is ready. A mirrored pair whose units each ran alone
   is not assembled until the unit to keep is chosen: the IPL waits for
   it (`A3003040`), for as long as it takes.
4. `QSMCDATA` at `/data`, from the device the storage service names:
   `C3001040`. When it cannot be mounted, `A3001040`, the state is in
   memory only and the IPL goes on. Nothing is ever reformatted.
   A restore the web server left is put in place now, before anything
   reads state: `C3001045`; one not confirmed at the IPL before is
   taken back: `A3001045` (`docs/BACKUP.md`).
5. The clock from the RTC: `C3001050`.
6. The seed: `C3001060` or `C3001061` (`docs/SEED.md`).
7. The state: `host.toml`, `network.toml`, `identity.toml`, with the
   defaults for what is absent; the inventory read and written to
   `inventory.toml`; the host name set: `C3001070`.
8. The identity, made once, at the first IPL with a `/data`:
   `C3001075` (`base/identity`).
9. The services that waited started: `C3001080`.
10. "Base ready" when every service has said so: `C3001090`; the slot
    confirmed by the rule: `C3001095`.

Then one loop: the signals, the control socket, the power button, the
services' pipes, the heartbeat on the serial port and the status screen.
The serial port is lent to the local console while it has a session
there. On the installation media PID 1 runs the installer after step 1
and the clock, in place of everything else, and restarts the machine
when it ends.

The service table is fixed: the storage service (5), the log service
(10), the network service (20), the local console (25), the web server
(30, from Phase 3, absent on a processor without what the cryptography
needs), the managed-systems service (40, from Phase 4, absent with the
web server); the scheduler (50) takes its place in a later phase. PID 1 binds the web server's 443 and 80
on the `open` address and hands them over (`docs/WEB.md`). The orderly shutdown (the control socket, `SIGTERM`,
the power button) ends them in reverse, unmounts `/data`, ends the
storage service last and posts `D3001090` or `D3001091`.

## The load source

As `base/loadsource` lays it out, on one disk or mirrored on two: each
disk has its own ESP with both slots and its own record partition, so
the firmware and PID 1 can start from either alone. Mirrored, the data
partitions are the members of one set (QS/400's `protect`), which the
storage service serves as `/dev/ublkb0` and `/data` is mounted from; its
label, beside the record on every disk, says which partitions are the
members and how each stands. A unit that is missing or failed is posted
at every IPL (`A3003020`); a returned unit is brought back in step from
the write-intent maps, so only what changed is copied; a restarted
storage service takes the device over and loses nothing. The release
`disk` image is one disk; the installer writes either kind for an
operator (`docs/INSTALL.md`), and the harness with the base crate's
writer. A unit missing, the local console's option 2 replaces it, and
a load source of one disk is mirrored from there too.
