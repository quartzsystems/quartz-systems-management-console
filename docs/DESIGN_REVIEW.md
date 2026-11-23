# Design review: what V1 leaves out

For Cody and the design. Everything the prototype
(`design/qsmc-prototype/`) offers that V1 answers otherwise, every page
built without a design, and every field a page lacks or shows without a
source, in one place: gathered from `docs/OPEN_QUESTIONS.md` items 19,
20, 21, 23, 25, 29, 30, 31, 32, 37, 38, 43 and 45 to 48. Each says the
page, what V1 does today, and the two ways it could go. Nothing here is
decided, and no item changes V1: the list is `NEEDS-CODY`.

## 1. Choices the prototype offers that V1 answers with a message

The page is drawn as the prototype has it; the choice answers with the
message named. For each: **A**, it joins the locked decisions in a later
release (and what that needs); **B**, the design drops it.

| # | Page | Choice | Today | A | B |
|---|---|---|---|---|---|
| 1.1 | Backup and restore | Remote host (SFTP) | `QMC8405`: a backup leaves only through an Administrator's browser | SFTP to a named host: new outbound traffic, a host key to pin, a credential to keep | drop the choice; the browser is the only way |
| 1.2 | Backup and restore | Schedule... | `QMC8700` | a scheduled backup to a removable volume or SFTP (needs 1.1 or a volume) | drop it |
| 1.3 | Backup and restore | Reference code and event history | `QMC8700`: the log service's, not restored over a running one | restore the history into a log service stopped for it | drop it: a backup holds console data only |
| 1.4 | Backup and restore | Virtual optical media library index; unchecking Console data | `QMC8700`; a backup always holds console data | the index is the hosts', so a console backup would carry a copy | drop both; Console data is not optional |
| 1.5 | Date and time | Act as NTP server on the private network | `QMC8700`: only the web server listens | an NTP server on `private` only: a second listener | drop it |
| 1.6 | Network settings | DHCP for service interfaces; SSH to console | `QMC8700`: no listener but the web server; no SSH | a DHCP server on `private`; SSH is against the locked decisions | drop both |
| 1.7 | Language and locale | a language other than English | `QMC8700` | translated catalogues of messages, pages and help | English only |
| 1.8 | A managed system's General tab | its name | `QMC1008`: the system's own | a name the console keeps beside the system's | keep it the system's; show it read-only |
| 1.9 | Servers, a managed system | Power on | `QMC2201`: a machine that is off has no QSI | the BMC of each machine (see 3.4) | drop Power on for V1's machines |
| 1.10 | Servers | Manage system profiles | `QMC8700`: a QS/VS host has none | system profiles in QSI and QS/VS | drop the action |
| 1.11 | Terminal window's toolbar | Copy, Paste, Keypad, Keyboard Map | `QMC8700`: the script has the keyboard and keypad | clipboard and a key map in the window's script | drop the four buttons |
| 1.12 | Virtual optical | Fetch an image from a URL | `QMC2405` | the host fetching by URL: outbound traffic from a host | drop it; upload is the way |
| 1.13 | Virtual tape | Add pass-through drive | `QMC2435`: a drive is released at the host | the console releasing a drive through QSI | drop it |
| 1.14 | A partition's settings | Open firmware; a physical console adapter; the console's serial adapters changed | `QMC3123`, `QMC3124`, `QMC3125` | the VMM's firmware and consoles exposed through QSI | drop them |
| 1.15 | Add scheduled operation | Power on; Backup console data | `QMC2201`, `QMC3407` | Power on with 1.9; a backup with 1.2 | drop both operations |
| 1.16 | Virtual storage | Extend of a QS/VS pool | the host's refusal, `QSI1616` | QS/VS extending a pool | drop Extend for QS/VS pools |
| 1.17 | Any page | a request the host's hello does not offer | `QMC1114` | none: older hosts lack it | hide the choice for a host that lacks it |
| 1.18 | Deploy system plan, step 1 | Replace matching partitions | `QMC6108`: a deployment never deletes or changes what exists | a deployment that changes what it finds, with its own confirmation | drop the choice |
| 1.19 | System plans | Create plan from a standalone QS/400 | `QMC6105`: it has no partitions | a plan of a standalone system's own settings | hide the choice for standalone systems |

## 2. Pages built without a design

Each is `STYLE.md`'s shared pattern closest to it, with no new style and
only the fields of its requests. **A**, the design draws it as built;
**B**, the design draws it otherwise (and the page follows).

