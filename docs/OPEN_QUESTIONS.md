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

## 6. The QEMU job in CI (closed in Phase 5)

`.github/workflows/qemu.yml` runs on GitHub's hosted runners, which have
KVM (4 processors, 15 GB). One job builds the images and the build tool;
eight shards run the suites on them side by side, and one more the ISO's
steps. The first runs found QEMU 8.2 there dropping a monitor command
sent as it starts (the `sockets` suite's capture), which the harness now
sends again. The suites on a real QS/VS host or QS/400 stay on the build
machine: CI has neither system's images.

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

## 8. The ISO's name (closed)

`buildkit` names the ISO `<VENDOR>-<release>-<hash>.iso`, so it is
`QSMC-V1R0M0-<hash>.iso` beside `qsmc-V1R0M0-...` for the UKIs and the
disk image. QS/VS accepted the same for its own (its item 5). The name
is `buildkit`'s.

**Closed in Phase 8:** `build release` keeps `buildkit`'s name in
`target/release/<VRM>/` and the stream, as QS/VS's release directory
keeps `QSVS-V1R0M0-<hash>.iso`.

## 9. Secure Boot (closed)

The UKIs are not signed for Secure Boot in Phase 0: `buildkit`'s
manifest records no db certificate (`Product::secure_boot_db` gives
none). QS/400's `pesign` and QS/VS's `build release` show how; that is
Phase 8's.

**Closed in Phase 8:** `build release` signs the three UKIs with
QS/MC's db key (`QSMC_SB_KEY`, `QSMC_SB_CERT`) before the disk and the
ISO are made of them, and puts `qsmc-db.cer` on the media and in the
release; the installer's summary and the status screen say whether
Secure Boot is on; `docs/INSTALL.md` says how to enrol it, and booting
does not need it. `sec-secure-boot` boots a release under OVMF's Secure
Boot build with the certificate enrolled, and is refused without it.

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

## 13. A mirrored unit absent at the IPL is copied whole when it returns (QS/400, QS/VS) (closed)

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

**Closed in Phase 5:** QS/400 `7e65f618e` starts the map of a member
absent at a clean open; QS/VS `4053b26` takes it in `base/stgd` and its
`mirror-degraded` suite requires the disk back by its map. QS/MC took
both with its pins (`d2312d9`); its `mirror` suite asserts the member
absent at the IPL comes back by its map (`docs/PHASE5.md`).

## 14. A degraded IPL waits about twenty seconds for the missing disk (QS/VS) (closed)

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

**Closed in Phase 5:** QS/VS `ced5a19` looks for the load-source disks
once a start and passes what it found on; QS/MC took it with its pins
(`d2312d9`).

## 15. The installation media's check is QS/VS's own (QS/VS) (closed)

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

**Closed in Phase 5:** QS/VS `49b117c` (its P7) gave
`loadsource::payload` the media's search by volume identifier, its
mount and its check (`open`, `verify`, `signed`, `matches`). QS/MC's
`lic/qsmcinstall/src/media.rs` is deleted and the installer calls
`payload::open` (`50e9e34`). The build's writing of the payload stays in
`tools/build/src/product/media.rs`: `base/` has none.

## 16. The disk unit panels are QS/VS's own (QS/VS) (closed)

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

**Closed in Phase 5:** QS/VS `49b117c` (its P8) moved the panels into
`base/panels::units` behind `LoadSource` and `Product`. QS/MC's
`svc/console/src/disks.rs` is now its `Product` (no options of its own,
its help) and `LoadSource` (`50e9e34`). The panels are laid out as QS/VS's
were, so their golden screens changed (item 35, `docs/PHASE5.md`).

## 17. Words of `base/panels` that are a host's (QS/VS) (closed)

Found in Phase 2. `panels::tools::power` says "Every partition that runs
on this host ends with it" and its help "Partitions that run on the host
end with it": not so of a console. QS/MC takes `panels::tools::Confirm`
and gives its own words (`svc/console/src/menu.rs`). The change, for
QS/VS: `power` takes the lines about what ends with the machine from its
caller. The rest of the base help says "the host" where it means the
machine, which reads true of a console too, and is left.

**Closed in Phase 5:** QS/VS `49b117c` (its P10) gave
`panels::tools::power` the product's `PowerWords`: what ends with the
machine and the help. QS/MC's own confirmation is gone (`50e9e34`). The
first line and the stopping screen still say "The host", as do two
lines of `panels::units`' watch: item 35.

## 18. Writing a load source for an installer is QS/VS's own (QS/VS) (closed)

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

**Closed in Phase 5:** QS/VS `49b117c` (its P9) added
`install::write`, the eight steps for a product's names and codes.
QS/MC's `Target` is now its words, its disks and links, and the
writer, with one step of its own after the state step: the installer's
log copied to `/data/log/install.log` (`50e9e34`).

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
- Backup and restore: "Schedule...", which needs a destination other
  than the browser; "Reference code and event history", which is the
  log service's and is not restored over a running one; and "Virtual
  optical media library index", which Phase 5 brings. A backup always
  holds the console data: unchecking it answers that it does.
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

## 22. `qsweb`'s help window closes by script (QS/400) (closed)

Found in Phase 3. The help window's Close window link carries an
inline `onclick` handler. QS/MC serves no script, and its content
security policy blocks inline handlers, so the link does nothing there
but go to `#`. The change, for QS/400: a plain link when the product
serves no script (`Product::nav_script` false).

**Closed in Phase 5:** QS/400 `8d69937bb` closes the window by a plain
link to the product's start when it serves no script. The help page's
golden correction is that link (`docs/WEB.md`, `d2312d9`). The same
commit gave `qsweb` the grouped action list and carried fields, which
the Servers list and the network confirmation now use (`50e9e34`).

## 23. The users' forms are not designed

Found in Phase 3. On `console-users.html` Properties and Change
password lead to the list itself and Create user... is a bare button:
the prototype has no form for any of them. QS/MC shows each under the
list as `qsweb`'s properties pattern (`?edit=`, `?password=`, after
Create user...), with the password rules stated; the list is the
golden. To decide: designed pages for the three.

