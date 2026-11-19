# Open questions

What needs a decision, or a change outside this repository. Each item
says who it waits on.

## 1. The console's reference code families (closed)

A console posts in the families whose second character is `3`: `C3`,
`A3`, `B3`, `D3` (`docs/REFCODES.md`). No QS/400 family (`C1`, `C6`,
`A6`, `B1`, `D1`) and no QS/VS family (`C2`, `A2`, `B2`, `D2`) ends in
`3`, checked at both pinned commits, and a unit test keeps every code
out of both. The word is `REF`, as a host's is.

**Closed in Phase 1:** confirmed by Cody, `C3`, `A3`, `B3`, `D3`,
posted as `REF`. Phase 1's codes are in those families, in blocks laid
out as QS/VS lays out its own.

## 2. The web UI prototype (NEEDS-CODY)

Phase 3 serves the prototype's pages as templates. It is not needed
until then, but Phase 3 wants it committed under `design/` in this
repository first.

## 3. The processor check is not in QS/VS's `base/` (QS/VS)

QSI's and the web server's cryptography (graviola) needs AVX2, ADX, AES
and PCLMULQDQ (QS/400's `docs/QSI.md`, section 12). QS/VS's PID 1, its
QSI server and its Service Tools ask through `qsilocal::processor`, in
QS/VS's `qsi/local`: not `base/`, and its words are a host's ("the
service interface is down"). Neither is QS/400's `qsitrust` the place
it is now: QS/400 has no such check in a crate.

QS/MC's PID 1 therefore makes the same check itself
(`lic/qsmcinit/src/cpu/mod.rs`: the four features, by
`is_x86_feature_detected!`, no text of QS/VS's). What would let QS/MC
pin it instead: QS/VS moves `NEEDED` and `lacking()` of
`qsi/local/src/processor.rs` into `base/` (into `base/identity`, which
both appliances use for QSI's certificate, or a small `base/processor`),
leaving the words of the message to each product; or QS/400 adds it to
`qsitrust`, which owns the dependency on graviola. Then the pins move
and `lic/qsmcinit/src/cpu/` goes.

**Phase 1:** Cody's answer: QS/VS's to change, in a follow-up there.
Not waited for: PID 1 keeps its own check, and takes QS/VS's when the
pins next move (`BLOCKED(phase1)` in `docs/PHASE1.md`). The same
answer covers items 4, 5 and 10.

## 4. QS/VS's QEMU driver is not usable from outside (QS/VS)

QS/VS's `tools/conformance` depends on most of QS/VS (`qsvshook`,
`qsvmm`, `partd`, `qsictl`, `release`, ...) and reads `QSVS_`
variables, so QS/MC cannot pin it. QS/MC has the small driver Phase 0
needs (`tools/conformance`): launch, OVMF located, the serial port
captured, and the boot entry written into a copy of the variable store
for the slot B boot (`qemu/vars.rs`, the same EDK II format as QS/VS's
`qemu/vars.rs`; the load option itself is QS/400's
`diskimg::efi::load_option`).

What QS/VS would have to expose for QS/MC to pin its driver: a crate
under `base/` (or a `tools/` crate with `base/`'s rules) holding
`qemu::firmware` with the variables' names passed in, `qemu::vars`,
the serial capture and the launch arguments, with no dependency on any
QS/VS crate outside `base/` and no `QSVS_` names in its code.

**Phase 1:** answered as item 3: not waited for. QS/MC's driver grew
what Phase 1's suites need (`tools/conformance/src/qemu/session.rs`
and `channel.rs`: a second serial port for the test hooks' channel,
the monitor, a NIC with its frames captured, SMBIOS OEM strings,
several disks), on the same pattern as QS/VS's and with no code of
it. `BLOCKED(phase1)` until QS/VS exposes one.

## 5. QS/VS's kernel has debugfs (QS/VS)

QS/VS's fragment says `# CONFIG_DEBUG_FS is not set`, but
`x86_64_defconfig` has `CONFIG_BLK_DEV_IO_TRACE=y`, which selects
`DEBUG_FS`, and QS/VS's check does not list `DEBUG_FS`: its built
`.config` (Phase 11's release target, at the pinned commit) has
`CONFIG_DEBUG_FS=y` and `CONFIG_BLK_DEV_IO_TRACE=y`. QS/MC's fragment
turns `BLK_DEV_IO_TRACE` off and checks both. QS/VS should do the same
and add `DEBUG_FS` to its `REQUIRED_N`.

## 6. The QEMU job in CI (NEEDS-CODY)

`.github/workflows/qemu.yml` is written for a GitHub-hosted runner with
KVM, as QS/VS's is, and fails with a clear message where the runner has
none; it then needs a self-hosted runner. Nothing has been pushed, so
neither `ci.yml` nor `qemu.yml` has run on GitHub.

## 7. The outbound rule and a packet filter (closed)

The locked decision names what a console may reach. Phase 0's kernel
has no netfilter (`# CONFIG_NETFILTER is not set`, as QS/VS's), so the
rule is the services' own: nothing connects anywhere else.

**Closed in Phase 1:** no packet filter in V1, by Cody's decision. The
rule is kept by construction: each service connects only where its job
is, and netfilter stays out of the kernel. The `sockets` suite shows it:
nothing listens, and of every frame the console sends on its management
interface, captured by QEMU, all are DHCP, DNS and NTP besides what the
link itself needs (ARP, and IPv6 neighbour discovery and listener
reports for the link-local address; `docs/PHASE1.md`).

## 8. The ISO's name

`buildkit` names the ISO `<VENDOR>-<release>-<hash>.iso`, so it is
`QSMC-V1R0M0-<hash>.iso` beside `qsmc-V1R0M0-...` for the UKIs and the
disk image. QS/VS accepted the same for its own (its item 5). The name
is `buildkit`'s.

## 9. Secure Boot

The UKIs are not signed for Secure Boot in Phase 0: `buildkit`'s
manifest records no db certificate (`Product::secure_boot_db` gives
none). QS/400's `pesign` and QS/VS's `build release` show how; that is
Phase 8's.

## 10. The ISO hangs on an IDE CD-ROM under OVMF (QS/VS)

Booted as an IDE CD-ROM of q35 (`-device ide-cd,bus=ide.0`, as QS/VS's
harness boots its ISO), with Ubuntu's OVMF 4M images and QEMU 10.2.1
under KVM, the ISO hangs before the kernel prints anything on a large
share of boots: 6 of 12 for QS/MC's, 4 of 12 for QS/VS's Phase 11 ISO,
each time with the processor running in the firmware's memory (RIP near
`0x7d5xxxxx` or `0x7d8xxxxx`, OVMF's page tables in CR3) until the
limit. The same ISO booted 20 of 20 times as a virtio-scsi CD-ROM, and
as a virtio disk every time. QS/MC's harness uses virtio-scsi. Not
seen on a real machine; QS/VS's `test --media iso` is exposed to it.

## 11. A second network interface (QS/VS)

The prototype's Network settings page has two sections: a private
management network, toward the systems a console manages, and a public
one, toward its operators and the product stream. `base/netd` manages
one interface, and `base/state`'s `network.toml` describes one, so a
console has one in Phase 1 (`docs/PHASE1.md`).

The recommendation, as a change for QS/VS: `base/netd` manages a list
of interfaces, each as it manages one now (by name or MAC, DHCP or
static, an optional VLAN, its own carrier and trial), and
`network.toml` goes to a version 2 with a migration from version 1:
an `[[interface]]` table each, one of them marked as the one the
default route and the name servers come from. The status screen's
management line becomes a line an interface. QS/VS keeps its one
management interface as a list of one. Nothing of it is written here.

## 12. The hardware inventory is not in QS/VS's `base/` (QS/VS)

QS/VS's scanner, `lic/hostinv`, reads the processors, memory, disks
(name, model, serial, size, transport), network interfaces and PCI
devices with their location codes, and the system serial number from
DMI. It is outside `base/` and has a host's fields (VMX or SVM, huge
pages, the IOMMU), though `inventory.toml`'s schema is already
`base/state`'s.

QS/MC has no inventory in Phase 1, by the phase's decision. What that
leaves: the storage service is told each disk's name and the serial
number its device reports (`lic/qsmcstgd/src/disks.rs`, two sysfs
files) and no location code, so a failed unit is named by its number
and serial; the console's serial number, in `identity.toml` and its
certificate, is always derived from the first link's MAC (`M` and
twelve hexadecimal digits), as QS/VS's is when DMI has none; the status
screen shows what `netd` and the load source report.

What QS/MC needs, as a change for QS/VS: the product-neutral part of
`hostinv` (disks, links, PCI devices, location codes, the DMI serial)
moved into `base/`, a host's own fields left in `lic/`. Phase 2's
installer needs the disks and links with their location codes.
