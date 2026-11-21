# Partitions

What the console does with a host's partitions (Phase 5): the list, a
partition's properties, activate and shut down, the create wizard and
the profiles. The code is `web/pages/src/partitions/`; what a host
answers is QSI's partition, partitions and profiles families (QS/400's
`docs/QSI.md`, section 7).

## Where each field lives

A partition has three things a page reads and writes:

- its **definition** (`partitions::Get`, changed by `partition::Change`):
  name, keylock, default profile, auto start. A host changes it only
  while the partition is not activated; it refuses otherwise, and the
  page shows its message.
- its **default profile** (`profiles::Get` with its revision, put back
  by `profiles::Put` with that revision): processors, memory, boot
  settings, virtual adapters, physical I/O, optical and tape. A change to
  a running partition's profile takes effect at its next activation; the
  page says so (`QMC3121`).
- its **settings** (`partition::GetSettings`/`SetSettings`, QSI bit 21):
  description, panel functions allowed, how it ends with its host and
  the delay. Changed at any time; a host without bit 21 answers
  `QMC1114`.

A running shared-processor partition's processing units, capping and
weight are also changed at once (`partition::Entitlement`, bit 18) when
the Processors tab is applied (`QMC3122`).

| Page | Field | Lives in |
| --- | --- | --- |
| General | Name | definition |
| General | ID, environment, managed system | definition, shown |
| General | Description | settings |
| General | State, reference code | status, shown |
| General | OS version, cumulative PTF level | the partition's channel, shown; Navigator at the address its channel says |
| General | Active profile | status: the profile last activated |
| General | Running since | activation times (bit 22) |
| General | Boot mode, keylock | definition's keylock and the profile's IPL type, shown |
| Processors | Mode, pool, units, virtual processors, sharing, weight | default profile; units, capping and weight also now when running and shared |
| Processors | Current | status, shown |
| Memory | Minimum, desired, maximum | default profile |
| Memory | Current, region size, available | status and the host's resources, shown |
| I/O | Physical adapters | default profile's `physical_io`, from the devices the host released |
| Virtual adapters | Ethernet, SCSI client, optical, tape | default profile |
| Virtual adapters | Serial 0 and 1 | the console's: shown and fixed |
| Settings | Auto start | definition |
| Settings | Boot mode | definition's keylock and the profile's IPL type (below) |
| Settings | IPL source | default profile |
| Settings | Keylock | definition |
| Settings | Console device | the terminal window (below) |
| Settings | Allow control panel functions | settings |
| Settings | Shut down with managed system | settings |
| Other | Serial number, created, default profile, start order | definition, shown |

The profile's revision is kept in the session when the page is read. A
put that the host refuses as stale (`QSI0401`) shows `QMC3201` and the
page again with the operator's entries and the revision read now, so
OK sends them again against it. Manage profiles' Edit opens the
Processors tab with that profile (`?profile=`).

## Boot mode and console device

| Page's choice | Keylock | Profile's IPL type |
| --- | --- | --- |
| Normal | Normal (unless Manual is chosen beside it) | Unattended |
| Service - manual IPL | Manual | Attended |
| Open firmware | answers `QMC3123`: a host has none | |

The console device is the terminal window: the profile's console is
always the partition console (`ConsoleMode::Partition`). Physical
adapter answers `QMC3124`: a host gives a partition no physical console.

## Activate

The profile (the default first), a terminal window opened after, and
for this activation the keylock, IPL source and IPL type. OK shows the
confirmation, a GET that lists the profile's processors, memory and
loaded optical media; its OK posts back with its token and the choices
carried. An IPL type other than the profile's is put into the profile
first. A refusal shows `QMC3110` with the host's first message, then
every message the host gave.

## Shut down and restart

Delayed with its delay (`ShutdownKind::Delayed`), through the operating
system (`System`), or immediate; restarted after or not. Restart from
the action list is the same page with Restart chosen. The confirmation
is a GET; only its POST with its token sends the request.

## Create partition

Nine steps, their entries kept in the session (`wiz.*`). Step 1's ID is
checked against the host's partitions (`QMC3131`) and its name against
theirs; each step's Next checks its own fields, Back keeps them, Cancel
clears them and nothing has been made. Finish makes the partition at
its ID with its settings (`partition::CreateWith`, bit 21), then puts
its first profile (no revision), sets it default when asked, and sets
the definition's keylock and auto start. A tape drive of its own is a
new volume of the host's media library. The partition is Not
activated (`QMC3130`).

## Manage profiles

Every profile `mgmtd` keeps of the partition (each read again when its
revision changes) with its processors, memory, adapters and last
activation (bit 22). Activate opens Activate with that profile; Copy
puts a new profile with the copy's values; Create profile copies the
default; Save current configuration is `profiles::SaveCurrent`; Set
default is `profiles::SetDefault`; Delete is a confirmation and
`profiles::Delete` (a host refuses the default's).

## The list

Every partition of every host from what `mgmtd` keeps, and each
standalone system as one row with no ID and `(standalone)` as its
profile. A host not connected keeps its last rows, marked No connection;
the list never waits. A partition waiting for a console (`A6005008`)
carries `QMC3002`. The action list's Manage dumps is Phase 6's
(`QMC9030`).
