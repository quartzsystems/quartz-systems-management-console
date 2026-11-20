# The managed-systems service

`mgmtd` (`mgmt/mgmtd`, run by `lic/qsmcmgmtd`) is the one part of the
console that speaks QSI. It holds the console's QSI identity and a
session with every managed system; the web server holds neither and
asks it on two local sockets (`mgmt/proto`, crate `mgmtproto`). A fault
in a page cannot reach a managed system's session, and the listener
that faces browsers never holds the key managed systems trust.

QSI's messages, its session and its trust rules are QS/400's `qsi`,
`qsisess` and `qsitrust`, pinned with the others. Nothing of them is
written here: `mgmtd` is the socket around them and what it keeps.

## The process

PID 1 starts it at 40 in its table, after the web server
(`lic/qsmcinit/src/host/table.rs`), with two capabilities only:
`SETUID` and `SETGID`. As root it binds its two sockets (mode 600,
root's, which the web server, root without capabilities, owns); then it
becomes user and group 23990 (`release::MGMT_ID`) for good: no
supplementary group, real, effective and saved IDs all its own, no
capability left, `NO_NEW_PRIVS`, and a check that root cannot be taken
back. Only then does it read the console's identity.

At each IPL, beside the identity step, PID 1 makes `/data/mgmt` and
everything in it that user's (mode 700 and 600), and the identity's key
`/data/state/identity.key` too (mode 600; the certificate stays 644).
A restore writes its files as root; the next IPL gives them back. A
step that fails posts `A3001076`, and `mgmtd` cannot read its key.

PID 1's control socket is `root:23990`, mode 660, so `mgmtd` posts its
codes through it.

On a processor without what the cryptography needs (AVX2, ADX, AES,
PCLMULQDQ) PID 1's table has neither the web server nor `mgmtd`;
`A3001015` says why, as for the web server.

With no identity yet (no `/data` at the first IPL) it says so in the
log and looks again every 5 seconds.

## What it keeps

| File | What | Who writes it |
| --- | --- | --- |
| `/data/mgmt/systems.toml` | each managed system as paired: name, address, port, kind, its certificate's SHA-256, when paired, its serial number then, the last event seen, the console's description of it | `mgmtd` only, through `base/state` |
| `/data/mgmt/mgmt.toml` | the limits, below; defaults when absent | read by `mgmtd` |
| `/data/mgmt/refcodes/<NAME>.codes` | each system's reference code history, a code a line | `mgmtd` |

Both TOML files are in a backup, with the identity, under `mgmt/` and
`state/` (`docs/BACKUP.md`); the history is not, as the console's own
history is not.

In memory, for each system (`mgmtproto::Kept`), all of it QSI's own
types: what it says of itself (`Identity`), its state and current code
(`State`), a host's resources and hardware (`Host`, `Hardware`), its
partitions (`Status`) and their last profiles, the capabilities agreed,
the connection and the reason it last failed, when it last answered,
the console's address its session leaves from, and a serial number its
hello said that is not the one paired. A list page reads only this: it
never waits on a managed system.

| Limit (`mgmt.toml`) | Default |
| --- | --- |
| `max_systems` | 256 |
| `max_setups`: connections being set up at once | 8 |
| `max_consoles`: terminal windows open at once, across every system | 32 |
| `history`: reference codes kept for each system | 4,096 |

## Sessions

One thread a system (`system::worker`). It connects (TCP, then TLS 1.3
against the pin, then the hellos), subscribes to events from the last
it saw, and reads everything the pages show when the system says to
(`Fresh` or `ReadAgain`) or when nothing is kept yet: `Identify`,
`State`, the system's codes by their cursor, and for a host `GetHost`,
`GetHardware`, the partitions' list, each partition's last profile and
its codes by their cursor. Then it turns the session: events update
what is kept (a code posted reads the state again; a partitions or
profiles event reads the list again; a host's item reads the hardware
again), the service's requests go out and their replies back, consoles
are fed. The last event seen is written down every 2 seconds.

A session that ends is tried again after 1 second, doubling to a minute;
a request meanwhile is answered at once with the reason. Requests
outstanding when it ends are refused with it. A request the system does
not answer in 60 seconds is refused (`QMC1110`).

The session's own limits are `qsisess::Limits`' defaults (1 MiB
messages, 64 requests, 16 streams, keep-alive 15 s, closed after 45 s
of silence); a peer that breaks the protocol ends its own session and
nothing else.

Every request carries the signed-on user's ID for the managed system's
log; what the thread reads for itself carries `QSMC`.

## Pairing

Add managed system takes an address, a port (2300 unless told) and the
one-time password the managed system showed its operator.

1. A connection that accepts any certificate (`qsitrust`'s pins with
   pairing open), TLS 1.3, the hellos.
2. QS/400's exchange (`qsitrust::pair::Console`): SPAKE2 over the
   connection's exporter, the host's confirmation checked, the
   console's sent; the host pins the console.
3. The console pins the certificate the connection showed, keeps the
   system with the name and kind its hello said, and starts its thread.

The password is used in that one exchange and dropped: it is in no
file, no log, no message; the web server holds it only for the
request that sends it. The result
page shows the system's certificate's SHA-256, to compare with the one
its Service Tools shows.

Reset connection pairs a kept system again with a new password: a new
pin, a new pairing time, the description kept.

A system that answers with the pinned certificate but another serial
number is reported (`said_serial`, Connectivity's Test connection,
`QMC1109`), not trusted silently as another machine.

## The sockets

`release::MGMTD_SOCKET` (`/run/qsmcmgmtd.sock`): one request a
connection, an `ipc` frame each way (16 MiB at most, 90 seconds),
each connection a thread so a slow answer holds up no other.

| Request | Answer |
| --- | --- |
| `systems`, `system` | what is kept |
| `add`, `reset`, `remove` | pairing, pairing again, forgetting a system and its history |
| `test` | a new connection timed: TCP, TLS, hellos, a heartbeat (base `Time`) |
| `power` | base `Power` |
| `panel` | the panel function, and the partition's code before and after |
| `history` | a page of the kept history, by system, partition, dates, code |
| `screen` | the console family's `Screen` |
| `partition` | partitions `Get` |
| `describe` | the console's note about a system |
| `status` | counts, limits, the console's QSI certificate |
| `backup_files` | `mgmt/systems.toml`, `mgmt/mgmt.toml`, the identity's two files |

`release::MGMTD_CONSOLES` (`/run/qsmcmgmtd-consoles.sock`): a terminal
window's connection (`docs/TERMINAL.md`).

## What each failure shows an operator

| Failure | Shown |
| --- | --- |
| No TCP connection | `QMC1101` with the address and the system's error |
| Turned away at TLS: no pairing password out, or it expired or its attempts were spent | `QMC1102` |
| A wrong password | `QMC1111`, then the system's own `QSI0202` |
| A password used, or expired during the exchange | the system's own `QSI0201` |
| Not a QSI system (no hello) | `QMC1104` |
| The certificate is not the one pinned | `QMC1105`; Connectivity shows Certificate changed; `A3005020` once |
| Not connected now | `QMC1106` with the reason |
| The limit of systems | `QMC1107` |
| The service does not answer | `QMC1120` |
| A system refuses a request | the console's message for the action, then every message the system gave, its own identifiers |

A system lost and found again posts nothing on the local screen; it is
in the log (`<name>: connected`, `<name>: the connection was lost: ...`)
and on Connectivity.

## Reference codes

| Code | When |
| --- | --- |
| `C3005000` | started: how many systems, its certificate |
| `C3005010` | a system paired, with its address and the user |
| `C3005020` | a system paired again |
| `C3005030` | a system removed |
| `A3005010` | `systems.toml` or `mgmt.toml` could not be read |
| `A3005020` | a system's certificate changed |
| `A3001076` | PID 1 could not give the service its files |
