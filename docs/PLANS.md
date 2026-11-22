# System plans

A system plan is a saved configuration of a QS/VS host and its
partitions: viewed, exported, imported, and deployed to a host. It is
built from nothing but what QSI's host family reads and writes, which
QS/VS's Phase 9 (`svc-capture`) proved captures a host and puts it
back. `plan/model` (crate `plan`) is the document and its rules;
`sched/schedd` owns the files and runs deployments; `web/pages`
(`plans/`) is the pages.

## Where plans are

`/data/plans/`, written by the scheduler (`qsmcschedd`, root without
capabilities) and by nothing else: the web server asks it on its socket
for every capture, import, export, change and delete, and every file is
written whole (`state::atomic_write`).

| File | What |
| --- | --- |
| `plans.toml` | the index: a row a plan (name, description, when, by whom, managed system, automatic, partitions, size), and `automatic_kept`, how many automatic plans are kept of a host (10) |
| `<name>.qmcplan` | a plan's document, as exported |
| `deploy/<HOST>.toml` | a host's deployments: its latest job and its steps, and the partitions deployments made there |

A backup holds the folder whole (`docs/BACKUP.md`).

## The document

One TOML text through `base/state`: `schema_version` first (1), then
the plan's values and tables, every key known. A file is refused when it
is over 1 MiB, is not UTF-8, is a newer schema version, has a key or a
value the document does not have, or has a name or number out of its
bounds (`plan::doc::check`). The exported file is the document as kept,
named `<name>.qmcplan`.

