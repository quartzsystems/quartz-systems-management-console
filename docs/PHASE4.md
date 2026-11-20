# Phase 4: QSI and managed systems; the terminal window

What Phase 4 builds, how to run it, and the evidence for the acceptance
table. Phases 0 to 3 still hold (`docs/PHASE0.md` to `docs/PHASE3.md`);
what changed is said here.

Phase 4 makes the console manage something. A service of its own,
`mgmtd`, holds the console's QSI identity, pairs with QS/VS hosts by
their one-time password and keeps a session open with each, resumed
with its events; the web server asks it on local sockets and holds no
key and no session. The managed-system pages, the reference code pages,
the terminal window with its control panel and the control panel page
are live against what it keeps. Partitions' own pages, profiles and
virtual resources are Phase 5's.

QSI's messages, its session and its trust rules are QS/400's `qsi`,
`qsisess` and `qsitrust`; the terminal's look and its script are
QS/400's `termlook`; a partition's reference codes are QS/400's
`srcdata`; a host's codes and messages are the catalogue QS/VS's build
writes. Nothing of them is written or copied here.

## Step 0: the pins

| Commit | What |
| --- | --- |
| `177181b` Move the QS/VS pin to fd245e6 and QS/400 to cb58b01 | QS/VS from `3de567ea27d350ebe8897530cfd03da84c005440` to `fd245e6593d61906191e183d43c0724bdb9cb5e5` ("Record the full run at QS/400's handoff 23"); QS/400 from `155aba213638b178cb730cff4a4113ef1d8fb525` to `cb58b010a8b96ac48a66e06079e1ff4e4af371c7` (its `main` with handoff 23: `termlook` and `srcdata`), which that commit pins. `Cargo.lock` started again from QS/VS's at its pin; it and the fuzzer's lock changed in the revisions only |

At the new pins `build doctor`, `build deny`, `build notices --check`,
`fmt --check`, the three clippy runs and the workspace's tests passed.
Built from empty targets in a fresh clone at `177181b`
(`~/qsmc-p4-s0`), release and test images, every suite of Phases 0 to 3
passed: the 32 disk steps and suites of Phases 0 to 2 and the ISO step,
`installed-base` on its second run (its first, on the same images, timed
out waiting for the guest to power off; 683.1 s the second time), and
the fourteen web suites of Phase 3.

QS/VS's catalogue was written only by its `build release`. QS/VS commit
`8c54d47` ("Write the catalogue alone for a console") adds `build
catalogue FILE`, which gives the release's `catalogue.json` exactly
(201 codes, 274 messages), and changes nothing else: its build tool,
`CLAUDE.md` and `docs/PHASE11.md`. It pins the same QS/400 commit, so
QS/400 stays where it is.

| Commit | What |
| --- | --- |
| `a20cd87` Move the QS/VS pin to 8c54d47 | QS/VS from `fd245e6593d61906191e183d43c0724bdb9cb5e5` to `8c54d47bed19e4e597272ab939220241329da986`; QS/400 unchanged at `cb58b010a8b96ac48a66e06079e1ff4e4af371c7`. `Cargo.lock` changed in the QS/VS revision only |

The image's step (`tools/build/src/product/catalogue.rs`) runs that
command in cargo's checkout of the pinned QS/VS, with the commit as its
build identifier, and puts the result in a slot's initramfs at
`/qsmc/catalogue.json`; it is made once a pin, under the target.

## What exists

