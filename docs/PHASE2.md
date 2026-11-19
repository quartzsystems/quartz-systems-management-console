# Phase 2: Installer, ISO and the local console

What Phase 2 built, how to run it, and the evidence for the acceptance
table. Phase 0's host prerequisites and environment variables
(`docs/PHASE0.md`) and Phase 1's base system (`docs/PHASE1.md`) still
hold; what changed is said here.

Phase 2 makes a console installable on a blank machine and configurable
by a person at its own screen: a text-screen installer on the ISO, and
the local console with sign-on and a short menu in place of Phase 1's
status screen. There is nothing of the web interface or QSI, and nothing
listens on the network.

## Step 0: the pins and what they brought

Two commits, before anything of the phase:

| Commit | What |
| --- | --- |
| `8cd7186` Move the QS/VS pin to f0ef0f1 | QS/VS from `9f11fd5660556dabf789a062768418adf0548583` to `f0ef0f15c216c4ac6203f05e141c92926aee5705` ("Document the base crates QS/MC takes"); QS/400 stays at `423c65dea0b8c9fab3d2c3efaba6e9f27e679c99`, which that commit pins too. `Cargo.lock` started again from QS/VS's at its pin. The `Appliance` names its interface roles now (`release::INTERFACE_ROLES`); `network.toml`'s interface list reached the churn hook. Built from empty targets: every suite of Phases 0 and 1 and the ISO step passed |
| `a893cc1` Take the processor, inventory and QEMU driver from base | what the follow-ups moved into `base/` taken, and what stood in for it here deleted (below). Every suite of Phases 0 and 1 and the ISO step passed on that tree, with one assertion of Phase 1's `mirror` suite brought to the inventory: a unit that fails while the console runs is named by its location code now (`unit 2 at P0000-B00-D03-F0, serial QSMCHD1: failed`) |

What the new pin changed, and what it means here:

| QS/VS commit | What | Here |
| --- | --- | --- |
| `9e35754` | `base/processor` | `lic/qsmcinit/src/cpu/` deleted; PID 1 asks `processor::lacking()`, the words are `release::processor`'s |
| `267c0bd` | `base/inventory` | PID 1 reads it at every IPL into `inventory.toml` (a derived serial kept once in `identity.toml`) and shows a Machine row; `qsmcstgd`'s two sysfs reads deleted, the storage service is told each disk's location code; the test hooks' own SMBIOS reader deleted (`inventory::oem_strings`) |
| `650a6b7` | `base/qemu` | the harness's `qemu/firmware.rs`, `qemu/vars.rs`, its launch and its pipes deleted; it launches every machine with `base/qemu` (named `driver`, as QS/VS's harness names it). Kept: QS/MC's variables and machine size, the machine a suite describes, the expect cursor, the hooks' channel, and the NIC's frame capture, added through QEMU's monitor (`object_add filter-dump`), which `base/qemu` has not |
| `bd9a4ea` | `netd` manages a list of interfaces; `network.toml` version 2 | QS/MC names `open` and `private`; PID 1 shows an Open and a Private row; a version 1 file migrates by `base/state` |
| `20cf33d` | debugfs off in QS/VS's kernel | nothing: QS/MC's fragment had it; the kernel pin did not move |
| `641c0fd` | the display's keyboards read as they come | the installer and the local console have USB keyboards that arrive late |

Open questions 3, 4, 5, 10, 11 and 12 are closed
(`docs/OPEN_QUESTIONS.md`).

## The base crates

Every crate of QS/VS's `base/` at `f0ef0f1`, and its use. Nothing of
them is copied or written again here.

