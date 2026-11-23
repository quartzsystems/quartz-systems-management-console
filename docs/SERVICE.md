# Serviceable events and dumps

What an operator sees when something breaks (`web/pages/src/service/`,
`svc/console/src/service/`): the problems of every managed system and
of the console itself on one list, every dump in one table, and the
console's own problem log and service dump. A managed system keeps its
own problem log and its own dumps; the console shows them and acts on
them over QSI, and copies nothing a system reports into a record of its
own.

## Serviceable events

`/qsmc/events`: every problem the console has seen, the fleet's and the
console's, newest first. Filters: Status (`Open` by default, `Closed`,
`All`), From and To (dates as the console writes them, the first
seen), Managed system (each kept system and the console's name).
Sorted by Problem, Status, Reference code, Managed system or Partition;
paged as the other lists.

**Numbers.** The list has one Problem column for the fleet. `mgmtd`
gives each problem it has seen of a system a number of its own, from 1,
kept in `problems.bin` in `/data/mgmt` (`mgmt/mgmtd/src/keep/problems.rs`)
by the system's name and its own number, so a number is the same across
a reconnect and the console's restart. The detail page shows the
system's own number beside it (`Managed system's number`). The
console's own problems are numbered by its problem log and are at
`/qsmc/event/console/<n>`.

**What is read.** For a system whose hello offers the `service` family
(QSI bit 2; notes bit 25), `mgmtd` lists its problems when a session
begins (`ListRated` where the hello offers severity, else `List`) and
keeps them by the family's events: `Opened`, `Seen`, `Cleared`,
`Closed`, `Noted`. A page never waits on a system.

**The detail page** (`/qsmc/event/<n>`): the description, the reference
code with its explanation link, words 2 to 9, the severity (the
system's), the partition when the problem is a partition's, the
resources as the FRU table (a virtual resource named as one, with what
to check), and the analysis log as the system keeps it.

**Priority.** The prototype has Severity and Priority; QSI has no
priority. The first resource is `High` for a terminal problem,
`Medium` for an attention or unrated one, `Low` for an informational
one, and each resource after it one lower. The fixture's event 1042
shows what the prototype shows. Recorded for the design
(`docs/OPEN_QUESTIONS.md`).

**Actions.**

- *Close* goes through `/qsmc/confirm/event-close?n=` (or `?c=` for
  the console's own) to the system that owns the problem, with the
  user's name. When its condition still holds, the page says so and
  carries its leave (`anyway`); a POST without it is refused by the
  system (`QSI1001`). Closed: `QMC7401`.
- *Add note* (`/note`) sends the text to the owner with the user's name
  (`QMC7402`); the note is in the analysis log.
- *Export* (`/export`) downloads the problem as a text file.
- *Repair* (`/repair`) is a page of the recovery text for the reference
  code from the catalogue built into the image, with the resources
  named. It changes nothing.

## Dumps

`/qsmc/dumps`: every dump with its type, size, date, source and status:

- a partition's dumps, from a host's `dumps` family (bit 14, its space
  with bit 26);
- a host's and a standalone system's own service dumps, from the base
  family (bit 4, their space with bit 26);
- the console's own (below).

Dump space is shown for each system and for the console under the
table. **Download** streams the dump from its system through `mgmtd`'s
console socket to the browser: QSI's stream credit is the back-pressure
end to end, and nothing of it is kept on the console. A browser that
goes away ends the stream at the system. **Delete** leads to
`/qsmc/confirm/dump-delete`, whose OK alone deletes (`QMC7501`). **Copy
to remote host** answers `QMC7503`: copying anything off the console is
not done. `POST /qsmc/dumps/take` takes a console dump (`QMC7505`).

## The console's own problems

`lic/qsmcprobd` runs `base/problem` with QS/MC's table
(`lic/qsmcprobd/src/lib.rs`): every attention (`A3`) and fatal (`B3`)
code of `docs/REFCODES.md` either opens a problem, with its message,
the kinds of resource it names and the codes that clear it, or says
why it does not. The service reads every posted code from PID 1 with
replay, so a code posted while it restarts is neither lost nor
doubled. Its log is in `/data/problems`.

| Condition | Code | Resource | Cleared by |
| --- | --- | --- | --- |
| A kernel panic found at the next IPL | `A3001005` | | |
| A load-source unit failed, or the units ran apart | `A3003020`, `A3003030`, `A3003040` | the disk unit | `C3003025`, `C3003035`, `C3003050` |
| A service stopped after its retries | `A3001080` | the service | |
| A service ended by its confinement (`docs/SECURITY.md`) | `A3001085` | the service | |
| A pending slot not confirmed, the console back on the other | `A3001035` | the code slot | |
| An update that failed to install | `A3007030` | the code slot | `C3007040` |
| `/data` nearly full | `A3006010` | | `C3006010` |
| A dump that could not be written | `A3006040` | | |
| The processor lacking what the cryptography needs | `A3001015` | | |
| The web certificate within 30 days of its end | `A3006050` | | `C3006050` |
| A managed system not reached for longer than `unreached_minutes` (`mgmt.toml`, 30) | `A3005030` | the system | `C3005040` |
| A download that failed its check | `A3007110` | | `C3007110` |

The other codes of the table (the record, the state, the identity, the
network's, the storage set's) are held by the table's tests in
`lic/qsmcprobd`. The console's problems are on Serviceable events with
the fleet's and at option 8, 1 of the local console, where one is
displayed and closed.

## The console's dump

`lic/consdump` over `base/servicedump`: one ZIP archive,
`QSMCDUMP-YYYYMMDD-HHMMSS.zip` in `/data/dumps/console`, at most 4
kept and 1 GiB together. It holds the logs, the problem log, the state
files, each service's status from `/proc`, and the console's own
folders (`mgmt`, `fetch`, `update`, `web`, a file whole, a folder's
files one level down), with `MANIFEST.txt` listing each entry and its
size and each file left out and why.

Left out by name: the QSI identity's key (`identity.key`) and the
service tools users (`users.db`), the base's; the web users and their
hashes (`webusers.toml`), the web certificate's key (`key.pem`) and the
managed systems as paired with their pins (`systems.toml`), the
console's. A file that holds a private key or a password hash is left
out whatever its name. A test (`consdump`'s
`every_secret_file_of_the_console_is_on_the_list`) fails the build
when a crate's file of a secret is not on the list.

PID 1 takes it on request (the web interface, option 8, 2, the test
hooks), when a service stops after its retries, when a pending slot
does not confirm, and after a kernel panic found at the next IPL.
