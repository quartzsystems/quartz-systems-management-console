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
59 suites and steps of Phases 0 to 4, and the ISO's `ipl-media`, ran
then (`run.log`, `web.log`): all but two passed,
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
QSMC_QSVS_DISK=... QSMC_QS400_ISO=... cargo run -p build -- test --suite mgmt-host-pages
QSMC_QS400_IMAGE=... cargo run -p build -- test --suite mgmt-standalone
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

`tools/conformance/src/suite/parts/`, each on a console of the test
image against the simulator's `QSHQVS1` (the whole host family) and
`QSHQ400` (standalone), run in the harness's process so a suite reads
what the host received:

| Suite | What it shows |
| --- | --- |
| `parts-pages` | every page of this phase (231, and 35 they link to) answers 200; 229 forms post with their token to an action that answers, 200 of them with Cancel; no script but the terminal windows' |
| `parts-requests` | each request kind reaches the host as QSI encodes it and the host keeps it: switch, pool (a disk with another system's label only after its confirmation), virtual disk, image load and unload, tape volume, processor pool, definition, settings, entitlement, each profile operation (Edit opening the profile chosen), activate, shut down, restart, delete, power-on settings; refusals with the host's QVS messages |
| `parts-stale` | a profile changed in a second session: the first's apply refused `QMC3201` with `QSI0401`, its entries kept, applied again against the new revision |
| `parts-wizard` | each step's check, `QMC3131`, Back keeps, Cancel leaves nothing at the host, Finish makes exactly the summary; a profile the host refuses leaves no partition (`QMC3134`) |
| `parts-confirm` | 11 destructive actions: nothing by GET, nothing without the token (403 `QMC9011`) or by Cancel, done by OK |
| `parts-upload` | 3 GiB streamed in 63 s with webd and mgmtd growing 0 MiB and nothing written to the console's disks; a cut upload leaves nothing (`QSI0702`); past 64 GiB refused before any of it is read (`QMC2404`, item 40); the host's pace is the browser's |
| `parts-sched` | once, daily, weekly and monthly runs completed with their next times; an owner lowered refused (`QMC3406`), nothing sent; a run due while the console was down Missed; Run now |
| `parts-groups` | a group's page is the Partitions list of its members; a partition deleted at the host leaves it |
| `parts-roles` | 308 route, method and role pairs for the four roles as the matrix says, 131 of them 403 `QMC9010` |
| `parts-lists` | a stalled host: every list within 203 ms, its rows No connection after 44 s |
| `parts-older` | a host whose hello lacks bits 19 to 24: 91 pages served, `QMC1114` naming what is lacking for each, nothing sent |
| `parts-audit` | 39 kinds of action audited with the user, a scheduled run with its owner; no password or key in the logs |

Two suites run by name on real systems, each in a second machine
whose QSI port the build machine forwards:

| Suite | What it shows |
| --- | --- |
| `mgmt-host-pages` (suite 13) | a QS/VS host at `0488d96` (its installed test disk, `QSMC_QSVS_DISK`) through the web UI alone: paired from the Add page; a pool on its blank disk made the media repository; a thin virtual disk; a switch; QS/400's install ISO (`QSMC_QS400_ISO`) streamed through Virtual Optical; a shared partition from the wizard with the disk, an Ethernet adapter and the ISO in OPT01, shown on Virtual Optical and Activate's confirmation before its first activation; activated from D with its codes in the history to `C6001090` and QS/400 installed by itself; QSECOFR signed on in the terminal window; its entitlement changed while it runs (`QMC3122`); shut down through its operating system; its activation scheduled a minute ahead and run, from A to Sign On; its configuration saved as a profile; deleted with its disk, gone at the host |
| `mgmt-standalone` (suite 14) | a standalone QS/400 at `cf8f014` (`QSMC_QS400_IMAGE`, its release image): its service console signs QSECOFR on and `STRQSIPAIR` gives the password; paired from the Add page; its row and properties; 20 codes in the history to `C6001090`; its console (ID 0) in the terminal window at Sign On, the 24 rows its own display's; the control panel lists its 15 functions; 21 at a normal keylock refused (`QMC5010`, `QSI0452`); a controlled power down from its confirmation page (`QMC2202`), the system off and away on Connectivity |

Suite 13 found that a partition never activated showed no optical
device, as the pages read only the last activated profile; they read
its default profile then (`c83d699`). QS/400's install from this ISO
asks nothing, so the steps an attended install would add, and an image
loaded into a running partition from Load (QS/VS loads media only into
a partition starting or running), are for a real machine (below).
Suite 14 needs no test image: QS/400's service console is on its
release image's serial port.

The first run found seven faults in the pages, fixed in `32ae99c`:
Manage profiles' Edit opened the default profile; the pool form left
out a disk with another system's label, so its confirmation was never
reached (QSI's `free` means "without a confirmation"); the Settings
tab, activation times and the panel's list of a host without bits 21,
22 and 24 said nothing or claimed success instead of `QMC1114`; Finish
left a partition without a profile when the host refused it; a Viewer
could post Virtual Optical's Fetch. The seventh, the 64 GiB refusal
seen only after 30 s, is `qsweb`'s (item 40).

## Evidence

Three runs, each from empty targets:

- **The gate and two builds** at `ed873f1`, in a fresh clone on the
  build machine (`~/qsmc-p5-two/run.log`). `build doctor`, `build deny`,
  `build notices --check`, `fmt --check`, the three clippy runs and
  `cargo test --workspace` passed. `build all` ran twice, the second
  with its own empty targets and the first's signing key. Every image
  and binary is the same in both:

  | File | SHA-256 |
  | --- | --- |
  | `qsmc-V1R0M0-815712e0189f.img` | `007e0c8d6c63c3694ac8efaa0fca7d21d9a7e57e717bc5cedbeb55dda32cfdd6` |
  | `qsmc-V1R0M0-a-815712e0189f.efi` | `815712e0189f31cd07c91de35f44ea7446d0708818f6a72cbe51fd96de45e448` |
  | `qsmc-V1R0M0-b-4759d49d341a.efi` | `4759d49d341af4290008f1517122d2b2d0bb3837e959d8c9449cb4d590d6c70d` |
  | `qsmc-V1R0M0-d-2bbeca6817e3.efi` | `2bbeca6817e34fa1c06809f7a15834727b50e5049bd80e99dc3a0deb32ab630c` |
  | `QSMC-V1R0M0-2bbeca6817e3.iso` | `ab74038aae620815b6f0a97dd8d49499ef28a9268ae6056542b7384c5c28ee20` |
  | `initramfs.cpio` | `28513c9094b71937a36359b613cef6262df75d66a71b135a6af788da75518414` |
  | `initramfs-media.cpio` | `96b55c02d6b2d4c8241b087e87e1e1886b4be3ef053cdd2bc19a68c7628ded5c` |
  | `catalogue-0488d96.json` | `a00403001892e1e362fa1fcef47045e39d74bc8ddcfae6429195abc51ef4643e` |
  | `qsmcinit` | `e99d96b1320d9d4fa620a3ae8a2433aaebfa7b13517a32f6ca338a8dba7471fd` |
  | `qsmcstgd` | `ba0579a2af1a43f338e0cfce6fc8c99aab32f5dc11eacaa6e8e8fdd7bc31d095` |
  | `qsmclogd` | `3b41541cac8d3915ca4963c42b769f62ba478a92950df9e0f5d010028a9bf806` |
  | `qsmcnetd` | `3f5b87516e597299264c7a251d6eac2abd54e11a9c10c1caf7c80d6c09f3881d` |
  | `qsmccons` | `99a000f20e8705076416f2d72f6631d5ab80327e7b0169f8ab3a1b52515ed1e6` |
  | `qsmcinstall` | `5abd242ea08ff6d2fafb69785b805cc2cba779fd82f8f3684122ea61ea672d62` |
  | `qsmcwebd` | `0d194c81fffab369b7474fc7e13cacb773a4a7b0a4add4e3f7a7fb6ebcca78fd` |
  | `qsmcmgmtd` | `04e33b7bdda04ebb5ab2b6e613cc800ced43d6c4423e823339c548660e8ad794` |
  | `qsmcschedd` | `cdb3524dd848971475280a837bd27311653323495cc9d4334688b03f9c9ae1fe` |

  The release initramfs names no hook (`qsmchook`, `TEST IMAGE`,
  `qsmc.hook`, `hook=`, `webd-fails`: none found).
- **Every QEMU suite** at `ed873f1` on GitHub's runners (`qemu.yml`,
  run 44984533211, 45 minutes: the build 15, then eight shards and the
  ISO side by side): the 71 suites and steps on the disk and the ISO's
  `ipl-media`, 72 in all, passed. Among them `sockets` (TCP only on
  10.0.2.15:443 and :80, UDP only 68; DHCP, DNS and NTP the only
  traffic), `web-pages` (56 pages, none with a script),
  `mgmt-pages` (1,491 pages and files; 312 terminal windows carry the
  script under their own policy, no other page does), `parts-pages`
  (231 pages and 229 forms), `parts-upload` (3 GiB in 82 s, webd and
  mgmtd growing 0 MiB) and `parts-audit` (39 kinds of action).
  `mgmt-pages`' limit was 1,500 pages then; it is 4,000 since.
- **The real machines and the fuzzer** at `ccc662d`, on the build
  machine (`~/qsmc-p5-final/run.log`): the ISO's `ipl-media`;
  `mgmt-host` (466.7 s), `mgmt-host-pages` (312.2 s) and
  `mgmt-standalone` (98.4 s) against the QS/VS host at `0488d96` and
  QS/400 at `cf8f014`; `build fuzz`, every target 60 s, no finding:

  | Target | Runs | Corpus | Edges |
  | --- | --- | --- | --- |
  | `backup-file` | 7,689,595 | 516 | 402 |
  | `backup-payload` | 6,450,484 | 691 | 598 |
  | `mgmt-peer` | 2,554,717 | 1,380 | 3,412 |
  | `mgmt-host` | 390,914 | 1,295 | 3,899 |
  | `cert-import` | 6,822,017 | 1,047 | 930 |

  What changed between `ccc662d` and `ed873f1` is the terminal window's
  policy when it is not connected and the harness; neither touches what
  those suites drive.

The same run at `ccc662d` stopped at `mgmt-pages`, and the first CI
runs found four more, all fixed before `ed873f1`:

- `sockets` on CI: QEMU 8.2 drops a monitor command sent as it starts,
  so the capture was never made; the harness sends it until the file
  is there (`b9d1989`).
- `mgmt-pages`: a terminal window not connected carried no script but
  the policy that allows one; it has the same policy without the script
  now (`a308cd1`). Its crawl then took each sort and page of the lists
  as a page of its own and did not end; it keys them as `parts-pages`
  does (`ed873f1`).
- `parts-audit`: its partition had no load source, which the host
  refuses since Finish says so (`QMC3134`); it is given a disk of its
  own. `mgmt-audit` looked for Phase 4's words for the IPL side, which
  Step 1 changed (`230d85e`).
- `parts-upload`: the harness gave up on the refusal past 64 GiB at
  30 s, the server's own read timeout (item 40); it waits 75 s
  (`ed873f1`).

## Acceptance

| # | Item | Evidence |
| --- | --- | --- |
| 1 | Step 0 committed on its own; every Phase 0–4 suite passes on the new pins; what `base/` now has is deleted here | Step 0 (above): `d2312d9`, `50e9e34`, `ad9c372`; the 59 suites and steps of Phases 0 to 4 at `ad9c372` with the two fixes of `a9b531b`, and again in the CI run |
| 2 | A standalone QS/400 managed; the control panel's list the system's; a host's two power-on settings set | `mgmt-standalone` (paired, its console, its 15 functions listed, a refusal and a power down); `parts-requests` (Power on partitions and Default power off reach the host and are kept, `QMC2221`); `parts-older` (a host without bit 23 or 24 answers `QMC1114`) |
| 3 | Suites 2–12, 15 and 16 pass | the CI run: 2 `parts-pages` and the goldens, 3 `parts-requests`, 4 `parts-stale`, 5 `parts-wizard`, 6 `parts-confirm`, 7 `parts-upload`, 8 `parts-sched`, 9 `parts-groups`, 10 `parts-roles`, 11 `parts-lists`, 12 `parts-older`, 15 `parts-audit`; 16 the fuzzer's `mgmt-host` target |
| 4 | Suites 13 and 14 against a real host and a real standalone system, or the steps the build machine cannot run listed | `mgmt-host-pages` (13) and `mgmt-standalone` (14) passed; what QEMU's nesting leaves out is under Not done, with the reason |
| 5 | Every page of steps 9–17 live and equal to its golden; the count; the pages built without a design | 35 pages added, 74 of the 90 prototype files live, each equal under `cargo test -p webpages --test golden` (in `cargo test --workspace`); the pages without a design are open question 37 |
| 6 | Two clean builds byte-identical; `build deny` and `build doctor` pass | the gate and two builds, above |
| 7 | Nothing of QSI, `qsweb`, the terminal or `base/` written or copied here; 443 and 80 on `open` only; script on the terminal page only | `build doctor` (the two pins, no shared crate twice); `sockets` and `web-listeners` (only 443 and 80, on the `open` address); `web-pages`, `mgmt-pages` and `parts-pages` (no script or inline handler but a connected terminal window's, under its own policy) |

## Not done

- NEEDS-CODY: on a real host (the Dell), what QEMU's nesting leaves out of suite 13: an attended install of QS/400 answered in the terminal window, and an image loaded from Virtual Optical's Load into a running partition.
- NEEDS-CODY: a standalone QS/400 paired with a console on real hardware; every page of this phase beside its prototype file in a browser.
