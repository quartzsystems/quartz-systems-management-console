# Phase 6: service management and updates

What Phase 6 builds, how to run it, and the evidence for the acceptance
table. Phases 0 to 5 still hold (`docs/PHASE0.md` to `docs/PHASE5.md`);
what changed is said here.

Phase 6 is what an operator does when something breaks or needs a new
level: serviceable events across the fleet and the console, dumps, code
updates of hosts and of the console itself, and the fleet's levels on
one page. It makes the console serviceable in its own right: its own
problem log, its own service dump, and its own update into its other
slot with roll back.

No pin moved: QS/VS at `0488d96b067358688f35f34b86a2a4662bcfe93f`, QS/400
at `cf8f01453846fdf78129ca79589e18798aa03fae` (`docs/PHASE5.md`).

## What exists

| Where | What |
| --- | --- |
| `lic/qsmcprobd` | the console's problem log: `base/problem` with QS/MC's table, at 12 in PID 1's table, no capability (`docs/SERVICE.md`) |
| `lic/qsmcupdd` | the console's update service: `base/update` with QS/MC's image format, codes and declared schemas, at 35 (`docs/UPDATES.md`) |
| `upd/fetchd`, `lic/qsmcfetchd` | the download service: the three streams, their keys, checks, downloads verified and resumed, its settings; at 47, its own user for good (`docs/UPDATES.md`) |
| `lic/consdump` | the console's service dump over `base/servicedump` |
| `lic/qsmcinit` | the slot requests (write, pending, cancel, activate); the state kept before a pending slot's first IPL and put back on a roll back (`update::state_copy`, and `ipl/carry.rs` for `mgmt`, `web`, `fetch`: item 41); a panic's records at IPL (`logd::panics`), posted with a dump; a dump when a service gives up and when a slot fell back; pstore mounted |
| `lic/release` | the new services' names, users and folders; `trial`: a service migrates state only once its slot is confirmed |
| `lic/refcode` | `A3001005`, `A3005030`/`C3005040`, the `C3006`/`A3006` and `C3007`/`A3007` blocks (`docs/REFCODES.md`) |
| `mgmt/mgmtd`, `mgmt/proto` | per system: its problems with the console's numbers (`problems.bin`), its dumps and their space, its update status and log; the requests `problems`, `problem`, `stage`; a dump's download and an image's stage as streams with credit (`docs/MGMT.md`); the unreached limit |
| `sched/schedd` | Install code update for a host and for the console, the page's checks at run time (`docs/SCHED.md`) |
| `web/pages` | `service/` (Serviceable events, the event detail with Close, Add note, Export and Repair, Manage dumps), `updates/` (Code updates with its settings and Check now, the install confirmation, progress and log, the notes, Updates), Welcome's counts live, the web certificate's end watched (`certwatch.rs`) |
| `web/msgs` | `QMC7001`–`QMC7019`, `QMC7101`–`QMC7117`, `QMC7201`–`QMC7210`, `QMC7301`–`QMC7341`, `QMC7401`–`QMC7402`, `QMC7501`–`QMC7506` (`docs/MESSAGES.md`); `QMC3408` gone |
| `web/backup` | `fetch/updates.toml` in a backup (`docs/BACKUP.md`) |
| `svc/console` | option 8, Service functions (`docs/LOCALCONSOLE.md`) |
| `tools/build` | `build update` (`base/buildaid`'s update image with the declared schemas), the streams' keys into the image, the test image's variants (`--variant next`, `--variant broken`) |
| `tools/sim` | the `service`, `update` and dump families with event 1042, dumps, slots and scripted outcomes (`docs/SIM.md`) |
| `tools/conformance` | a stream server of its own; `suite/serv/`, Phase 6's suites |
| `tools/fuzz` | `mgmt-servicing`, `stream-index`, `qsmc-update-image` |

## How to run

```
cargo run -p build -- all && cargo run -p build -- --test-image all
QSMC_RELEASE=V1R0M1 QSMC_NEXT_SCHEMA=1 cargo run -p build -- --test-image --variant next all
QSMC_RELEASE=V1R0M1 QSMC_TEST_WEBD_BROKEN=1 cargo run -p build -- --test-image --variant broken all
cargo run -p build -- test                      every suite of Phases 0 to 6 but those of a real machine
cargo run -p build -- test --suite serv-console
QSMC_QSVS_DISK=... cargo run -p build -- test --suite serv-real-host
QSMC_QS400_IMAGE=... cargo run -p build -- test --suite serv-real-standalone
cargo run -p build -- fuzz --target stream-index
cargo test -p webpages --test golden            every live page against its prototype file
```

The pages landed in one commit (`da776f7`), each with its golden test
passing, not one commit a page as Section 2 asked: they share the
service and updates modules, and were written and compared together.

## The pages

Steps 23 to 26 of the prototype's build order, each equal to its file
under `qsweb`'s comparison with the corrections in `docs/WEB.md`:

| Step | Prototype | Route |
| --- | --- | --- |
| 23 | `events.html`, `event-detail.html` | `/qsmc/events`, `/qsmc/event/<n>`, `/qsmc/event/console/<n>` |
| 24 | `dumps.html` | `/qsmc/dumps` |
| 25 | `code-updates.html`, `code-update-progress.html` | `/qsmc/updates/code`, `/qsmc/updates/code/<target>/progress` |
| 26 | `updates.html` | `/qsmc/updates` |
| | `welcome.html` (live since Phase 3) | its serviceable events, dumps and updates counted |

80 of the 90 files are live. The ten that are not are system plans'
six (Phase 7) and Navigator's four (QS/400's own pages). Built from the
shared patterns without a design of their own: Repair
(`/qsmc/event/<n>/repair`), Add note (`/note`), the close confirmation
(`/qsmc/confirm/event-close`), the dump delete confirmation
(`/qsmc/confirm/dump-delete`), the install confirmation for a host and
for the console (`/qsmc/confirm/update-install`), View log
(`/qsmc/updates/code/<target>/log`) and the notes.

## Not done

- NEEDS-CODY: the three streams' release keys (`qsmc.pub`, `qsvs.pub`,
  `qs400.pub`, `QSMC_STREAM_KEYS`); the build refuses a release image
  signed with a release key without them.
- NEEDS-CODY: a real update of the Dell's host and of a real console,
  with a forced roll back of each.
- NEEDS-CODY: a disk pulled by hand on a real host and its event read at
  the console.