The same holds for the Restore tab of `console-backup.html`, which the
prototype links and does not draw, and for View on
`console-certs.html`, which leads to the page itself: the upload with
what the backup holds, and the certificate's every field, are
`qsweb`'s properties under the page's own.

## 24. A network change kept only when confirmed (closed)

Found in Phase 3. A change to the address a browser came in on must be
kept only if the operator reaches the console again within the trial.
`base/netd`'s trial keeps a change as soon as each interface it
changes has carrier and an address (`trial.rs`), so a static address
the operator cannot reach would be kept. The change, for QS/VS: an
apply that is kept only by `confirm` within the trial and taken back
otherwise, whatever the interface says. Until it was there, the Network
settings page refused a change to the `open` interface's address, mask
and gateway, and the local console's option 3 changed them.

**Closed in Phase 3:** QS/VS `ff181be` added `apply_confirmed`, kept
only when `confirm` is called in its window; QS/MC moved its QS/VS pin
to `3de567e`. A change of the `open` address is said on a confirmation
page, put on a 120-second confirmed trial, and kept when someone signs
on at the new address in time.

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

## 26. `httpc` has no pinned mode (QS/400) (closed)

Found in Phase 3. The harness's browser must trust only the certificate
it was told to expect. QS/400's `httpc` trusts its shipped roots or,
tolerating, anything; it cannot be told one certificate. The harness
therefore writes its requests itself over rustls with `webd`'s pinned
verifier (`tools/conformance/src/web`). The change, for QS/400: a
pinned SHA-256 on `httpc::Request`; then the harness's requests are
`httpc`'s.

