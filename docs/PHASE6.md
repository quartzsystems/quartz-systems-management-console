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

## Evidence

### The gate and two builds

At `3c14d2d`, in a fresh clone on the build machine
(`~/qsmc-p6-ev/run.log`). `build doctor`, `build deny`,
`build notices --check`, `fmt --check`, the three clippy runs and
`cargo test --workspace` passed. `build all` ran twice, the second with
its own empty targets and the first's signing key. Every image, the
update image and every binary is the same in both:

| File | SHA-256 |
| --- | --- |
| `qsmc-V1R0M0-9d5e0b31624e.img` | `8777f41b9f7707befd201b63dcf22b098ee7e7968f44faa3ffcadb912faca630` |
| `qsmc-V1R0M0-a-9d5e0b31624e.efi` | `9d5e0b31624efef26525a58c9362b2e49904a7040ae016bc3b0610f8c179d39c` |
| `qsmc-V1R0M0-b-6c94128b90cb.efi` | `6c94128b90cbe499430179d41844a80b9867c1712d36ae7918ba310a0f02aebf` |
| `qsmc-V1R0M0-d-4ec3d9d114b4.efi` | `4ec3d9d114b443dad56435d084dc5dbf2abdfac6374dd99600a6699de763866b` |
| `QSMC-V1R0M0-4ec3d9d114b4.iso` | `84bd9021c190830daf55093bd637d82643724b1a90573d60049537d7f35eaa1f` |
| `QSMC-V1R0M0-update-1874ead67045.upd` | `1874ead670450bc3c97036f7cf959e4d42c61524c9cbaac98d34d295e4faa7b0` |
| `initramfs.cpio` | `f5d2efb3b5bcbbe8057372afa1b0e0377607b27166502705196e742137425a74` |
| `initramfs-media.cpio` | `18fea369bb70c55f9570e70d606ac87a1a215060fc3cc19b81c5dbd201b5097f` |
| `qsmcinit` | `a19ba535f744356b0950b0805e1d55ed9d1f2d5ee735d55cd2a9a186bdedf7c8` |
| `qsmcstgd` | `b39844a4d5d5560d462281f6af4f5d0fb45fa5154ced78a303ecd789b9e85eb5` |
| `qsmclogd` | `5e64b16279fe96760aac6243e627d8b1821ec7ac36e606f799916bc061d64929` |
| `qsmcnetd` | `12492fe347ee678be3bb71610390e0f4f1828a209252b0e993255930a0ed1392` |
| `qsmccons` | `15f611c210aa5d8ef5606d4a067f47c5141a8d21009f80fdb9683ded663381de` |
| `qsmcinstall` | `8527c9bfd92760fe767dd9f78e794c94215d15bbb79dce68c31e118ef53a8e55` |
| `qsmcwebd` | `ca3921b0a438a2d219b9f1c1633fcd95910f3707ea60292b5cf26afba893a97d` |
| `qsmcmgmtd` | `75f5b7cf157e6559e105881c21b8aa15ce7ffad90dc48557c7f3b1a6fbdbdec4` |
| `qsmcschedd` | `5ef80aaf449fa908cfd6e425751a101e080ff8b69d2d896b95e5c7bf39281949` |
| `qsmcprobd` | `e1998f570df36de7676eef5cb50392900fb93374dffe15e0e79d081409434f99` |
| `qsmcupdd` | `534d4cd6b90ef28f452b8d9ee6668f552b38c103f7a033e7de85c250840383d8` |
| `qsmcfetchd` | `a4eeb617e760de8c964d3af61481c6145f26479f81e2ff4e49d2bf3b351ddf20` |

The kernel's `.config` and the manifest differ between the two, and
only by the target folder: `CONFIG_INITRAMFS_SOURCE` names the
initramfs by its absolute path, and the manifest holds the `.config`'s
hash. The release image holds none of `qsmchook`, `TEST IMAGE`,
`qsmc.hook`, `hook=`, `Pattern reference`, `webd-fails`, `short-trial`.

