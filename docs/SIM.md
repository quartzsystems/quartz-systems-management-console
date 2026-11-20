# The simulated managed systems

`tools/sim` (crate `sim`, binary `qsmcsim`): managed systems for tests.
Each is a QS/VS host or a standalone system on QS/400's `qsisess` (its
managed side) and `qsitrust`, speaking real QSI over TLS 1.3 on a real
socket, as a QS/VS host's server does: its own identity, its pins, a
pairing password good for 15 minutes and five attempts, an event log,
one viewer of a partition's console. It is a host tool, in no image.

## The fixture

The prototype's data (`design/qsmc-prototype/README.md`):

| System | What |
| --- | --- |
| `QSHQVS1` | QS/VS host, `9Q10-V1`, serial `10-9F11A2`, 8 cores (3.50 available), 131,072 MiB (40,960 available), `DefaultPool`; partitions `QSDEV01` (1, running, shared uncapped), `QSTEST01` (2, not activated, `A6005008`), `QSPROD01` (3, running, dedicated); its PCI adapters as the I/O page shows them |
| `QSHQ400` | standalone, `9Q04-M2`, serial `10-7A2B9C` |
| `QSBR01VS1` to `QSBR25VS1` | the branch hosts of the paged lists: their available processors, memory, serial numbers and partition counts as the prototype has them; 4, 11, 18 and 25 starting (Standby); 6, 13 and 20 with `B6000100`, which the suites take down (No connection) |

`QSDEV01`'s console shows the recorded screen
(`screens/wrkactjob.screen`): Work with Active Jobs, the prototype's
terminal picture as console cells, its status row naming `QSHQ400`.
Drawn by QS/400's look it is the prototype's PNG byte for byte (a test
of the crate). A key typed is kept, and a character is put at the
cursor and sent back as an update.

Identities are made from a seed of the system's name, so they are the
same each run; `renew` gives another.

## What it answers

Base (identify, state, power, the codes by cursor, subscribe, time);
host (resources, hardware); partitions (list, get); refcodes (history by
cursor); panel (QS/VS's functions and their keylock rules and
messages, `QVS2003`, `QVS2306`, `QVS2307`, `QVS2309`); console (attach,
one viewer, take over, `QVS2314`; screen); pairing. A power down shuts
partitions down (or ends them), posts its codes, goes away and, for a
restart, comes back after 3 seconds. An activation posts the host's
`C2005` codes, then QS/400's own from `srcdata`.

## What it is told

`sim::control::command`, or a line on `qsmcsim`'s standard input:

| Command | What |
| --- | --- |
| `pair S` | make a pairing password: the answer |
| `expire S` | the password out expires now |
| `drop S` | every session closes |
| `stall S on\|off` | read and send nothing, keeping the connections |
| `refuse S on\|off` | close every connection before TLS |
| `down S`, `up S` | a machine off (no answer at all) and on |
| `refuse-power S ID TEXT`, `refuse-panel S ID TEXT`, `allow S` | refuse power or panel requests with that message |
| `nonsense S`, `oversize S` | after the hello, bytes that are no frame; a frame past every limit |
| `flood S on\|off` | events as fast as the log takes them |
| `no-credit S on\|off` | grant no credit on a console's stream |
| `renew S` | another certificate under the same name |
| `activate S P`, `shutdown S P` | a partition's IPL codes and states |
| `code S CODE TEXT`, `pcode S P CODE TEXT` | post a code of the system or a partition |
| `keylock S P normal\|manual` | a partition's keylock |
| `keys S P` | the keys its console received |
| `lines S` | what it logged: every request with its user, pairings, sessions |
| `pins S` | the consoles it pinned |

```
cargo run -p sim --bin qsmcsim -- --port 2300
```

lists each system's address and certificate, one port each from the
first; the console reaches them at `10.0.2.2` from QEMU's user network.
The conformance suites (`tools/conformance/src/suite/mgmt`) run the
same systems in the harness's process.
