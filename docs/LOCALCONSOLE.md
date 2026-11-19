# The local console

The console's own screen, on its display and its serial port. It looks
after the management console itself: its status, its load-source disks,
its `open` network, its users, its clock, its logs and its power. It
works with no QS/VS host, partition or web user; those are the web
interface's.

The panels are `base/panels`' where it has them; the main menu, the
status display, the network menu, the two placeholders, the power
confirmation's words and option 2 are QS/MC's (`svc/console`). Every
panel has help on F1.

## Sessions

The display and the serial port each have a session of their own; both
may be signed on at once, as the same user or not.

- **The display** shows the console's status (the rows of the status
  screen below) until Enter is pressed. Enter leads to sign-on.
- **The serial port** is PID 1's while nobody has a session there: it
  shows the reference codes as they are posted, the status screen
  between two marker lines when it changes, and a heartbeat
  (`C3001005`) every five seconds. Enter there starts a session: the
  local console has the port until the session ends, PID 1 writes
  nothing there meanwhile (its codes still go to the display and the
  history), and then the port is PID 1's again.
- **Sign-on** takes a service tools user ID and its password. IDs are not
  case sensitive; passwords are. Five wrong passwords in a row disable
  an ID for five minutes, and every refusal is logged with where it
  happened (never with the password). The rule is `base/users`'.
- **Idle sign-off.** A session with no key for ten minutes is signed
  off. A sign-on screen with no key for a minute goes back to the status.

The first user is made by the installer. Phase 3's web interface makes
its first Administrator from the same ID and password hash at its first
start; after that the two lists are separate.

## The status screen

The rows PID 1 keeps, as the display's idle screen and the serial
port's block show them:

| Row | What |
| --- | --- |
| Slot | the slot this IPL came from, with the A/B record |
| Host name | the console's name |
| Open | the `open` interface: its address and gateway, or why there is none |
| Private | the `private` interface, or `none configured` |
| Date and time | in the display zone |
| Load source | the protection and each unit's state |
| Reference | the last reference code posted |
| Processor | whether it has what QSI and the web server need |
| Machine | the serial number, and the processors, memory, disks and interfaces the inventory counted |
| Service | a row a service, with its state and failures |

## The main menu

```
                           QS/MC Local Console
                                                    System:  mc-01
 Select one of the following:

      1. Display console status
      2. Work with disk units
      3. Configure network
      4. Work with the web interface
      5. Work with service tools user IDs
      6. Date and time
      7. Display logs
      8. Service functions
      9. Power off or restart
```

The numbers keep the places QS/VS's Service Tools gives the same
things. F3 or F12 signs off.

### 1. Display console status

The release and its build, the slot and the record, the host name, each
interface by its role as the network service has it now, the load
source and `/data`, the processor and the features needed, the machine,
the uptime, the last reference code, and each service with its state.
Drawn again every second; F7 and F8 page it.

### 2. Work with disk units

The protection and each load-source unit with its location code, serial
number, size and state; a unit at work shows how far it has got.

- **1. Start mirrored protection** (a load source of one disk): pick a
  free disk of at least 16 GiB; it is erased, and the load source is a
  mirrored pair from the next IPL, the new unit written from the first.
- **2. Replace a failed unit** (a unit Missing, Failed or Suspended):
  pick a free disk; it is erased and rebuilt in the unit's place while
  the console runs, and the rebuild is shown as it goes. Leaving the
  display does not stop it.
- **5.** Shows the units again.

Each choice of a disk asks for F10 first. The storage service does the
work (`base/stgd`); these panels ask it. They are QS/MC's own until
QS/VS's move into `base/panels` (`docs/OPEN_QUESTIONS.md` item 16).

### 3. Configure network

The `open` interface as configured and as it is now, and the `private`
one, shown only.

- **1. Change the open interface**: `base/panels`' form, as the
  installer has it: the host name, the interface by its number in the
  list (with its location code, hardware address and carrier), DHCP or
  a static address with prefix and gateway, a VLAN, name servers. Enter
  applies it through the network service, on trial: the change is kept
  as soon as the interface has carrier and an address, or at once with
  F10; if it has neither after 30 seconds the configuration of before is
  put back and nothing was written. Nothing restarts. A change of the
  host name alone is made at once.

The `private` interface is set from the web interface's Network
settings (Phase 3) or by a seed volume.

### 4. Work with the web interface

Not available yet: the web interface arrives in Phase 3. The option will
then reset a web interface Administrator's password and show or replace
the web server's certificate.

### 5. Work with service tools user IDs

The users of this console. F6 adds one (ID and password twice); `2` by
an ID changes its password, which also enables an ID that wrong
passwords disabled; `4` removes it after F10. The last ID cannot be
removed.

### 6. Date and time

The UTC offset times are shown in (fixed offsets only, no summer time),
the time servers, and, with none, the clock set by hand. The network
service sets the clock and keeps it in step with the servers.

### 7. Display logs

- **1. Reference code history**: every code posted, newest first, kept
  across IPLs.
- **2. System log**: what PID 1 and the services said, newest first.

Both page with F7 and F8; `2` or `3` in the Show field keeps only
warnings and errors, or errors.

### 8. Service functions

Not available yet: the console's dumps and its code updates arrive in
Phase 6.

### 9. Power off or restart

`1` powers off, `2` restarts, each after F10 on a confirmation. The
shutdown is the orderly one: the services end in turn, `/data` is
unmounted, and the machine powers off or restarts. The hosts and
partitions the console manages run on.

## When the units ran apart

When both disks of a mirrored load source each ran without the other,
the IPL stops before `/data` is mounted (`A3003040`) and the console
shows one panel, `base/panels`' own, on the display and the serial port
at once: each unit by its location code, serial number and the time it
was last written. Type `1` by the unit to keep and confirm with F10; the
other is written again from it and the IPL goes on. Nobody signs on: the
users are on the data file system this is about. Times are UTC.

## Where changes go

The panels never edit a file. The network service applies and keeps the
network and sets the clock; PID 1 keeps the host name, the zone and the
time servers, and does the power; the storage service has the disks.
The service tools users are kept by `base/users` in
`/data/state/users.db`, as salted hashes.
