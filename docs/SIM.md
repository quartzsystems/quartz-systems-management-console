# The simulated managed systems

`tools/sim` (crate `sim`, binary `qsmcsim`): managed systems for tests.
Each is a QS/VS host or a standalone system on QS/400's `qsisess` (its
managed side) and `qsitrust`, speaking real QSI over TLS 1.3 on a real
socket, as a QS/VS host's server does: its own identity, its pins, a
pairing password good for 15 minutes and five attempts, an event log,
one viewer of a partition's console, an upload's stream. It is a host
tool, in no image.

## The fixture

The prototype's data (`design/qsmc-prototype/README.md` and its pages):

| System | What |
| --- | --- |
| `QSHQVS1` | QS/VS host, `9Q10-V1`, serial `10-9F11A2`, 8 cores (3.50 available), 131,072 MiB (40,960 available), memory block 256 MiB, MAC prefix `02:51:53`; partitions `QSDEV01` (1, running, shared uncapped), `QSTEST01` (2, not activated, `A6005008`), `QSPROD01` (3, running, dedicated); everything below |
| `QSHQ400` | standalone, `9Q04-M2`, serial `10-7A2B9C` |
| `QSBR01VS1` to `QSBR25VS1` | the branch hosts of the paged lists: their available processors, memory, serial numbers and partition counts as the prototype has them, each partition with one profile `NORMAL`; 4, 11, 18 and 25 starting (Standby); 6, 13 and 20 with `B6000100`, which the suites take down (No connection) |

`QSHQVS1`'s partitions and profiles (the partition pages,
`manage-profiles.html`, `activate.html`):

| Partition | Profiles (revision; last activated) | Holds now |
| --- | --- | --- |
| `QSDEV01`, `Development partition`, auto start, since 10/30/26 06:02:14 | `default` (7, default; 10/30/26 06:00): shared 0.25/1.00/2.00 units, 1/2/4 virtual, uncapped 128, 8,192/16,384/32,768 MB; `install` (3; 10/02/26 09:15): 0.25/0.50/1.00 capped, 8,192 MB, IPL from D; `big-test` (5; 10/14/26 18:40): 1.00/2.00/3.00 capped, 32,768 MB | 1.00 units, 2 virtual, 16,384 MB |
| `QSTEST01`, `Test partition` | `install` (4, default): 0.50/1.50/4.00 capped, 8,192 MB, IPL from D attended; `default` (2); `test-small` (1) | its IPL from D stopped at `A6005008`: still 1.50 units and 8,192 MB, as the processors page counts it |
| `QSPROD01`, `Production partition`, since 10/12/26 05:30 | `prod` (9, default): dedicated 1/2/4, 32,768/65,536/98,304 MB, the `C3` adapter as physical I/O | cores 0 and 1, 65,536 MB |

Their adapters, slot by slot (`partition-vadapters.html`): Ethernet 2 on
`ETHSW0` (VLAN 20, 20, 10) and 4 on `ETHSW1` (VLAN 99), MACs
`02:51:53:00:<id>:<slot>`; SCSI 3 (`VD03`, `VD04`, `VD01` and `VD02`),
`QSPROD01`'s SCSI 4 (`VD05`); optical 5; tape 6 (`VTAP01`, `VTAP02`,
`QSPROD01`'s physical drive). Loaded now: `QSDEV01`'s optical has
`HLINV_SAVF_20261028.iso`, its tape `VTAP01` (removal prevented);
`QSTEST01`'s optical the installation image.

Virtual resources (the `vr-*.html` pages and `create-vdisk.html`):

