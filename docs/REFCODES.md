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
| `C3004xxx`, `A3004xxx` | the installer (`lic/qsmcinstall`), in the order of its steps |
| `C3005xxx`, `A3005xxx` | the managed-systems service (`mgmt/mgmtd`): its start, systems paired and removed, a certificate changed, a system not reached for longer than its setting and reached again. A system lost and found again within the setting posts nothing: it is in the log and on the Connectivity page |
| `C3006xxx`, `A3006xxx` | the console's serviceability: the problem log's look at `/data` (`lic/qsmcprobd`), the console's dumps (PID 1), its web certificate's end (`web/webd`) |
| `C3007xxx`, `A3007xxx` | the console's own code update (`lic/qsmcupdd` over `base/update`, and PID 1 for the state across it) and its product streams (`lic/qsmcfetchd`) |

Later phases take further blocks and record them here. Which `A3` and
`B3` codes open a problem in the console's own log is `docs/SERVICE.md`.

The installer's codes are in its log (`/run/install.log` on the media,
copied to `/data/log/install.log` by its state step) and on its progress
screen, a step a line with its code. The installer runs before there is
a data file system, so they are not in the reference code history.

## Progress and power down

| Code | Text | When |
|---|---|---|
| `C3001000` | Console IPL started | the first thing PID 1 posts, once the console is open |
| `C3001005` | Console running | the heartbeat, every 5 s, on the serial port only, once the services are started |
| `C3001010` | Kernel file systems mounted | proc, sysfs, devtmpfs, the tmpfs on `/run`, efivarfs and cgroup2 are mounted |
| `C3001030` | A/B record read | the load-source disks were found and the record read, with the record and how many disks |
| `C3001040` | Data file system mounted | `QSMCDATA` is mounted at `/data`, with the device: the data partition, or the mirrored set's `/dev/ublkb0` |
| `C3001045` | Restore applied | a restore made from the web interface was put in place before the state was read, with the backup's console and date; the state before it is kept until the restore is confirmed |
| `C3001050` | Clock set from the RTC | the system clock was set from the RTC, with the time |
| `C3001060` | Seed checked | the seed step applied nothing: the console is configured, or there is no seed volume |
| `C3001061` | Seed applied | `network.toml` and `host.toml` were written from a `QSMCSEED` volume |
| `C3001070` | State loaded | the state files were read and the host name set, with the host name |
| `C3001075` | Console identity generated | the console's key pair and certificate were made: its first IPL with a `/data` |
| `C3001080` | Services started | what waited for `/data` was started, with the table |
| `C3001090` | Base ready | every service said it is ready, with the seconds since the kernel started |
| `C3001095` | Slot confirmed | a slot on trial became the default in the record and in `BootOrder` |
| `C3001096` | Restore confirmed | the web server answered on the restored state: the state before the restore is deleted |
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
| `C3004000` | Installation started | F10 on the summary, or valid answers: the first disk is about to be written |
| `C3004010` | Installation media verified | the payload's manifest is signed by a key of the installer's initramfs and both UKIs match it, with the product, release and build |
| `C3004020` | Disks partitioned | each chosen disk has its new table and the kernel sees its partitions |
| `C3004030` | Protection set up | two disks: the mirrored set's label is written; one disk: nothing to do |
| `C3004040` | Data file system made | `QSMCDATA` is made on the data partition |
| `C3004050` | Slot A copied | slot A's UKI is on every disk's ESP and reads back |
| `C3004055` | Slot B copied | slot B's UKI is on every disk's ESP and reads back |
| `C3004060` | A/B record written | the initial record, slot A the default, is on every disk |
| `C3004070` | Boot entries written | the firmware has an entry for each disk, slot A first, or `A3004070` was posted |
| `C3004080` | State written | `network.toml`, the service tools users and `host.toml`, last, are on the data file system |
| `C3004090` | Installation complete | every step is done; the console restarts into slot A |
| `C3005000` | Managed-systems service started | `mgmtd` has its identity and its systems, with how many and its certificate's SHA-256 |
| `C3005010` | Managed system paired | a managed system was paired from the web interface, with its name, its address and the user |
| `C3005020` | Managed system paired again | Reset connection paired a managed system again, with its name and the user |
| `C3005030` | Managed system removed | a managed system and its pin were forgotten, with its name and the user |
| `C3005040` | Managed system reached again | a managed system that had not been reached for longer than the setting was reached, with its name |
| `C3006010` | Data file system has room again | `/data` is under 85% full again, with how full |
| `C3006040` | Console dump started | a console dump began, with its name and who asked |
| `C3006041` | Console dump written | it was written, with its name and size |
| `C3006045` | Console dump removed | a console dump was deleted, or put out to keep within the limits, with its name |
| `C3006050` | Web certificate has time left | the web server's certificate in use ends more than 30 days from now, after one that did not |
| `C3007000` | Update image staged | an update image was written to `/data/update` whole, with its name and size |
| `C3007010` | Update image verified | it is signed by a trusted key, whole, of this product and readable by this release's state, with its release |
| `C3007015` | Staged update discarded | what was staged was thrown away |
| `C3007020` | Update install started | the staged image is being written to the slot that is not running |
| `C3007025` | Update slot written | the slot's UKI is on one load-source disk and reads back, with the disk |
| `C3007040` | Update installed | the slot is written on every disk and pending, with the slot and release |
| `C3007045` | Pending update cancelled | the pending slot is pending no more |
| `C3007050` | Update activating | the restart into the pending slot began, with the slot and who asked |
| `C3007060` | Update confirmed | the pending slot confirmed when the web server answered, with the release and build |
| `C3007065` | State kept for the trial | the pending slot's first IPL copied `/data/state` before anything read it |
| `C3007070` | State put back | the slot that came back after the pending one did not confirm put the copy of state back |
| `C3007075` | State migrated | after the confirmation, state files an older release wrote were written in this release's form, with which |
| `C3007100` | Product streams checked | `fetchd` read each stream's index or catalog and verified it, with each stream's sequence |
| `C3007110` | Update downloaded | a file of a stream was downloaded whole and matched its index, with its name |
| `C3007115` | Downloaded update removed | a downloaded file was deleted, with its name |
| `D3001000` | Console shutdown started | an orderly shutdown began: power off or restart, and who asked (the control socket, `SIGTERM`, the power button) |
| `D3001090` | Console powering off | the services are ended and `/data` unmounted; on the console only |
| `D3001091` | Console restarting | the same, before a restart |

