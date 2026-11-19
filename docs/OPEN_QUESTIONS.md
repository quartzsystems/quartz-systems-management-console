# Open questions

What needs a decision, or a change outside this repository. Each item
says who it waits on.

## 1. The console's reference code families (NEEDS-CODY)

A console posts in the families whose second character is `3`: `C3`,
`A3`, `B3`, `D3` (`docs/REFCODES.md`). No QS/400 family (`C1`, `C6`,
`A6`, `B1`, `D1`) and no QS/VS family (`C2`, `A2`, `B2`, `D2`) ends in
`3`, checked at both pinned commits, and a unit test keeps every code
out of both. The word is `REF`, as a host's is. Confirm the range, or
name another, before Phase 1 adds the base system's codes.

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

## 7. The outbound rule and a packet filter

The locked decision names what a console may reach. Phase 0's kernel
has no netfilter (`# CONFIG_NETFILTER is not set`, as QS/VS's), so the
rule is the services' own: nothing connects anywhere else. Whether a
console also enforces it with a packet filter (QS/400's `netlink` has
nftables) is for Phase 1, with the management network.

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
