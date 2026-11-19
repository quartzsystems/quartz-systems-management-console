# Phase 1: PID 1 and the base system

What Phase 1 built, how to run it, and the evidence for the acceptance
table. Phase 0, the toolchain, is `docs/PHASE0.md`: the host
prerequisites, the environment variables and both pinned commits there
still hold.

Phase 1 turns the banner-printing PID 1 into a base system: the load
source and its storage service, the data file system, state, logging,
the management network and time, the seed, the console's identity, a
supervised service table, slot confirmation and an orderly shutdown.
There is no installer, no local console menu, nothing of the web UI or
QSI, and nothing listens on the network.

## The pins

Unchanged: QS/VS at `9f11fd5660556dabf789a062768418adf0548583`, QS/400
at `423c65dea0b8c9fab3d2c3efaba6e9f27e679c99`, the one QS/VS pins. No
base crate needed a fix, so neither moved. `Cargo.lock` was started
again from QS/VS's lock at its pin, as in Phase 0, so the crates the two
share are at the versions QS/VS tests, and cargo cut it to this graph.

New git dependencies, all at those two commits: QS/VS's `state`, `logd`,
`netd`, `seed`, `identity` and `stgd`; QS/400's `abrec`, `ipc` and `ntp`
(the harness's SNTP responder). `ipc` is declared for the record; the
base crates use it themselves.

## The base crates

Every crate of QS/VS's `base/`, and how Phase 1 uses it. Nothing of
them is copied or written again here.

| Crate | Linked | How QS/MC uses it |
| --- | --- | --- |
| `appliance` | yes | `release::appliance`: QS/MC's names, directories, load-source names; PID 1 hands it to every base crate with no-op `post` and `note` (the crates report to PID 1 by what they return), the services with `refcode::post::remote`'s, which ask PID 1. Civil time, Ethernet links |
| `supervise` | yes | PID 1: the fixed table (`lic/qsmcinit/src/host/table.rs`), the runner driven from PID 1's loop, `hold_after` and `release` around `/data`, `begin_stop_above` for the shutdown, signals from a descriptor, the control socket's protocol (`Control` implemented in `host/control.rs`). The services: the readiness byte. The harness: the status types |
| `state` | yes | PID 1 reads `host.toml`, `network.toml` and `identity.toml` with their defaults, writes `identity.toml` once and `host.toml` on `set_host`. The churn and the harness parse them |
| `logd` | yes | `qsmclogd` is `logd::serve::run` as QS/MC. PID 1 sends its records through `logd::client::Sender`, which queues what is said before `/data` is mounted; `refcode::post` makes the history's records |
| `netd` | yes | `qsmcnetd` is `netd::serve::run` as QS/MC, with no interface reserved. The test hooks use its socket's `reload` and `set_clock` |
| `seed` | yes | PID 1's seed step, `seed::run` with the label `QSMCSEED` (`docs/SEED.md`) |
| `identity` | yes | PID 1 makes the console's key and certificate at the first IPL with a `/data` (`identity::ensure`) |
| `loadsource` | yes | PID 1: the load-source disks, the A/B record on every disk, the boot entries at confirmation. The build tool's disk step as in Phase 0. The harness and `build loadsource`: the writer (`prepare::write_table`, `esp_image`, `format_data`), the record and the label (`multi::Copies`, `record`, `set`) |
| `stgd` | yes | `qsmcstgd` is `stgd::serve::run` as QS/MC, first in the table; PID 1 reads the device it names, its choose marker and, on a thread of its own, its status |
| `users`, `panels`, `install` | no | Phase 2. `panels` has no plain status screen; QS/VS's Phase 1 screen is its `release::screen`, so QS/MC has its own placeholder (`release::screen`) |
| `fdpass` | no | Phase 3 |
| `stream` | no | Phase 6 |

## What exists

### PID 1 (`lic/qsmcinit`)

The IPL in order, each step posting its `C3` code
(`docs/ARCHITECTURE.md`, "The IPL"): the kernel's file systems and
cgroup2, the banner, the slot, the processor; the load-source disks and
the A/B record; the control socket and the table started with all after
the storage service held back, the loop turned until it is ready; `/data`
from the device it names; the clock from the RTC; the seed; the state
and the host name; the identity once; the services; "Base ready"; the
slot confirmed.

- `/data` that cannot be mounted: `A3001040`, state in memory (the
  initramfs's `/data`), and the IPL goes on. Nothing is reformatted.
- Units that each ran alone: the storage service makes its choose marker
  and posts `A3003040`; PID 1 says the IPL waits, keeps the storage
  service from being timed out (`patient`) and starts nothing else, for
  as long as it takes. In Phase 1 the test image's channel answers
  (`keep <disk>`); the screen that asks an operator is Phase 2's.
- The table: `stgd` (5), `logd` (10), `netd` (20); 30, 40 and 50 are
  left for the web server, the managed-systems service and the
  scheduler. Restarts are `supervise`'s (1, 2, 4 ... 30 s; five failures
  in ten minutes end them), then `A3001080` naming the service. The
  storage service given up while `/data` is on the mirrored set is
  `B3003000`; PID 1's own panic is `B3001000`.
- The control socket `/run/qsmcinit.sock`: status, power off, restart,
  `post`, `post_about`, `note`, `set_host`, `state`, `power_down` (at
  once only), `slots`. What a console does not do yet (a service dump,
  a code update's slots) is refused with the reason.
- The orderly shutdown from the socket, `SIGTERM` and the ACPI power
  button: `D3001000`, PID 1's log queue handed over, the services above
  the storage service ended in reverse (each given 5 s), sync, `/data`
  unmounted, the storage service ended (it marks its set clean), then
  `D3001090` or `D3001091` on the console. Ctrl-Alt-Del is ignored with
  a line.
- The status screen, plain text on the display and between two marker
  lines on the serial port, drawn again when it changes: the banner
  (product and release), the slot with the record, the host name, the
  management line, the date and time, the load source (protection and
  each unit's state, from the storage service), the current reference
  code, the processor, a line a service. The heartbeat `C3001005` on
  the serial port every 5 s once the services are started.
- The slot rule is `RULE` in `lic/qsmcinit/src/slot/mod.rs`:
  `Rule::BaseReady` now, `Rule::ServiceReady(<web server>)` from Phase 3,
  a one-line change. A slot on trial that has not confirmed in 15
  minutes restarts the console into the other slot.
- On the media UKI PID 1 stops after the clock: the installer is
  Phase 2's.

### The services

`qsmcstgd`, `qsmclogd`, `qsmcnetd`: each the base crate's `serve` with
QS/MC's `Appliance`, a dozen lines, static and stripped, under
`/qsmc/bin/` in a slot's initramfs. `qsmcstgd` tells the storage service
each disk's name and serial number from sysfs (no location codes:
`docs/OPEN_QUESTIONS.md` item 12).

### State, seed, reference codes

- `host.toml`, `network.toml`, `identity.toml` as `base/state` defines
  them. Absent, the defaults: host name `QSMC` and the last six
  hexadecimal digits of the first Ethernet link's MAC, UTC, no time
  servers, DHCP on the first link with carrier. `identity.toml` holds a
  serial number derived once from that MAC (`M` and twelve digits), as a
  QS/VS host derives one when DMI has none.
- The seed: `docs/SEED.md`.
- `lic/refcode`: 47 codes in the blocks `C3001`/`A3001`/`B3001`/`D3001`
  (PID 1), `C3002`/`A3002` (network and time), `C3003`/`A3003`/`B3003`
  (the load source), each `A3` and `B3` code with what an operator
  should do, held to `docs/REFCODES.md` by its test; the code of every
  base event; the poster (the console line and the history record) and
  `post::remote`, how a service posts through PID 1.

### Build

`cargo run -p build -- --test-image <step>` builds the test image: the
same sources with the hooks, under `<target>/test-image/`, a target of
its own (its kernel is built again there; the tarball is copied).

- The userland step builds PID 1, the three services and, in a test
  image, `qsmchook`, and sets `QSMC_TEST_HOOKS` itself (`1` or `0`,
  whatever the caller's environment). It then checks PID 1 for the
  hooks' marker and every release binary for the hooks' binary's name.
  The manifest step checks again and reads the manifest back: a release
  manifest has `"product": "QS/MC"` and no `qsmchook`, a test manifest
  `"QS/MC (test image)"` and lists it.
- The initramfs: `/init`, `data`, `dev` with a console node, `etc`,
  `proc`, `run`, `sys`, `qsmc/bin` with the services (a slot's) or
  nothing (the media's).
- `build loadsource a.img [b.img]` writes a load source, one disk or a
  mirrored pair, onto image files with `base/loadsource`'s writer, from
  the built slot UKIs (`tools/conformance/src/image/disks.rs`; both units
  hold the same data file system and the label says both are Active).
  The release `disk` image stays `buildkit`'s single disk.

### The test hooks (`lic/qsmchook`, PID 1 under `cfg(test_hooks)`)

As QS/VS has them. In a test image only.

| Hook | Where | Switched on |
| --- | --- | --- |
| the channel: the harness's way to the control socket, the storage service and the console's files, on the second serial port; it also kills a service or PID 1 (`SIGTERM`), writes and removes files, sets the host name and the clock, answers the two-units question, runs a write load | `qsmchook channel`, first in a test image's table | always, in a test image |
| a service that fails on demand | `qsmchook flaky` | `qsmc.hook=flaky-fails` |
| the state churn: `host.toml` and `network.toml` written over and over once `/data` is mounted | a thread of the channel | `qsmc.hook=churn` |
| a slot that refuses to confirm | PID 1 | `qsmc.hook=refuse-confirm` |

A switch is an SMBIOS OEM string the harness gives QEMU for one boot,
read from the raw table (`/sys/firmware/dmi/tables/DMI`). The channel is
ordered before the storage service, unlike QS/VS's: it must answer while
the IPL waits for the unit to keep.

## Kernel additions

None. Every symbol Phase 1 uses was set and checked in Phase 0
(`docs/PHASE0.md`, "Kernel configuration"): the RTC (`RTC_CLASS`,
`RTC_DRV_CMOS`, `RTC_INTF_SYSFS`, `RTC_INTF_DEV`, `RTC_HCTOSYS` off),
`ACPI_BUTTON`, `INPUT_EVDEV`, `DMI`, `DMIID` (the raw SMBIOS table the
hooks read is `DMI`'s), `SIGNALFD`, `EFIVAR_FS`, `XFS_FS`, `VFAT_FS`
with its code pages, `BLK_DEV_UBLK`, `IO_URING`, `VLAN_8021Q`, `UNIX`,
`PACKET`. The check still covers 194 symbols. The test image's kernel is
the release image's: QS/MC has no test fragment.

## Where the base crates differ from the handoff

The crates were followed:

- **A unit absent at the IPL is copied whole when it returns.** The
  handoff's suite 7 says the returned disk is resynchronized only where
  it changed. QS/400's `protect` keeps a member's map only from when it
  is seen leaving a running set, so that holds for a unit that fails
  while the console runs, and the suite shows it so; a unit already
  absent at the IPL comes back by a whole copy. `docs/OPEN_QUESTIONS.md`
  item 13.
- **A restarted storage service copies the regions in flight again.**
  Killed under writes, it takes the device over and then brings the
  regions its write-intent map holds dirty back in step (80 to 112 of
  765 MiB here). QS/VS's own Phase 2 record says "no resynchronization",
  from before the maps; nothing is lost either way, which is what the
  handoff asks.
- **A degraded IPL takes 16 to 34 s.** `base/stgd` waits ten seconds
  for a missing disk each time it looks, twice. The 10 s "Base ready"
  of suite 1 holds for whole load sources (under 1 s); the harness gives
  a degraded boot twice its boot timeout. `docs/OPEN_QUESTIONS.md`
  item 14.
- **The storage service's status before it has found the disks** is an
  empty one; PID 1 shows "not known: the storage service is finding the
  load-source disks" for it.
- **Control groups.** `supervise` puts no service in a group of its
  own; PID 1 mounts cgroup2 as Phase 0 did and does nothing more.
- **`identity` needs a serial number** for its certificate; without
  QS/VS's inventory, QS/MC derives it from the MAC as above.

## Running

```
cargo run -p build -- all
cargo run -p build -- --test-image all
cargo run -p build -- test
cargo run -p build -- test --suite list
cargo run -p build -- test --suite mirror
cargo run -p build -- test --media iso
cargo run -p build -- loadsource /tmp/a.img /tmp/b.img
```

`test` runs Phase 0's three steps on the release image, then the base
suites on the test image; `--suite` runs one. Logs are under
`<target>/conformance/`. `QSMC_BOOT_TIMEOUT` (60 s) is a boot's limit,
twice that for a degraded mirrored boot.

| # | Suite | What it asserts |
| --- | --- | --- |
| 1 | `status` | every IPL step's code in order; "Base ready" under 10 s after the kernel started under KVM; the status screen (slot, load source, reference, each service); the control socket's status all ready and of a test image. On the test image's disk, then on a mirrored pair written by the base crate's writer (`/data` on `/dev/ublkb0`) |
| 2 | `dhcp` | on QEMU's user network, `10.0.2.15/24 on eth0, gateway 10.0.2.2` posted, on the status screen and in the status |
| 3 | `static-vlan` | a seed with a host name and `192.0.2.10/24` on VLAN 100 is applied (`C3001061`); the screen has the address on `eth0.100` and the name; `eth0.100` exists; `network.toml` is the seed's |
| 4 | `time` | an SNTP responder three days ahead moves the console's clock to within 10 s of it, and the RTC follows |
| 5 | `supervision` | `netd` killed is started again and ready, nothing else touched; with `flaky-fails`, `flaky` is stopped after five failures with `A3001080`, the base is not ready, and the console runs on |
| 6 | `power-pull` | with the churn running, QEMU killed at 50 random moments; every IPL mounts `/data` and every state file parses and is a whole generation, never more than one back |
| 7 | `mirror` | either unit absent at the IPL: the console IPLs and posts `A3003020` naming it, the status says Missing; unit 2 failed while running (QEMU's `drive_del`), written without, returned: resynchronized from what changed only, then both Active and the files read back; the storage service killed three times under 4 MiB writes: taken over each time, files whole, only the regions in flight copied again |
| 8 | `apart` | each disk run alone, then both: `A3003040`, the IPL waits (nothing mounted after 15 s) with both units `Apart`; unit 1 kept through the channel, then `C3003050`, `/data` mounted and base ready |
| 9 | `slot-confirm` | slot B on trial confirms: `C3001095`, default B with V1R0M0 in the record on the disk, `BootOrder` 0001,0000 in the firmware's store; with `refuse-confirm` it does not, `BootOrder` is unchanged, and the next IPL is on slot A with `A3001035` and the record cleared |
| 10 | `shutdown` | Ctrl-Alt-Del ignored; the power button: `D3001000`, `D3001090`, QEMU ends within 30 s, nothing killed, `/data` unmounted; the next mount needs no xfs log recovery and the history has the shutdown; then a restart by the control socket (`D3001091`) and a power off by `SIGTERM` |
| 11 | `identity` | made once (`C3001075`); the key mode 600 and root's; the same certificate after a host name change and a restart, and after the clock moved 400 days and a restart |
| 12 | `sockets` | with a time server by address and one by name: no TCP socket listens, UDP only the DHCP client's port 68 or a client's ephemeral one; every frame the console sent (QEMU's `filter-dump`) is DHCP, DNS or NTP, or ARP, or IPv6 neighbour discovery and listener reports; DHCP, DNS and NTP all seen |
| 13 | `processor` | on `qemu64`: `A3001015`, base ready, and the screen's processor line says what is lacking |
| 14 | Phase 0's | `ipl-slot-a`, `ipl-slot-b`, `processor-lacking` on the release image, and `--media iso`'s `ipl-media` |

## Acceptance evidence

One script on 2026-11-18, in WSL Ubuntu (12 threads, Intel, KVM), Rust
1.98.0, Ubuntu clang 21.1.8, QEMU 10.2.1, cargo-deny 0.20.2, in a fresh
`git clone` of this repository at commit `e67410d` on ext4: two release
builds each from an empty target, the test image, the checks, `build
test`, `build test --media iso`. Every command ended with status 0 but
the `diff` of the two builds' hashes, which shows the two files below.
Another project's kernel build was running on the same machine for part
of the run, so the times are a loaded machine's.

| # | Check | Result |
| --- | --- | --- |
| 1 | Suites 1 to 14 pass under `build test` | pass: `test: PASS, 16 step(s), media disk` (Phase 0's three and the thirteen below) and `test --media iso` passed |
| 2 | Two clean builds of the release image are byte-identical, and it holds no test hook | pass: the fourteen files below are the same in both but `.config` and the manifest (as in Phase 0: the target directory's path); `qsmchook`, `TEST IMAGE`, `qsmc.hook` and `hook=` occur nowhere in the release initramfs; its manifest says `QS/MC`, 12 artifacts, no hooks; the test manifest `QS/MC (test image)`, 13 with `qsmchook` |
| 3 | `build deny` and `build doctor` pass | pass: `cargo deny check`, then 177 crates compiled for the host and for musl with no `links` key and no C build helper; `doctor` every row ok: 75 `.rs` files start with the header, `QS/VS 9f11fd566, QS/400 423c65dea (QS/VS's own)`, `cargo tree -d names no QS/VS or QS/400 crate` |
| 4 | Nothing of `base/` or QS/400 is written or copied here; every gap is a change for QS/VS | pass: the base table above. What QS/MC needed and `base/` does not have is in `docs/OPEN_QUESTIONS.md` as changes for QS/VS (and QS/400): the processor check (3), the QEMU driver (4), the second interface (11), the inventory (12), a member absent at the IPL (13), the degraded IPL's wait (14). Written here instead, small and marked: the processor check, the harness, two sysfs files for the storage service's disks, the derived serial number, the status screen, the OEM-string reader of the hooks |
| 5 | Open questions 1 and 7 are closed | pass: both closed with Cody's answers in `docs/OPEN_QUESTIONS.md` |
| 6 | Nothing listens on the network | pass: suite 12, no TCP socket at all and UDP bound only to port 68; frames as above |

Also status 0 in the run: `build notices --check`, `cargo fmt --all
--check`, `cargo clippy --workspace --all-targets --locked -- -D
warnings`, the same for PID 1 and the three services on the musl target
and for PID 1 and `qsmchook` with `QSMC_TEST_HOOKS=1`, `cargo test
--workspace --locked -- --include-ignored` and `QSMC_TEST_HOOKS=1 cargo
test -p qsmcinit` (75 tests in the two runs).

### The suites in that run

| Step | Time | What was seen |
| --- | --- | --- |
| `ipl-slot-a` | 12.7 s | `QS/MC V1R0M0 e67410d`, `IPL slot A`, `REF C3001010`, the processor's line, two heartbeats |
| `ipl-slot-b` | 7.5 s | the same from slot B by its firmware boot entry |
| `processor-lacking` | 8.0 s | `REF A3001015 ...: AVX2, ADX, AES, PCLMULQDQ`, a heartbeat |
| 1 `status` | 12.6 s | one disk: base ready 0.88 s after the kernel started under KVM, 5 services ready, `/data` on `/dev/vda3`; mirrored: 0.91 s, `/data` on `/dev/ublkb0` |
| 2 `dhcp` | 3.1 s | `10.0.2.15/24 on eth0, gateway 10.0.2.2` on the screen and in the status |
| 3 `static-vlan` | 3.1 s | `192.0.2.10/24 on eth0.100, gateway 192.0.2.1`, `eth0.100` exists |
| 4 `time` | 4.1 s | the clock moved 259 202 s to the responder's; the RTC 0 s from it |
| 5 `supervision` | 31.6 s | `netd` process 90 killed, ready again as 95; `REF A3001080 Service stopped: flaky`; heartbeats go on |
| 6 `power-pull` | 183.6 s | 50 cuts, 51 IPLs, `/data` mounted and every state file whole each time; `host.toml` from generation 8 to 4134 |
| 7 `mirror` | 96.2 s | `REF A3003020 Load-source unit failed: unit 2, serial QSMCHD1: missing` and the same for unit 1; unit 2 failed and returned: 128 of 765 MiB copied, 637 already in step; three kills (processes 76, 98, 103), each `taken over as /dev/ublkb0`, 427 files written, the last 3 read back whole, 112, 96 and 80 of 765 MiB copied again |
| 8 `apart` | 70.8 s | `REF A3003040`, the IPL waited 15 s and more; unit 1 kept, base ready 16.01 s after the kernel started |
| 9 `slot-confirm` | 13.0 s | `REF C3001095 Slot confirmed: slot B is the default; BootOrder 0001,0000`, default B and V1R0M0 in the record; refused: `REF A3001035 Pending slot not confirmed: slot B; this IPL is on slot A` |
| 10 `shutdown` | 10.4 s | `REF D3001000 Console shutdown started: power off, by the power button`, `REF D3001090` within a second; the next mount clean; 20 codes in the history; `REF D3001091` by the control socket; power off by `SIGTERM` |
| 11 `identity` | 10.0 s | a 420-byte certificate, made once; key mode 600, root's; the same after `mc-renamed` and the clock 34 560 000 s ahead |
| 12 `sockets` | 9.2 s | 0 TCP sockets; UDP on port 68 only; 15 frames sent: DHCP 2, DNS 2, NTP 3, and ARP and IPv6 neighbour discovery and listener reports |
| 13 `processor` | 3.0 s | on `qemu64`: base ready 1.25 s after the kernel started; `Processor      lacks AVX2, ADX, AES, PCLMULQDQ: the web server and QSI cannot run on it` |
| `ipl-media` | 12.4 s | `IPL from installation media (qsmc.install=1)`, `The installer arrives in Phase 2; this IPL starts no services`, two heartbeats |

### Reproducibility

The same SHA-256 after both release builds (the build ID `e67410d` in
both, the second with the first's signing key):

```
41536092178cd633725e48911fedd523a347d9b53a61158eb71cdd82560d5582  images/initramfs.cpio                  (3666248 bytes)
84b9d5e64e6b7143ee3b4110dc9161a82a78333f26124a1c06f476d2be0f0327  images/initramfs-media.cpio            (1226672 bytes)
f3c52fcada3eff9cbbe2eff7404ca210b45acc66c1c74bebe742324376f93a2d  images/qsmc-V1R0M0-a-f3c52fcada3e.efi  (10834944 bytes)
ed35ab5623020be63c8a65ec3036cb681522304ad5848dd63cd167860601883c  images/qsmc-V1R0M0-b-ed35ab562302.efi  (10834944 bytes)
ced1680bb4ab4c4fa76384a09932b245f81d9db8436929604ac182e9e2dba9ae  images/qsmc-V1R0M0-d-ced1680bb4ab.efi  (10433536 bytes)
2458865cd41e12c2214233fd8a6d1bf4ff78d714826f0ea7ed5e53a28ce6b089  images/qsmc-V1R0M0-f3c52fcada3e.img    (1073741824 bytes)
aeedc760857b18ae97dd820cb94bea4d238a86cd45569714d743eefa94a9f110  images/QSMC-V1R0M0-ced1680bb4ab.iso    (43022336 bytes)
3688736ca7e9dd361de8f900ee0dc4e8a553aa6fce40aaad53b94d378f6c5589  x86_64-unknown-linux-musl/release/qsmcinit (1225376 bytes)
ffc5955eea954b6719daad333ff08ba5674a9ab45f396a7d58f4b79457db432c  x86_64-unknown-linux-musl/release/qsmcstgd (963296 bytes)
0b6de837447875a7bd35ed7fc472e8e12e38d1c7ac9a5aa3336c14f433059be8  x86_64-unknown-linux-musl/release/qsmclogd (455288 bytes)
e2d0f30bc20ba75aa1df180ef7449b7074c3bc2fc3c9167bc38fff4fbaf9c9d9  x86_64-unknown-linux-musl/release/qsmcnetd (1020608 bytes)
```

`file` says of each of the four binaries: static-pie linked, stripped.
What differed: the kernel's `.config` (`CONFIG_INITRAMFS_SOURCE` names
the target directory) and so `manifest.json` and `manifest.sig`, as in
Phase 0. The test image was built once and its reproducibility was not
measured.

## Found while testing

Defects the suites found in this phase's own code, fixed before the
evidence above:

- The test hooks read the SMBIOS OEM strings from
  `/sys/firmware/dmi/entries`, which needs `DMI_SYSFS`; they read the raw
  table now, as QS/VS's inventory does, and the kernel is unchanged.
- The status screen's load-source line read the storage service's
  status from before it had found the disks as `: `; it says "not
  known" now, and the mirror suite waits for the line.
- Suite 7 first took a disk away with `device_del`, which needs PCIe
  hotplug the guest kernel has not; `drive_del` fails every transfer to
  the disk instead, which is how a failing disk looks to the set.
- Suite 11 first moved the clock and looked after a restart; each boot is
  a QEMU of its own whose RTC starts from the host's clock, so the clock
  is checked in the same boot now.

## Not done

- **NEEDS-CODY: the QEMU job in CI** has still not run on GitHub: nothing
  is pushed. It now builds both images and runs every suite
  (`docs/OPEN_QUESTIONS.md` item 6).
- **NEEDS-CODY: real hardware.** Everything ran under QEMU with KVM: the
  RTC write, a real firmware's power button and efivarfs, a NIC slow to
  get carrier, and disks pulled from a running machine are not
  exercised.
- `BLOCKED(phase1)`: the processor check and the QEMU driver are QS/MC's
  own until QS/VS moves them (`docs/OPEN_QUESTIONS.md` items 3 and 4).
- `BLOCKED(phase1)`: no hardware inventory: no location codes, and the
  serial number is always derived from the MAC (item 12).
- `BLOCKED(phase1)`: one network interface (item 11).
- `BLOCKED(phase1)`: a mirrored unit absent at the IPL is copied whole on
  its return (item 13); a degraded IPL waits about twenty seconds
  (item 14).
- `BLOCKED(phase1)`: the unit to keep of two that ran apart is chosen
  through a test image's hook only; the screen is Phase 2's.
- `BLOCKED(phase1)`: nothing writes the other slot, sets a pending slot
  or `BootNext`; the suite does that from the host side. That is the
  update phase.
- Attention codes not seen under QEMU, only by reading the code:
  `A3001010`, `A3001030`, `A3001040`, `A3001050`, `A3001060`,
  `A3001070`, `A3001075`, `A3001095`, `A3002010`, `A3002020`,
  `A3003010`, `A3003070`, and the fatal `B3001000` and `B3003000`. The
  status screen saw `A3002005` (no NIC) in most suites, and `A3003030`
  when the failed unit's first resynchronization was refused by its
  disk.
- The status screen is a placeholder: no keys, no sign-on.
- Reproducibility of the test image and across checkout paths was not
  measured.

Not in Phase 1, and not started: the installer and the local console
(Phase 2); the web server, its certificate and any listener (Phase 3);
QSI (Phase 4).