**Closed in Phase 5:** QS/400 `108adc963` made `httpc` neutral with the
caller's trust, `Trust::Pinned` among it. The harness's requests to 443
are `httpc`'s (`50e9e34`); what it does not do stays in
`tools/conformance/src/web`: a request to 80 that names the guest as a
browser would (`httpc` names the URL's authority), a hostile client's
bytes, and the WebSocket.

## 27. PKCS #12 iteration counts are not bounded (QS/400) (closed)

Found in Phase 3. `x509kit`'s PKCS #12 reader takes the MAC's and the
key's iteration counts from the file, at least 1 and with no upper
bound. A file asking for four billion holds the web server thread that
reads it for a very long time. Only a signed-on Administrator can
upload one, and the other sessions go on, so it is not a hole; but the
reader should refuse a count past what any exporter uses (a few
hundred thousand), as `qmcbackup` refuses an Argon2 cost past its own.
The change belongs in `x509kit`. The fuzz target `cert-import` does
not seed a PKCS #12 file, so it does not stop at this.

**Closed in Phase 5:** QS/400 `4c4f5102d` refuses a count past
1,000,000 (`x509kit::pkcs12::pbe::MAX_ITERATIONS`) and fuzzes the
reader with a PKCS #12 corpus (`7a7455f9a`); QS/MC took it with its
pins (`d2312d9`).

## 28. No line for a role the network service no longer has (QS/VS) (closed)

Found in Phase 3. When `private` is removed from `network.toml`,
`base/netd` stops configuring it but notes no line for the role, so
PID 1's `private` fact, the status screen's row and Console settings
keep the last line it had (`192.168.250.1/24 on eth1`) until the next
IPL. The change, for QS/VS: a note for each role a change removes
(`none`, or the reason). QS/MC's `web-network` suite reads the kernel's
addresses instead.

**Closed in Phase 5:** QS/VS `b12c2b4` notes a line for each role a
change removes; QS/MC took it with its pins (`d2312d9`).

## 29. Power on and a machine's BMC

Found in Phase 4. A powered-off machine does not answer QSI, and the
design has no page that gives the console a machine's BMC (its address,
its protocol, its credentials). In V1 Power on answers `QMC2201`; its
confirmation page is drawn as the prototype has it. The outbound rule
already names the BMC of a machine the console is asked to power on.
What is needed: where an operator gives a managed system's BMC, which
protocol the console speaks to it, and where its credentials are kept.

## 30. A host's power-on parameters (closed as far as QSI goes)

Found in Phase 4. Power-on parameters shows what a QS/VS host reports
(its slot from base `State`, its keylock). QSI cannot set any of the
page's fields on a host: the IPL side (the host's slots are its A/B
record, changed by an update's activation), the keylock, the boot mode,
which partitions start (each partition's auto start is the partition
family's `Change`, a partition's page in Phase 5), the default power
off. Apply and OK answer `QMC2220`. A later QSI version could carry the
host's own boot settings, or the page could lose the fields.

**Phase 5:** QSI's bit 23 (`caps::POWER_ON`, QS/VS `49b117c`'s A7) reads
and sets the two a host keeps: the partitions marked for auto start
started with it ("Power on partitions") and how its partitions end
when it powers off ("Default power off", delayed with its wait or
immediate). The page shows them as the host says and sets them with
`QMC2221`; a host without the bit answers `QMC1114`. The IPL side,
keylock and boot mode are shown and answer `QMC2220` when changed: a
host's slot moves only by an update's activation, and QSI sets neither
its keylock nor its boot mode.

## 31. The service password age

Decided in Phase 4: the design's "service password age" is the time
since the system was paired, and past 90 days the page shows its warning
(`(rotate)`); Reset connection pairs again, which is what Change
password leads to. QSI pairs once with a one-time password and keeps no
password, so nothing ages but the pin. Whether a pin should expire, and
whether the console should say so on Welcome, is open.

## 32. What the prototype offers that V1 answers with a message (Phase 4)

As item 19, for this phase's pages: a host's name (`QMC1008`: it is the
system's), its power-on parameters (item 30), Power on (item 29),
Servers' actions that later phases bring (Schedule operations, Create
partition, Manage partition profiles, Manage system profiles, Virtual
resources, Manage dumps: `QMC9030`), a standalone system's console
(`QMC3004`, until Phase 5: QS/400 had no QSI listener), and the terminal toolbar's
Copy, Paste, Keypad and Keyboard Map (`QMC8700`; the script has the
keyboard and keypad). The screen size and code page of Open terminal
window are the system's, not chosen by the console.

## 33. The control panel's list is the design's, not a host's (closed)

Found in Phase 4. The design lists functions 01 to 70 with their texts;
a QS/VS host carries out 01 to 03, 08, 11 to 22 and 34
(`docs/PANEL.md` in QS/VS), and refuses the others with its own
message (`QVS2306`). The page sends what is chosen and shows the
refusal, as the handoff asks. Whether the list should show only what the
system says it has (a capability of the panel family) is for the
design and QSI.

