# Console reference codes

A QS/MC console posts reference codes the way a QS/400 system and a
QS/VS host post theirs: eight characters, a two-character family and
six digits. The table is `lic/refcode/src/lib.rs`; nothing else spells
a code, and a unit test (`refcode::tests::the_document_lists_the_table`)
holds this document to it, what to do for each `A3` and `B3` code
included.

A code is posted as one line, `REF <code> <text>`, sometimes with what
this posting adds after a colon:

```
REF C3001000 Console IPL started
REF A3001015 Processor lacks what the cryptography needs: AVX2, ADX
```

## The console's range

An operator reads a console's lines beside a QS/400 system's and a
QS/VS host's, so a console's code must never read as either's.

- QS/400's families (`lic/slic/src/src.rs` in its repository, at the
  pinned commit) are `C1` (LIC initialization), `C6` (OS
  initialization), `A6` (attention), `B1` (fatal) and `D1` (power
  down).
- QS/VS's (`lic/refcode/src/lib.rs` and `docs/REFCODES.md` in its
  repository, at the pinned commit) are `C2`, `A2`, `B2` and `D2`.

No family of either ends in `3`. The console takes the families whose
second character is `3`, and the first character keeps the meaning it
has in both (confirmed for Phase 1, `docs/OPEN_QUESTIONS.md` item 1):

| Family | Meaning |
|---|---|
| `C3` | console IPL and progress |
| `A3` | attention; the console continues |
| `B3` | fatal; the console parks |
| `D3` | power down |

A unit test (`refcode::tests::no_code_is_in_a_qs400_or_qsvs_family`)
fails if a code lands in a family of either. The word before the code
is `REF`, as a QS/VS host's is, not QS/400's `SRC`.

## Posting

PID 1 posts: the line on the display and the serial port, and a record
for the log service, which keeps the reference code history
(`/data/log/refcodes.ndjson`, append-only). A service posts through
PID 1's control socket (`refcode::post::remote`), so the console's lines
and the history keep one order; PID 1 refuses a code that is not in the
table. What PID 1 posts before the log service is up waits in its queue
and reaches the history first. The heartbeat is on the serial port only
and is never in the history; the power-down codes after the log service
has ended are on the console only.

The code of each event a base crate reports is `refcode::event`'s; a
unit test holds every storage event to the `C3003`/`A3003` block.

## The blocks

As QS/VS lays out its own: the third to sixth digits name whose code it
is, and an attention code has the number of the step it belongs to.

| Block | Whose |
|---|---|
| `C3001xxx`, `A3001xxx`, `B3001xxx`, `D3001xxx` | PID 1: the IPL in order, supervision, the slot, shutdown |
| `C3002xxx`, `A3002xxx` | the management network and time (`base/netd`) |
| `C3003xxx`, `A3003xxx`, `B3003xxx` | the load source and its protection (`base/stgd`, and PID 1 when the storage service is lost) |

Later phases take further blocks and record them here.

## Progress and power down

| Code | Text | When |
|---|---|---|
| `C3001000` | Console IPL started | the first thing PID 1 posts, once the console is open |
| `C3001005` | Console running | the heartbeat, every 5 s, on the serial port only, once the services are started |
| `C3001010` | Kernel file systems mounted | proc, sysfs, devtmpfs, the tmpfs on `/run`, efivarfs and cgroup2 are mounted |
| `C3001030` | A/B record read | the load-source disks were found and the record read, with the record and how many disks |
| `C3001040` | Data file system mounted | `QSMCDATA` is mounted at `/data`, with the device: the data partition, or the mirrored set's `/dev/ublkb0` |
| `C3001050` | Clock set from the RTC | the system clock was set from the RTC, with the time |
| `C3001060` | Seed checked | the seed step applied nothing: the console is configured, or there is no seed volume |
| `C3001061` | Seed applied | `network.toml` and `host.toml` were written from a `QSMCSEED` volume |
| `C3001070` | State loaded | the state files were read and the host name set, with the host name |
| `C3001075` | Console identity generated | the console's key pair and certificate were made: its first IPL with a `/data` |
| `C3001080` | Services started | what waited for `/data` was started, with the table |
| `C3001090` | Base ready | every service said it is ready, with the seconds since the kernel started |
| `C3001095` | Slot confirmed | a slot on trial became the default in the record and in `BootOrder` |
| `C3002010` | Management interface up | the management interface is up and has carrier |
| `C3002020` | Management address acquired | it has its address, with the gateway |
| `C3002030` | Time synchronized | the clock is in step with a time server, and the RTC written from it |
| `C3003010` | Mirrored load source assembled | the mirrored set is served (or taken over by a restarted storage service) |
| `C3003020` | Resynchronization started | a unit is brought back in step, with how much already is |
| `C3003025` | Resynchronization finished | the unit is in step again |
| `C3003030` | Rebuild started | a replacement unit is being written |
| `C3003035` | Rebuild finished | the replacement unit is in step |
| `C3003040` | Mirrored protection started | protection was started; it takes effect at the next IPL |
| `C3003050` | Load-source unit kept | of two units that ran apart, the one to keep was chosen |
| `D3001000` | Console shutdown started | an orderly shutdown began: power off or restart, and who asked (the control socket, `SIGTERM`, the power button) |
| `D3001090` | Console powering off | the services are ended and `/data` unmounted; on the console only |
| `D3001091` | Console restarting | the same, before a restart |

