# QS/MC V1R1M2 release notes

QS/MC, the Quartz Systems/Management Console, is the appliance from
which an operator manages QS/VS hosts with their QS/400 partitions, and
standalone QS/400 systems. It is its own small operating system, on its
own machine or virtual machine, managed from a web browser and, for the
basics, a local console on its display and serial port. It reaches the
systems it manages over QSI on TCP 2300.

V1R1M2 is a fix release of V1R1M1. A console on V1R0M0, V1R1M0 or
V1R1M1 takes it as an update from the `qsmc` stream or a removable
volume, its state kept; a new console installs it from its ISO. It needs
nothing new of the QS/VS hosts or QS/400 systems it manages.

## Fixed

- **The terminal window showed its keypad at first**, taking room from
  the screen. It is hidden until the Keypad button shows it, or the
  window is opened with `?pad=1`.
- **The terminal window's breadcrumbs were not links.** Each now leads
  back: Partitions, the system, the partition.
- **Reference code history showed the prototype's columns of words**
  (`WORD2` to `WORD9`) and no description. It is now a list: the time,
  the code linked to its explanation (which keeps the words the system
  reported), what the code says, and who posted it.
- **A standalone QS/400 system showed no processors or memory** in the
  Partitions list. It shows what the system reports of itself.
- **The local console named the build after the oldest tag**
  (`v1.0.0-19-g…`). A build is now named by its commit.

## What it needs

| | |
|---|---|
| Processor | x86_64 with AVX2, ADX, AES and PCLMULQDQ (the web interface and QSI need them). In a virtual machine, give it the host's processor model |
| Firmware | UEFI. Secure Boot is optional: a release's images are signed for it with `qsmc-db.cer`, which an operator enrols to have the firmware check them (`docs/INSTALL.md`) |
| Memory | 2 GiB or more |
| Disks | one of at least 16 GiB, or two for a mirrored load source; 64 GiB or more is what a console wants. SATA, SAS, NVMe or virtual (virtio, VMware, Hyper-V) |
| Network | one Ethernet port for `open`, where operators reach the console; an optional second for `private`, toward the managed systems |
| Ports | the console listens on TCP 443 (HTTPS) and 80 (redirected to 443) of its `open` address, and nothing else. It connects out only by DHCP, DNS, NTP, QSI to the systems it is paired with (TCP 2300 unless a system says another port), and HTTPS to the configured product stream |
| A browser | any current browser. Two scripts: the terminal window's, and the navigation script that filters, sorts and pages lists in place. Every page works with script off |

## What it manages

| System | How far |
|---|---|
| QS/VS V1R0M0 hosts | wholly: the host, its partitions and their profiles, its virtual resources, its events and dumps, its code updates, system plans |
| An older QS/VS host, whose hello lacks a capability | everything its hello offers; a page that needs what it lacks says so (`QMC1114`) and sends nothing |
| Standalone QS/400 systems | paired, their console and control panel, their status and dumps, their codes and events. Their fixes are compared with the `qs400` stream; they are installed on the system itself |
| QSI | version 1, mutual TLS 1.3 with both ends pinned, pairing by the system's one-time password |

## Known limits

- **Power on.** A managed system that is off cannot be powered on from
  the console: V1 has no page for a machine's BMC (`QMC2201`).
- **Standalone systems' and partitions' fixes** are shown against the
  `qs400` stream and installed on the system itself, not by the console.
- **What the design offers that V1 does not:** a backup copied to a
  remote host or scheduled; the console as a time server or a DHCP
  server; SSH; languages other than English; renaming a managed system;
  system profiles; fetching an image by URL; pass-through tape drives; a
  partition's firmware and console adapters; a plan that changes what it
  finds. Each answers with a message saying it is not in V1
  (`docs/DESIGN_REVIEW.md` has every one).
- **The sign-on notice** is set only in `web.toml` or a seed; no page
  sets it.
- **The network settings page** has no VLAN or `private` listening
  fields; they are set from a seed or at the local console.
- **The local console** is not confined by the file-system rules every
  other service is: its option 8 mounts a removable volume. It keeps its
  system call filter (`docs/SECURITY.md`).
- **Check now** reads the three streams at once and waits for the
  slowest, up to 30 seconds, when a server is slow.
- **A device for partitions** is released at the QS/VS host (Service
  Tools option 10), not from the console: QSI reads a host's devices
  only. The I/O tab says where.

## Reporting a problem

1. Note the reference code the console shows (the status screen's
   Reference row, or Serviceable events), and its problem's number.
2. Take a console dump: at the local console, option 8 and then 2
   (Take a console dump), or from Manage dumps. A dump holds the console's logs, state and the codes it
   posted, and no password, hash, key or passphrase.
3. Download the dump from Manage dumps, or copy it to a removable volume
   at option 8 and then 3 (Work with dumps), and send it with the code and what was being done.

`docs/REFCODES.md` explains every code of the console and what to do.

## For developers

What this release takes from QS/VS and QS/400 as stand-ins, because the
last pins (QS/VS `c65646262a73`, QS/400 `de8e467f066f`) do not have the
change; each is an open question there:

| Item | Stand-in here | The change asked |
|---|---|---|
| 35 | the base's first line and stopping words in the power confirmation | the words in `PowerWords`, the machine in `units::Product` (QS/VS) |
| 36 | `tools/build/src/host/fuzz.rs` builds the fuzzer, a workspace of its own | a manifest path in `buildaid::fuzz::Fuzzer` (QS/VS) |
| 39 | the harness's own sign-on request | `httpc` naming a request's `Host` (QS/400) |
| 40 | `parts-upload` sends the first parts only | `qsweb` answering a streamed request before its next read (QS/400) |
| 41 | `lic/qsmcinit/src/ipl/carry.rs` copies `mgmt`, `web`, `fetch` and `plans` beside the state | `update::state_copy` over a list of folders (QS/VS) |
| 42 | `svc/console/src/service/volume.rs` mounts option 8's volume | a removable volume in `base/panels` (QS/VS); then the local console under Landlock |
| 49 | `webd::serve::framing_refused` refuses a body said two ways | `qsweb` refusing it (QS/400) |
| 50 | `lic/sandbox` confines QS/MC's services with Landlock beside `base/confine`'s filter | Landlock in `base/confine` (QS/VS) |
