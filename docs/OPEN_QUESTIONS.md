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

## 2. The web UI prototype (closed)

Phase 3 serves the prototype's pages as templates. It is not needed
until then, but Phase 3 wants it committed under `design/` in this
repository first.

**Closed in Phase 3:** committed as `design/qsmc-prototype/` (`900107f`).

## 3. The processor check is not in QS/VS's `base/` (closed)

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

**Closed in Phase 2:** QS/VS moved the check into `base/processor`
(`9e35754`, in its `f0ef0f1`). PID 1 asks `processor::lacking()`;
`lic/qsmcinit/src/cpu/` is gone, and QS/MC's words for the line and the
status fact are `release::processor`.

## 4. QS/VS's QEMU driver is not usable from outside (closed)

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

**Closed in Phase 2:** QS/VS moved its driver into `base/qemu`
(`650a6b7`). The harness launches every machine with it (as `driver`,
the name QS/VS's harness gives it): q35 and OVMF, the variable store,
the serial port, the monitor, disks, NICs, the CD-ROM and USB stick,
the 24x80 terminal and its keys, sparse copies and seed volumes. QS/MC's
`qemu/firmware.rs`, `qemu/vars.rs`, its launch and its session's pipes
are gone. What stays QS/MC's in `tools/conformance/src/qemu/`: its
variables and the machine's size, the machine a suite describes mapped
onto `driver::Launch`, the expect cursor its suites use, the test hooks'
channel, the golden screens. One thing `base/qemu` has not, the capture
of the NIC's frames the `sockets` suite reads, is added through QEMU's
monitor (`object_add filter-dump`) once the machine is launched: no
driver code is written here. Two small gaps, as changes for QS/VS: a
frame capture of the user-network NIC in `Launch`, and `cpu` honoured
without KVM (it is `-cpu host` or nothing).

## 5. QS/VS's kernel has debugfs (closed)

QS/VS's fragment says `# CONFIG_DEBUG_FS is not set`, but
`x86_64_defconfig` has `CONFIG_BLK_DEV_IO_TRACE=y`, which selects
`DEBUG_FS`, and QS/VS's check does not list `DEBUG_FS`: its built
`.config` (Phase 11's release target, at the pinned commit) has
`CONFIG_DEBUG_FS=y` and `CONFIG_BLK_DEV_IO_TRACE=y`. QS/MC's fragment
turns `BLK_DEV_IO_TRACE` off and checks both. QS/VS should do the same
and add `DEBUG_FS` to its `REQUIRED_N`.

**Closed in Phase 2:** QS/VS did (`20cf33d`): its fragment turns block
tracing off and its check holds debugfs off. QS/MC's fragment already
had both; the kernel pin did not move.

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

## 10. The ISO hangs on an IDE CD-ROM under OVMF (closed)

Booted as an IDE CD-ROM of q35 (`-device ide-cd,bus=ide.0`, as QS/VS's
harness boots its ISO), with Ubuntu's OVMF 4M images and QEMU 10.2.1
under KVM, the ISO hangs before the kernel prints anything on a large
share of boots: 6 of 12 for QS/MC's, 4 of 12 for QS/VS's Phase 11 ISO,
each time with the processor running in the firmware's memory (RIP near
`0x7d5xxxxx` or `0x7d8xxxxx`, OVMF's page tables in CR3) until the
limit. The same ISO booted 20 of 20 times as a virtio-scsi CD-ROM, and
as a virtio disk every time. QS/MC's harness uses virtio-scsi. Not
seen on a real machine; QS/VS's `test --media iso` is exposed to it.

**Closed in Phase 2:** `base/qemu` attaches an ISO as a virtio-scsi
CD-ROM, and QS/VS's harness boots its ISO that way since `f0ef0f1`; its
`docs/QUALIFICATION.md` keeps a real SATA or USB drive as the case to
watch. QS/MC boots its ISO through `base/qemu` the same way.

## 11. A second network interface (closed)

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

**Closed in Phase 2:** `base/netd` manages a list of interfaces since
`bd9a4ea`, each with the role the appliance names, and `network.toml`
is version 2, migrated from version 1 by `base/state`. Cody's answer
for the roles: QS/MC names two, `release::INTERFACE_ROLES`. `open`, the
first, is the one operators reach the console on, with the default
route, the name servers and the time; `private`, optional, faces the
systems it manages and has no default route. The installer and the
local console configure `open` (the base forms edit the first
interface); `private` is set from a seed and, from Phase 3, the web
interface's Network settings. PID 1 shows both lines on the status
screen. Phase 2's unattended suite installs both and shows that only
`open` carries the default route.

## 12. The hardware inventory is not in QS/VS's `base/` (closed)

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

**Closed in Phase 2:** `base/inventory` (`267c0bd`). PID 1 reads it at
every IPL into `inventory.toml` (a serial number derived from the MAC
is kept once in `identity.toml`, as before) and shows it on the status
screen's Machine row; the storage service is told each disk's location
code from it, so a missing unit is named by its place; the installer
and option 3 list the disks and links from it. `qsmcstgd`'s two sysfs
reads and the test hooks' own SMBIOS reader are gone
(`inventory::oem_strings`).

## 13. A mirrored unit absent at the IPL is copied whole when it returns (QS/400, QS/VS)

Found by Phase 1's mirror suite. A mirrored set keeps a write-intent map
of its own for a member that leaves while it runs, so the member comes
back by what was written meanwhile: a unit failed under a running
console and returned was brought back from "637 of 765 MiB already in
step" (`docs/PHASE1.md`). A member already absent when the set is
assembled is never seen leaving and gets no map (QS/400's `protect`,
`set/intent.rs`: the store is read only for a member out of service,
and nothing starts one for a member that is absent): when it returns,
all of it is copied, "0 of 765 MiB in step". On a real console that is
a disk pulled while it was powered off.

The change, for QS/400 and QS/VS: when the set is assembled from a
clean label with a member absent, `protect` starts that member's map
there (the set's own map is clear after a clean stop, so the map is
empty and correct), and `base/stgd` lets it. QS/MC's suite asserts the
case that works and records this one; nothing is written here.

## 14. A degraded IPL waits about twenty seconds for the missing disk (QS/VS)

`base/stgd` waits for a missing member's disk ten seconds each time it
looks for the load-source disks (`found_disks`, `DISK_WAIT`), and it
looks twice before it serves a degraded set (the check for units that
ran apart, then the assembly). A mirrored console with a unit missing
reaches "Base ready" 16 to 34 s after the kernel started, by the
machine's load, where a whole one takes under 2 s. The wait is there
for controllers that are probed late. The change, for QS/VS: look once
per start and pass what was found on. The phase's 10 s for "Base ready"
is held for whole load sources; the harness gives a degraded IPL twice
the boot timeout (`docs/PHASE1.md`).

## 15. The installation media's check is QS/VS's own (QS/VS)

Found in Phase 2. The payload an installer writes from is
`base/loadsource::payload` (its manifest, its file names, the volume
identifier), but the check of it, the ed25519 signature against the
initramfs's trusted keys and each UKI against the manifest, is in
QS/VS's `lic/qsvsinstall/src/media.rs`, and the build's writing of the
payload and of the trusted keys in QS/VS's `tools/build`. Nothing in
them is a host's.

QS/MC writes the same check (`lic/qsmcinstall/src/media.rs`, marked
`BLOCKED(phase2)`) and its build writes the payload
(`tools/build/src/product/media.rs`). The change, for QS/VS:
`loadsource::payload` gains `verify(root, keys)` with its `signed` and
`matches`, and the device's search by volume identifier; then the pins
move and QS/MC's copy goes.

## 16. The disk unit panels are QS/VS's own (QS/VS)

Found in Phase 2. A load source's panels, the units with their states,
starting mirrored protection with a second disk, replacing a failed unit
and watching its rebuild, are the same for every appliance with a
mirrored load source, and they only ask `base/stgd`. QS/VS's are in its
`svc/tools/src/disks.rs`, beside its pool and virtual disk options.

QS/MC writes its own over `stgd`'s client (`svc/console/src/disks.rs`,
marked `BLOCKED(phase2)`). The change, for QS/VS: the three panels move
into `base/panels` behind a small trait (the storage service's status,
start protection, replace, the machine's disks), as `keep::Units` is for
the unit to keep; QS/VS adds its own options beside them.

## 17. Words of `base/panels` that are a host's (QS/VS)

Found in Phase 2. `panels::tools::power` says "Every partition that runs
on this host ends with it" and its help "Partitions that run on the host
end with it": not so of a console. QS/MC takes `panels::tools::Confirm`
and gives its own words (`svc/console/src/menu.rs`). The change, for
QS/VS: `power` takes the lines about what ends with the machine from its
caller. The rest of the base help says "the host" where it means the
machine, which reads true of a console too, and is left.

## 18. Writing a load source for an installer is QS/VS's own (QS/VS)

Found in Phase 2. QS/VS's installer target (`lic/qsvsinstall/src/target.rs`)
writes a load source in eight steps that use only `base/loadsource`
(the table, the set's label, the data file system, both slots, the
record, the boot entries) and `base/users` and `base/state` for the
state. QS/MC's `Target` (`lic/qsmcinstall/src/target.rs`) makes the same
calls, in its own words; the products differ only in the welcome's
text, the smallest disk and the codes. The change, for QS/VS: a
`base/install` helper that carries out those steps for a product's
`LoadSourceNames` and codes, so a product's `Target` is its names and
its words.

## 19. What the prototype offers that V1 answers with a message

Found opening Phase 3. Some choices on the prototype's Console
Management pages go past the locked decisions or past Phase 3. Each
page is drawn as the prototype has it, and the choice answers with a
`QMC` message saying it is not in V1:

- Backup and restore: "Remote host (SFTP)". Copying a backup off the
  console is not in V1 (the Phase 3 handoff): a backup holds the
  console's identity and users, it leaves only through the browser of
  a signed-on Administrator, and SFTP is outbound traffic the outbound
  rule (item 7) does not list.
- Date and time: "Act as NTP server on the private management
  network". The console is an NTP client only: a server would listen,
  and only the web server listens.
- Network settings: "DHCP for service interfaces" on the private
  network, and "SSH to console". A DHCP server would listen; the locked
  decisions have no SSH.
- Language and locale: languages other than English (the Phase 3
  handoff: English only), and "Keep terminal window sessions open",
  which waits for Phase 4's terminal.

To decide, for each: whether it joins the locked decisions in a later
release, or the design drops it.

## 20. The prototype's shared pages are QS/400's

Found in Phase 3. The handoff says `shared/confirm.html`, `error.html`,
`help.html` and `notauth.html` are framed for QS/MC; they carry QS/MC's
navigation tree but QS/400's wordmark, header (`System: QSHQ400`,
`User: QSECOFR`), footer and content (an HTTP Server Administration
error, a Navigator refusal, the help for Active jobs). QS/MC renders
the same patterns through `qsweb`; its golden tests compare them with
the frame corrected (`docs/WEB.md`). To decide: QS/MC versions of the
four pages in the design.

## 21. No page sets the sign-on notice

Found in Phase 3. The sign-on page carries a configurable notice, but
no page of the prototype has a field for it. It is `notice` in
`web.toml`, empty unless written there. To decide: which page sets it
(Language, locale and session would take one row).

## 22. `qsweb`'s help window closes by script (QS/400)

Found in Phase 3. The help window's Close window link carries an
inline `onclick` handler. QS/MC serves no script, and its content
security policy blocks inline handlers, so the link does nothing there
but go to `#`. The change, for QS/400: a plain link when the product
serves no script (`Product::nav_script` false).

## 23. The users' forms are not designed

Found in Phase 3. On `console-users.html` Properties and Change
password lead to the list itself and Create user... is a bare button:
the prototype has no form for any of them. QS/MC shows each under the
list as `qsweb`'s properties pattern (`?edit=`, `?password=`, after
Create user...), with the password rules stated; the list is the
golden. To decide: designed pages for the three.

## 24. A network change kept only when confirmed (QS/VS)

Found in Phase 3. A change to the address a browser came in on must be
kept only if the operator reaches the console again within the trial.
`base/netd`'s trial keeps a change as soon as each interface it
changes has carrier and an address (`trial.rs`), so a static address
the operator cannot reach would be kept. The change, for QS/VS: an
apply that is kept only by `confirm` within the trial and taken back
otherwise, whatever the interface says. Until it is there, the Network
settings page refuses a change to the `open` interface's address, mask
and gateway with `QMC8104` (`BLOCKED(phase3)`); the local console's
option 3 changes them.

## 25. Fields the Network settings page does not have

Found in Phase 3. The handoff asks for DHCP or static, a VLAN and
whether the web interface also answers on `private`; the prototype's
page has none of them. Cody's decision for this phase: an empty IP
address is DHCP; the VLAN and listening on `private` stay as
`network.toml` and `web.toml` have them, set from a seed or at the
local console. The page also offers a domain, which `network.toml`
does not keep, a DHCP server and SSH: each answers `QMC8700`. To
decide: the fields in the design, and whether the domain joins
`base/state`.
