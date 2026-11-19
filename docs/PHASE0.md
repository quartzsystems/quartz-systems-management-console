# Phase 0: toolchain and build system

What Phase 0 built, how to run it, and the evidence for the acceptance
table. QS/MC is x86_64 only.

## The two pinned commits

QS/VS's crates are git dependencies on
`https://github.com/quartzsystems/quartz-systems-virtual-server` at one
commit, its `main` as this phase started:

```
9f11fd5660556dabf789a062768418adf0548583
```

QS/400's crates are git dependencies on
`https://github.com/quartzsystems/quartz-systems-400` at one commit,
exactly the one that QS/VS commit pins in its own `Cargo.toml` (and
QS/400's `main` as this phase started):

```
423c65dea0b8c9fab3d2c3efaba6e9f27e679c99
```

Cargo resolves QS/VS's own QS/400 dependencies (`abrec`, `ipc`,
`protect`, ...) to the same source, so the graph holds one copy of each.
`build doctor` holds it (the rows `the two pins agree` and `no shared
crate twice`), and so does CI's pin job. Moving either pin is its own
commit, and they move together (`docs/ARCHITECTURE.md`, "The two-pin
rule").

Taken from QS/VS: `appliance`, `supervise`, `loadsource`. From QS/400:
`buildkit`, `vrm`, `diskimg`; `ipc`, `abrec`, `protect`, `blockdev`,
`xfsfmt` and `optical` come with them. No source of either is in this
repository. Three files are the same as QS/VS's at its pinned commit
(their git blob hashes match): `rust-toolchain.toml` (`3d4ab0f3`),
`kernel/VERSION` (`a1dda482`) and `kernel/SHA256` (`b0e2ec8b`).
`Cargo.lock` was started from QS/VS's lock file, so the crates the two
share are at the versions QS/VS tests, and cargo cut it down to this
workspace's graph.

## The base crates

Every crate of QS/VS's `base/` at the pinned commit, and whether Phase
0 links it. Each of the fourteen was also built from outside QS/VS, as
a git dependency at the pin, for the host and for musl, in a scratch
crate beside this repository: all build, the graph has nothing of
QS/VS outside `base/`, no QS/VS or QS/400 crate is there twice, and
the only `QSVS` in their code outside comments is test data of
`seed`'s tests.

| Crate | Linked in Phase 0 | Where |
| --- | --- | --- |
| `appliance` | yes | `lic/release` (`LOAD_SOURCE`, `appliance()`), `qsmcinit` (the description with its poster and notes), `lic/refcode` |
| `supervise` | yes | `qsmcinit`: `SIGCHLD` taken from a descriptor (`supervise::signal`) |
| `loadsource` | yes, host tools | `tools/build` (the layout, the A/B record, the media's volume id), `tools/conformance` (the GPT read for the slot B entry) |
| `state` | no | Phase 1 |
| `logd` | no | Phase 1 |
| `netd` | no | Phase 1 |
| `seed` | no | Phase 1 |
| `users` | no | Phase 2 |
| `panels` | no | Phase 2 |
| `install` | no | Phase 2 |
| `stgd` | no | Phase 2 |
| `identity` | no | Phase 4 |
| `fdpass` | no | Phase 3 |
| `stream` | no | Phase 6 |

**No base crate is unusable from outside QS/VS.** Two things QS/MC
needs live in QS/VS outside `base/`, and are reported
(`docs/OPEN_QUESTIONS.md`):

- The processor check PID 1 makes (AVX2, ADX, AES, PCLMULQDQ) is
  QS/VS's `qsilocal::processor`, in `qsi/local`. QS/MC's PID 1 makes the
  same check itself (`lic/qsmcinit/src/cpu/`) until it moves to `base/`
  (item 3).
- QS/VS's QEMU driver (`tools/conformance`) depends on most of QS/VS.
  QS/MC has the small driver this phase needs (item 4).

## What exists

- `tools/build/`: the orchestrator, package `build`, binary
  `qsmc-build`: `cargo run -p build -- <command>`. `main.rs` is a clap
  enum over `buildkit`; `src/product/` is QS/MC's `Product`.
  - `fetch`, `kernel`, `userland`, `initramfs`, `uki`, `iso`,
    `manifest`, `deny`, `notices`: `buildkit`'s steps, unchanged. The
    initramfs is `buildkit`'s plain one: `/init` (`qsmcinit`), the mount
    points, a console node.
  - `disk`: `buildkit` writes the GPT of `loadsource::layout` under
    QS/MC's names (ESP `EFI/BOOT/BOOTX64.EFI` slot A, `EFI/QSMC/B.EFI`
    slot B, volume label `QSMC ESP`, volume id `0x51534d43`; the record
    partition; the empty xfs `QSMCDATA`). The step then writes the first
    A/B record through `loadsource::record` (QS/400's `abrec` under the
    magic `QSMCREC` padded with a zero and QS/MC's partition type) and
    reads the image back: the record, the ESP's label, volume id and
    exactly the two UKIs, and the xfs label.
  - `iso`: the UEFI ISO whose boot image carries the media UKI, volume
    `QSMC_V1R0M0`. No payload yet.
  - `manifest`: `buildkit`'s signed manifest, with `QSMC_SIGNING_KEY` or
    else a development key it generates under `target/keys/`, saying so.
  - `all`: fetch, kernel, userland, initramfs, uki, disk, iso, manifest.
  - `test`: the conformance suite, `--media disk` (default) or `iso`.
  - `doctor`: `buildkit`'s rows (the toolchain, KVM, the target
    directory), then OVMF (code image and variable store), the license
    header on every `.rs` file, the two pins agreeing and no shared
    crate twice.
- `lic/qsmcinit/`: PID 1. Installs its panic hook; mounts proc, sysfs,
  devtmpfs, a tmpfs on `/run` and cgroup2; posts `REF C3001000` and
  prints the banner (`QS/MC V1R0M0 <build id>`), the slot from the
  command line, `REF C3001010` (or `A3001010` with what did not mount)
  and the processor's line on `/dev/console` and `/dev/ttyS0`; on a
  processor without what the cryptography needs, `REF A3001015` with
  the features lacking, and it runs on. Heartbeats `REF C3001005` on
  serial every 5 s; reaps children on `SIGCHLD`; a panic posts
  `REF B3001000` and parks. On the media UKI it says the installer
  arrives in Phase 2 and otherwise behaves the same. Its `Appliance`
  (`lic/qsmcinit/src/product.rs`) is `release::appliance` with PID 1's
  poster (an event's line on the console; no event has a code yet) and
  its notes (facts kept in memory for Phase 1's status screen).
- `lic/release/`: `V1R0M0` over QS/400's `vrm`, from `VERSION` and `git
  describe`, honouring `QSMC_RELEASE` and `QSMC_BUILD_ID`; the product
  text `Quartz Systems/Management Console`, the banner, the prefixes,
  the default host name, `LOAD_SOURCE` and the `Appliance` description.
- `lic/refcode/`: the console's reference codes and the poster
  (`docs/REFCODES.md`), with the tests that hold the document to the
  table and every code out of QS/400's and QS/VS's families.
- `tools/conformance/`: the QEMU driver (`-machine q35`, OVMF by pflash,
  serial captured, KVM with `-cpu host` when `/dev/kvm` is usable) and
  the suite.
- `kernel/config/qsmc.config`, `.github/workflows/`, the documents.

### Disk layout of the image

| # | Name | GPT type | Size | Content |
|---|---|---|---|---|
| 1 | `QSMC ESP` | EFI System | 256 MiB | FAT32 with the two UKIs |
| 2 | `QSMC-AB` | `7e12d3ff-d7f5-454b-a065-b4d898ca2ffc` | 1 MiB | the A/B record, version 2, slot A, sequence 1 |
| 3 | `QSMCDATA` | `b2e14a0f-e805-43b6-8bed-a9266d0cdd6c` | the rest (765 MiB of the default 1 GiB) | empty xfs, UUID `e72f9d32-46f4-4246-9259-352ddcf3f26e` |

Disk and partition GUIDs are fixed (`QSMC` prefix), as are the xfs UUID
and the FAT volume id.

### Kernel configuration

`qsmc.config` is merged over `x86_64_defconfig` after `buildkit`'s
baseline. The product's required-symbol lists
(`tools/build/src/product/kernel.rs`) name every symbol the fragment
sets, both ways, and a unit test (`the_lists_are_the_fragment`) fails
if either drifts from the fragment. The independent check of the final
`.config` covers **194 symbols**: the baseline's 14 (11 required, 3
forbidden), QS/MC's 123 required and 56 forbidden, and the command
line. By group:

| Group | Symbols |
| --- | --- |
| networking (web server, QSI client, DHCP, Unix sockets, a VLAN management port) | `NET`, `INET`, `IPV6`, `UNIX`, `PACKET`, `VLAN_8021Q` |
| supervision | `CGROUPS`, `CGROUP_PIDS`, `MEMCG`, `SECCOMP`, `SECCOMP_FILTER` |
| the load source (xfs, `ublk` and `io_uring` for the mirror, NVMe, SATA, SCSI, QS/VS's HBAs) | 16 |
| ESP and firmware variables | `VFAT_FS`, the code pages, `EFIVAR_FS` |
| removable volumes (seed, media, updates) | `ISO9660_FS`, `JOLIET`, `BLK_DEV_SR`, USB host controllers, `USB_STORAGE`, `USB_UAS` |
| the management port | QS/VS's Intel, Broadcom and Mellanox drivers |
| as a guest | virtio (PCI, block, SCSI, net, console, entropy); VMware (`FUSION`, `FUSION_SPI`, `FUSION_SAS`, `VMWARE_PVSCSI`, `ATA_PIIX`, `VMXNET3`); Hyper-V (`HYPERVISOR_GUEST`, `HYPERV`, `HYPERV_NET`, `HYPERV_STORAGE`, `HYPERV_KEYBOARD`), as QS/400's fragment has them; QS/400's vsock (its guest agent) left out |
| the local console | the VT on simpledrm, the 8250 UART, keyboards |
| the base system | the RTC (`RTC_HCTOSYS` off), `ACPI_BUTTON`, DMI, `SIGNALFD` |
| left out: what QS/VS has for partitions | `VIRTUALIZATION`, `KVM`, `VHOST_MENU`, `VHOST_NET`, `BRIDGE`, `BONDING`, `TUN`, `VFIO`, `HUGETLBFS`, `CHR_DEV_ST`, `SCSI_FC_ATTRS`; with `NETFILTER`, `EXT4_FS`, wireless, sound and media |
| hardening | every symbol of QS/VS's Phase 11 hardening, all present at the pinned commit: 23 set (`SECURITY_DMESG_RESTRICT` to `LEGACY_VSYSCALL_NONE`) and 23 off (`SLAB_MERGE_DEFAULT` to `IPV6_SIT`, `DEBUG_FS` among them); and `BLK_DEV_IO_TRACE` off, without which `DEBUG_FS` comes back (`docs/OPEN_QUESTIONS.md` item 5) |

Baked command line: `console=ttyS0,115200 console=tty0 panic=-1 quiet`,
then `qsmc.slot=A` or `qsmc.slot=B`; the media UKI carries
`qsmc.install=1` instead.

### The conformance suite

Within 60 s (`QSMC_BOOT_TIMEOUT`) each boot must show, in the order
PID 1 prints: `REF C3001000`, the banner with this repository's release
(`QS/MC V1R0M0 `), where the IPL came from, `REF C3001010`, the
processor's line and heartbeats.

- `test` boots the disk image as slot A (two heartbeats), again as slot
  B (`IPL slot B (qsmc.slot=B)`), and as slot A on QEMU's `qemu64`
  processor, which lacks AVX2, ADX, AES and PCLMULQDQ: it must show
  `Processor lacks ...` and `REF A3001015`, then a heartbeat.
- `test --media iso` boots the ISO as a CD-ROM on virtio-scsi and
  requires `IPL from installation media (qsmc.install=1)` and the
  installer's line, then two heartbeats.
- Slot B is chosen by firmware boot entry: the harness writes `Boot0000`
  for `EFI/QSMC/B.EFI` (QS/400's `diskimg::efi::load_option`) and
  `BootOrder` into a copy of OVMF's variable store.
- Every drive is a QEMU snapshot or read-only, so a boot leaves the
  images as built. The serial log is printed on failure and kept under
  `target/conformance/`.

## Host prerequisites (Linux; WSL Ubuntu works)

`cargo run -p build -- doctor` checks them.

| What | Why | Debian/Ubuntu package |
|---|---|---|
| rustup with `x86_64-unknown-linux-musl` | pinned by `rust-toolchain.toml` | `rustup target add x86_64-unknown-linux-musl` |
| clang, lld, llvm (LLVM 21) | `LLVM=1` kernel build; clang links the host tools | `clang lld llvm` |
| make, flex, bison, bc, perl | kernel build | `make flex bison bc perl` |
| libelf headers | objtool | `libelf-dev` |
| tar, xz, gzip | source extraction | `tar xz-utils gzip` |
| qemu-system-x86_64 | boot test | `qemu-system-x86` |
| OVMF, code image and variable store | test-only UEFI firmware | `ovmf` |
| `/dev/kvm` usable | the boots run in seconds | `sudo usermod -aG kvm $USER` |
| git | build ID, manifest, the pins | `git` |
| cargo-deny | `build deny` | `cargo install cargo-deny --locked` |

OVMF and QEMU are host test tools standing in for a real machine. They
never ship in the image. On WSL keep the build output on ext4:
`export QSMC_TARGET_DIR=$HOME/qsmc-target`.

## Running

```
cargo run -p build -- doctor
cargo run -p build -- all
cargo run -p build -- test
cargo run -p build -- test --media iso
cargo run -p build -- deny
cargo run -p build -- notices --check
```

| Variable | Use |
|---|---|
| `QSMC_TARGET_DIR` | the output root; else `CARGO_TARGET_DIR`, else `target/` |
| `QSMC_JOBS` | caps the kernel's `make -j` |
| `QSMC_SIGNING_KEY` | the ed25519 key (hex seed in a file) the manifest is signed with; else a generated development key under `target/keys/` |
| `QSMC_BUILD_ID`, `QSMC_RELEASE` | the build ID instead of `git describe`; a release other than `VERSION`'s |
| `QSMC_OVMF_CODE`, `QSMC_OVMF_VARS` | the firmware, when not in a distribution path |
| `QSMC_QEMU` | the QEMU binary |
| `QSMC_NO_KVM` | run QEMU without KVM (`-cpu max`) |
| `QSMC_BOOT_TIMEOUT` | seconds a boot may take (60) |

## Acceptance evidence

Measured on 2026-11-18 in WSL Ubuntu (12 threads, Intel, KVM), Rust
1.98.0, Ubuntu clang 21.1.8, QEMU 10.2.1, cargo-deny 0.20.2, in a fresh
`git clone` of this repository at commit `37f9570` on ext4, with no
environment set. The suite's ISO step was run again at `f61c7cf`, which
changed only the harness's CD-ROM (item 10 of `docs/OPEN_QUESTIONS.md`),
over the same images.

| # | Check | Result |
| --- | --- | --- |
| 1 | From a clean clone, `cargo run -p build -- all` produces the UKIs, disk image, ISO and signed manifest | pass: no environment set, 9 m 32 s from nothing (the kernel tarball fetched); three UKIs, the disk image, the ISO, `manifest.json` (9 artifacts, product `QS/MC`, release `V1R0M0`, build `37f9570`) and `manifest.sig`, signed by a development key the build made and said so |
| 2 | `build test` passes on the disk image and on the ISO; slot B boots and reports itself | pass. Slot A: `REF C3001000`, `QS/MC V1R0M0 37f9570`, `IPL slot A (qsmc.slot=A)`, `REF C3001010`, `Processor has what the cryptography needs: AVX2, ADX, AES, PCLMULQDQ`, two heartbeats, 12.8 s. Slot B, by its firmware boot entry: `IPL slot B (qsmc.slot=B)`, 7.8 s. On `qemu64`: `Processor lacks AVX2, ADX, AES, PCLMULQDQ: the web server and QSI cannot run on it`, `REF A3001015 ...`, a heartbeat, 7.9 s. ISO: `IPL from installation media (qsmc.install=1)`, `The installer arrives in Phase 2; ...`, two heartbeats, 12.3 to 14.6 s in five runs |
| 3 | Two clean builds are byte-identical | pass: the hashes below |
| 4 | `build deny` passes; no C anywhere | pass: `cargo deny check`, then 163 crates compiled for the host and for musl with no `links` key and no C build helper |
| 5 | `build doctor` passes: license headers, the two pins agree, no shared crate twice | pass: every row ok; 35 `.rs` files start with the header; `QS/VS 9f11fd566, QS/400 423c65dea (QS/VS's own, from <cargo's checkout of 9f11fd5>)`; `cargo tree -d names no QS/VS or QS/400 crate` |
| 6 | PID 1 links `base/appliance` and at least one more base crate from QS/VS's repository, unchanged; nothing of `base/` or QS/400 is copied here | pass: `qsmcinit` links `appliance` and `supervise` at `9f11fd5` (and the host tools `loadsource`), as git dependencies; no `path =` to either repository, no file of either |
| 7 | `qsmcinit` is static and stripped; the `.config` check covers every symbol of Section 2 | pass: `buildkit`'s userland check (no `PT_INTERP`, no `DT_NEEDED`, no `.symtab`); `file` says static-pie, stripped, 430672 bytes; 194 symbols checked, every symbol the fragment sets among them |
| 8 | Every base crate that could not be used from outside QS/VS is reported, with what QS/VS must change | pass: none of the fourteen; the two things outside `base/` QS/MC would have taken (the processor check, the QEMU driver) are reported with the change QS/VS would make (`docs/OPEN_QUESTIONS.md` items 3 and 4), and a finding in its kernel (item 5) and its harness (item 10) |

Also checked: `cargo fmt --all --check`; `cargo clippy --workspace
--all-targets -- -D warnings`, and the same for `qsmcinit` on the musl
target; `cargo test --workspace -- --include-ignored` (37 tests, the
pin checks among them); `build notices --check`.

### Reproducibility

Two builds, each from an empty target directory, in the same clone:
the first with no environment (it generated the development key), the
second with `QSMC_TARGET_DIR` set to another empty directory and
`QSMC_SIGNING_KEY` set to the first's key. The build ID was `37f9570`
in both. Every file below had the same SHA-256 in both builds:

```
3b543ac912d748cf608bf4538b1e6932890808b57af10f491b3fa860d3dd652c  images/initramfs.cpio             (431500 bytes)
3b543ac912d748cf608bf4538b1e6932890808b57af10f491b3fa860d3dd652c  images/initramfs-media.cpio       (431500 bytes)
c861f1f575a0a208f156e8a4ab9f719da8de9bdf36168cb6e221c67a73d665c1  images/qsmc-V1R0M0-a-c861f1f575a0.efi  (10130432 bytes)
e25c68cd54473dd04559e70453d45582386be85fa3ac620a7501b05e8f18ba1b  images/qsmc-V1R0M0-b-e25c68cd5447.efi  (10130432 bytes)
7920390cb63d8918bbf36d92fe49772435e72972ec7d0625ce95df2b4495ff15  images/qsmc-V1R0M0-d-7920390cb63d.efi  (10134528 bytes)
de8d8d0c24eef0481ae23144d96f642dce1596f0b80bf2efb628730b5478a542  images/qsmc-V1R0M0-c861f1f575a0.img    (1073741824 bytes)
48d95f52e30bd916da09b8c2c7cbf6853678733e5e4f75b7f06c939198fe2800  images/QSMC-V1R0M0-7920390cb63d.iso    (42723328 bytes)
5cd2b48cd09c6d144eed7f566af2ab4ca62326fc0c064a06f9342181f9f13a7d  x86_64-unknown-linux-musl/release/qsmcinit (430672 bytes)
```

What differed, and why:

- **The kernel's `.config`**, in one line: `CONFIG_INITRAMFS_SOURCE`
  names the cpio under the target directory, and the two target
  directories differ. The kernels built from it are the same bytes.
- **`manifest.json`**, which records the `.config`'s hash, and so
  **`manifest.sig`**. With the same target directory, as QS/VS
  measured, they are the same too.

What the hashes depend on, besides the sources:

- **Not the signing key.** The initramfs carries no key in Phase 0, so
  the UKIs, the disk and the ISO are the same bytes whatever signs the
  manifest. That changes when the installer's media carries a signed
  payload (Phase 2).
- **The checkout path.** `qsmcinit` carries it, as QS/400's and QS/VS's
  binaries do (`docs/BUILDKIT.md` there, section 7). A build at another
  path was not compared.
- **The commit**, in the banner and the manifest's `git_rev`, unless
  `QSMC_BUILD_ID` is set.

Kernel pin: `linux-6.18.55.tar.xz`,
`f410638061a165c12f42ab871d2f3fcd525515359b5faeee80969cff84524df9`, as
QS/VS's and QS/400's.

## Not done

- **NEEDS-CODY: the reference code families.** The `3` families are a
  proposal (`docs/REFCODES.md`; `docs/OPEN_QUESTIONS.md` item 1).
- **NEEDS-CODY: the web UI prototype.** Phase 3 wants it committed under
  `design/` (`docs/OPEN_QUESTIONS.md` item 2).
- **NEEDS-CODY: the QEMU job in CI.** `.github/workflows/qemu.yml` is
  written for a GitHub-hosted runner with KVM and fails clearly where
  there is none. Nothing has been pushed, so neither `ci.yml` nor
  `qemu.yml` has run on GitHub (`docs/OPEN_QUESTIONS.md` item 6).
- **NEEDS-CODY: real hardware and other hypervisors.** Everything ran
  under QEMU with KVM. The HBA, NIC, VMware and Hyper-V drivers are
  configured and checked in `.config`, not exercised.
- `BLOCKED(phase0)`: the processor check is QS/MC's own copy of four
  feature names until QS/VS moves `qsilocal::processor` into `base/`
  (`docs/OPEN_QUESTIONS.md` item 3).
- `BLOCKED(phase0)`: the QEMU driver is QS/MC's own until QS/VS exposes
  one usable from outside (`docs/OPEN_QUESTIONS.md` item 4).
- `BLOCKED(phase0)`: no base event has a reference code; the poster
  prints an event with none. The codes come with the services that
  report them, in Phase 1.
- `BLOCKED(phase0)`: PID 1 does not read the A/B record or write
  firmware boot entries; that is Phase 1. The test chooses slot B from
  the host side.
- `BLOCKED(phase0)`: the ISO is named `QSMC-V1R0M0-<hash>.iso`, by
  `buildkit` (`docs/OPEN_QUESTIONS.md` item 8).
- `BLOCKED(phase0)`: the UKIs are not signed for Secure Boot; Phase 8
  (`docs/OPEN_QUESTIONS.md` item 9).
- Reproducibility across checkout paths was not measured.
- The workspace was built and tested on Linux only. On Windows the host
  tools are meant to compile (`qsmcinit` builds a stub there); not tried.

Not in Phase 0, and not started: the service table, networking, state
and logging (Phase 1); the installer and the local console (Phase 2);
anything of the web UI or QSI.