| Crate | Linked | How QS/MC uses it |
| --- | --- | --- |
| `appliance` | yes | `release::appliance`, now with `interface_roles`; the Ethernet links, civil time |
| `supervise` | yes | PID 1's table (the console at 25), the control socket, signals; the console's readiness; the harness's status types |
| `state` | yes | PID 1, the installer (`network.toml`, `host.toml`), the console (reads), the hooks, the harness |
| `logd` | yes | `qsmclogd`; PID 1's records; the console reads the history and the system log by its file names |
| `netd` | yes | `qsmcnetd` with the two roles; the console's network panel (`apply`, `trial`, `confirm`, `interfaces`, `set_clock`); the installer's RTC write and its role check of answers |
| `seed` | yes | PID 1's seed step; the installer's answers |
| `identity` | yes | PID 1, at the first IPL with a `/data` |
| `loadsource` | yes | PID 1; the installer's steps (`prepare`, `disks`, `record`, `set`, `boot`) and its payload (`payload`); the build's payload; the harness's writer |
| `stgd` | yes | `qsmcstgd` with the inventory's disks; the console's option 2 and the panel for the unit to keep |
| `users` | yes | the installer's first user; the console's sign-on and users panel; the harness's answers hash |
| `panels` | yes | the installer (through `install`); the console: `screens`, `tools` (sign-on, the network form and its trial, users, date and time, logs, `Confirm`), `keep`, `kit`, `forms` |
| `install` | yes | `qsmcinstall`: the flow, in its order, and the answers |
| `processor` | yes | PID 1's line, the console's status |
| `inventory` | yes | PID 1, `qsmcstgd`, the installer, the console, the hooks |
| `qemu` | yes (harness) | the harness, as `driver` |
| `fdpass` | no | Phase 3 |
| `stream` | no | Phase 6 |

## What exists

### The installer (`lic/qsmcinstall`)

PID 1 of the media UKI (`qsmc.install=1`) posts its first codes, sets
the clock and runs `/qsmc/bin/qsmcinstall` in place of the service table
(`lic/qsmcinit/src/ipl/install.rs`); when it ends with status 0 the
machine restarts, any other way it is started again. The installer:

- shows `base/install`'s screens on the display and the serial port at
  once and takes keys from both;
- finds the media by its ISO volume identifier on a CD-ROM or a USB
  stick, mounts it, and checks the payload (`base/loadsource::payload`:
  both slots' UKIs and a manifest signed by the build's key) against
  `/qsmc/keys/system.pub` before anything else; refused media stops on
  `base/install`'s refusal screen (`A3004010`);
- with a `QSMCSEED` volume holding `[install]`, takes the answers
  through `install::answers` and QS/MC's role check, and shows only the
  progress (`A3004015` and the stopping screen when they cannot be used);