| What | Values |
| --- | --- |
| Storage pools | `POOL1` raid6, 6 × 1.92 TB SSD (`sdc` to `sdh` on `C2`), 5,400 GiB, 2,870 allocated (2,530 free); `POOL2` none, `sdb` 8 TB HDD, 7,450 GiB, 1,200 allocated (6,250 free), the media repository |
| Disks for a pool | `sda` the load source, the pools' units, `sdi` not configured, `sdj` another host's (foreign: a create names it in `confirm`) |
| Virtual disks | `VD01`, `VD02` 400 GiB thick (`QSPROD01` slot 3), `VD03` 120 thin (`QSDEV01` 3), `VD04` 120 thin (`QSTEST01` 3), `VD05` 2,000 thick in `POOL2` (`QSPROD01` 4), `VD06` 80 thin, unassigned |
| Images | `QS400_V1R0M0_INSTALL.iso` 4,120 MiB (`QSTEST01` 5), `QS400_CUM_26290.iso` 412, `QS400_LICPGM_V1R0M0.iso` 2,840, `HLINV_SAVF_20261028.iso` 96 (`QSDEV01` 5), `blank_4700MB.iso` 0 |
| Tape | volumes `VTAP01` (`QSDEV01` 6) and `VTAP02` (`QSTEST01` 6), 200 GiB each; the physical drive `U9Q10.V1.9F11A2-P1-C3-T1-L0` behind `C3`, held by `QSPROD01` slot 6 |
| Switches | `ETHSW0` (0) VLANs 10, 20, 30, uplink `C1-T1` (10 GbE, up); `ETHSW1` (1) VLAN 99, internal; limits 16 switches, 256 VLANs, 8 adapters, 256 ports |
| Processor pools | `DefaultPool` cores 2 to 7, 6.00, 2.50 entitled; `TESTPOOL` cores 6 and 7, 2.00, unused (both as the pages show them: the prototype's eight cores do not hold both) |
| Hardware | the PCI slots as the I/O page shows them, then the interfaces (`C1-T1`, `C1-T2`, `C4-T1` the management interface, `C4-T2` to `T4`) and the disks |
| Devices (physical I/O) | `C3` released, held by `QSPROD01`; `C1`, `C2`, `C4` the host's; `T1` not released |

The prototype's names that QS/VS's rules would refuse are taken as they
are: lower-case profile names with `-` (`big-test`) and media names
with `.` (`.iso`).

`QSDEV01`'s console shows the recorded screen
(`screens/wrkactjob.screen`): Work with Active Jobs, the prototype's
terminal picture as console cells, its status row naming `QSHQ400`.
Drawn by QS/400's look it is the prototype's PNG byte for byte (a test
of the crate). A key typed is kept, and a character is put at the
cursor and sent back as an update.

Identities are made from a seed of the system's name, so they are the
same each run; `renew` gives another.

## What it answers

A refusal is QS/VS's message (`docs/MESSAGES.md` of QS/VS), led by
QSI's identifier where QSI names the conflict, as QS/VS's server leads
it; a profile's validation gives every message at once.

| Family | Answers, and refuses |
| --- | --- |
| base | identify, state, power, the codes by cursor, subscribe, time |
| host | resources, settings, hardware, devices, power-on settings (`QVS1022`) |
| partitions | list, get, adapters of every profile, activation times; `QVS2001` |
| partition | create (lowest free ID, serial `K2M41nn`; `QVS1001`, `QVS1005`, `QVS2006`), create at an ID (`QSI0362`/`QVS2016`, `QSI0363`/`QVS2017`), change (`QVS2002`, `QVS1006`, `QVS1007`), delete with or without its disks (`QVS3108`), activate (below), shut down, restart (`QVS2003`, `QVS2009`, `QVS2301` with no channel), entitlement (`QSI1401` to `QSI1405` then `QVS2501` to `QVS2505`), settings; `QSI0361` for an enumeration value it has not |
| profiles | list, get, put at a revision (`QSI0401`/`QVS2013` stale, `QSI0402`/`QVS2012` a name there; MACs made), delete not the default (`QVS2007`), set default, save current (`QSI0403`/`QVS2015`), activated; validation `QVS1101` to `QVS1226`, `QVS2401` |
| vres, pools | pools, disks, create (`QSI1601` to `QSI1611` then `QVS3001` to `QVS3017`), delete (`QSI1612`), rename (`QSI1613`), replace a failed unit (`QSI1615`; rebuilt at once), spares (`QSI1610`), designate (`QSI1618`), extend always `QSI1616`/`QVS3019` |
| vres, virtual disks | list, create thin or thick, grow, delete (`force`), named; `QVS3101` to `QVS3108`, `QVS3014` |
| vres, media | images, rename, delete, upload (below); drives of a running partition, load, unload (`QVS2113` to `QVS2115`, `QSI1307`/`QVS2421`); volumes, create, delete, protect (`QVS3201` to `QVS3209`); tape drives |
| vres, switches | switches with their ports counted, ports with counters, forwarding table, VLANs, limits; create, change, rename, delete, uplink set and clear (`QSI1501` to `QSI1514` then `QVS4001` to `QVS4107`; the management interface `QSI1510`/`QVS4102`) |
| vres, processor pools | create, change, delete (`QVS1110`, `QVS2105`, `QVS2202` to `QVS2209`) |
| refcodes | history by cursor |
| panel | QS/VS's functions and their keylock rules (`QVS2003`, `QVS2306`, `QVS2307`, `QVS2309`, `QVS2318` when the partition's settings keep panel functions from consoles) |
| console | attach, one viewer, take over, `QVS2314`; screen |
| pair | pairing |
| service | list, list rated, get, get rated, close (`QSI1001` while its condition holds and the close has no leave, `QSI1002`, `QSI1003`), note; `Opened`, `Seen`, `Cleared`, `Closed`, `Noted` |
| update | status, stage (a stream; an image that does not begin `QSVSUPDT` is refused `QSI1102`, as is the next one told), install into the other slot (`QSI1106`, `QSI1108`, `QSI1114`), activate (`QSI1113` with `QVS5313` while partitions run and the activation said refuse), cancel, discard; `Staged`, `Verified`, `Installing`, `Installed`, `Activating`, `Confirmed` or `RolledBack` as told |
| dumps, base dumps | a host's partitions' dumps and their space, a system's own service dumps and their space, download as a stream, delete; the fix level (bit 27) |