| Key | What |
| --- | --- |
| `name` | 1 to 32 of `a-z`, `0-9`, `-`, `_` and `.`, a letter or digit first |
| `description` | at most 256 bytes, no control characters |
| `created`, `by` | when (Unix seconds) and by whom it was made |
| `system` | the managed system it was captured from; empty for a template |
| `automatic` | saved by the console before a code update |
| `[host]` | `name`, `model` (type and model), `serial`, `cores`, `memory_mib` (installed) |
| `[[pool]]` | a storage pool: `name`, `protection` (`none`, `mirrored`, `raid5`, `raid6`), `bytes`, `repository` (the media library's), and `[[pool.unit]]` each: `location` (its location code), `disk` (`sdb`), `bytes`, `serial` |
| `[[switch]]` | a virtual switch: `name`, `vlans`, `mtu` (576 to 9216), `uplink` (the interfaces' location codes; none for internal), `bond` (`active-backup`, `lacp`), `native_vlan` |
| `[[procpool]]` | a shared processor pool but the host's default: `name`, `cores`, `maximum_units` (hundredths of a processor) |
| `[[vdisk]]` | a virtual disk a profile names: `pool`, `name`, `mib`, `thick` |
| `[[volume]]` | a tape volume a profile names: `name`, `identifier`, `mib` |
| `[[image]]` | an optical image a profile names: `name`, `bytes` (never its bytes themselves) |
| `[[partition]]` | `id` (1 to 999), `name`, `serial`, `keylock` (`normal`, `manual`, `auto`, `secure`), `default_profile`, `[partition.auto_start]` (`enabled`, `order`, `delay_secs`, `wait`, `wait_secs`), `[partition.settings]` (`description`, `panel`, `with_host` `delayed` or `immediate`, `delay`) where the host keeps them |
| `[[partition.profile]]` | every profile whole: `name`; `[processors]` `mode` (`dedicated`, `shared`), `minimum`, `desired`, `maximum`, and shared `minimum_units`, `desired_units`, `maximum_units`, `sharing` (`capped`, `uncapped`), `weight`, `pool`; `[memory]` `minimum_mib`, `desired_mib`, `maximum_mib`; `[boot]` `source` (`A`, `B`, `recorded`, `D`), `ipl_type` (`unattended`, `attended`), `console` (`serial`, `local`, `partition`); `scsi` (`slot`, `luns`: `lun`, `backing` `POOL/NAME`, `serial`), `ethernet` (`slot`, `switch`, `vlan`, `tagged`, `mac`, `required`), `optical` and `tape` (`slot`, `media`: an image, a volume, or `@` and a tape drive's location code), `physical_io` (`location`, `required`) |

Bounds: names are 1 to 64 printable characters without spaces (QSI's),
location codes and backings 1 to 256 (a host's type and model and a
unit's serial number are the machine's own words: at most 64 bytes, spaces
allowed, no control characters); at most 256 of any list QSI
bounds, 64 storage pools and processor pools, 4,096 virtual disks,
volumes and images, 999 partitions; VLANs 1 to 4094; slots 0 to 1023,
each once a profile; minimum, desired and maximum in that order; a
weight to 255; a delay to 99,999 s; MAC addresses as
`xx:xx:xx:xx:xx:xx`. Every ID, partition name, profile name, pool, disk
and switch once.

### What a plan never holds

No disk's contents, no image or volume's bytes, no dump, no password,
key, certificate or pin, and nothing of the console. MAC addresses and
serial numbers are kept: they are configuration, put back on the host
they came from.

## Capture

Create plan, on System plans, reads the host now through `mgmtd` in the
signed-on user's name (`plan::view::read`): what `mgmtd` keeps of the
connection, then the host family's reads it already carries (`ask`) and
each partition's definition (`partition`): host, hardware, devices,
disks, pools, virtual disks, images, volumes, tape drives, switches,
the partitions, each one's settings and every profile. `mgmtd` gained
nothing for plans. A read the host's hello does not offer is left out
(settings without QSI's partition family version 3).

`plan::capture` makes the document: every pool, switch and processor
pool but the default (the first the host lists, which every host has),
the virtual disks, volumes and images the profiles name, and every
partition that has a profile, each sorted by name or ID. Captured twice
from a host that did not change, the two are the same configuration
(`plan::compare`: who, when and the name are not compared). A
standalone QS/400 has no partitions to plan: Create from one answers
`QMC6105`.

A template is a plan with no managed system: imported, or saved from a
plan with its host's name and serial number, its partitions' serial
numbers and every MAC address removed (Save as template, `QMC6109`).

## Validation

Every item of the plan against what the target host reports now, a row
each: OK, Warning or Error, with a message naming the thing and the
number. An Error is skipped at deployment; a Warning is deployed with
the change its message names. `plan::validate::rules` is this table;
a test holds this file to it.

| Item | OK | Warning | Error |
| --- | --- | --- | --- |
| Managed system | A QS/VS host answering the host family | A different type and model; fewer processors or less memory than the plan's host but enough for the chosen partitions' minimums | Not a host; not connected; not enough for any partition's minimum |
| Storage pool | Exists with room for the plan's disks; or can be made on unlabelled disks at the plan's location codes | Exists with another protection; will be made smaller than the plan's but large enough | No disk at those location codes; a disk there carries a label; not enough room |
| Virtual switch | Exists with the plan's VLANs; or will be made | Will be made without its uplink because the interface is absent or in use | Exists under that name with other VLANs; the uplink is the management interface |
| Shared processor pool | Exists the same; or will be made | Made with fewer processors | Cannot be made |
| Virtual disk, tape volume | Exists the same; or will be made | Thick asked where only thin fits | Its pool is in Error; a different one exists under the name |
| Optical device | The image is in the target's library | The image is absent: the device is made empty |  |
| Physical I/O | The device at that location is released and free | Absent, not released or held: the assignment is skipped |  |
| Partition | Its ID and name are free and everything it needs is OK; or it exists the same, or as an earlier deployment of the plan left it | Something it needs has a Warning; its MAC addresses and serial number will be new | Its ID or name is taken; something it requires is in Error; more than the host has left |

How each is decided:

- **Places.** Two hosts' units, interfaces and devices are the same when
  their places are: a location code without its machine's own prefix
  (`U9Q10.V1.9F11A2-P1-C3` is `C3`; a QS/VS code such as `SPCIE-SLOT-3`
  is itself). A message says a place as `C3` or `C1 port 1`.
- **Room.** A pool has room when its free space holds its new thick
  disks; otherwise thick disks are made thin, largest first, until the
  rest fit, each a Warning. A new disk larger than the whole pool is
  Error: not enough room. A pool to be made is as large as the plan's
  in proportion to the disks found at its units' places.
- **Labels.** A disk at a unit's place that is another system's
  (foreign), in a pool, or not free carries a label: the pool is Error,
  and no request names it.
- **Uplinks.** An interface the host owns is its management interface
  (Error); one another switch or the host uses is in use, and one not
  there is absent (Warning: made without it).
- **Processor pools.** A new one has the plan's cores the target has and
  no other named pool or reservation holds.
- **What a partition needs**: its profiles' processor pool, switches,
  virtual disks, volumes, images, physical devices and tape drives.
  **What it requires**: a named processor pool, the switch of an adapter
  marked required, and the disks of its load source (each profile's first
  SCSI adapter). A required thing in Error makes it Error; anything else
  it needs in Warning or Error makes it Warning, and is left out of the
  profile as deployed: a disk in Error and an adapter on a switch in
  Error removed, an absent image's or volume's device made empty, an
  absent or held device's assignment and drive skipped.
- **Identity.** On the host the plan came from (the same serial number)
  every MAC address is put back as the plan has it, and a partition made
  again at its ID has its serial number again (QS/VS makes it from the
  host's serial number and the ID). On another host, the MAC addresses
  are left to the host and its serial numbers are its own: a Warning.
- **There already.** A partition at its ID with its name whose profiles
  are the plan's (as deployed) is used as it is. One an earlier
  deployment of the console made there, with some of the plan's
  profiles, is completed. Anything else at its ID or name is Error.
- **Size.** A partition whose default profile's minimum processors or
  memory is more than the host has available is Error; the managed
  system is Error when no partition fits.

### Against the prototype

The prototype's step 2 (`plan-deploy-2.html`) lists eight rows. Served
for the fixture's plan of `QSHQVS1` against the simulator's `QSHQVS2`,
the table has each of them with the prototype's status and message,
and (`plan::validate::tests`, suite `plans-validate`):

- a row for every item the prototype leaves out: `ETHSW1`, `TESTPOOL`,
  `VD01` to `VD05`, `VTAP01`, `VTAP02`, the installation image and
  `QSPROD01`'s tape drive at `C3-T1-L0`;
- the partitions' identity on another host: `QSDEV01` and `QSTEST01`
  are Warning, "Its MAC addresses and serial number will be new", and
  `QSPROD01`'s message ends with it. The decisions of Phase 7 say so;
  the prototype, drawn before them, shows them OK.

## Deployment

From a plan's Deploy: the target (step 1), the validation (2), the
partitions to create with "Activate created partitions" (3), the
confirmation, then the progress page. Step 1's "Replace matching
partitions" is answered `QMC6108`: a deployment never deletes or
changes what exists.

The scheduler runs it as a job, one at a time a host, started only from
the confirmation's POST. It validates again then, and the steps are
(`plan::deploy::steps`):

1. each switch to be made (`SwitchCreate`, then `UplinkSet` when it has
   its uplink);
2. each storage pool to be made (`PoolCreate` on the target's disks at
   the units' places, `confirm` always empty);
3. each shared processor pool to be made (`ProcPoolCreate`);
4. each virtual disk the chosen partitions name (`VdiskCreate`; a thick
   disk's progress is shown as the host reports its allocation, In
   progress when it does not);
5. each tape volume they name (`VolumeCreate`);
6. each chosen partition: made at its ID with its settings
   (`CreateWith`), every profile put as deployed (`Put`), then its
   definition's keylock, start and default profile (`Change`, the
   serial number and creation time as the host made them); one an
   earlier deployment left is completed;
7. with "Activate created partitions", each partition made, with its
   default profile (`Activate`).

An item in Error is a step Skipped with its message; one there already
is no step. Each step is asked through `mgmtd` in the deploying user's
name and audited.

The job and its steps are written to `deploy/<HOST>.toml` before and
after each step. The progress page reads it, so it survives a restart
of the web server; a restart of the console finds the step that was
running, marks it Interrupted and the job Stopped.

**Cancel** stops the job before its next step; **a step the host
refuses** stops it with the host's messages. What was made stays and
is listed; nothing is undone. Deploying the same plan again skips what
now exists and goes on.

### What a deployment never does

It never takes a disk that carries another system's label, never
deletes or changes anything that exists (a partition it completes is
one a deployment made), and never releases a device for partitions.
Those are an operator's own acts, on their own pages.

## The plan before an update

Installing a host's code update (Code updates' Install, its
confirmation, or a scheduled install) saves a plan of the host first,
after the image is verified and before it is installed: named for the
host and the date (`qshqvs1-20261030`, `-2` and so on for another that
day), described by the update (`Before QS/VS update 26300`). If it
cannot be saved the install stops there. Automatic plans are kept to
`automatic_kept` a host, the oldest removed first; plans an operator
made are never removed for them. The install's progress shows the step
done with the plan linked.

## Roles

The Roles page's one row, System plans, is every plan task: the list,
View, Create, Import, Export, Save as template, Deploy and Delete. The
prototype's matrix gives it to Administrator alone; Deploy and Delete
go through their confirmation and a POST with its token.

## Pages

| Route | What |
| --- | --- |
| `/qsmc/plans` | the list; Create plan from a managed system; Import |
| `/qsmc/plan/<name>` | View: the managed system and each partition's profiles |
| `/qsmc/plan/<name>/export` | the document, downloaded |
| `/qsmc/plan/<name>/edit` | the description; Save as template (built from the shared patterns) |
| `/qsmc/confirm/plan-delete?p=` | Delete's confirmation |
| `/qsmc/plan/<name>/deploy/step/1` to `3` | the wizard |
| `/qsmc/confirm/plan-deploy?p=` | Deploy's confirmation |
| `/qsmc/plan/<name>/deploy/progress?t=` | the job's steps, reloading every 10 s; Cancel deployment, Close |
