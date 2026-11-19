# Installing QS/MC

How to install the management console on a machine or a virtual
machine, by hand at its screen or unattended from a seed volume. The
console is its own small operating system: installing it erases the
disks you give it and nothing else.

## What the machine needs

| What | Needs |
| --- | --- |
| Processor | x86_64 with AVX2, ADX, AES and PCLMULQDQ: the web interface and QSI need them. Without them the console installs and starts but says so (`A3001015`). In a virtual machine, give it the host's processor model |
| Memory | 2 GiB or more |
| Firmware | UEFI. Secure Boot is not needed (the console's images are not signed for it yet) |
| Load-source disk | one disk of at least 16 GiB, or two for mirrored protection. 64 GiB or more is what a console wants: its data file system holds its state and logs, update images on their way to the hosts, and the dumps it fetches from the systems it manages |
| Network | one Ethernet port for the `open` network, the one operators reach the console on. A second, `private`, toward the systems it manages, is optional |
| Console | a display and keyboard, or a serial port at 115200 baud, or both: the installer is on both at once |

The disks may be SATA, SAS, NVMe or virtual (virtio, VMware, Hyper-V).
Two disks of different sizes are mirrored to the size of the smaller.

## The media

The ISO is `QSMC-V1R0M0-<hash>.iso`. Boot it as a virtual CD (a BMC's
virtual media, a hypervisor's CD drive) or write it whole to a USB stick
and boot that. The installer finds its media either way.

Before it shows anything else the installer checks the media: the
payload's manifest must be signed by a key the installer trusts, and
both images on it must be the ones the manifest names. Media that fails
is refused before anything is written (`A3004010`); use media of a
QS/MC build, written whole.

## An attended installation

The screens follow each other in this order. F1 shows help on each; F12
goes back a screen and F3 leaves. Nothing is written until you press
F10 on the summary.

1. **Welcome.** What the installation does. Enter goes on.
2. **Load-source disks.** Every disk of the machine by its location
   code, model, serial number and size. Type `1` by one disk, or by two
   for mirrored protection. A disk smaller than 16 GiB is marked `*` and
   cannot be chosen.
3. **Confirm erase.** The disks chosen, again. Type `ERASE` to go on.
4. **Host name and network.** The host name, and the `open` interface:
   pick it by its number in the list (each interface is shown by its
   location code and hardware address, with whether it has carrier
   now), DHCP or a static address with its prefix length and gateway, a
   VLAN ID when the network is tagged, and name servers. The `private`
   interface is not set here: it is set from the web interface's
   Network settings, or by a seed volume.
5. **Time.** A fixed UTC offset for how times are shown, time servers,
   and, without servers, the date and time to set the clock to.
6. **The first user.** A service tools user ID and its password, twice.
   This user signs on to the console's local console; the web interface
   makes its first Administrator from the same ID and password when it
   arrives (Phase 3). Only a salted hash of the password is kept.
7. **Summary.** Everything you chose. F10 installs.
8. **Progress.** The steps, each with its reference code: partitioning,
   protection, the data file system, slot A, slot B, the A/B record, the
   firmware's boot entries, the state. A step that fails is named with
   its code (`A3004090`); the disks are then partly written, and Enter
   starts the installer over from the beginning.
9. **Complete.** Remove or detach the media and press Enter. The console
   restarts from its load source.

When it is up, the console's display shows its status; press Enter to
sign on with the user you made (`docs/LOCALCONSOLE.md`).

## An unattended installation

Put a seed volume beside the media: a FAT volume labelled `QSMCSEED`
with `seed.toml` at its root (a second USB stick, or a small virtual
disk). With an `[install]` table in it, the installer shows only its
progress and restarts the machine by itself. The format is QS/VS's
`base/seed` (`docs/SEED.md`):

```toml
schema_version = 2

[host]
name = "mc-01"
time_zone = "UTC+01:00"
ntp_servers = ["192.0.2.123"]

[[network.interface]]
role = "open"
mode = "static"
address = "192.0.2.10/24"
gateway = "192.0.2.1"
dns = ["192.0.2.53"]

[[network.interface]]
role = "private"
device = "52:54:00:12:34:57"
mode = "static"
address = "198.51.100.1/24"
carries_route = false
carries_dns = false
carries_time = false

[install]
disks = ["S3Z9NB0K123456", "P0000-B00-D06-F0-U0"]
user_id = "admin"
password_hash = "$argon2id$v=19$m=19456,t=2,p=1$..."
```

- `disks`: one or two, each by serial number or by location code.
- `user_id` and `password_hash`: the first service tools user. The hash
  is Argon2id, as `base/users` makes it; a password is never written in
  a seed.
- `[network]`: either form of `network.toml`. Only the roles `open`
  (first) and `private` are QS/MC's.

Answers that cannot be used (a disk that is not there or too small, a
role QS/MC does not have, a bad user ID or hash) stop the installer on a
screen that says what is wrong, before anything is written (`A3004015`).
Correct `seed.toml`, or press Enter to install by hand.

Leave the seed volume attached for the first IPL or not, as you like:
an installed console is configured, and it reads no seed again.

## If the power fails during an installation

Start the machine from the media again. The installer begins from the
first step and writes every disk afresh; nothing of the attempt before
is used.

## Afterwards

- The load source and its protection: option 2 of the local console. A
  console installed on one disk can be mirrored later by adding a
  second; a failed unit of a mirrored pair is replaced there.
- The `open` network: option 3 of the local console.
- What the installer did is in `/data/log/install.log`, its reference
  codes among it.