## Attention and fatal

| Code | Text | When | What to do |
|---|---|---|---|
| `A3001010` | Kernel file systems not mounted | a kernel file system did not mount, with which and why; the IPL goes on | Note the file systems named and report them with this code; the console runs without them |
| `A3001015` | Processor lacks what the cryptography needs | the processor lacks AVX2, ADX, AES or PCLMULQDQ, with which: the web server and QSI cannot run on it; the console still IPLs | Run the console on a processor with AVX2, ADX, AES and PCLMULQDQ, or give a virtual machine the host's processor model |
| `A3001030` | A/B record not valid | no load-source disk was found, or the record could not be read or written | The console runs from the slot the firmware started; if this is posted at every IPL, the load-source disk may be failing: replace it |
| `A3001035` | Pending slot not confirmed | the slot on trial did not confirm: this IPL is on the other | The console went back to the slot it ran before; the update in the other slot did not start its services and is not used |
| `A3001040` | Data file system not mounted | `/data` could not be mounted; state is in memory only | Nothing is kept across a restart: check the load-source disks and their connections; the data file system is never reformatted by the console |
| `A3001050` | Clock not set from the RTC | the RTC could not be read | Configure a time server; if the machine has a real-time clock, check its battery |
| `A3001060` | Seed not valid | a seed volume was found and refused, with why; nothing was written | Correct seed.toml on the QSMCSEED volume as the text says and restart; the console runs on its defaults meanwhile |
| `A3001070` | State file not valid | a state file was refused, with which and why | The file named is not used and its defaults are; set those settings again |
| `A3001075` | Console identity not generated | the key pair or certificate could not be written | It is made again at the next IPL; if this is posted again, check that the data file system has room |
| `A3001080` | Service stopped | a service failed five times in ten minutes and is not started again, with which | Restart the console; if the service stops again, report this code with the system log |
| `A3001095` | Boot order not written | the slot was confirmed in the record but `BootOrder` could not be written | The record holds the slot; in the firmware's setup, put the confirmed slot's entry first |
| `A3002005` | No management interface | no Ethernet link with carrier, or not the device configured | Connect a cable to a supported Ethernet port, or correct the device named in the network settings |
| `A3002010` | No carrier on the management interface | the interface has had no carrier for 30 s | Check the cable and the switch port of the interface named |
| `A3002020` | No management address | carrier, but no address for 30 s | Check that a DHCP server answers on the interface's network, or set a static address |
| `A3003010` | Mirrored load source not assembled | the set could not be assembled, with why | Check that both load-source disks are present and connected, then restart |
| `A3003020` | Load-source unit failed | a unit is missing or failed, with its number and serial; at every IPL while it lasts | The console runs on the other unit without protection: return or replace the disk of the unit named |
| `A3003030` | Rebuild not finished | a resynchronization or rebuild ended before the unit was in step | The unit is still out of step; it is tried again at the next IPL. If it fails again, replace its disk |
| `A3003040` | Load-source units ran apart | each disk of the pair ran without the other: the IPL waits for the unit to keep | Each disk ran without the other: choose the unit to keep; the other is written again from it |
| `A3003070` | Boot entries not written | a new unit was written but the firmware took no entry for it | The new unit holds both slots but the firmware has no entry for it: add one in the firmware's setup |
| `B3001000` | Console PID 1 failed | by PID 1's panic handler, with the panic message, before it parks | Restart the console; report this code with the text after it |
| `B3003000` | Storage service lost | the storage service failed five times while `/data` was on the mirrored set | The data file system has no server and nothing more is written: restart the console |

In Phase 1 the unit to keep (`A3003040`) is chosen through a test
image's hook; the screen that asks an operator is Phase 2's local
console.
