# Phase 8: release engineering and hardening

What Phase 8 does, how to run it, and the evidence for the acceptance
table. Phases 0 to 7 still hold (`docs/PHASE0.md` to `docs/PHASE7.md`);
what changed is said here. Phase 8 adds no function: it moves the pins
for the last time, confines every service, hardens the console, puts
keys, signing and the release build in order, and ends with a release
candidate of V1R0M0. The tag is Cody's.

## Step 0: the last pins

| Commit | What |
| --- | --- |
| `06af1c5` Move the QS/VS pin to dec674b and QS/400 to 960861d | QS/VS from `0488d96b067358688f35f34b86a2a4662bcfe93f` to `dec674bd0b432da728a1dc00aa1adda1303d351b` (its `main`: "Type the IPL selection again while the menu stays"); QS/400 from `cf8f01453846fdf78129ca79589e18798aa03fae` to `960861dc3a948f6c9004f30b0f155732029d025c` (its handoff 25), which that commit pins. The workspace's and the fuzzer's manifests and locks. Nothing else had to change to build |

The release candidate is built on these pins. The kernel did not move
(6.18.55, QS/VS's `kernel/VERSION` and `SHA256` unchanged), and QS/VS's
hardening symbols were already all in `kernel/config/qsmc.config`
(Phase 1).

### What changed in `base/`, and what it did here

QS/VS's twelve commits since `0488d96` touch three base crates:

| Crate | Change | Here |
| --- | --- | --- |
| `panels` | a pointer: a click puts the cursor in a field or presses a key's label, on the display (QS/400's `console`) and on the serial port (xterm's SGR reports); Page Up and Down page a list (`Panel::roll`, F7 and F8 unless a panel says otherwise); lists paged without being built again; Enter named on every keys line where it acts; the time form's F4 lists the UTC offsets; disk rows and the units menu fitted to the screen | taken as it is: `panels::screens` turns the pointer on for both the installer and `qsmccons` |
| `users` | service tools user IDs of 1 to 32 letters and digits (QS/400's `svcusers` lets its caller raise its 10); the rule and the message say so | taken: the installer's user, the local console's users and the first Administrator (whose IDs were already up to 32, `web/users`) |
| `install` | the time panel is `TimeForm`, with F4 | taken |

The golden screens that changed, and why, are below (Golden screens).

### Open questions 35, 36, 39 to 42

None is answered at the pins, so each stand-in stays and is in the
release notes' list for developers (`docs/RELEASE_NOTES_V1R0M0.md`):

| Question | At the pin | Stand-in kept |
| --- | --- | --- |
| 35, `base/panels`' words of a host | `PowerWords` and `units::Product` unchanged | the console takes the base's first line and stopping words |
| 36, `buildaid::fuzz` and a fuzzer outside the workspace | `buildaid` unchanged | `tools/build/src/host/fuzz.rs` |
| 39, `httpc`'s `Host` | QS/400's `httpc` changed only by rustfmt | the harness's own sign-on request |
| 40, a streamed request's early answer | `qsweb` unchanged | `parts-upload` sends the first parts only |
| 41, `update::state_copy` over a list of folders | `update` unchanged | `lic/qsmcinit/src/ipl/carry.rs` |
| 42, a removable volume in `base/panels` | `panels` has none | `svc/console/src/service/volume.rs` |

### Golden screens

The install and local console suites were run at `06af1c5` with
`QSMC_GOLDEN=update`; all 16 passed, and these screens changed, every
one by `base/panels` or `base/users`:

| Golden | Change | Why |
| --- | --- | --- |
| `console-disks`, `console-disks-missing`, `console-disks-unprotected`, `console-disk-select` | the keys line begins `Enter=Select` | `panels`: Enter named on every keys line where it acts (QS/VS `7fddf26`) |
| `console-users` | `Enter=Select` before `F6=Add` | the same |
| `console-user-add` | `Enter=Apply` on the keys line; the rule says 1 to 32 letters and digits | the same; `users`: IDs of up to 32 (`74ebab2`) |
| `install-user` | the rule says 1 to 32 letters and digits | `users` |
| `console-time`, `install-time` | `F4 for a list` beside the UTC offset; `F4=Prompt` on the keys line | `panels`: the time form's F4 lists the offsets (`ac8d91d`) |
| `console-log-system-help`, `console-log-history-help` | "The log is read when the display opens and at each Enter." | `panels`: lists paged without being built again (`b0a77a3`) |

The pointer needs no golden: a click moves the cursor or presses a
key's label and the screens are the same. User IDs as long as QS/VS's
console takes are the base's rule (`users::ID_MAX`), here unchanged.

## Fuzzing

`fuzz-long` run 45113495933 (at `4ddaf52`) ran every target for four
hours (14,400 s), one runner each:

| Target | Corpus at the end | Finding |
| --- | --- | --- |
| `backup-file` | 508 files, 1,119,143 bytes | none |
| `backup-payload` | 763 files, 2,609,261 bytes | none |
| `cert-import` | 1,288 files, 1,536,934 bytes | none |
| `mgmt-host` | 3,251 files, 1,220,304 bytes | none |
| `mgmt-peer` | 2,097 files, 11,238,840 bytes | none |
| `mgmt-servicing` | 1,906 files, 444,793 bytes | none |
| `qsmc-update-image` | 428 files, 38,999 bytes | none |
| `sched-store` | 4,910 files, 14,706,011 bytes | none |
| `seed-answers` | 4,568 files, 4,652,093 bytes | none |
| `stream-index` | 537 files, 200,739 bytes | none |
| `system-plan` | 5,328 files, 16,943,695 bytes | none |
| `terminal-input` | 428 files, 67,370 bytes | none |
| `web-request` | 976 files, 1,274,022 bytes | one, at 4 h 04 min, fixed |

The finding: port 80's redirect joined a request target that is not a
path to the console's address, so `GET .evil.example/x` was sent to
`https://10.0.0.1.evil.example/x`. A target that does not begin with
`/` is now sent to `/` (`5854cee`); the input is in
`tools/fuzz/corpus/web-request/d60d89e4572c4048` and
`a_redirect_never_leaves_the_console` holds it. `fuzz-long` takes a
`target` input, so one target can run alone.

## Evidence

### The runs

| Run | Commit | What | Result |
| --- | --- | --- | --- |
| QEMU 45086679139 | `2dad9c6` (Step 0) | every Phase 0 to 7 suite on the last pins | 14 of 14 jobs; 95 suites passed, none failed |
| fuzz-long 45113495933 | `4ddaf52` | every target four hours | clean but `web-request` (Fuzzing, above; fixed, `5854cee`) |
| fuzz-long 45173327969 | `eb1ace7` | `web-request` four hours again, after `5854cee` | clean: 977 files, 1,241,449 bytes of corpus, no crash |
| QEMU 45193087442 | `fe3608d` | every suite, every service's confinement enforced | 14 of 14 jobs, `ci` green; 103 suites passed, none failed |
| nightly 45216378918 | `ae31cbd` | the full suite, the release, a short fuzz, the soak | release, stream, Secure Boot and short fuzz passed; `sec-soak`, `plans-restart`, `serv-pages`, `serv-streams` failed (below) |
| QEMU 45216241201 | `ae31cbd` | every suite after the accept change | `plans-restart`, `serv-pages`, `serv-streams`, `serv-own`, `install-display` failed (below); cancelled by the next push |
| QEMU 45231390970 | `119b2e4` | every suite after the fixes | 14 of 14 jobs; 103 suites passed, none failed, the five above among them |
| nightly 45231462663 | `119b2e4` | the full suite, the release, a short fuzz, the half-hour soak | the full suite, the release twice with its stream and Secure Boot, and the short fuzz passed; `sec-soak` failed on `QSI0102` (below) |
| QEMU 45267308132 | `fd75ab2` | every suite with `sec-power` and `sec-imports`, after every fix | 14 of 14 jobs; 105 suites passed, none failed |
| release 45274171896 | `94c380b` | `release.yml`: V1R0M0, the release tagged `v1.0.0` | passed: built twice, 12 files the same, byte for byte; its stream made and verified (The release, below) |
| nightly 45260315948 | `b8763be` | the full suite, the release, a short fuzz, the half-hour soak | `sec-soak` passed (below); the short fuzz passed |
| release 45265148007 | `90b0295` | `release.yml` with the release keys | passed: the signing key's public half `qsmc.pub`; built twice, 12 files the same, byte for byte; its stream made and verified |

### The failures at `ae31cbd`, and whether it caused them

`ae31cbd` makes `webd`'s listener block in `accept` for 200 ms at most
(`SO_RCVTIMEO`) instead of polling it and sleeping 200 ms. Before it, a
new connection waited most of a sleep: `sec-baselines` at `fe3608d`
measured every page at 200 ms; at `ae31cbd`, sign-on 62 ms and the
Partitions list 43 to 61 ms. Nothing else in the product changed
between the two runs. The listener is `webd`'s alone (PID 1 closes its
copies once sent), and every connection sets its own 30 s timeouts, so
the listener's timeout reaches no connection. The change stays.

It did not cause `sec-soak`'s failure, and it exposed the others:

| Suite | Failure | Cause | Fix |
| --- | --- | --- | --- |
| `sec-soak` | `QMC6101 not shown: QSI0301 The partition manager did not answer` | not `ae31cbd`: the simulator's fleet hosts had no hardware, so a plan could not be captured; the soaks before stopped earlier, at a partition made without `VD06` (`54b1902`) | the fleet hosts have `QSHQVS1`'s hardware and devices; `a_soak_host_can_be_captured` (`106bede`) |
| `plans-restart` | `QMC6007 ... Not connected: 10.0.2.2: Network unreachable` | the web server answered after the console's restart before `mgmtd` had the hosts again; the suite deployed at once | the suite waits for both hosts Connected (`fc1c3da`) |
| `serv-pages` | `/qsmc/confirm/event-close?c=1 answered 303` | the console's problem is opened a moment after its code is posted; the crawl took it as number 1 before it was listed (the event page answers 200 with a message when there is none, so only the confirmation showed it) | `serv-pages` and `serv-roles` wait for the problem and use its number; a redirect's message is shown (`fc1c3da`) |
| `serv-streams` | `a bad download opened no problem of the console's` | the same: the events list read once, at once | it waits for `A3007110` (`fc1c3da`) |
| `serv-own` | `the answer has no end to its head` | `kill` returned before PID 1 saw the service end, so `webd_ready` read the killed `webd`'s `ready` and the suite signed on to it | `kill` waits until the killed process is gone (`fc1c3da`) |
| `install-display` | the installer's first screen did not move on | not `ae31cbd`: the installer runs no web server; it passed in the nightly at the same commit | none; it passed at `119b2e4` |

### The soak's second failure: a product fault

At `119b2e4` the soak's first partition was made, but its Change was
refused with `QSI0102 64 requests are outstanding`. The page said
`QMC3130` beside the refusal and kept the wizard's draft, so the next
partition made on that host had `VD06` twice (`QVS1209`). The cause was
`mgmtd`'s: after a host's partitions are listed it reads each one's
profiles, uses, drives and functions at once, about 120 requests for
the soak's 30 running partitions, past QSI's 64 outstanding. While
they were out, any operator's request to that host was refused, and
the reads past the limit were dropped without a word. A host of 17 or
more running partitions could meet it.

`mgmtd`'s own reads now wait while fewer than 16 of a session's
request slots are free and go as replies come, so an operator's
request always has room (`4d6e603`;
`a_host_s_reads_wait_for_room_and_leave_some` reads 40 running
partitions: at most 48 outstanding, the operator's request taken, every
read sent once). The soak's create now fails on a partition not made
whole (`b8763be`). A five-minute soak of 2 hosts in WSL then passed:
124 turns, 21 partitions made and deleted, 5 plans captured and
deployed, no service restarted, descriptors and memory back to idle.

### The soak in CI

nightly 45260315948 at `b8763be`, `mgmtd`'s fix in: 1,800 s, 335 turns
over 100 hosts (3,000 partitions); 13,400 pages by 4 roles, 56
partitions made and deleted, 56 systems dropped and back, 12 plans
captured and deployed, terminals open throughout; no service
restarted; staging and temporary files as idle. Held (idle/peak/end):
`mgmtd` 115/115/105 descriptors, 38,960/50,636/50,572 KiB (its 100
hosts' sessions); `webd` 27/27/7, 6,848/8,352/8,028 KiB; every other
service within a descriptor and 1 MiB of idle.

### Baselines

`sec-baselines` at `ae31cbd`, recorded not held: the sign-on page
46,603 ms after the machine started; sign-on 62 ms; the Partitions list
43 ms with 25 partitions (last page 42 ms), 45 ms with 500 (44 ms),
61 ms with 5,000 (61 ms). Base ready is `status`'s: 1.17 s on one disk,
1.21 s mirrored.

### The release, built twice

nightly 45216378918's release job at `ae31cbd`, with keys made for the
run and thrown away: `build release 1.0.0` built twice from clean, "12
files the same, byte for byte"; `stream add` and `stream verify`
passed; `sec-secure-boot` (the UKIs verify against the run's
certificate, start under OVMF's Secure Boot with it enrolled, are
refused without it; the test image refused) and `sec-stream` (V1R0M0
found on the release's own stream, with its notes) passed. Its
hashes are that run's key's, not the release's: the release's are
written here when Cody builds it with the release keys
(`docs/RELEASING.md`).

## The release

V1R0M0, build `94c380b`, tagged `v1.0.0`: release run 45274171896,
with the release keys (the signing key's public half
`c4cc6899a0ee785ca42918ff29c1277174c8e51d52a066b9d825bb385a72e12b`,
the Secure Boot db certificate's SHA-256
`F4:B5:72:24:F3:64:90:63:F1:2B:05:FB:F2:58:F1:A0:F6:84:4D:2A:98:CB:79:E5:3D:F5:82:45:28:9E:FE:5F`),
built twice from clean and the same byte for byte. The directory is the
run's artifact `release-V1R0M0`, its `qsmc` stream beside it.

| SHA-256 | File |
| --- | --- |
| `230911d2d85b186bf5821d23abb9b07564e76990417d0d3a9ed1842e094362ac` | `MANIFEST.json` |
| `dd6ec9cfbe63cf2359177c2d932ef2a655c06d06cc2859b899c8890dd5d61549` | `MANIFEST.sig` |
| `037e2651ca262c2f5dafc285f5cfef0ddda0dd55c19a52ca1ca13818280748b1` | `NOTICES.txt` |
| `db57a31165df19440ef508cccd7bbcd6b01f1846869c6da238b58c9c27835c72` | `QSMC-V1R0M0-27000675a950.iso` |
| `fd803c2c7b28656ff2eb6973145d5f7e5606cc50f37ed3411c8fb1cf3e3b9947` | `QSMC-V1R0M0-update-fd803c2c7b28.upd` |
| `9d66ef4a9a54f2ce52c3503da1db9e74f4b2960c57c0c246b232ac5a5e0aa9d1` | `RELEASE_NOTES_V1R0M0.md` |
| `3c8662e5c194bd32fc86134efc5d34994217d8d89d3e066a910c4400005011c3` | `catalogue.json` |
| `6c62e40def94df69400351f4be242dbd4203069fdf55a3a20e1c982a9d980cc1` | `catalogue.schema.json` |
| `f4b57224f3649063f12b05fbf258f1a0f6844d2a98cb79e53df58245289efe5f` | `qsmc-db.cer` |
| `08c834300580712efdc24c71846c4b97e4b8cce0798182a6e093bb96b13cabc7` | `stream/index.toml` |

The code at `94c380b` is `fd75ab2`'s, whose QEMU run 45267308132
passed every suite; `94c380b` changed documents alone.

V1R1M0, build `bbb8e8c`, tagged `v1.1.0`: release run 45499599931,
with the same keys (`qsmc-db.cer` unchanged), built twice from clean
and the same byte for byte; artifact `release-V1R1M0`. Its CI run
45461829348 and QEMU run 45461829159 passed at `bbb8e8c`. The GitHub
release's files match its `SHA256SUMS`.

| SHA-256 | File |
| --- | --- |
| `c958d2b5e46e0995c0ffce3fa012a80dd070e28cbbf9ab5667ecec048d3889f7` | `MANIFEST.json` |
| `b571940950f0ec7531db01938c4a8205982fd71f59093135f65f793ffe783dcd` | `MANIFEST.sig` |
| `b97cc993d5204a130f95ec536c9400f9bfb6af1731f3c938ea6e60351437ad1e` | `NOTICES.txt` |
| `6390c22d4e50ca81c19aa3faa58b1471935f6b200719e30f810fa34bc8a576ed` | `QSMC-V1R1M0-d0b5485b1726.iso` |
| `090a5ff800ec26a4a0bc95a50f0e3cb883d7dc6c9e7c49c5f58d2a4612ec7fc1` | `QSMC-V1R1M0-update-090a5ff800ec.upd` |
| `d1ba53c310b189e7e00711b19a0d8676752c1a46b6729b36d13c41fd3e65f0a4` | `RELEASE_NOTES_V1R1M0.md` |
| `f693d3d7243d7f20549fa79852b56eafb6e7129ab1d380c84ad15166ade244e3` | `catalogue.json` |
| `6c62e40def94df69400351f4be242dbd4203069fdf55a3a20e1c982a9d980cc1` | `catalogue.schema.json` |
| `f4b57224f3649063f12b05fbf258f1a0f6844d2a98cb79e53df58245289efe5f` | `qsmc-db.cer` |
| `e27d9a1d9d5da20d7b53d75a3154fcb693d44f50d98b0472cde743f9ae8c1d38` | `stream/index.toml` |

V1R1M1, build `2f84225`, tagged `v1.1.1`: a fix release of V1R1M0 (the
Create partition wizard's profile name and load source, an uploaded
image's name). Release run 45553670103, with the same keys
(`qsmc-db.cer` unchanged), built twice from clean and the same byte for
byte; artifact `release-V1R1M1`. Its CI run 45538704218 and QEMU run
45538704225 passed at `2f84225` (shard 5's `install-display` passed on
its rerun, the Enter on the welcome screen lost the first time). The
GitHub release's files match its `SHA256SUMS`.

| SHA-256 | File |
| --- | --- |
| `6109cca2969368625500fa6c8d65b9b188cc37f73e065d21690ffbca953097b8` | `MANIFEST.json` |
| `ac48a2e8623ffa2a545f25fcf222a2dd136dd7c54198ce090867707622351334` | `MANIFEST.sig` |
| `ceea38f2a10b0dbfda6e197160659787c48e039f64791288212c0386b38d1ab0` | `NOTICES.txt` |
| `a92f19121538f9a33602b6f70b4381b7c0563d961b28a6ec86bc813da1c3a93b` | `QSMC-V1R1M1-939fd011b042.iso` |
| `9a38d9430ae449d48c6437614f5056c8ba2727404cf0fdb781fd4995ded2e6d6` | `QSMC-V1R1M1-update-9a38d9430ae4.upd` |
| `7778520339b9b52be9d7283ea335b44bdc411edee200ac968f1a206134bb2d95` | `RELEASE_NOTES_V1R1M1.md` |
| `40b8dbd7566b7b86dab6e3326e93d3cdcab4942a379d0168a32d121835d6f3d1` | `catalogue.json` |
| `6c62e40def94df69400351f4be242dbd4203069fdf55a3a20e1c982a9d980cc1` | `catalogue.schema.json` |
| `f4b57224f3649063f12b05fbf258f1a0f6844d2a98cb79e53df58245289efe5f` | `qsmc-db.cer` |
| `e2da4aad14f69888beff38cb43e296c3597ee4a2f810706ea66b95f4013fcf28` | `stream/index.toml` |

## Every `BLOCKED` line of Phases 0 to 7

Phases 3 to 7 wrote none; every one of Phases 0 to 2 is done, and none
is left for the release notes:

| Where | Line | Done |
| --- | --- | --- |
| `PHASE0.md` | the processor check is QS/MC's own copy (item 3) | `base/processor`, Phase 2 (item 3 closed) |
| `PHASE0.md`, `PHASE1.md` | the QEMU driver is QS/MC's own (item 4) | `base/qemu` as `driver`, Phase 2 (item 4 closed) |
| `PHASE0.md` | no base event has a reference code | the console's codes, Phase 1 (`docs/REFCODES.md`) |
| `PHASE0.md` | PID 1 does not read the A/B record or write boot entries | Phase 1 (`lic/qsmcinit/src/slot`) |
| `PHASE0.md` | the ISO's name is `buildkit`'s (item 8) | settled as `buildkit`'s `QSMC-<release>-<hash>.iso`, as QS/VS's (item 8 closed here) |
| `PHASE0.md` | the UKIs are not signed for Secure Boot (item 9) | `build release` signs them with QS/MC's db key (Section 3; item 9 closed here) |
| `PHASE1.md` | no hardware inventory (item 12) | `base/inventory`, Phase 2 (item 12 closed) |
| `PHASE1.md` | one network interface (item 11) | `open` and `private`, Phase 2 (item 11 closed) |
| `PHASE1.md`, `PHASE2.md` | a mirrored unit absent at the IPL is copied whole (item 13); a degraded IPL waits twenty seconds (item 14) | QS/400's resume map and QS/VS's wait, Phase 5 (items 13 and 14 closed) |
| `PHASE1.md` | the unit to keep of two that ran apart is chosen through a hook only | the local console's panel, Phase 2 (`apart-panel`) |
| `PHASE1.md` | nothing writes the other slot or sets a pending slot | the console's own update, Phase 6 (`docs/UPDATES.md`) |
| `PHASE2.md` | the media's check, the disk unit panels, the power words, the installer's writes are QS/MC's own (items 15 to 18) | `base/loadsource`, `panels::units`, `panels::tools::power`, `install::write`, Phase 5 (items 15 to 18 closed) |

## What exists

| Section | Where | What |
| --- | --- | --- |
| 1, confining every service | `lic/sandbox` | Landlock's paths and TCP ports (`landlock.rs`), each service's policy (`policy.rs`), the call every service makes before its first thread; `base/confine`'s seccomp filter; `confine-learn` and `confine-trip-<service>` in a test image; `A3001085` and `QMC7342` when a filter ends a service. Cody chose Landlock beside `base/confine` (open question 50). `docs/SECURITY.md` has the table |
| 2, hardening | `web/webd`, `web/pages`, `lic/qsmcinit`, `tools/sim` | port 80's redirect kept on the console; a body said two ways refused (open question 49); the terminal socket from the console's own page only; the root read-only after the IPL, its writable folders bound `nodev,nosuid,noexec`; the simulator's `markup`, `lie` and `bad-screen` |
| 3, keys and signing | `lic/release`, `tools/build` | `QSMC_KEY_KIND` and the Signature fact and row (`release key` or `development key`, Secure Boot on, off or unknown); a development key said at the IPL and on Console version; `QSMC_TRUSTED_KEYS` for a rotation; the `rotated` and `stranger` variants; `docs/RELEASING.md`; open question 9 closed |
| 4, the release build | `tools/build/src/image/release.rs`, `host/catalogue.rs` | `build release`, `build stream add` and `verify` over `buildaid::release`; `catalogue.json` and `docs/catalogue.schema.json`; open question 8 closed |
| 5, updating | `tools/conformance/src/suite/serv/console.rs` | `sec-rotation` beside Phase 6's `serv-console`, `serv-refusals`, `serv-local` and `serv-power` |
| 6, fuzzing | `tools/fuzz` | four more targets: `web-request`, `terminal-input`, `sched-store`, `seed-answers`; `.github/workflows/fuzz-long.yml`, every target for four hours |
| 7, soak | `sec-soak`, `sim::fixture::fleet` | the soak, run by name |
| 8, limits | `web/users`, `web/pages`, `sched/schedd`, `mgmt/mgmtd` | web users 256 (`QMC8012`), groups 256 (`QMC3304`), schedules 500 (`QMC3410`), operators' plans 500 (`QMC6115`), sessions 256 (`QMC9007`), uploads to a host at once 4 (`QMC2442`); the managed systems' limit held where a system is kept; `sec-limits` |
| 9, the design's acceptance | `sec-web` | every link of every page resolving, beside Phases 3 to 7's suites |
| 10, what V1 leaves out | `docs/DESIGN_REVIEW.md` | every choice, page and field, for the design |
| 11, documents and audits | `tools/build/src/host/doctor.rs` | `build doctor`'s rows for other vendors' names and every panel's help, beside the headers and the catalogue; `docs/OPERATIONS.md`; `docs/INSTALL.md`'s Secure Boot; `docs/LOCALCONSOLE.md`'s Signature row |
| 12, CI | `.github/workflows/nightly.yml` | every night: the full suite (`qemu.yml`), the release twice and compared, its stream, Secure Boot, a short fuzz of every target, a half-hour soak; the 24-hour soak is Cody's (`NEEDS-CODY`) |
| 13, the release candidate | `docs/RELEASE_NOTES_V1R0M0.md`, `docs/QUALIFICATION.md` | the notes and the runbook |

### The suites

| Suite | What it shows |
| --- | --- |
| `sec-confined` | every service under its filter with no new privileges; at one IPL each, each service's forbidden call ends it, `A3001085` is posted, it is started again, and no other service fails |
| `sec-machine` | the root read-only; `/data`, `/run`, `/proc`, `/sys`, `/etc` and `/tmp` `nodev,nosuid,noexec`, `/dev` `nosuid,noexec`; dmesg and ptrace restricted; the baked command line; a service under its filter |
| `sec-web` | a body said two ways refused alone; no path climbs out; no redirect leaves the console; a session fixed before sign-on is not taken and one signed off is no session; every page crawled with its headers and policy, every link resolving, every form refused without its token, nothing changed by GET; error pages show nothing of the inside; the terminal socket refuses another origin, none, and no session; the password hash's parameters and sign-on's time |
| `sec-systems` | markup in names, a model, a code and console cells shown as text; a hello that lies shown as text; a wrong screen answered; with one system a minute slow, no page about the others waits; no service notices |
| `sec-secrets` | after a pairing, a user's passwords, a backup's passphrase and a dump, no secret in any file under `/data`, no hash or key outside its own file, none on any page |
| `sec-limits` | each limit filled and one past it refused |
| `sec-rotation` | V1R0M1 signed by a key not trusted refused with nothing changed; V1R0M1 signed by the next key installed and confirmed, its systems connected again |
| `sec-power` | the power pulled at random moments while the writers outside `base/state` write: 8 cuts while a host posts codes and opens problems (`problems.bin` whole and not set aside, no number given twice or lost, every history line whole and once), 5 among certificate imports (an imported pair served after each, none made again, no half pair or mark left), 3 just after a restore is confirmed (applied, or taken back, the users as that says) |
| `sec-imports` | every file of the `backup-file`, `cert-import` and `system-plan` corpora sent through its page: each answered with a page and its message, never an error of the inside; no service restarted; the plans kept the ones imported |
| `sec-secure-boot` (by name) | the release's UKIs verify against its certificate, start under OVMF's Secure Boot build with it enrolled, and are refused without it; the test image refused |
| `sec-stream` (by name) | the release's own stream, served, found by a console |
| `sec-soak` (by name) | the soak |

## Acceptance

| # | Check | Evidence |
| --- | --- | --- |
| 0 | The last pins, every Phase 0 to 7 suite on them, each golden change explained | Step 0; QEMU 45086679139 |
| 1 | Every service under its filter and its file-system rules; a forbidden call ends that service alone, posts `A3001085` and it starts again | `sec-confined`, `sec-machine`; QEMU 45193087442 (enforced) |
| 2 | Hardening: no redirect leaves the console; a body said two ways refused; no path climbs out; the terminal socket from the console's page only; the root read-only; a hostile managed system shown as text | `sec-web`, `sec-machine`, `sec-systems`; QEMU 45193087442; the one fuzz finding fixed (`5854cee`) |
| 3 | Keys and signing: a development key said; a rotation installs, a stranger's key is refused | `sec-rotation`; QEMU 45193087442 |
| 4 | The release built twice from clean, byte for byte; its stream verified; its UKIs under Secure Boot | The release, built twice; nightly 45216378918 |
| 5 | Updating: a stranger's image refused with nothing changed; a power pull at any moment of an update leaves a good slot | `sec-rotation`, `serv-console`, `serv-power` (30 pulls); QEMU 45193087442 |
| 6 | Every fuzz target four hours, every finding fixed | Fuzzing; fuzz-long 45113495933 and 45173327969 |
| 7 | The soak: no service restarted, no descriptors, memory or staging left behind | the half-hour soak, nightly 45260315948 (passed: 1,800 s, 335 turns over 100 hosts and 3,000 partitions, no service restarted); the 24-hour soak `NEEDS-CODY` |
| 8 | Each limit filled and one past it refused | `sec-limits`; QEMU 45193087442 |
| 9 | The design's acceptance: every page's links resolving, no script but the terminal's | `sec-web` (279 pages, 230 forms) beside Phases 3 to 7's page suites; 86 of the prototype's 90 pages live (`docs/PHASE7.md`); in a browser, `NEEDS-CODY` |
| 10 | What V1 leaves out, written for the design | `docs/DESIGN_REVIEW.md`; its answers `NEEDS-CODY` |
| 11 | Documents and audits: the headers, other vendors' names, every panel's help, the catalogue | `build doctor`; `ci` green at every commit above |
| 12 | CI: the nightly runs the full suite, the release, a short fuzz and the soak | `.github/workflows/nightly.yml`; nightly 45260315948 |
| 13 | A release candidate: the notes and the runbook | `docs/RELEASE_NOTES_V1R0M0.md`, `docs/QUALIFICATION.md` |
| - | Power pulled while every writer outside `base/state` writes; each file whole after | `sec-power` (Power pulls and hostile files, below); QEMU 45267308132 (seed 41659813) |
| - | Imported files as hostile, through the pages | `sec-imports` (Power pulls and hostile files, below); QEMU 45267308132 |
| - | No secret in any file under `/data` or on any page | `sec-secrets`: 21 files and 316 pages at `ae31cbd` (at `fe3608d` it walked no folder; `6d19743`) |

## What the tag waits on

Each is `NEEDS-CODY`, in `docs/QUALIFICATION.md`'s order:

- The release keys are made: the signing key and the Secure Boot key
  and certificate are the repository's secrets (`QSMC_SIGNING_KEY`,
  `QSMC_SB_KEY`, `QSMC_SB_CERT_B64`), and the three streams' public
  keys are in `tools/build/keys/streams/` (`qsmc.pub` checked against
  the signing key's own). `.github/workflows/release.yml` builds the
  release with them, twice and compared, and its stream: the release
  run at the commit to be tagged, its directory's hashes written here.
- NEEDS-CODY: real machines: a console installed on a real machine and
  as a virtual machine, Secure Boot with QS/MC's key enrolled, the Dell
  as the real host and a standalone QS/400 (steps 2 to 5).
- NEEDS-CODY: the design's answers: every item of
  `docs/DESIGN_REVIEW.md` answered, and every page beside its prototype
  file, at 1024 by 768, with scripts off and with the stylesheet off
  (step 6).
- NEEDS-CODY: the 24-hour soak beside the Dell (step 7); the nightly's is
  half an hour.

## Power pulls and hostile files

Phase 6's `power-pull` churns `base/state`'s files. Four writers write
around it, and each could leave a file half written:

| Writer | Before | Now |
| --- | --- | --- |
| `mgmtd`'s problems (`/data/mgmt/problems.bin`) | a new file renamed over the old, neither synced: after a power loss it could be empty, set aside as `.bad`, and the console's problem numbers given again from 1 | `state::atomic_write` |
| `mgmtd`'s history (`/data/mgmt/refcodes/<system>.codes`) | appended; its rewrite renamed unsynced. A line cut short took the next code's line into it | the rewrite through `state::atomic_write`; a cut last line removed when the file is opened, its code read again from the system (`a_line_cut_by_a_power_loss_goes_and_the_next_is_whole`) |
| The web certificate (`/data/web/certificate.pem`, `key.pem`, `made-here`) | the key and the certificate renamed one after the other: a cut between left a pair that does not match, and the web server made a new self-signed one over an imported certificate. The mark changed after the pair, so an imported certificate could be marked the console's own and made again at the next address | both new files written and synced, then renamed and the folder synced; at the start a pair cut short is finished when its new halves match, else taken back (`a_pair_cut_short_is_the_old_pair_or_the_new_one`); the mark first, synced |
| A restore confirmed (`/data/restore/pending`) | renamed unsynced: a power loss after OK could lose the restore the page said would happen | the folder synced before the answer (`9d554ff`) |
| `/data/restore` itself | removed by PID 1 when a restore was confirmed, while the web server ran: its Landlock rule held the folder removed, so every backup opened after a restore was refused (`Permission denied`) until the web server restarted. Found by `sec-power` | emptied, the folder kept (`fd75ab2`); `qsmcinit`'s restore tests hold it |

`sec-power` pulls the power at random moments of each, and
`sec-imports` sends the fuzzers' corpora through the three pages that
take a file. In WSL at `fd75ab2`, `sec-power` (seed 775244109): 8 cuts
while QSHQVS1 posted codes and opened 168 problems, `problems.bin`
whole each time, no number given twice or lost, 4,497 history lines
each whole and once; 5 cuts among 101 certificate imports, an imported
pair served after each; 3 cuts just after a restore was confirmed, each
applied with the users it held. `sec-imports`: 379 backups (all
refused), 897 certificates (all refused), 3,087 system plans (6
imported, 3,081 refused), each answered with a page and its message,
no service restarted. Before passing, `sec-power` was itself corrected
twice: the events list shows the newest 100, so each problem is asked
for by its number and its code (`6a6ad33`, `4b9be3e`), and the churn
opens 20 problems a cut, under the 500 kept (`ef0b67d`).

## After the release: the tree and the lists as QS/400's

Cody asked that the web interface behave as QS/400's Navigator does.
The tree's `[+]` and `[-]` now open and close a group in place, several
at once, kept for the session (`qsweb::session::nav_visit`), where `[+]`
used to load the group's first page. Every framed page loads `qsweb`'s
navigation script: each list's Filter row and sorted headers, pages
swapped in place. This replaces the locked decision that the terminal's
was the only script (`docs/ARCHITECTURE.md`, `docs/SECURITY.md`,
`docs/WEB.md`). The page suites now allow that one script tag. Their
crawls follow each `[+]` once and never `[-]`. `web/pages/tests/app.rs`
holds the tree.

Then, also at Cody's request:

- System Plans is an item of Systems Management and Updates an item of
  Service Management; the prototype's tree and crumbs say so.
- A QS/VS host's pages have a Virtual resources tab, also shown above
  the virtual resources' own tabs; General's row for them is gone.
- Each breadcrumb entry but the last links to its page
  (`web/pages/src/crumb.rs`), on the served page only: the prototype's
  crumbs are text.
- A visited link stays blue: the console serves `qsweb`'s stylesheet
  with a few rules after it (`webd::serve::CSS_MORE`).
- A switch's uplink interfaces are chosen from a list that drops down,
  a checkbox each: the host's free interfaces and the switch's own.
- The default update server is `https://updates.quartzsystems.net`;
  QS/400's `docs/UPDATE_PROTOCOL.md` still names the old one.
- An image upload refused (by the token, the role, the size, `mgmtd`
  or the host) read none of the rest of the body, so the connection
  was reset under the browser instead of showing why; the rest is now
  read before the answer (`web/pages/tests/app.rs`).
- A system's I/O tab names each device's type (PCI's class code),
  vendor and IDs, who uses it, and whether a partition can be given it
  or why not, with its isolation group (`web/pages/src/systems/hw.rs`).
  A host has a Network interfaces tab: each port's MAC, link, driver
  and use.
- Releasing a device for partitions from the console is not done:
  QSI's host family reads devices only, and a device is released at the
  QS/VS host (Service Tools option 10). It needs a release and a
  take-back request in QS/400's `qsi`, served by QS/VS; the pages say
  where to release one meanwhile.
- A standalone system's Servers action Virtual resources goes back to
  its General tab with `QMC1114`: only a host has the tab.
- A backup holds the managed systems' reference code histories
  (`mgmt/refcodes/`, from `mgmtd`) when Reference code and event history
  is checked; the media library index is no longer offered: the
  library is each host's own (`docs/DESIGN_REVIEW.md`, 1.3 and 1.4).
  The remote host's fields are on one line.
- The wordmark and the title suffix are `Quartz Systems/Management
  Console` (`release::PRODUCT`), on every page and golden.
- The terminal window works as Access for Web's 5250 display: a click
  on the screen places the cursor (or presses the F key under it), the
  menus open their own items, Keypad hides and shows the keypad, and
  Copy, Paste and Keyboard Map work with and without script.
- Create virtual tape drive... makes a tape device for a partition
  (`QMC2436`); before, its form made only a media library volume, so
  no drive appeared on Virtual Tape.
- A standalone QS/400 has its own pages: General (its cumulative fix
  level, no Partitions group), Status, Dumps, Capabilities and
  Reference code; no Processors, Memory, I/O or Power-on parameters,
  which are a host family's. Status is QSI base's `Resources` (bit 28,
  added to QS/400's `qsi` and served by its `*QSI` job for this:
  processors, processor used, main storage, the system ASP, temporary
  storage, jobs), asked when the tab is shown; Servers lists the
  processors and main storage read when it connected. Its Partitions
  row already opened these pages.
- The pins moved together for it: QS/400 from `960861dc` to
  `de8e467f066f30acd38b00a753dac97374e767f7` (its `main` after
  V1R2M0, with bit 28) and QS/VS from `dec674bd` to
  `c65646262a73d7e9c07003c5ec65b4233feec1fc`, which pins that QS/400
  commit and refuses bit 28 as a host. Of the crates taken here only
  `qsi` and `qsisess` changed; `kernel/`, the toolchain and `deny.toml`
  did not. The workspace's and the fuzzer's manifests and locks.

## Not done

- For QS/VS: `refcodes.ndjson` grows without a bound; the fix is QS/VS's.
