# Operating QS/MC

An operator's guide to a QS/MC console, from its machine to its
recovery. Each section says what to do and where the detail is.

## What a console needs

**Its machine.** x86_64 with AVX2, ADX, AES and PCLMULQDQ; UEFI; 2 GiB
of memory or more; one load-source disk of 16 GiB or more (64 GiB is
what a console wants), or two for a mirrored load source; a display and
keyboard, a serial port at 115200 baud, or both. A virtual machine does
as well: give it the host's processor model. `docs/INSTALL.md` has the
whole table.

**Its network.** One Ethernet port on the `open` network, where
operators' browsers reach it; it takes its address by DHCP or a static
one. A second, `private`, toward the systems it manages, is optional and
has no default route. The console listens on TCP 443 and 80 of its
`open` address and nothing else. It connects out only to:

| To | For |
|---|---|
| a DHCP server | its address, when it has no static one |
| its DNS servers | names |
| its time servers (NTP, UDP 123) | the clock |
| each paired system, TCP 2300 unless the system says another port | QSI |
| the product stream's server (HTTPS) | updates of the console, its hosts and their partitions |

A firewall between operators and the console passes 443 and 80; one
between the console and its systems passes 2300.

## Installing

Boot the ISO from a virtual CD or a USB stick and answer the installer
at the display or the serial port: the disks (one, or two mirrored),
the `open` network, the time, and the first service tools user. It
erases the disks named, installs both slots and restarts into slot A.
To install unattended, put a `seed.toml` on a volume labelled
`QSMCSEED` (`docs/SEED.md`). `docs/INSTALL.md` is the whole procedure,
with Secure Boot's enrolment.

## The first sign-on

Browse to `https://<open address>/`. The console's certificate is its
own until one is imported: its SHA-256 is on the local console's
option 4; compare it before trusting it. Sign on as the user the
installer made, with its password: that user is also the first web
Administrator. Then, in Console settings: Date and time, Network
settings, Certificates (import the organization's), Language, locale
and session.

## Users and roles

Console users (`/qsmc/console/users`): an Administrator makes, changes,
disables and deletes web users and sets their roles. The roles are
Administrator, Operator, Viewer and Service; what each may do is the
matrix on Roles (`docs/WEBUSERS.md`). A password is 8 to 128 characters
with a letter and a digit, not containing the user ID; five wrong ones
lock the ID for five minutes. The last enabled Administrator cannot be
disabled, lowered or deleted. Every change is in the audit log.

Service tools users are the local console's (option 5); the first is
the installer's.

## Pairing a host and a standalone system

1. On the system, make a pairing password: on a QS/VS host, its Service
   Tools; on a standalone QS/400 system, its own service tools.
2. On the console, Servers, Add managed system: its address, its port
   (2300) and the password. The console shows the system's name and
   certificate; Add keeps it.
3. The system is Connected on Connectivity once its session is up.

The console pins the system's certificate; a system that later answers
with another is refused (`QMC7333`) until Reset connection pairs it
again. `docs/MGMT.md` has what each failure shows.

## Partitions, from creation to an installed system

1. Partitions, Create partition: the wizard's steps, its processors,
   memory, adapters and load source (`docs/PARTITIONS.md`).
2. A host's Virtual resources: a storage pool and a virtual disk for the
   load source; QS/400's installation image uploaded to Virtual optical
   (`docs/VRES.md`).
3. The partition's profile: the disk as its load source, the image in
   its optical device, IPL from D, attended.
4. Activate it, then Open terminal window: answer QS/400's install at
   its console in the browser (`docs/TERMINAL.md`).
5. When it is installed, set the profile to IPL from B, unattended.

## Virtual resources

A host's switches, storage pools, virtual disks, optical images, tape
volumes and processor pools are on its Virtual resources pages
(`docs/VRES.md`). Deleting one goes through its confirmation; what a
partition uses is refused.

## Service

**Serviceable events** list each system's problems and the console's
own; an event's detail has its code, its resources and its guided
repair. Close an event when its condition is dealt with
(`docs/SERVICE.md`).

**Dumps.** Manage dumps lists each system's and the console's; Download
streams one to the browser. A console dump is taken at the local
console's option 8, 2, or from Manage dumps; it holds no secret.

**Repair.** A failed load-source disk of a mirrored console: Replacing a
load-source disk, below. A host's: its own guided repair, from its event.

## Updates

**Of hosts and partitions.** Code updates shows what each host and
partition runs against the `qsvs` and `qs400` streams. Download a host's
update, then Install: the console saves the host's system plan first,
brings the image to the host over QSI, and the host verifies, installs
and activates it in its other slot, back on the old one if it does not
confirm. A partition's or standalone system's fixes are installed on the
system itself.

**Of the console.** Its row on Code updates, from the `qsmc` stream, or
a removable volume at the local console's option 8, 4. Install writes
the other slot on every load-source disk; Activate restarts into it.
The slot is confirmed when its web server answers; the console's state
is kept, and migrated only after the confirmation. A slot that does not
confirm in 15 minutes is rolled back with every state folder as it was
(`A3001035`). `docs/UPDATES.md` has every step and failure.

## System plans

System plans (`/qsmc/plans`): Create plan captures a host's
configuration; Export and Import move it as a file; Deploy validates it
against a host and makes only what is missing, never deleting, changing
or taking what it did not make (`docs/PLANS.md`). A plan is saved before
each host update.

## Backup and restore

Backup and restore (`/qsmc/console/backup`): Back up now makes a file
sealed under a passphrase, downloaded by the browser; it holds the
console's state, users, certificates, managed systems and plans
(`docs/BACKUP.md`). Keep the passphrase apart from the file: without it
the file cannot be read.

To restore: the Restore tab, the file and its passphrase. The console
restarts and applies it at the IPL, and takes it back if the restored
console does not start its web server (`A3001045`). A backup of a newer
release is refused.

## Replacing a load-source disk

On a mirrored console, a failed disk is said by `A3003020` and the
status screen's Load source row. Replace the disk, then at the local
console: option 2 (Work with disk units), 2 (Replace a failed unit),
the new disk. The unit is rebuilt from the other while the console runs
(`docs/LOCALCONSOLE.md`). A console of one disk is made mirrored with
option 2, 1, given a second disk.

## A lost Administrator password

At the local console, sign on as a service tools user, then option 4
(Work with the web interface), 1 (Reset or make an Administrator): a web
user ID and its new password. The user is enabled and its sessions end.
With no service tools user's password either, there is no way in: install
again, then restore the last backup and reset the Administrator as
above.

## Recovering a console that will not start

| What is seen | What to do |
|---|---|
| The console restarts after an update and comes back on the other slot (`A3001035`) | Nothing: the update was not confirmed and the console runs what it ran before. Read its problem and the system log |
| `A3001040`, the data file system not mounted, state in memory | Check the load-source disks and their cables; on a mirrored console, the status screen says which unit is missing. The data file system is never reformatted |
| `A3001080`, a service stopped after its retries | Restart the console (option 9); if it stops again, take a dump and report the code |
| `A3001085`, a service ended by its confinement | It is started again by itself; if it is posted again, report the code with a dump |
| `A3001005` at the IPL after a kernel panic | The console restarted itself; report the code with the dump taken at that IPL |
| Nothing on the display or serial port | In the firmware's boot menu, choose the other slot's entry (`QS/MC slot A` or `QS/MC slot B`); if neither starts, boot the ISO, install again and restore the last backup |

`docs/REFCODES.md` explains every code of the console and what to do.