**Closed in Phase 5:** QSI's bit 24 (`caps::PANEL_FUNCTIONS`, QS/VS
`49b117c`'s A8) lists a system's functions with their texts, which need
the keylock in manual and which need confirmation. The page lists them
as the system does, and a function it marks for confirmation goes
through the confirmation page; a system without the bit is shown the
design's list, with 08 and 22 confirmed. The golden is the prototype's
page with the fixture's list, which is the design's.

## 34. A real host's partition needs a real machine

Found in Phase 4. Suite 10's partition steps (codes as they happen, the
console at QS/400's sign-on, function 21) need a QS/400 partition in a
QS/VS host that runs in a second QEMU beside the console's: a third
level of virtualization, and a QS/400 LIC image built for it. The build
machine (WSL 2, 12 processors, 11 GiB) runs the host and the console;
the partition steps are listed for a real machine (`NEEDS-CODY`, the
Dell).

**Phase 4:** answered: the build machine runs them. The host's QEMU
is given 5 GiB and six processors, the partition runs under it, and
suite 10's partition steps passed there against a host at QS/VS
`8c54d47` and QS/400 at `cb58b01` (`docs/PHASE4.md`, Evidence). The
Dell remains for a host on real hardware.

## 35. Words of `base/panels` that are still a host's (QS/VS)

Found in Phase 5, taking `panels::units` and `panels::tools::power`
(items 16 and 17). `PowerWords` gives what ends with the machine and the
help, but the confirmation's first line ("The host will shut down in
order and restart.") and the stopping screen ("The host is
restarting.") are the base's, and the watch of a unit's work says
"restart the host from the main menu" and "while the host runs". On a
console they read wrong. QS/MC takes them as they are, and their golden
screens changed with the units' wider columns and the `Other disks`
line (`docs/PHASE5.md`, Step 0). The change, for QS/VS: the first line
and the stopping words in `PowerWords`, and `units::Product` naming the
machine in its own word.

## 36. `buildaid`'s fuzz runner builds only a member of the workspace (QS/VS)

Found in Phase 5. `buildaid::fuzz` builds the fuzzer with `-p` from the
repository's root and runs it as `list` and `run NAME --secs`.
QS/MC's fuzzer is a workspace of its own (`tools/fuzz`): QS/400's
fuzzer brings QS/400's `release` crate, which has the name of QS/MC's.
So `tools/build/src/host/fuzz.rs` (52 lines) stays. The change, for
QS/VS: a manifest path in `buildaid::fuzz::Fuzzer` for a fuzzer outside
the workspace.

## 37. The pages built without a design (Phase 5)

Found in Phase 5. The prototype links to these and does not draw them;
each is the shared pattern of `STYLE.md` it is closest to (a properties
form, a short form, a confirmation), with no new style and only the
fields of its requests. They are for the design to draw:

| Page | Route | Pattern | Fields |
| --- | --- | --- | --- |
| A switch's create and properties | `/server/<h>/virtual/ethernet/switch?n=` | properties | name, VLAN IDs, MTU, uplink interfaces, bond, native VLAN |
| A storage pool's create and properties, Extend | `/server/<h>/virtual/storage/pool?n=&extend=` | properties, the free disks as a list | name, protection, disks and spares; a pool's state, units, the media repository |
| A disk that carries another system's label | the pool form's confirmation | confirmation | the disks named |
| A virtual disk's properties and resize | `/server/<h>/virtual/storage/resize?p=&n=` | properties | its size (a disk only grows) |
| Load or unload an image | `/server/<h>/virtual/optical/load?n=` | short form | the partition's device |
| Tape volumes, a device's volume | `/server/<h>/virtual/tape/volume?p=&s=` | list and short form | a new volume's name, identifier, capacity; protect; the device's volume |
| A processor pool's change | `/server/<h>/virtual/pools/change?id=` | short form | name, cores, maximum units |
| A partition's Other tab | `/partition/<h>/<id>/general?tab=other` | properties | serial number, created, default profile, start with the host |
| A virtual adapter's create and edit, a physical adapter added | `/partition/<h>/<id>/edit?s=&new=&io=` | properties | Ethernet: switch, VLAN, tagged VLANs, MAC, required; SCSI client: the disks; physical: a released device |
| A profile's copy, create, save current, set default | `/partition/<h>/<id>/profiles?copy=` and its buttons | short form | the new name; the default |
| The wizard's adapter | `/partitions/create/step/6?edit=` | short form | type, adapter ID, switch and VLAN or disk, required |
| A group's edit | `/groups/create?g=` | `group-create.html` with the group's values | |
| A scheduled operation's change | `/schedule/add?id=` | `schedule-add.html` with its values | |
| The confirmations of Delete partition, Remove adapter, Delete profile, the virtual resources' deletes, Remove and Run now of a scheduled operation, Delete group | `/confirm/<what>` | the shared confirmation | what is affected; Delete partition: its disks too, as chosen |

