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
| 12, CI | `.github/workflows/nightly.yml` | every night: the full suite (`qemu.yml`), the release twice and compared, its stream, Secure Boot, a short fuzz of every target, a one-hour soak |
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
| `sec-secure-boot` (by name) | the release's UKIs verify against its certificate, start under OVMF's Secure Boot build with it enrolled, and are refused without it; the test image refused |
| `sec-stream` (by name) | the release's own stream, served, found by a console |
| `sec-soak` (by name) | the soak |