Every change pushes the events a QS/VS host pushes: partitions
`StateChanged`, `ResourcesChanged`, `Defined`; partition
`EntitlementChanged`; profiles `Changed`; host `Resources`, `Item`,
`Assigned`, `Returned`, `PowerOn`; vres `PoolChanged`, `VdiskChanged`,
`ImageChanged`, `UploadEnded`, `VolumeChanged`, `DriveChanged`,
`SwitchChanged`, `PortChanged`, `TapeDriveChanged`; refcodes `Code`.

**An activation** checks the profile's virtual disks, required ports,
physical I/O, media and tape (`QVS2110`, `QVS2117`, `QVS2401` to
`QVS2403`, `QVS3105`, `QVS3204`, `QVS3205`), gives the desired
processors and memory when there are that many, else the most there is
down to the minimum (`QVS2101` to `QVS2105`), dedicated cores from the
default pool's highest not in a named pool, and answers what it gave.
Holders follow what each partition holds. Then its IPL: from side A or
B the host's `C2005000` to `C2005090` and QS/400's own codes from
`srcdata`, then Running; from D `C2005045` with the image's volume
identifier and the installer's `A6005008`, waiting at Starting; from D
with no medium `A2005032`, `A2005100`, Not activated. A shut down is at
once whatever its delay: delayed `C2009040`, operating system
`C2009041`, each Shutting down then `C2005100`; immediate `A2005140`;
then Not activated, holding nothing, and with `restart` activated again
with the same profile. A restart (and function 03) posts `C2005110` and
IPLs again holding what it held. A power down shuts partitions down (or
ends them), posts its codes, goes away and, for a restart, comes back
after 3 seconds.