## 38. What the prototype offers that V1 answers with a message (Phase 5)

As items 19 and 32, for this phase's pages: Fetch an image from a URL
(`QMC2405`), Add pass-through drive (`QMC2435`: a drive is released at
the host), Open firmware (`QMC3123`), a physical console adapter
(`QMC3124`), the console's serial adapters changed (`QMC3125`), a
scheduled Power on (`QMC2201`), Backup console data (`QMC3407`) and
Install code update (`QMC3408`) as scheduled operations, the
partitions' Manage dumps (`QMC9030`, Phase 6), Extend of a QS/VS pool
(the host's own refusal, `QSI1616`), and anything a host's hello does
not offer (`QMC1114`). The values QSI has no field for are left empty
or derived and said so in `docs/VRES.md` and `docs/PARTITIONS.md`: an
image's upload date and read-only flag, a switch's mode, a pool's type
and backing devices' words, the confirmation's active jobs and users
signed on, the SCSI client's server adapter number.

## 39. `httpc` names only its URL's host (QS/400)

`httpc` writes `Host` from the request's URL and adds a caller's
`Host` beside it. The harness reaches a guest through a forward on
127.0.0.1, and the console keeps an `open` change on trial only when a
sign-on names the address it moved to, as a browser of that address
does (`web/pages/src/app/routes.rs`, `reached`). So the harness's
sign-on is still its own request over the pinned TLS
(`tools/conformance/src/web/mod.rs`), and every other request to 443 is
`httpc`'s. For QS/400: a request's `Host` header, when given, in place
of the URL's, or an address to connect to beside the URL.

## 40. A streamed request's answer waits for the next read (QS/400)

`qsweb::http::guard::handle_stream` sends a page's answer only when its
pump's next read of the body returns. A page that refuses from the
parts before the file (an upload past 64 GiB, `QMC2404`) has answered,
but a client that sends nothing more waits for the read timeout, 30 s,
before it sees the answer. A browser goes on sending, so it is told at
once; `parts-upload` sends only the first parts and records the time.
For QS/400: the pump stops reading when the handler has answered.

## 41. `update::state_copy` takes one folder (QS/VS)

`base/update`'s `state_copy` keeps a copy of the state directory before
a pending slot's first IPL and puts it back on a roll back. A console's
state is also in `/data/mgmt`, `/data/web` and `/data/fetch`, each its
own service's folder with its own owner, so PID 1 copies those beside
it itself (`lic/qsmcinit/src/ipl/carry.rs`), the same rule over three
more folders. For QS/VS: `state_copy` over a list of folders, each
copied with its owner and mode; the console's own then goes.

## 42. A removable volume belongs in `base/panels` (QS/VS)

Option 8 of the local console mounts a removable volume to read an
update image from or copy a dump to (`svc/console/src/service/volume.rs`):
its device by kernel name, never a load-source disk, the file systems
tried in turn, read-only to read, no device files, set-user-ID or
execution. QS/VS's Service Tools will want the same for a host. For
QS/VS: the volume in `base/panels` (or `base/loadsource`, which knows
the load-source disks); the console's then goes.

## 43. Priority on Serviceable events (design)

The prototype's event detail has Severity and Priority; QSI has a
severity and no priority. The console derives a resource's priority
from the severity (`docs/SERVICE.md`) and shows what the prototype
shows for event 1042. For the design: Priority dropped, or a field QSI
would carry.

## 44. Check now waits on the slowest stream (Phase 6)

Check now reads the three streams in turn within the page's request,
each given 30 s, so a server that is slow but not absent holds the page
for up to 90 s before it says `QMC7204`. A browser waits; the harness's
does not, and posts it from a thread of its own (`serv-streams`). A
check begun in the background with its result on the page, as a
download is, would answer at once.

## 45. The pages of system plans built without a design (Phase 7)

As item 37, for Phase 7. The prototype has no page for these; each is
the shared pattern closest to it, with no new style:

| Page | Route | Pattern | Fields |
| --- | --- | --- | --- |
| A plan's description and Save as template | `/plan/<name>/edit`, linked from View as Edit | properties | the description; the template's name |
| Delete a plan | `/confirm/plan-delete?p=` | the shared confirmation | the plan, its description, managed system and date |
| Deploy a plan | `/confirm/plan-deploy?p=&t=&c=&act=` | the shared confirmation | the plan, the managed system, the partitions to create, whether they are activated |

For the design to draw, and to say whether Edit belongs on View or in
the list's actions.

## 46. Validation of a partition on another host (design)

The prototype's step 2 shows `QSDEV01` and `QSTEST01` OK when the plan
of `QSHQVS1` is deployed to `QSHQVS2`. Phase 7's decisions make a
partition whose MAC addresses and serial number will be new a Warning,
and the console says so (`docs/PLANS.md`, "Against the prototype").
Step 3 still ticks such a partition, as its only Warning is its
identity. The prototype's table also lists eight items where the
console lists every item of the plan. For the design: the rows as the
console has them, or the identity said elsewhere (on the managed
system's row, or in the confirmation).

## 47. Every prototype file not live (Phase 7)

86 of the prototype's 90 files are live. The four that are not are
Navigator for QS/400's pages that QS/MC links to and that the handoff
lists as reference only (`nav/active-jobs.html`,
`nav/end-job-confirm.html`, `nav/job-props-general.html`,
`nav/ptfs.html`): they are QS/400's own pages, served by a partition's
Navigator, not by the console. Updates' partition rows link to
`ptfs.html`'s place on the partition (`docs/UPDATES.md`). Nothing waits
on this repository.

## 48. What V1 answers with a message, gathered (Phase 7)

Items 19, 32 and 38 and this phase's, in one list: each choice the
prototype draws that V1 answers with a `QMC` message, the page drawn as
the design has it. To decide, for each, whether it joins the locked
decisions in a later release or the design drops it.

| Where | Choice | Answer |
| --- | --- | --- |
| Backup and restore | Remote host (SFTP); Schedule...; Reference code and event history; Virtual optical media library index; unchecking Console data | `QMC8405`, `QMC8700` (item 19) |
| Date and time | Act as NTP server on the private management network | `QMC8700` (item 19) |
| Network settings | DHCP for service interfaces; SSH to console | `QMC8700` (item 19) |
| Language and locale | a language other than English | `QMC8700` (item 19) |
| A managed system's General tab | its name | `QMC1008`: the system's own (item 32) |
| A managed system's Power-on parameters | the fields QSI has none for | left as the host says them (item 30) |
| Servers, a managed system | Power on | `QMC2201` (item 29) |
| Servers | Manage system profiles | `QMC8700`: a QS/VS host has none (Phase 7; it said `QMC9030` before) |
| The terminal window's toolbar | Copy, Paste, Keypad, Keyboard Map | `QMC8700`: the script has the keyboard and keypad (item 32) |
| Virtual optical | Fetch an image from a URL | `QMC2405` (item 38) |
| Virtual tape | Add pass-through drive | `QMC2435`: a drive is released at the host (item 38) |
| A partition's settings | Open firmware; a physical console adapter; the console's serial adapters changed | `QMC3123`, `QMC3124`, `QMC3125` (item 38) |
| Add scheduled operation | Power on; Backup console data | `QMC2201`, `QMC3407` (item 38) |
| Virtual storage | Extend of a QS/VS pool | the host's own refusal, `QSI1616` (item 38) |
| Any page | a request the host's hello does not offer | `QMC1114` (item 38) |
| Deploy system plan, step 1 | Replace matching partitions | `QMC6108`: a deployment never deletes or changes what exists |
| System plans | Create plan from a standalone QS/400 | `QMC6105`: it has no partitions to plan |

The partitions' and Servers' Manage dumps, which items 32 and 38 listed
as `QMC9030`, lead to Manage dumps from Phase 7. No function of the
tree says it arrives in a later release (`QMC9030`) any more.