| # | Page | Route | Pattern today | Fields |
|---|---|---|---|---|
| 2.1 | A user's properties, Change password, Create user | `/console/users?edit=`, `?password=`, after Create user... | properties under the list | ID, role, enabled; the password twice, its rules |
| 2.2 | Restore (Backup and restore's tab) | `/console/backup?tab=restore` | properties | the file, its passphrase, what the backup holds |
| 2.3 | A certificate's View | `/console/certs` with View, `?ca=` for an authority | properties | every field of the certificate |
| 2.4 | QS/MC's shared pages (confirm, error, help, not authorized) | the shared patterns | the prototype's are QS/400's, framed for QS/MC (item 20) | QS/MC's wordmark, header and content |
| 2.5 | A switch's create and properties | `/server/<h>/virtual/ethernet/switch?n=` | properties | name, VLAN IDs, MTU, uplink interfaces, bond, native VLAN |
| 2.6 | A storage pool's create and properties, Extend | `/server/<h>/virtual/storage/pool?n=&extend=` | properties, the free disks as a list | name, protection, disks and spares; state, units, the media repository |
| 2.7 | A disk that carries another system's label | the pool form's confirmation | confirmation | the disks named |
| 2.8 | A virtual disk's properties and resize | `/server/<h>/virtual/storage/resize?p=&n=` | properties | its size (a disk only grows) |
| 2.9 | Load or unload an image | `/server/<h>/virtual/optical/load?n=` | short form | the partition's device |
| 2.10 | Tape volumes, a device's volume | `/server/<h>/virtual/tape/volume?p=&s=` | list and short form | a new volume's name, identifier, capacity; protect; the device's volume |
| 2.11 | A processor pool's change | `/server/<h>/virtual/pools/change?id=` | short form | name, cores, maximum units |
| 2.12 | A partition's Other tab | `/partition/<h>/<id>/general?tab=other` | properties | serial number, created, default profile, start with the host |
| 2.13 | A virtual adapter's create and edit; a physical adapter added | `/partition/<h>/<id>/edit?s=&new=&io=` | properties | Ethernet: switch, VLAN, tagged VLANs, MAC, required; SCSI client: the disks; physical: a released device |
| 2.14 | A profile's copy, create, save current, set default | `/partition/<h>/<id>/profiles?copy=` | short form | the new name; the default |
| 2.15 | The wizard's adapter | `/partitions/create/step/6?edit=` | short form | type, adapter ID, switch and VLAN or disk, required |
| 2.16 | A group's edit | `/groups/create?g=` | `group-create.html` with the group's values | |
| 2.17 | A scheduled operation's change | `/schedule/add?id=` | `schedule-add.html` with its values | |
| 2.18 | The confirmations of Delete partition, Remove adapter, Delete profile, the virtual resources' deletes, Remove and Run now of a scheduled operation, Delete group | `/confirm/<what>` | the shared confirmation | what is affected; Delete partition: its disks too, as chosen |
| 2.19 | A plan's description and Save as template | `/plan/<name>/edit`, from View as Edit | properties | the description; the template's name. And: Edit on View, or in the list's actions |
| 2.20 | Delete a plan | `/confirm/plan-delete?p=` | the shared confirmation | the plan, its description, managed system and date |
| 2.21 | Deploy a plan | `/confirm/plan-deploy?p=&t=&c=&act=` | the shared confirmation | the plan, the managed system, the partitions to create, whether they are activated |

## 3. Fields a page lacks, or shows without a source

| # | Page | Field | Today | A | B |
|---|---|---|---|---|---|
| 3.1 | Sign-on | its notice | `notice` in `web.toml`, empty unless written there; no page sets it | a row on Language, locale and session | set only from a seed |
| 3.2 | Network settings | DHCP or static, a VLAN, listening on `private`, a domain | an empty address is DHCP; the VLAN and `private` as `network.toml` and `web.toml` have them, set from a seed or the local console; a domain answers `QMC8700` | the fields on the page, and the domain joining `base/state` | the seed and the local console only |
| 3.3 | A managed system's Power-on parameters | the IPL side, keylock, boot mode | shown as the host says, `QMC2220` when changed: QSI sets none of them | QSI carrying the host's own boot settings | the page loses the fields |
| 3.4 | Servers (Power on) | a machine's BMC: address, protocol, credentials | none: Power on answers `QMC2201` | a page for each managed system's BMC, its protocol (Redfish or IPMI) and where its credentials are kept | no Power on (1.9) |
| 3.5 | A managed system's Security | the service password age | the time since pairing; past 90 days, `(rotate)` | a pin that expires, said on Welcome | the age shown only, never expiring |
| 3.6 | Serviceable events' detail | Priority | derived from the severity: QSI has no priority | a field QSI would carry | Priority dropped |
| 3.7 | Deploy system plan, step 2 | a partition that will have new MAC addresses and serial number | a Warning, which step 3 still ticks; every item of the plan listed where the prototype lists eight | the rows as the console has them | the identity said elsewhere (the managed system's row, or the confirmation) |
| 3.8 | Open terminal window | screen size and code page | the system's, not chosen by the console | QSI letting a console choose | the fields shown, not chosen |
| 3.9 | The control panel's functions | the list | as the system says (QSI bit 24); the design's list for a system without it | the design's list always | the system's list always |
| 3.10 | Virtual resources | an image's upload date and read-only flag, a switch's mode, a pool's type and backing devices' words, a confirmation's active jobs and users signed on, the SCSI client's server adapter number | left empty or derived, as `docs/VRES.md` and `docs/PARTITIONS.md` say: QSI has no field for them | fields QSI would carry | the design drops them |

## 4. The prototype's files

86 of the prototype's 90 files are live. The four that are not are
QS/400's own Navigator pages (`nav/active-jobs.html`,
`nav/end-job-confirm.html`, `nav/job-props-general.html`,
`nav/ptfs.html`), served by a partition's Navigator, not by the console
(item 47). **A**, they stay reference only; **B**, the design marks them
as QS/400's.
