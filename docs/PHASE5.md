# Phase 5: partitions, profiles and virtual resources

What Phase 5 builds, how to run it, and the evidence for the acceptance
table. Phases 0 to 4 still hold (`docs/PHASE0.md` to `docs/PHASE4.md`);
what changed is said here.

Phase 5 is everything an operator does with partitions: a host's
virtual resources, the Partitions list across every host, a partition's
properties, activate and shut down, the create wizard, profiles,
scheduled operations and custom groups. A standalone QS/400 is managed
for real, through its own QSI listener.

## Step 0: the pins, and what they replaced

| Commit | What |
| --- | --- |
| `d2312d9` Move the QS/VS pin to 0488d96 and QS/400 to cf8f014 | QS/VS from `8c54d47bed19e4e597272ab939220241329da986` to `0488d96b067358688f35f34b86a2a4662bcfe93f` (its `main`: "Record CI on hosted runners and the release again"); QS/400 from `cb58b010a8b96ac48a66e06079e1ff4e4af371c7` to `cf8f01453846fdf78129ca79589e18798aa03fae` (its `main` with handoff 24), which that commit pins. `Cargo.lock` started again from QS/VS's at its pin. Two changes the pins forced: the simulator names the partitions family's new `Activations` request, and the help window's golden takes `qsweb`'s plain Close window link (item 22) |
| `50e9e34` Take base's media check, unit panels and writer | deleted for `base/`: the installer's media check (`lic/qsmcinstall/src/media.rs`, now `loadsource::payload::open`), its eight writing steps (now `install::write`, the installer's log copied after the state step as before), the disk unit panels (`svc/console/src/disks.rs` is now `panels::units`' `Product` and `LoadSource`), the power confirmation's words (`panels::tools::power` with `PowerWords`), the license header check (`buildaid::header`), the harness's requests to 443 (`httpc`'s pinned trust); `qsweb`'s grouped action list and carried fields in place of Servers' and the network confirmation's own |
| `ad9c372` Close open questions 13 to 18, 22 and 26 to 28 | each with the commit that did it (`docs/OPEN_QUESTIONS.md`) |

What `base/` has and is not taken, and why: `buildaid`'s fuzz runner
builds a member of the workspace, and QS/MC's fuzzer is a workspace of
its own (item 36). The build's writing of the payload has no `base/`
counterpart.

The base panels are laid out as QS/VS's were, so seven Phase 2 goldens
changed, each by `base/`'s rendering alone (the user chose this over
keeping QS/MC's copies): `console-disks.txt`,
`console-disks-unprotected.txt`, `console-disks-missing.txt` (wider
location and serial columns, an `Other disks: N` line),
`console-disk-select.txt` (titled Work with Disk Units, the base's
prompt and columns), `console-disk-confirm.txt` (the base's words of
the rebuild), `console-disk-rebuilt.txt` (the base's watch) and
`console-restart.txt` (the base's first line, "The host will shut down
in order and restart."). The words that still say "the host" are item
35, for QS/VS. The installer's started code no longer carries the disks'
locations in its log line: `install::write` logs the code's text.

The mirror suite gained the case open question 13 was about: unit 2
absent at the IPL, written to without it, the console stopped and
started with it back, returns by the map its set began at the assembly.

Built from empty targets in a fresh clone at `ad9c372`
(`~/qsmc-p5-s0`, its log `run.log`): `build doctor`, `build deny`,
`build notices --check`, `fmt --check`, the three clippy runs and the
workspace's tests passed; the release and test images were built. The
53 suites of Phases 0 to 4 ran then (`run.log`, `web.log`): 51 passed,
and two failed on what Step 0 itself changed:

- `web-pages` and `mgmt-pages`: the help window's Close window is now
  `qsweb`'s link to `/qsmc/`, which answers 303 to Welcome, and the
  crawls took only 200. They follow a redirect to a page of the console
  now, and no page may carry an inline handler (item 22 is closed).
- `web-network`: an `open` change on trial was not kept, because the
  harness's sign-on through `httpc` named 127.0.0.1, not the address the
  console moved to. The sign-on is the harness's own request again
  (item 39).

Both fixed in `a9b531b`, and `web-network`, `mgmt-pages`, `web-signon`
and `web-listeners` passed on the same clone with it. The webd test of
the tree's routes did not know the new pages live (`7647470`).

## Step 1: what the new pins unlock

| Where | What |
| --- | --- |
| `mgmt/proto/src/ask.rs`, `mgmt/mgmtd/src/service/ask.rs` | `ask`: any request the pages send, as QSI encodes it, checked against what the pages send (`QMC1115`) and what the system's hello offers (`QMC1114` naming what it lacks) |
| `mgmtproto::Kept::part`, `itself` | a standalone system is its own partition at ID 0: its panel and console there, its code and keylock its state's |
| `web/pages/src/terminal/` | Open terminal window offers a standalone system's console; the control panel lists the functions the system lists (bit 24) with their confirmation, and the design's for a system that does not (open question 33) |
| `web/pages/src/systems/props.rs` | a host's Power on partitions and Default power off read and set (bit 23, `QMC2221`); the IPL side, keylock and boot mode shown and answered `QMC2220` (open question 30) |
| `tools/sim` | hosts say every bit to 24; the standalone `QSHQ400` says `caps::STANDALONE` and answers its panel and console at ID 0 with QS/400's refusals (`QSI0451` to `QSI0453`, `QSI0331`); `withhold S new` takes bits 19 to 24 away |

## What exists

| Where | What |
| --- | --- |
| `mgmt/mgmtd` | what it keeps of a host (`docs/MGMT.md`, `docs/VRES.md`): virtual resources, power-on settings, profiles by name and whole, activation times, functions, read when the session begins and kept by events; uploads streamed to the host with its stream's credit as the back-pressure |
| `mgmt/proto` | `ask`, the kept lists, the console socket's `Open` (a window or an upload) |
| `sched/schedd`, `lic/qsmcschedd` | the scheduler (`docs/SCHED.md`), PID 1's service at 45 with no capability |
| `web/webd` | a page may read its request's body as it arrives (`Pages::streams`) |
| `web/pages` | `vres/` (a host's virtual resources, their forms and confirmations, the upload), `partitions/` (the list, properties, activate, shut down, the wizard, profiles, their confirmations), `groups.rs`, `schedule.rs`; Servers' actions lead to them |
| `web/msgs` | `QMC1114`, `QMC1115`, `QMC2221`, `QMC2401` to `QMC2441`, `QMC3002`, `QMC3101` to `QMC3133`, `QMC3201` to `QMC3205`, `QMC3301` to `QMC3303`, `QMC3401` to `QMC3409` (`docs/MESSAGES.md`); `QMC3004` gone |
| `tools/sim` | the whole host family with the prototype's fixture data, revisions, refusals with QS/VS's identifiers, an upload sink, a partition that IPLs and ends by its codes (`docs/SIM.md`) |
| `tools/fuzz` | `mgmt-host`: a host's replies to every read and its events, each family's |

## How to run

```
cargo run -p build -- all && cargo run -p build -- --test-image all
cargo run -p build -- test                      every suite of Phases 0 to 5 but those of a real machine
cargo run -p build -- test --suite parts-upload
cargo run -p build -- fuzz --target mgmt-host
cargo run -p sim --bin qsmcsim -- --port 2300   the prototype's systems for a browser
cargo test -p webpages --test golden            every live page against its prototype file
```

## The pages

The prototype has 81 pages under `qsmc/` (its README's count; 90 files
with the shared pages and Navigator's). Phase 4 left 39 of the 90 live;
this phase adds 35, each equal to its file under `qsweb`'s comparison
with the corrections in `docs/WEB.md`:

| Step | Prototype | Route |
| --- | --- | --- |
| 9 | `vr-ethernet.html`, `vr-storage.html`, `vr-optical.html`, `vr-tape.html`, `vr-pools.html`, `create-vdisk.html` | `/qsmc/server/<host>/virtual/<tab>`, `/qsmc/server/<host>/virtual/storage/create` |
| 10 | `partitions.html`, `-empty`, `-error`, `-paged` | `/qsmc/partitions` |
| 11 | `partition-general.html`, `-processors`, `-memory`, `-io`, `-vadapters`, `-settings` | `/qsmc/partition/<host>/<id>/<tab>` |
| 12 | `activate.html`, `confirm-activate.html` | `/qsmc/partition/<host>/<id>/activate` |
| 13 | `shutdown.html`, `confirm-shutdown.html` | `/qsmc/partition/<host>/<id>/shutdown` |
| 14 | `create-1.html` to `create-9.html` | `/qsmc/partitions/create/step/<n>` |
| 15 | `manage-profiles.html` | `/qsmc/partition/<host>/<id>/profiles` |
| 16 | `schedule.html`, `schedule-add.html` | `/qsmc/schedule`, `/qsmc/schedule/add` |
| 17 | `groups.html`, `group-create.html`, `group-view.html` | `/qsmc/groups`, `/qsmc/groups/create`, `/qsmc/group/<name>` |

74 of the 90 files are live. The pages built without a design are open
question 37; what the prototype offers that answers with a message is
item 38.

## The suites

BLOCKED(phase5): the suites of Section 3 (`tools/conformance/src/suite/parts/`) and their evidence are recorded here when they have run.

## Evidence

BLOCKED(phase5): the acceptance run from empty targets (two builds compared, the gate, every suite, the fuzzer) is recorded here when it has run.

## Not done

- BLOCKED(phase5): suite 13, a real QS/VS host through the web UI alone from an ISO's upload to a QS/400 installed in a partition, and suite 14, a real standalone QS/400 paired from the Add page.
- NEEDS-CODY: the Dell's host managed from a real console through an install of QS/400; a standalone QS/400 paired with a console on real hardware; every page of this phase beside its prototype file in a browser.
