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
QSMC_QSVS_DISK=... QSMC_QS400_IMAGE=... cargo run -p build -- test --suite mgmt-host
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
| `mgmt-host` | 10 | a real QS/VS host (`QSMC_QSVS_DISK`) with a QS/400 partition (`QSMC_QS400_IMAGE`): paired from the Add page, its properties, a partition made and IPLed with its codes in the history, its console in the window to QSECOFR's command line, function 21 refused in normal and DST in manual, a restart and the session back |
| `mgmt-roles` | 11 | this phase's 18 routes for each role, as the matrix says |
| `mgmt-hostile` | 12 | nonsense, an oversized frame, a flood, no credit: the good session stays, the services never fail |
| `mgmt-outbound` | 13 | the key `mgmtd`'s alone; the web server without capabilities and with connections on 443 and 80 only; `mgmtd` only to its systems |
| `mgmt-backup` | 14 | a backup restored on a second console resumes the session without pairing |
| `mgmt-audit` | 15 | each kind of this phase's action audited with its user; no password or key |

## Evidence

The acceptance run is a fresh clone at `f79fb27`, built twice from
empty targets (`~/evidence5.sh`, its log `~/qsmc-p4-ev/run.log`): the
two builds, `doctor`, `deny`, `notices --check`, `fmt --check`, the
three clippy runs, `cargo test --workspace -- --include-ignored` and
with the test hooks (275 tests), then every suite of Phases 0 to 4
alone under the shared QEMU lock, `build test --media iso` and `build
fuzz`. Everything passed but `mgmt-refcodes` (Found while testing,
below). With only `tools/conformance` changed, the clone moved to
`c8f288d`; there `fmt --check` and the harness's clippy passed, the
test image was built again, and `mgmt-refcodes` and `mgmt-host`
passed.

Suite 10's host was built for the run at exactly the pins, in
directories of its own: QS/VS at `8c54d47` (`build --test-image all`,
then its `tools-sign-on`, which keeps the installed disk), and QS/400
at `cb58b01` (`build all`, `qs400-V1R1M0-5b5a31a9193b.img`).

| # | Check | Evidence |
| --- | --- | --- |
| 1 | Step 0 is committed on its own and every Phase 0 to 3 suite passes on the new pins | `177181b` and `a20cd87`, each its own commit (Step 0, above); at `f79fb27` the 32 disk steps and suites of Phases 0 to 2, the ISO step and the fourteen web suites passed (`ipl-slot-a` ... `web-processor`, `ipl-media`) |
| 2 | Suites 2 to 9 and 11 to 15 pass | at `f79fb27`: `mgmt-pages` (27 systems from the Add page; 506 pages and files answered 200; 13 terminal windows carry the script and no other page), `mgmt-pair` (QMC1102, QMC1111 with the host's QSI0202, QSI0201; none of the 5 passwords in 13907 bytes of logs or `systems.toml`), `mgmt-sessions` (a replay once; a log of 4,096 events read again; a changed certificate not trusted; 100 more systems in 100 s), `mgmt-lists` (10 of 20 stalled: every list within 202 ms), `mgmt-power` (403 without the token; QMC2210 then the host's QVS1011; a restart and the session back), `mgmt-terminal` (the picture byte for byte, 13156 bytes; a Viewer's keys dropped), `mgmt-panel` (01, 11, 12, 20; 03 refused with QVS2307; 08 only through its confirmation), `mgmt-roles` (72 route and role pairs), `mgmt-hostile` (the good session connected 60 s, pages within 203 ms, no service failed), `mgmt-outbound`, `mgmt-backup`, `mgmt-audit` (11 kinds audited; 15998 bytes of logs, no password or key); at `c8f288d`: `mgmt-refcodes` (filters, 4 pages of 10, A6005008 from QS/400's table, C2005090 from QS/VS's catalogue, A2999999 with QMC1201). `mgmt-peer` fuzzed 60 s, 3.7 million runs, no finding (`backup-file`, `backup-payload`, `cert-import` likewise none) |
| 3 | Suite 10 passes against a real QS/VS host | at `c8f288d`, 483.4 s: host `vs-test` paired from the Add page with the password its Service Tools showed; its properties as it says them; partition 1 made and activated, 30 of its codes in the history as it IPLed to `C6001090`; its console in the window at Sign On and QSECOFR signed on from the browser to a command line; 21 refused in normal (QVS2307) and DST's sign-on in manual; the host restarted from the console, away, and its session back |
| 4 | Every page of steps 3 to 8 and 18 to 20 of the prototype's build order is live and equal to its golden file; the count is recorded | 39 of the prototype's 90 files, Phase 3's 17 and this phase's 22 (The pages, above), each a test of `web/pages/tests/golden.rs` |
| 5 | Two clean builds are byte-identical; `build deny` and `build doctor` pass | the UKIs, the disk image, the ISO, both initramfs images, the eight binaries and the QS/VS catalogue have the same SHA-256 in both builds (below); what differs is the kernel's `.config` (`CONFIG_INITRAMFS_SOURCE` names the target directory) and so `manifest.json` and `manifest.sig`, as in Phases 0 to 3. `deny`: 239 crates on each target, no `links`, no C build helper; `doctor`: every required row present, no shared crate twice |
| 6 | QSI, the terminal's look and script, and the reference code data are QS/400's and QS/VS's; nothing of them is written or copied here | `qsi`, `qsisess`, `qsitrust` (messages, session, trust), `termlook` (the picture, the exchange, the script), `srcdata` (a partition's codes), QS/VS's catalogue written by its own build at the pin (Step 0) |
| 7 | The console listens on 443 and 80 on `open` only; script is served on the terminal page only | `sockets`, `no-secrets`, `web-listeners`: TCP only on the open address's 443 and 80; `mgmt-outbound`: the web server with no capability and connections on 443 and 80 only, `mgmtd` only to its paired systems; `mgmt-pages`: only the 13 terminal windows carry a script, under their own policy |