| Where | What |
| --- | --- |
| `mgmt/proto` | `mgmtproto`: what is kept of a system (`Kept`, QSI's own types), the requests on `mgmtd`'s socket, a terminal window's connection |
| `mgmt/mgmtd` | the service (`docs/MGMT.md`): `systems.toml` and `mgmt.toml` through `base/state`, the history bounded on disk, a QSI connection and pairing, a thread a system, consoles shared among windows, the requests and the console socket, the fuzzer's entry |
| `lic/qsmcmgmtd` | the binary: its sockets bound as root, its own user for good, the identity read |
| `lic/qsmcinit` | `mgmtd` at 40 in the table; `/data/mgmt` and the key made its user's at each IPL (`A3001076`); the control socket shared with its group |
| `lic/refcode` | `A3001076`; the managed-systems block `C3005000` to `C3005030`, `A3005010`, `A3005020` |
| `web/pages` | `systems/` (Connectivity, Add managed system, Servers, a system's properties, the power confirmations), `refcodes/` (the explanations, the history), `terminal/` (Open terminal window, the window, its socket and picture, the control panel); Welcome's strip; backups with `mgmtd`'s files |
| `web/msgs` | `QMC1001` to `QMC5010` (`docs/MESSAGES.md`) |
| `web/backup` | the `mgmt/` folder |
| `tools/sim` | the simulated managed systems (`docs/SIM.md`) |
| `tools/conformance` | the WebSocket of a terminal window, `readlink` through the hooks, the `mgmt` suites |
| `tools/fuzz` | `mgmt-peer` |

## How to run

```
cargo run -p build -- all && cargo run -p build -- --test-image all
cargo run -p build -- test                      every suite of Phases 0 to 4 but mgmt-host
cargo run -p build -- test --suite mgmt-terminal
QSMC_QSVS_DISK=... cargo run -p build -- test --suite mgmt-host
cargo run -p build -- fuzz --target mgmt-peer
cargo run -p sim --bin qsmcsim -- --port 2300   the prototype's systems for a browser
cargo test -p webpages --test golden            every live page against its prototype file
```

## The pages

39 of the prototype's 90 files are live, each equal to its file under
`qsweb`'s comparison with the corrections in `docs/WEB.md`: Phase 3's
17 and these 22.

| Prototype | Route |
| --- | --- |
| `connectivity.html` | `/qsmc/connectivity` |
| `add-system.html`, `add-system-result.html` | `/qsmc/servers/add`, `/qsmc/servers/reset`, `/qsmc/confirm/system-remove` |
| `welcome.html` | `/qsmc/welcome`, its strip from what is kept |
| `servers.html`, `servers-empty.html`, `servers-error.html`, `servers-paged.html` | `/qsmc/servers` |
| `system-general.html`, `-processors`, `-memory`, `-io`, `-power`, `-capabilities`, `-refcode` | `/qsmc/server/<name>/<tab>` |
| `confirm-poweroff.html`, `confirm-poweron.html` | `/qsmc/server/<name>/confirm?op=` |
| `refcode-history.html`, `refcode.html` | `/qsmc/refcodes`, `/qsmc/refcode/<code>` |
| `terminal-config.html`, `terminal.html`, `terminal-disconnected.html` | `/qsmc/terminal`, `/qsmc/terminal/<host>/<id>` |
| `control-panel.html` | `/qsmc/partition/<host>/<id>/panel`, `/qsmc/confirm/panel` |

Welcome was Phase 3's file and is counted there. The Roles page's
matrix already had the prototype's twelve tasks; this phase's routes
take theirs (`docs/WEBUSERS.md`).

Servers' actions that a later phase brings answer `QMC9030`: Schedule
operations, Create partition, Manage partition profiles, Manage system
profiles, Virtual resources (Phase 5), Manage dumps (Phase 6); so do a
partition's own pages and a host's virtual resources.

## The suites

| Suite | Handoff | What it shows |
| --- | --- | --- |
| `mgmt-pages` | 2 | the prototype's 27 systems added from the Add page; the fixture's values on Welcome, Servers, Memory; every link from Welcome and the terminal answers; only a terminal window has a script, under its own policy |
| `mgmt-pair` | 3 | no password out, a wrong one, a used one, an expired one refused with the host's message; a good one pairs and shows the host's certificate; no password in a log or in `systems.toml` |
| `mgmt-sessions` | 4 | a dropped session resumes and replays once; a system away past its log read again; a changed certificate refused and shown; 100 more systems connect |
| `mgmt-lists` | 5 | with half of 20 stalled, the lists answer at once and show them not connected |
| `mgmt-power` | 6 | the confirmation lists the partitions; without the token nothing; the host's refusal; a restart and the session back; Power on's message |
| `mgmt-refcodes` | 7 | filters and paging; a partition's code from QS/400's table, a host's from QS/VS's catalogue, an unknown one |
| `mgmt-terminal` | 8 | the picture byte for byte; keys posted and on the socket; a Viewer cannot type; two browsers one screen; the option to outlive the session; the disconnected page |
| `mgmt-panel` | 9 | functions answered; 03 refused in the host's words; 08 only through its confirmation |
| `mgmt-host` | 10 | a real QS/VS host (named only: `QSMC_QSVS_DISK`) |
| `mgmt-roles` | 11 | this phase's 18 routes for each role, as the matrix says |
| `mgmt-hostile` | 12 | nonsense, an oversized frame, a flood, no credit: the good session stays, the services never fail |
| `mgmt-outbound` | 13 | the key `mgmtd`'s alone; the web server without capabilities and with connections on 443 and 80 only; `mgmtd` only to its systems |
| `mgmt-backup` | 14 | a backup restored on a second console resumes the session without pairing |
| `mgmt-audit` | 15 | each kind of this phase's action audited with its user; no password or key |

## Evidence

EVIDENCE

## Not done


- `NEEDS-CODY`: the Dell's QS/VS host paired from a real console; a
  partition's console used in a browser; every page looked at in a
  browser beside the prototype.
- Suite 10's partition steps, listed for a real machine
  (`docs/OPEN_QUESTIONS.md`, item 34).
- The joint gate with QS/VS: QS/VS's own suites that drive a host over
  QSI ran at its pin (`docs/PHASE11.md` in QS/VS); a host paired with
  this console is suite 10's, which needs the disk above.
