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
