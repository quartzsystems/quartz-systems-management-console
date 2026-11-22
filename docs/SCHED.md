# Scheduled operations

`sched/schedd`, run as `lic/qsmcschedd`: a service of its own that runs
each scheduled operation at its time in its owner's name. The pages
(`schedule.html`, `schedule-add.html`) ask it on its socket.

## The process

PID 1 starts it at 45 in its table, after `mgmtd`, with no capability,
as it starts the web server (`lic/qsmcinit/src/host/table.rs`); it is in
no table without the web server and `mgmtd` (a processor without what
the cryptography needs). It binds `release::SCHEDD_SOCKET`
(`/run/qsmcschedd.sock`, mode 600, the web server its only client),
waits for the data file system, and keeps `/data/state/schedules.toml`
through `base/state`. It reads the users' roles from `webusers.toml`,
which holds the password hashes and so cannot be given to another user:
that is why it is root, though with nothing of root's left.

## What is kept

| Field | What |
| --- | --- |
| `id`, `owner` | its number; the user it runs as, who added or last changed it |
| `op` | activate (with a profile), shut down delayed or immediate, restart a partition; power off a managed system |
| `system`, `partition`, `partition_name` | its target: a partition by ID and its name then, or a system (0) |
| `profile`, `delay` | an activation's profile; a shut down's or power off's delay |
| `first`, `repeat`, `until` | the first time, how often (once, daily, weekly on chosen days, monthly on a day), the last day |
| `next` | the next time it is due, or 0 for never again |

Each run is kept with the time it was due, when it ran, its owner and
how it ended (completed, failed with the system's message, refused,
missed), the newest 200.

## Times

A time is the console's display offset's (`host.toml`'s zone, whole or
half or three-quarter hours, no summer time), so a daily operation at
06:00 is at 06:00 of that offset each day. A monthly operation on a day
a month does not have is passed over that month. The service looks each
half second at the wall clock, which the harness moves.

## A run

At its time the owner's role is read from the matrix as it is then: a
user gone, disabled, or whose role lacks the task (Activate / shut down
partitions for a partition's operation, Power on / off for a power off)
is refused, `QMC3406`, and nothing is sent. Otherwise `mgmtd` is asked
as a page asks it, the owner as the request's user: the partition
family's `Activate`, `ShutDown` or `Restart` through `ask`, or `power`.
The system's refusal is kept with the run. Every run is audited:
`audit: user <owner> from schedule <id>: <operation> <target>: <outcome>`.

A run whose time passed more than 60 seconds before the service saw it,
because the console was down or the clock jumped, is not run late: it
is kept missed, logged, and the next time computed from now.

Run now runs an operation at once in its owner's name, its role checked
the same way, after its confirmation page.

## Install code update

From Phase 6, for a QS/VS host or for `the console` (a schedule whose
system is empty; Code updates' Schedule opens the form with it chosen).
It needs Code updates, and runs the page's own steps with the page's
checks as they are at its time (`schedd::update`, `docs/UPDATES.md`):
the stream must offer the target a newer release whose image is
downloaded (`QMC7003`); a host's running partitions are shut down only
when the operation's delay gives them seconds to (`QMC7004` with no
delay); then the image is brought to the host and the host activated,
or, for the console, staged, installed and restarted into. A console's
run is kept `Completed` before its restart; that the new slot confirmed
is on Code updates.

## What is not scheduled

Power on answers as its page does (`QMC2201`). Backup console data
answers `QMC3407`: a backup leaves the console through a browser, under
a passphrase typed there.