## Attention and fatal

| Code | Text | When | What to do |
|---|---|---|---|
| `A3001005` | Console restarted after a kernel panic | at the IPL after a kernel panic, with what the kernel kept of it, moved to `/data/log/panics`; a console dump is taken | Report this code with the console dump taken at this IPL, which holds what the kernel kept of the panic |
| `A3001010` | Kernel file systems not mounted | a kernel file system did not mount, with which and why; the IPL goes on | Note the file systems named and report them with this code; the console runs without them |
| `A3001015` | Processor lacks what the cryptography needs | the processor lacks AVX2, ADX, AES or PCLMULQDQ, with which: the web server and QSI cannot run on it; the console still IPLs | Run the console on a processor with AVX2, ADX, AES and PCLMULQDQ, or give a virtual machine the host's processor model |
| `A3001030` | A/B record not valid | no load-source disk was found, or the record could not be read or written | The console runs from the slot the firmware started; if this is posted at every IPL, the load-source disk may be failing: replace it |
| `A3001035` | Pending slot not confirmed | the slot on trial did not confirm: this IPL is on the other | The console went back to the slot it ran before; the update in the other slot did not start its services and is not used |
| `A3001040` | Data file system not mounted | `/data` could not be mounted; state is in memory only | Nothing is kept across a restart: check the load-source disks and their connections; the data file system is never reformatted by the console |
| `A3001045` | Restore taken back | the IPL after a restore found it not confirmed: the restored console did not start its web server, or the restore was cut short; the state before it is back | The console runs on the state it had before the restore; restore the backup again, and if it is taken back again, report this code with the system log |
| `A3001046` | Restore refused | the restore waiting for this IPL could not be read, or is of a newer release, with why | Nothing was changed; restore a backup made by a console of this release or an earlier one |
| `A3001050` | Clock not set from the RTC | the RTC could not be read | Configure a time server; if the machine has a real-time clock, check its battery |
| `A3001060` | Seed not valid | a seed volume was found and refused, with why; nothing was written | Correct seed.toml on the QSMCSEED volume as the text says and restart; the console runs on its defaults meanwhile |
| `A3001070` | State file not valid | a state file was refused, with which and why | The file named is not used and its defaults are; set those settings again |
| `A3001075` | Console identity not generated | the key pair or certificate could not be written | It is made again at the next IPL; if this is posted again, check that the data file system has room |
| `A3001076` | Managed-systems files not given | the managed-systems service's folder or the console's identity could not be made its user's, with why | The managed-systems service does not start and the console manages no system: check that the data file system is mounted and has room, and report this code with the system log |
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
| `A3004010` | Installation media refused | the payload is missing, not signed by a trusted key, of another release, or a file does not match the manifest; before any write | Nothing was written: install from media of a QS/MC build, written whole, and start again |
| `A3004015` | Installation answers not valid | a seed volume's `[install]` table names a disk that is not there or too small, a bad user ID or hash, or an interface role QS/MC does not have | Nothing was written: correct seed.toml as the screen says, or install by hand |
| `A3004070` | Installation boot entries not written | the firmware's variables could not be written | The disks start by the firmware's default path; if the console does not start, add an entry for each disk in the firmware's setup |
| `A3004090` | Installation failed | a step failed, with the step and why; the progress screen names it | The disks are partly written: start the installation over; if the same step fails, check or replace the disk named |
| `A3005010` | Managed systems not read | `systems.toml` or `mgmt.toml` could not be read; the service runs without what it held | The console manages none of the systems the file held: restore a backup of the console, or add the systems again with new pairing passwords |
| `A3005020` | Managed system certificate changed | a managed system answered with a certificate other than the one pinned, with its name and address; it is not trusted | Find out at the managed system why its certificate changed; when it is the system, pair it again with Reset connection |
| `A3005030` | Managed system not reached | a managed system has not been reached for longer than the setting on Connectivity, with its name and why | Check the system and the network between it and the console; Connectivity shows why the connection fails |
| `A3006010` | Data file system nearly full | `/data` is 90% full, with how full; looked at every minute | Delete the downloaded updates and dumps the console no longer needs; if it stays full, report this code with a console dump |
| `A3006040` | Console dump not written | a console dump could not be written, with why; nothing of it is kept | Check that the data file system has room, then take the dump again |
| `A3006050` | Web certificate near its end | the web server's certificate ends within 30 days, with when | Make a new certificate or import one on Certificates before it ends: browsers refuse it after |
| `A3007010` | Update image refused | a staged image was refused, with why: not signed by a trusted key, damaged, another product's, older without leave, or unable to read this console's state | Nothing was kept: stage an update image of QS/MC from its product stream or from media of a QS/MC build |
| `A3007030` | Update not installed | a step of the install failed, with the step and the disk | The running slot is as it was: check the load-source disks named, then install the update again |
| `A3007100` | Product stream refused | a stream's index or catalog did not verify against the keys in the image, or its sequence went back, with the stream and why | Nothing was taken from it: check that the update server is the product stream's, then check again |
| `A3007105` | Product stream server not reached | the update server did not answer, or answered with an error, with the address and why | Check the update server's address on Code updates and the console's route and name servers, then check again |
| `A3007110` | Download failed its check | a downloaded file's size or SHA-256 is not its index's, with the file; it is removed | The file was removed: download it again; if it fails again, the server or the path to it changes what it sends |
| `B3001000` | Console PID 1 failed | by PID 1's panic handler, with the panic message, before it parks | Restart the console; report this code with the text after it |
| `B3003000` | Storage service lost | the storage service failed five times while `/data` was on the mirrored set | The data file system has no server and nothing more is written: restart the console |

When the units ran apart (`A3003040`) the console shows the one panel
of `base/panels` for the unit to keep, on the display and the serial
port at once (`docs/LOCALCONSOLE.md`).