`build deny` needed `url` 2.5.9 for an advisory against 2.5.8 (through
buildkit's HTTP client): `Cargo.lock` only, no pin moved (`62e70b2`).

### The suites

`build test` on the same clone: 85 steps and suites of Phases 0 to 6,
every one passed, 07:34 to 11:04 with both builds. Phase 6's:

| # | Suite | Time | What it showed |
| --- | --- | --- | --- |
| 1 | `serv-pages` | 61 s | 24 pages of the phase answered 200 with no script, 28 they link to, 22 forms with the token |
| 2 | `serv-roles` | 77 s | 100 route, method and role pairs as the matrix says, 18 Not authorized |
| 3 | `serv-events` | 60 s | a host's events opened, seen, cleared, closed at the host, noted and exported; 1042 refused without the leave; the numbers the same after a restart |
| 4 | `serv-own` | 83 s | A3007110, A3005030, A3006050 opened and cleared by their conditions; 40 codes with the problem service killed twice: 40 problems, none lost or doubled |
| 5 | `serv-dumps` | 62 s | 256 MiB through the console with the host's hash; at the peak the web server and mgmtd grew 128 KiB and 208 KiB and wrote nothing; an abandoned download ended at the host; delete through its confirmation |
| 6 | `serv-streams` | 99 s | QMC7201, 7202, 7204 (none and slow), 7205 with A3007110; a cut download resumed by range; only `fetchd` connected out |
| 7 | `serv-host` | 68 s | the host's shut-down list confirmed, refused without leave (QMC7004); its roll back shown; then V1R0M1 build 26300 on Code updates and Updates |
| 8 | `serv-console` | 161 s | the broken image: slot B on trial, not confirmed, back on A with its 14 state files as they were, A3001035 and a dump; V1R0M1 from the stream installed, confirmed, `web.toml` migrated after the confirm |
| 9 | `serv-refusals` | 48 s | five images from a removable volume: QMC7101 to 7105, nothing pending |
| 10 | `serv-power` | 2464 s | 30 pulls on a mirrored load source, 15 during an install and 15 in the pending slot's first IPL: back ready on a good slot every time with its users signing on (23 on V1R0M1); 3 written slots cancelled at option 8, 5; V1R0M1's slot B on both disks |
| 11 | `serv-local` | 56 s | option 8's 12 screens and helps against their golden files; a problem closed at the serial port; a dump taken, copied to a volume and deleted; V1R0M1 staged from a volume, installed and confirmed |
| 12 | `serv-dumpfile` | 47 s | the console's dump: 22 entries, each in its manifest; `webusers.toml`, the web key, `systems.toml`, `identity.key`, `users.db` left out and said so; no secret in any entry |
| 13 | `serv-sched` | 110 s | an install before the download: QMC7003; with partitions and no time: QMC7004; a host and the console installed at their times |
| 16 | `serv-audit` | 54 s | 11 kinds of action audited with their user; 17 lines, no password, hash or key |
| 17 | fuzz | 60 s each | `stream-index` 6.59 M runs, `mgmt-servicing` 608 k (3201 edges), `qsmc-update-image` 8.06 M; nothing found |

Earlier phases' suites changed: Phase 1's `supervision` takes the
console dump's codes after `A3001080` and checks the code was posted;
Phase 2's `console-panels` and option 8's golden screens hold the new
main menu and help.

Fixed in the product by the suites: the web certificate's first look
waited an hour (`certwatch`); `fetchd` could not read its own folders
on a later IPL (its capabilities before it drops root); `fetchd` lost
its index across a power pull (written atomically and synced); an
install a pull ended left its image staged (`QMC7107` on the next).

### On real machines

- 14, `serv-real-host`, on the QS/VS test disk at the pin under QEMU:
  paired, a pool made on a blank disk, the disk pulled by `drive_del`.
  The host at the pin opened no problem for that unit in 300 s, so the
  note and the close were shown on the host's own problem of its IPL:
  the note in the host's analysis log by admin, the problem closed at
  the host. The pulled disk's own event and a host's update (no QS/VS
  V1R0M1 is signed by a key a host trusts) are for a real machine,
  below.
- 15, `serv-real-standalone`, on the QS/400 image at `cf8f014`: V1R1M0
  paired, its 0 problems and 0 dumps listed with its dump space, its fix
  level `(none installed)` beside the qs400 catalog's `QS400-CUM-26300`.

### Outbound and listeners

Phase 3's `no-secrets` and `web-hostile` passed in the same run: TCP
on 443 and 80 only. `serv-streams` saw only `fetchd`'s connections to
the stream; `serv-dumpfile` and `serv-audit` found no secret.

## Acceptance

| # | Item | Evidence |
| --- | --- | --- |
| 1 | Suites 1 to 13, 16 and 17 pass | The suites, above |
| 2 | Suites 14 and 15 on real machines, or what cannot run listed | On real machines, above; Not done |
| 3 | The pages live, counted | The pages: 80 of 90 |
| 4 | The console updates and rolls back | `serv-console`, `serv-power`, `serv-local` |
| 5 | Two builds byte-identical with the update image; deny and doctor pass | The gate and two builds |
| 6 | base and QS/400 crates used, not copied | `build doctor` (the two pins, no crate twice); What exists |
| 7 | 443 and 80 only, outbound only `fetchd`'s, no secrets | Outbound and listeners |

## Not done

- NEEDS-CODY: two offline copies of QS/MC's release keys, made by
  `build keys` (`docs/UPDATES.md`), and the repository's secrets for
  them once a release workflow exists (Phase 8).
- NEEDS-CODY: a real update of the Dell's host and of a real console,
  with a forced roll back of each.
- NEEDS-CODY: a disk pulled by hand on a real host and its event read at
  the console (suite 14's step; under QEMU the host opens none).
- NEEDS-CODY: suite 14's host update, with a QS/VS V1R0M1 signed by
  the qsvs stream's release key.