The release initramfs names no hook (`qsmchook`, `TEST IMAGE`,
`qsmc.hook`, `hook=`, `webd-fails` each 0) and no pattern reference.

Hashes at `f79fb27`, both builds (`c8f288d` changed no file of the
image):

| SHA-256 | File |
| --- | --- |
| `e086f830ba7d3b3d364c150a054ec19d6c622bc4970fa893fdd0d165cbf44ed8` | `images/initramfs.cpio` |
| `5e37081cdad795f0217964aa336595b965f29d29284ade857161ffe8a462b7dd` | `images/initramfs-media.cpio` |
| `bb601c1b67516faf50e88d660578bdb2e449de6f4624feecd8633097fec3c1c8` | `images/qsmc-V1R0M0-a-bb601c1b6751.efi` |
| `9a8c3db45684ab70a891cb7a52535621f83631060f37f0d7ff95e9a3983f23e2` | `images/qsmc-V1R0M0-b-9a8c3db45684.efi` |
| `3f99f209614a869d6025e15cae03d468b2fd7538ac926b25ac976ec383102473` | `images/qsmc-V1R0M0-d-3f99f209614a.efi` |
| `3ba4eb36cf682a5adf918201c687ce80b2e747664958e902ff5b8d7f938ff13b` | `images/qsmc-V1R0M0-bb601c1b6751.img` |
| `fcaa5eb9cc5e399b22a64d9b2317c223938ee88bd8598113a4180a00087a38ae` | `images/QSMC-V1R0M0-3f99f209614a.iso` |
| `27efc40924e0e660c31b4ebdf6a9bf963c684b8109519d68e15ea41ac64c228a` | `qsmcinit` |
| `3d326f616e51465e84ac93e5926e741822b761114d7876894de4e6ccac9b3209` | `qsmcstgd` |
| `a23cec458540c76a44faf390f85e9e47b8f3fc94ef553623cf428bb20b495bed` | `qsmclogd` |
| `0f11e583c6957a85e8b01e286902a3db727d544562fa5ae2ad652d03b1044728` | `qsmcnetd` |
| `975015c3a3d11cf8635e16a069230aac2a57e605b8cf576144edb9f3aa21e934` | `qsmccons` |
| `902662bc16281adcac4dcace07f873721b96b16e16d0ccf7513fbf826ed7227d` | `qsmcinstall` |
| `181df810e2c3831b500e959963fad354dbd26e5bf4bdf6df6a101e698058fe6c` | `qsmcwebd` |
| `2710df63bd87c5033d5b61e0f600ee18fd3e8ae4cdaf1ee87cf27c8bc7bb3c6c` | `qsmcmgmtd` |
| `9e557536f6797fb137671e5666fc229d1aa366d57144f9249e875eae5a3a3403` | `qsvs-catalogue/catalogue-8c54d47.json` |

## Found while testing

- A QS/VS host numbers a partition's codes only in its history; the
  event it sends for one has no number. `mgmtd` dropped every such
  code, so a real host's partition codes never reached the history.
  An unnumbered code now reads the partition's newest page, one read
  at a time, once more if codes came while it was out (`cd989ec`); the
  simulated host sends them the same way.
- Suite 7 posted 30 partition codes and the host's two in the same
  millisecond. The partition's come through that history read, after
  the host's, and the history orders a millisecond's codes as they
  came, so the host's could fall off the first page. No host posts so;
  the suite now waits for the partition's codes before the host's.
- The harness's WebSocket read with a 200 ms timeout from before the
  TLS handshake, so a slow console's upgrade failed; and it dropped a
  frame cut across two reads, so a screen could come back empty. It
  waits as a browser would for the upgrade and keeps a partial frame.
- Suite 10 sent a key as soon as a screen's text appeared; a partition
  draws a panel before it opens the keyboard, and a key while the
  status row says `X SYSTEM` is lost, as on a terminal. It waits for
  the keyboard as well. A host's delayed restart waits its delay
  (300 s) before it ends.
- From Phase 4 `identity.key` is `mgmtd`'s user's; Phase 1's
  `identity` suite, run also inside `installed-base`, still asked for
  root's and now asks for `mgmtd`'s.

## Not done

- `NEEDS-CODY`: the Dell's QS/VS host paired from a real console; a
  partition's console used in a browser; every page looked at in a
  browser beside the prototype.
- The joint gate with QS/VS: QS/VS's own suites that drive a host over
  QSI ran at its pin (`docs/PHASE11.md` in QS/VS); a host paired with
  this console passed in suite 10, on QEMU (Evidence, above).
