# A host's virtual resources

`/qsmc/server/<host>/virtual/<tab>` (`web/pages/src/vres/`): what
`mgmtd` keeps of a host's switches, storage pools, virtual disks, media
library, tape and processor pools, from QSI's vres and host families,
and the forms that change them. A tab never waits on the host; a
change is a request, and a refusal shows the host's messages after the
console's. A host whose hello lacks a bit answers `QMC1114` naming it.

## What is kept

`mgmtd` reads each list when a session begins and keeps it by the vres
family's events (`mgmt/mgmtd/src/system/resources.rs`): `Pools`,
`Vdisks`, `Images`, `Volumes`, `Switches`, `Ports`, a running
partition's `Drives`, and with physical I/O (bit 17) the host's
`TapeDrives` and `Devices`. An event that carries the whole thing
(`PoolChanged`, `VdiskChanged`, `ImageChanged`, `VolumeChanged`,
`SwitchChanged`, `DriveChanged`, `TapeDriveChanged`) replaces it; one
that does not (`UplinkChanged`, `PortChanged`, a device released or
given) has the list read again. Processor pools are the host family's
`Host.pools`.

## Virtual Ethernet

Each switch with its VLANs, its uplink (its members' names and speeds,
or `none (internal)`), its mode (`VLAN aware` for more than one VLAN,
else `Untagged`) and the partitions whose profile's adapters are on it.
The adapters are each partition's last activated profile's (its
default profile's before its first activation: `Kept::devices`), with
the MAC its port has when the profile gives none. Properties is a short form
(`/virtual/ethernet/switch?n=`): name, VLANs, MTU, uplink interfaces,
bond and native VLAN, sent as `SwitchCreate` or `SwitchChange`,
`SwitchRename`, `UplinkSet` or `UplinkClear` (bit 19). Remove is a
confirmation and `SwitchDelete`, with "even when a profile names it".

## Virtual Storage

Each pool with its units (`protection (n x size)`), capacity and use,
and the use bar (red at 70 % and above); each virtual disk with its
size, pool, the partition and adapter holding it, thin or thick.
Create storage pool and a pool's Properties are one form
(`/virtual/storage/pool?n=`), the host's free disks read when it opens:
`PoolCreate`, `PoolRename`, `Designate` (the media repository),
`PoolExtend` (bit 20). A disk that carries another system's label is
named on a confirmation page and sent in `confirm` only after its OK.
QS/VS's pools cannot grow: Extend shows its refusal (`QSI1616`,
`QVS3019`).

Create virtual disk (`create-vdisk.html`) makes the disk
(`VdiskCreate`); assigning it to a partition is a second request, a LUN
on a client adapter of the partition's default profile, put with its
revision. When that is refused the disk stays made and the page says so
(`QMC2412`). Properties and Resize are the disk's form
(`/virtual/storage/resize`): a disk only grows (`VdiskResize`). Delete
and Unassign are confirmations.

## Virtual Optical

The media library's images with their sizes and the partition and
device each is loaded in; each partition's optical devices, those of
the profile it has (`Kept::devices`), or what a running one has loaded
now (`OPT01`, `OPT02` in
slot order). Load and Unload are a short form choosing the device. An
image's upload date and a read-only flag are not in QSI: the date is
empty, and every image is read-only to partitions.

**An upload** streams. The browser's body is read as it arrives
(`qsweb`'s streamed request), the parts before the file read and
counted, and the image passed to `mgmtd` on the console socket a frame
of 256 KiB at a time (`mgmtproto::console::Upload`). `mgmtd` reads the
next frame only when the host's stream took the last, so the browser
is paced by the host; nothing of the image is held whole, in memory or
on the console's disk. QSI's `ImageUpload` says the length first: the
form's Read-only box is shown and not sent and its address field and
Fetch are a form of their own, so after the file comes only Upload's
part, and the length is the request's less what came before the file
and that tail. A body that does not end so is cut, and the host keeps
nothing. A file past 64 GiB is refused before any of it is read
(`QMC2404`). Fetch answers `QMC2405`: the console fetches nothing from
an address typed into a page.

## Virtual Tape

Each partition's tape devices from the profile it has
(`Kept::devices`): a
volume of the media library (`Virtual (file)`, `VTAP01` in order) or one
of the host's own drives passed through (`Pass-through`, `TAP01`), a
drive released at the host. The volumes are their own form
(`/virtual/tape/volume`): made, protected or not, deleted through a
confirmation; with a partition's device, the volume given to it in its
default profile. Create virtual tape drive... opens the same form, whose
New tape drive adds a device to a partition's default profile in its
first free slot, with a volume or none yet (`QMC2436`); the partition
has it from its next activation. Add pass-through drive answers `QMC2435`: a drive is
released at the host and named on the partition's I/O page.

## Shared processor pools

Each pool of the host's resources, the default first: the cores given
it as its reserved units, its maximum, the partitions in it (current,
or by profile when not activated) and what they are entitled to now.
Change is a short form (`/virtual/pools/change`): `ProcPoolCreate` or
`ProcPoolChange`, and Delete through a confirmation.