- is QS/MC's `Target` (`target.rs`): the disks and links from
  `base/inventory` with their location codes, the media and seed disks
  left out, the smallest disk 16 GiB, the default host name, and eight
  steps each under its `C3004` code: partitioning, protection (the
  set's label, the second unit Resuming), the data file system, slot A,
  slot B, the A/B record, the firmware's boot entries, and the state
  (`network.toml` with every role given, the users file of the first
  service tools user, the installer's log as `/data/log/install.log`,
  `host.toml` last). Every step starts from what the one before left, so
  an installation cut off anywhere starts over cleanly.

The media check is QS/MC's own (`media.rs`, `BLOCKED(phase2)`): QS/VS's
is in its `lic/`, item 15 of `docs/OPEN_QUESTIONS.md`.

### The local console (`svc/console`, `qsmccons`)

In the table at 25, after `netd`, with `SYS_TTY_CONFIG` only. As
`docs/LOCALCONSOLE.md` describes, panel by panel: a session on the
display and one on the serial port; the display's idle screen is the
status screen's rows; Enter leads to `base/panels`' sign-on; the main
menu's nine options keep Service Tools' places; idle sign-off after ten
minutes (15 s with a test image's `short-idle` hook). It edits no file:
`netd`, PID 1 and the storage service do the changes; the users are
`base/users`' store.

Written here, because `base/` has no such panel: the main menu, the
status display (option 1), the network menu around the base form
(option 3), the two placeholders (4, Phase 3; 8, Phase 6), a choice
panel for the logs and the power, the power confirmation's words over
`Confirm`, and option 2's disk unit panels (`disks.rs`,
`BLOCKED(phase2)`, item 16; the power words are item 17).

When the units of a mirrored load source ran apart PID 1 runs
`qsmccons keep-unit`: `base/panels`' one panel on the display and the
serial port, in release and test images alike (`ipl/choose.rs`); a test
image's channel can still answer instead.

### PID 1

- The serial port is lent to the local console while it has a session
  there (`console.serial` noted `session`/`idle`): PID 1's codes go to
  the display and the history meanwhile, the status block and the
  heartbeat wait. A console that ends gives it back.
- The panel for the unit to keep, and the installer, as above.
- The status screen: the Open and Private rows (the roles), and Machine.

### Reference codes

`C3004xxx`/`A3004xxx`, the installer's, in the order of its steps
(`docs/REFCODES.md`). They are in the installer's log and on its
progress screen; the history begins at the first IPL.

### Build and media

- The media's initramfs holds the installer and the trusted key; a
  slot's holds the console beside the services.
- `build iso` (`tools/build/src/product/media.rs`): the media UKI as the
  boot image and, under `QSMC/`, `PAYLOAD.TXT`, `PAYLOAD.SIG`,
  `SLOTA.EFI` and `SLOTB.EFI`; reproducible, and in the manifest.
- The release image still holds no test hook: the console's `short-idle`
  is under `cfg(test_hooks)`, and the userland and manifest checks hold
  as before.

## Kernel additions

The installation's drivers were in the fragment since Phase 0 (ISO 9660
with Joliet, SCSI CD-ROM, virtio-scsi, USB with xHCI, EHCI, mass storage
and UAS, USB HID, the framebuffer through simpledrm). Phase 2 sets four
more that were the kernel's defaults, so a merge cannot drop them, and
the check holds them (198 symbols, from 194): `EFI_PARTITION` (the load
source's GPT), `MSDOS_PARTITION` (the ISO written to a stick),
`HID_GENERIC` (a USB keyboard), `FB_DEVICE` (`/dev/fb0`). The built
`.config` did not change.

## Where the base crates differ from the handoff

- **The installer's codes are in its log, not the history.** The flow
  logs each step through `Target::log`; the installer runs before there
  is a data file system, so its codes are in `/run/install.log`, copied
  to `/data/log/install.log`, and on the progress screen.
- **Starting mirrored protection is accepted, then done.** `stgd`'s
  `start_protection` answers once the work is accepted; its status says
  `restart_needed` when the disk is prepared. The console says so, and
  the suite waits for it.
- **Base wording.** `base/panels`' help says "the host" where it means
  the machine; its power confirmation speaks of partitions, which QS/MC
  replaces (item 17). Its sign-on is titled `Service Tools Sign On`, and
  the users are service tools users, as the phase has them.

## Running

```
cargo run -p build -- all
cargo run -p build -- --test-image all
cargo run -p build -- test                       Phase 0's steps, Phase 1's suites, Phase 2's
cargo run -p build -- test --suite install-serial
cargo run -p build -- test --media iso
QSMC_GOLDEN=update cargo run -p build -- test --suite console-panels   write the golden screens
```

Phase 2's suites install the test image from its ISO, so `--test-image
all` builds that ISO too. Installed load sources are kept under
`<target>/conformance/` by the ISO's name and copied for each suite.

| # | Suite | What it asserts |
| --- | --- | --- |
| 1 | Phases 0 and 1 | every step and suite, on the new pins (`8cd7186`, `a893cc1`) and again at the end |
| 2 | `install-serial` | an attended installation on one blank 16 GiB disk over the serial port: each screen (and the welcome's help, the progress complete and the end) equal to its golden; restart; base ready at the DHCP address; sign-on and the main menu equal to theirs |
| 3 | `install-display` | the same answers typed at the display's keyboard through QEMU's monitor; the console installed as asked; a sign-on at the display (logged as such) and one at the serial port at once |
| 4 | `install-unattended` | two blank disks, a seed with `[install]`, a static `open` address and a `private` interface on a second NIC: only the progress shows; both disks hold both slots and a valid record; `/data` on `/dev/ublkb0`, both units in step; `open` and `private` up with their addresses; the one default route on `open`'s device; `network.toml` with both roles |
| 5 | `installed-degraded` | the installed mirror with either disk removed IPLs, runs and names the missing unit by its location code (`A3003020`) |
| 6 | `replace-unit` | a blank disk put in the missing unit's place from option 2; the rebuild watched to its end; with the original disk removed the console IPLs from the rebuilt one |
| 7 | `start-protection` | option 2 starts protection on a single-disk installation; at the next IPL `/data` is on the protected set and both units in step |
| 8 | `install-power-pull` | 20 power pulls at random moments of unattended installations from the ISO as a USB stick: each IPL starts the installer over and reaches its progress; the last finishes and its console is good |
| 9 | `apart-panel` | each unit of an installed mirror run alone with a mark of its own, then both: `A3003040`, the panel on the display and the serial port (its log says both), unit 2 chosen at the display's keyboard; `/data` holds unit 2's mark and not unit 1's |
| 10 | `console-network` | option 3: static, then DHCP, without a restart, each kept; DHCP on a VLAN nobody answers is put back after its trial and `network.toml` is untouched |
| 11 | `sign-on` | users added, removed, the last not removed; a wrong password refused; disabled after five, even for the right one; refusals logged; idle sign-off |
| 12 | `refused-media`, `refused-answers` | an ISO with one byte of its manifest changed is refused before any write; three answers files (a disk not there, a role not QS/MC's, a bad user ID) each stop on the screen that says why, and no disk is written |
| 13 | `small-disk` | a 15 GiB disk is listed, marked `*`, and refused; the 16 GiB one beside it is offered |
| 14 | `console-panels` | the main menu, each of its nine panels (the placeholders among them), the logs and the restart's confirmation, and each one's help, equal to their goldens |
| 15 | `installed-base` | Phase 1's suites on a console installed from the ISO: `status`, `dhcp`, `supervision`, `power-pull`, `mirror`, `apart`, `slot-confirm`, `shutdown`, `identity`, `processor`; `time` with its server given as an installed console takes it (the installer's answers); `static-vlan` and `sockets` give theirs by a seed, which an installed console does not read, and are left out |
| 16 | `no-secrets` | on an installed console with a session open: no TCP socket listens, UDP is bound only to port 68; the password typed at the installer and at sign-on is in no log or state file, no serial log of the suite, and no golden screen |

## Acceptance evidence

In WSL Ubuntu (12 threads, KVM), Rust 1.98.0, QEMU 10.2.1, cargo-deny
0.20.2, on 2026-11-19, in a fresh `git clone` of this repository at
`b5ac248` on ext4: two release builds each from an empty target, the
test image, the checks, `build test`, `build test --media iso`. It ran
in three parts, for two reasons outside the code:

- The second release build first stopped on an internal compiler error
  in `rustc` 1.98.0 compiling `rustix` 1.1.6 (unchanged, and compiled
  in the first build); built again from an empty target it compiled.
- `build test` stops at its first failure, and Phase 1's `mirror`
  failed once: in its kill-under-load part a restarted storage service
  found no region in flight, so the "Resynchronization started" line it
  waits for never came. Run again it passed. The rest of the suites were
  then run one by one; during them the Windows drive under WSL filled
  up and WSL's disk went read-only. After space was freed the built
  images were checked against their hashes (16 of 16 as built), the
  suites' scratch files were deleted, and the remaining suites ran.

| # | Check | Result |
| --- | --- | --- |
| 1 | Step 0 is committed on its own; the stand-ins for `base/processor` and `base/qemu` are gone | pass: `8cd7186` and `a893cc1` before any of the phase, each followed by every suite of Phases 0 and 1 and the ISO step from empty targets; `lic/qsmcinit/src/cpu/`, `tools/conformance/src/qemu/firmware.rs` and `vars.rs`, the harness's launch and pipes, `qsmcstgd`'s `disks.rs` and the hooks' SMBIOS reader deleted |
| 2 | Suites 1 to 16 pass under `build test` | pass: the 33 below, every one passing in the run of `b5ac248`, `mirror` on its second try |
| 3 | Two clean builds produce byte-identical UKIs, disk image and ISO | pass: both slots' UKIs, the media UKI, the disk image, the ISO, both initramfs images and the six binaries have the same SHA-256 in both builds (below); what differs is the kernel's `.config` (`CONFIG_INITRAMFS_SOURCE` names the target directory) and so `manifest.json` and `manifest.sig`, as in Phases 0 and 1 |
| 4 | `build deny` and `build doctor` pass | pass: `cargo deny check`, then 196 crates compiled for the host and for musl with no `links` key and no C build helper; `doctor`: 94 `.rs` files start with the header, `QS/VS f0ef0f15c, QS/400 423c65dea (QS/VS's own)`, `cargo tree -d names no QS/VS or QS/400 crate` |
| 5 | Nothing that `base/` or QS/400 has is written or copied here; every gap is in `docs/OPEN_QUESTIONS.md` | pass: the base table above. Written here because `base/` has not: QS/MC's `Target`, its menu and panels, the media check (item 15), option 2's panels (item 16), the power confirmation's words (item 17), the installer's calls of `base/loadsource` (item 18); in the harness, the frame capture through the monitor and the golden screens' masking (item 4) |
| 6 | Open questions 3, 4, 5, 10, 11 and 12 are closed | pass: `docs/OPEN_QUESTIONS.md` |
| 7 | Nothing listens on the network | pass: `sockets` on the built image (no TCP socket, UDP port 68 only, every frame DHCP, DNS, NTP or the link's own) and `no-secrets` on an installed console with a session open (no TCP socket listens, UDP port 68 only) |

Also status 0 in the run: `build notices --check`, `cargo fmt --all
--check`, `cargo clippy --workspace --all-targets --locked -- -D
warnings`, the same for PID 1, the services, the console and the
installer on the musl target and for PID 1, `qsmchook` and the console
with `QSMC_TEST_HOOKS=1`, `cargo test --workspace --locked --
--include-ignored` and `QSMC_TEST_HOOKS=1 cargo test -p qsmcinit -p
qsmccons` (102 tests). The release initramfs images name none of
`qsmchook`, `TEST IMAGE`, `qsmc.hook`, `hook=`, `short-idle`; the
release manifest says `QS/MC`, 14 artifacts, no hooks; the test
manifest `QS/MC (test image)`, 15, with `qsmchook`.

### The suites in that run

| Step | Time | | Step | Time |
| --- | --- | --- | --- | --- |
| `ipl-slot-a` | 12.4 s | | `install-serial` | 18.4 s |
| `ipl-slot-b` | 7.3 s | | `install-display` | 26.1 s |
| `processor-lacking` | 7.4 s | | `install-unattended` | 91.6 s |
| `ipl-media` | 2.7 s | | `installed-degraded` | 282.4 s |
| `status` | 12.9 s | | `replace-unit` | 268.9 s |
| `dhcp` | 3.2 s | | `start-protection` | 134.5 s |
| `static-vlan` | 3.2 s | | `install-power-pull` | 187.1 s |
| `time` | 4.3 s | | `apart-panel` | 217.2 s |
| `supervision` | 31.4 s | | `console-network` | 76.3 s |
| `power-pull` | 178.7 s | | `sign-on` | 64.3 s |
| `mirror` | 108.6 s | | `refused-media` | 3.3 s |
| `apart` | 69.9 s | | `refused-answers` | 8.7 s |
| `slot-confirm` | 13.4 s | | `small-disk` | 3.0 s |
| `shutdown` | 11.1 s | | `console-panels` | 50.5 s |
| `identity` | 10.5 s | | `installed-base` | 699.0 s |
| `sockets` | 9.0 s | | `no-secrets` | 59.3 s |
| `processor` | 3.1 s | | | |

What they saw, in short: an attended installation in 18 s, every screen
equal to its golden; 53 keys at the display installed the same console;
the unattended mirror with `open` at `10.0.2.20/24 on eth0` and
`private` at `198.51.100.1/24 on eth1`, the one default route on
`eth0`; either disk removed named as `unit 2 at P0000-B00-D03-F0, serial
QSMCHD1: missing` and `unit 1 at P0000-B00-D02-F0, serial QSMCHD0:
missing`; QSMCHD2 rebuilt in unit 2's place, the console IPLing from it
alone; 20 power pulls (seed 483828159), every one mid-installation; the
panel for the unit to keep on a 1280x800 framebuffer and the serial
terminal, unit 2 chosen at the display and its mark the one in `/data`;
static, DHCP and a VLAN put back after its trial without a restart;
`User ID disabled after 5 wrong passwords. Try again in 5 min.`; the
tampered ISO refused with the disk untouched; 23 menu screens and helps
equal to their goldens; on an installed console ten Phase 1 suites and
the time from the installer's server, 1 s off; the password in none of
12 log and state files, the serial logs or the 49 golden screens.

### Reproducibility

The same SHA-256 after both release builds (the build ID `b5ac248` in
both, the second with the first's signing key):

```
93d4e41170cc9806a8b06f5e47c9aaebe38e753a5faf8fb3fcd36651fd875eaa  images/initramfs.cpio
d2ba6c731d2a6dacd3fa8f9de29edb1dc76377512a60a91a7f8667b5abf645da  images/initramfs-media.cpio
0ea1964ca00686158cd709351378e791389cc0d0951d952cf8d6e93d91a124ba  images/qsmc-V1R0M0-a-0ea1964ca006.efi  (11138048 bytes)
0cf2eade642dbbbe1f75796c30795ebc867e361829da90e3e653484555ed4249  images/qsmc-V1R0M0-b-0cf2eade642d.efi  (11138048 bytes)
c518ca6b4cbaa28242502f8bcb3c5755354c75703f5f5c0f5e78d762a51f0782  images/qsmc-V1R0M0-d-c518ca6b4cba.efi  (10810368 bytes)
b4d793ed36642e6094abcaa547cf58227c9c4b2cf9cf00a3b0f96d0611288481  images/qsmc-V1R0M0-0ea1964ca006.img    (1073741824 bytes)
4d3aafb5e65c2ab799c8cefab3f04e0975911c07457c445c56f5b88cfcafac24  images/QSMC-V1R0M0-c518ca6b4cba.iso    (65685504 bytes)
2ede5a1d46c580b172f4e3462bcaf25643234ea79deed19a2f2293c92e57db29  qsmcinit     (1385120 bytes)
bec37298e988e97d83c50b38a20836d0a48df760fde22ff374ef8398f7c9e998  qsmcstgd     (996064 bytes)
081f5d0f75c931dda561a6eabe91a081cc6d8cc248e5c520dce2d7b020c0c351  qsmclogd     (455288 bytes)
bd74ef5c4238e9dec6e5803147fe111a2393d341c5fb82b3fc492238b4c67dc5  qsmcnetd     (1061568 bytes)
9bfbbfe7795a52e80f5b99c9a729e94cd41e221e4612e1e9e489109242d18d93  qsmccons     (1385216 bytes)
652eee640cbf32d23eaf614838e46c2fdc2c54a7bcd3255395eced980f4953be  qsmcinstall  (1606304 bytes)
```

The binaries are under `x86_64-unknown-linux-musl/release/`, each
static-pie and stripped. The test image was built once; its
reproducibility was not measured.

## Found while testing

Defects the suites found in this phase's own code, fixed before the
evidence above:

- PID 1 said "the IPL waits until the unit to keep is chosen" after the
  panel had the serial port, so it reached only the display; it says it
  first now.
- Enter on the rebuild display went back one panel, to the selection of
  a disk with none left to offer; it goes back to the units.
- The harness kept the installation's variable store by copying it onto
  itself, leaving it empty; installed load sources were kept across
  runs whatever ISO they came from; it reads screens and the history
  with deadlines of their own now.

## Not done

- **NEEDS-CODY: real machines.** Everything ran under QEMU with KVM.
  Not yet done: an installation as a Proxmox virtual machine; on a
  physical machine; from virtual media (a BMC's) and from a USB stick
  on a real port; on the display and on the serial port of each; and a
  mirrored installation with a disk pulled from the running machine.
- **NEEDS-CODY: the QEMU job in CI** has still not run on GitHub
  (`docs/OPEN_QUESTIONS.md` item 6).
- `BLOCKED(phase2)`: the media's check (item 15) and option 2's disk
  unit panels (item 16) are QS/MC's own until QS/VS moves them into
  `base/`; the power confirmation's words are QS/MC's (item 17); the
  installer's steps are QS/MC's own calls of `base/loadsource` (item 18).
- `BLOCKED(phase1)`, unchanged: a mirrored unit absent at the IPL is
  copied whole on its return (item 13); a degraded IPL waits about twenty
  seconds (item 14).
- The display's screens are checked by the state they leave, not read:
  the harness reads the serial port's terminal only.
- The release image's ISO installs the release image; the suites install
  the test image's ISO. The release ISO is checked to its installer's
  first screen (`ipl-media`).
- Phase 1's `mirror` suite can miss its "Resynchronization started"
  line when a storage service is killed with no region in flight (seen
  once in the evidence run, passed when run again). The suite should
  accept a takeover with nothing to copy; not changed in this phase.
- Reproducibility of the test image was not measured.

Not in Phase 2, and not started: the web server, its certificate, web
users and any listener (Phase 3); QSI and managed systems (Phase 4); the
console's own dump, backup and update from a removable volume (Phases 3
and 6).