**Servicing** (`system/servicing.rs`, `fixture/service.rs`): `QSHQVS1`
has event 1042 (`B6000100`, `QSTEST01`, the virtual disk `VD04`, the
prototype's analysis log) open and 1039 and 1040 closed, slots A
(`V1R0M0` build `26280`, running) and B (empty), and a dump of
`QSDEV01` and of `QSTEST01`; `QSHQ400` has event 1041 closed, its
firmware dump and the fix level `QS400-CUM-26290`. A dump's bytes are
made from its name and offset (`content`), so a download is checked
without keeping it; a console that goes away ends the stream, and the
system logs `download NAME ended by the console after N of M bytes`.
An activation restarts the host into the pending slot, which confirms
or rolls back as `outcome` said.

**An upload** (`ImageUpload`) is checked first (`QVS3201` no
repository, `QVS3202`, `QVS3203`, `QVS3207` not whole 2048-byte blocks
or over 64 GiB, `QVS3107` no room, `QVS3209` one of that name under
way), then its stream opens. Nothing of the image is kept: its bytes
are counted, hashed (SHA-256) and the primary volume descriptor's few
bytes read for its volume identifier. Credit is given back as the bytes
are taken, at the pace `upload-rate` sets, so the sender waits; the
stream's own window (256 KiB) is all it can send ahead. More bytes than
said reset the stream: `UploadEnded` with `QSI0701`. Closed at the
size: the image is in the library (`ImageChanged`, `PoolChanged`,
`UploadEnded` with it), unless it is not ISO 9660 (`QVS3206`). Closed
short, reset or its connection gone: `QSI0702`, nothing kept.

## What it is told

`sim::control::command`, or a line on `qsmcsim`'s standard input:

| Command | What |
| --- | --- |
| `pair S` | make a pairing password: the answer |
| `expire S` | the password out expires now |
| `drop S` | every session closes |
| `stall S on\|off` | read and send nothing, keeping the connections |
| `refuse S on\|off` | close every connection before TLS |
| `down S`, `up S` | a machine off (no answer at all) and on |
| `refuse-power S ID TEXT`, `refuse-panel S ID TEXT` | refuse power or panel requests with that message |
| `refuse-family S FAMILY[/OP] ID TEXT` | refuse a family's requests (`vres`, `partition`, `profiles`, … or its number), or one operation of it (`vres/0x0011`), with that message |
| `missing S NAME`, `missing S none` | an activation finds `NAME` missing: a virtual disk `POOL/NAME`, a switch, a location code, an image or volume |
| `allow S` | refuse nothing, nothing missing |
| `nonsense S`, `oversize S` | after the hello, bytes that are no frame; a frame past every limit |
| `flood S on\|off` | events as fast as the log takes them |
| `no-credit S on\|off` | grant no credit on a console's stream |
| `renew S` | another certificate under the same name |
| `activate S P [PROFILE] [a\|b\|d]` | activate as the partition family does: the answer, or the refusal |
| `shutdown S P [delayed\|system\|immediate]` | shut it down so; with no kind, powered off at once (`C2005100`) |
| `state S P not-activated\|starting\|running\|shutting-down\|error` | set its state, and say so |
| `fail S POOL UNIT` | a pool's unit failed: the pool Degraded |
| `upload-rate S BYTES\|stall\|off` | the upload sink takes so many bytes a second, nothing, or as fast as they come |
| `uploaded S` | the last upload: its name, the bytes that arrived, their SHA-256 once it ended, and how it ended |
| `kept S pools\|disks\|vdisks\|images\|volumes\|switches\|procpools`, `kept S profiles P`, `kept S drives P` | what is kept, a line each; profiles with their revisions and the default |
| `code S CODE TEXT`, `pcode S P CODE TEXT` | post a code of the system or a partition |
| `keylock S P normal\|manual` | a partition's keylock |
| `keys S P` | the keys its console received |
| `lines S` | what it logged: every request with its user, pairings, sessions, uploads |
| `pins S` | the consoles it pinned |
| `withhold S BIT...\|new` | take capability bits from the hello (`new`: bits 19 to 24); the session comes back without them |
| `problem S open P CODE TEXT` | open a problem (of partition `P`, or the system's with 0): its number |
| `problem S seen\|clear\|close N` | problem `N` seen again, its condition cleared, closed by the system |
| `dump S P NAME MIB` | a dump of partition `P` (or the system's own with 0) of so many MiB |
| `stage-refuse S ID TEXT` | the next image staged is refused with that message |
| `outcome S confirm`, `outcome S rollback CODE REASON` | how the next activation ends |
| `fixlevel S LEVEL` | the fix level the system says |

```
cargo run -p sim --bin qsmcsim -- --port 2300
```

lists each system's address and certificate, one port each from the
first; the console reaches them at `10.0.2.2` from QEMU's user network.
The conformance suites (`tools/conformance/src/suite/mgmt`) run the
same systems in the harness's process. `cargo test -p sim` holds each
family against the fixture, the activation's codes and the upload
sink over `qsisess`'s loopback.
