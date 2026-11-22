# Phase 7: system plans

What Phase 7 builds, how to run it, and the evidence for the acceptance
table. Phases 0 to 6 still hold (`docs/PHASE0.md` to `docs/PHASE6.md`);
what changed is said here.

A system plan is a saved configuration of a QS/VS host and its
partitions: viewed, exported, imported and deployed to a host. It is
built only from what QSI's host family reads and writes. It is steps 21
and 22 of the prototype's build order, and it fills the step Phase 6
left skipped: the plan saved before a host's code update is installed.
`docs/PLANS.md` is the plan, field by field, and its rules.

No pin moved: QS/VS at `0488d96b067358688f35f34b86a2a4662bcfe93f`, QS/400
at `cf8f01453846fdf78129ca79589e18798aa03fae` (`docs/PHASE5.md`). Open
questions 35, 36 and 39 to 42 are QS/VS's and QS/400's and were not
waited for.

## What exists

| Where | What |
| --- | --- |
| `plan/model` (crate `plan`) | the document (`doc/`: its types, bounds, strict reader and writer, 1 MiB at most), a host read through `mgmtd` (`view/`), the capture, the comparison of two plans, the validation and its rules table (`validate/`, held to `docs/PLANS.md` by a test), the steps of a deployment and their requests (`deploy`), the index and the job as the scheduler keeps them |
| `sched/schedd` | `plans/`: the plans' files, which it alone writes (`/data/plans/`), captures, imports, templates, descriptions, deletes, the automatic plan before an update and its pruning; `plans/deploy.rs`: a deployment as a job, one a host, its steps kept on disk, Cancel, a restart's recovery; eleven requests on its socket; `update.rs`'s host install saves a plan first |
| `mgmt/mgmtd` | nothing: a capture and a validation use the reads `ask` and `partition` already carry |
| `web/pages` | `plans/`: System plans, View, Edit (description, Save as template), the wizard's three steps, the confirmations of Deploy and Delete, the progress page; Code updates' install shows its plan saved and linked; Servers' and Partitions' Manage dumps lead to Manage dumps |
| `web/msgs` | `QMC6001`–`QMC6010`, `QMC6101`–`QMC6114`; `QMC7013` now says an install stopped for want of a plan (`docs/MESSAGES.md`) |
| `web/backup`, `lic/qsmcinit` | `plans/` in a backup and its restore, and among the folders kept across an update (`ipl/carry.rs`) |
| `lic/release` | `PLANS`, the folder's name |
| `tools/sim` | `QSHQVS2`, the prototype's target; `slow S FAMILY[/OP] MS`; a partition's serial number of its host's; `direct`, `mgmtd`'s calls answered in the process (`docs/SIM.md`) |
| `tools/conformance` | `suite/plans/`, Phase 7's suites |
| `tools/fuzz` | `system-plan`: the reader, the writer, validation and steps of what reads |

## How to run

```
cargo run -p build -- all && cargo run -p build -- --test-image all
cargo test -p plan -p schedd -p webpages         the rules, capture, validation, deployments, the golden pages
cargo run -p build -- test --suite plans-deploy
QSMC_QSVS_DISK=... cargo run -p build -- test --suite plans-real-host
cargo run -p build -- fuzz --target system-plan
```

## The pages

Steps 21 and 22 of the prototype's build order, each equal to its file
under `qsweb`'s comparison with the corrections in `docs/WEB.md`:

| Step | Prototype | Route |
| --- | --- | --- |
| 21 | `plans.html`, `plan-view.html` | `/qsmc/plans`, `/qsmc/plan/<name>` |
| 22 | `plan-deploy-1.html` to `-3.html`, `plan-deploy-progress.html` | `/qsmc/plan/<name>/deploy/step/1` to `3`, `/qsmc/plan/<name>/deploy/progress?t=` |
| 25 | `code-update-progress.html` (live since Phase 6) | its Save system plan step done, the plan linked |

86 of the prototype's 90 files are live. The four that are not are
Navigator for QS/400's pages, which the handoff lists as reference only
and a partition serves (`docs/OPEN_QUESTIONS.md`, item 47). Built from
the shared patterns without a design of their own (item 45): Edit
(`/qsmc/plan/<name>/edit`), the confirmations of Delete and Deploy.

The pages landed in one commit (`8fc8917`), each with its golden test
passing, not one commit a page: they share the plans module and were
written and compared together, as Phase 6's were.

## Evidence

### The units

`cargo test -p plan -p schedd -p webpages -p sim` (`plan`'s 8, the
scheduler's install, runs and deploy tests, the pages' 78 golden tests,
the simulator's 40), every one passing:

- `plan::validate::tests`: a plan captured from the simulated
  `QSHQVS1` through `mgmtd`'s calls (`sim::direct`) and validated
  against `QSHQVS2` gives each of the prototype's eight rows of step 2
  with its status and message, the partitions' new identity added, and
  eleven rows of items the prototype leaves out (`docs/PLANS.md`,
  "Against the prototype"); every rule of the table met by one case or
  another; the document read back is the document written, two captures
  are the same configuration, a change at the host is a difference;
  a damaged, too large, newer or out-of-bounds file refused; the steps
  of a deployment in order, POOL2 and VD05 skipped, QSPROD01 without
  VD05, its C3 adapter and its drive, no MAC address on another host,
  and nothing to do on the plan's own host.
- `plan::validate::rules::tests`: `docs/PLANS.md` holds the rules table
  as the code has it.
- `schedd`'s `deploy`: the whole deployment to `QSHQVS2` with its
  activations, the switches with their uplinks by `QSHQVS2`'s own
  location codes, the host's own MACs (prefix `02:51:54`) and serial
  numbers, nothing of its pool, disks or library changed; a refused
  virtual disk stopping the job with the host's message and a second
  deployment finishing; Cancel before the next step; a second
  deployment to the host refused while one runs (`QMC6006`); a job left
  running found interrupted by a restarted scheduler, and deploying
  again finishing.
- `schedd`'s `install`: a host's install saves `qshqvs1-20261026` first,
  described by the update; the eleventh automatic plan removes the
  oldest and keeps the operator's.
- `webpages`' golden tests: `plans.html`, `plan-view.html`,
  `plan-deploy-1.html` to `-3.html`, `plan-deploy-progress.html` and
  `code-update-progress.html` equal to their files with the corrections
  of `docs/WEB.md`.

### The gate and two builds

At `31fcc5e`, in a fresh clone on the build machine
(`~/qsmc-ev/run.log`, `~/evidence7.sh`). `build doctor`, `build deny`,
`build notices --check`, `fmt --check`, the three clippy runs and
`cargo test --workspace` (337 tests) passed. `build all` ran twice, the
second with its own empty targets and the first's signing key. Every
image, the update image and every binary is the same in both:

| File | SHA-256 |
| --- | --- |
| `qsmc-V1R0M0-f19f547ccee5.img` | `4287ba1da75856b748c253556991015704ec8f3f62deed1c7e375e13086722c9` |
| `qsmc-V1R0M0-a-f19f547ccee5.efi` | `f19f547ccee5a5c46cc97ee20bdef07bfd430cfe02ee32f09f91347bd7ca8fdd` |
| `qsmc-V1R0M0-b-d791a7dafe00.efi` | `d791a7dafe00e42a5af01d605a76a753d8475658c7d04ae4d08175f6a861ac04` |
| `qsmc-V1R0M0-d-439111383e79.efi` | `439111383e790653c764930646570fdc2068743ab53b2f2857122d7bca56d0c6` |
| `QSMC-V1R0M0-439111383e79.iso` | `0851b863af3da48aa1b600bf697f558bd4a31dc1d77fb61d93a71f8766ec2cce` |
| `QSMC-V1R0M0-update-cbbb9d6ec9c3.upd` | `cbbb9d6ec9c360ed2b9f8873acef9d12dcdda4ca596fe04f1fed5bbb93b1e1a7` |
| `initramfs.cpio` | `a269da0efb75231fdb623eaed9e53c623038dc44043f324b8b05a330ae5f4e1d` |
| `initramfs-media.cpio` | `f0273f70c13f2474af37acf1cd2d415afb74885bc8b9c781d95f14f186cf699a` |
| `qsmcinit` | `d1152fb4bc1ed0d2b171f1cc16ec8da66938d90611fea95a56069d37d2d6709f` |
| `qsmcstgd` | `3903e8302282facf2e615e98c22155ed5fdecc93eba3f75a35b16d961f34eceb` |
| `qsmclogd` | `5e64b16279fe96760aac6243e627d8b1821ec7ac36e606f799916bc061d64929` |
| `qsmcnetd` | `9484c4634d616729e581b6a3f28a44c2e7c47df974123b82bf26cef8c1fb8010` |
| `qsmccons` | `10a1eb8e4431d278fa6fd5d3c443cfa3037eb2e630d39022c6019c91d07179ec` |
| `qsmcinstall` | `a49c9e0216d73dda14d64358244032f6e2e706bd28c306e2309e1fa742c6b20c` |
| `qsmcwebd` | `a1b4bb80acdd3ec0d3cbe187e6ff8c96f77d8e7f397f730210873b6a037e0018` |
| `qsmcmgmtd` | `75f5b7cf157e6559e105881c21b8aa15ce7ffad90dc48557c7f3b1a6fbdbdec4` (Phase 6's: `mgmtd` did not change) |
| `qsmcschedd` | `ad87db9d614982355b5489f6909fc5fa89fde47bbc6ae3b51303516019ea9791` |
| `qsmcprobd` | `e1998f570df36de7676eef5cb50392900fb93374dffe15e0e79d081409434f99` |
| `qsmcupdd` | `a83e8f86c7d02a7bc3d98a913d33bb36b64ae2771720a49d6fdd134badadc2e4` |
| `qsmcfetchd` | `d87b13ee627535ba2b831998397cde63a69177e958fd4171e3688ff7f3284405` |

The kernel's `.config` and the manifest differ between the two, and
only by the target folder, as in Phase 6: `CONFIG_INITRAMFS_SOURCE`
names the initramfs by its absolute path, and the manifest holds the
`.config`'s hash. The release image holds none of `qsmchook`,
`TEST IMAGE`, `qsmc.hook`, `hook=`, `Pattern reference`, `webd-fails`,
`short-trial`.

### The suites

Phase 7's suites on the test image at `c5359d5` (`31fcc5e` for suite
12), each run alone on the build machine (`~/qsmc-p7-suites.log`):

| # | Suite | Time | What it showed |
| --- | --- | --- | --- |
| 2 | `plans-pages` | 58 s | 10 pages of the phase (the list and its sorts, View, Edit, the wizard's three steps, both confirmations, the progress page, Code updates, Welcome) answered 200 with no script; 12 pages they link to answered; 8 forms with the token, 7 posted with Cancel |
| 3 | `plans-capture` | 77 s | two plans of `QSHQVS1`: its 3 definitions and 7 profiles each the host's own, the two the same configuration, the 8,540 bytes exported read back and written the same; `QSHQ400` answered `QMC6105` |
| 4 | `plans-validate` | 49 s | against `QSHQVS2` the prototype's 8 rows with their statuses and messages, the partitions' new identity added, 11 rows of items the prototype leaves out; `QSHQ400` Error alone; on `QSHQVS1` its 19 rows OK; step 3 ticked as validated |
| 5, 6 | `plans-deploy` | 64 s | Cancel during the first step stopped the job (`QMC6003`), `ETHSW0` made and the rest pending; a refused partition (`QVS2006`) stopped it (`QMC6004`) after 8 steps; then the steps in order, `POOL2` and `VD05` skipped, `QSPROD01` made without `VD05` and `C3`, the three partitions activated (`QMC6002`); again, every step skipped; `QSHQVS1`, and `QSHQVS2`'s pool, disks and library, as before; `sdj` in no pool |
| 7 | `plans-restart` | 59 s | the web server killed while a virtual disk was made: its 10 steps shown again; the console restarted during the next: the step Interrupted, the job stopped (`QMC6005`); deployed again, it finished (`QMC6002`) |
| 8 | `plans-import` | 56 s | `QMC6104` with its reason for a damaged file, one past 1 MiB, a newer version, an ID and a VLAN out of range, a name with a space, an unknown key; the plan exported, deleted and imported again byte for byte, deployed to `QSHQVS2` |
| 9 | `plans-update` | 265 s | eleven installs of `QSHQVS1`, each rolled back as told: each saved its plan first, the step Complete and the plan linked; after the eleventh, 10 automatic plans, the oldest removed, the operator's kept |
| 10 | `plans-roles` | 111 s | 68 route, method and role pairs as the matrix says (51 Not authorized, `QMC9010`); Deploy and Delete without their token refused (`QMC9011`); nothing deployed or deleted until confirmed; capture, export, delete, import and each step audited with the user (26 lines) |
| 11 | `plans-backup` | 53 s | the backup held `plans/<name>.qmcplan`; the plan deleted, the backup restored at the restart: listed again, exported byte for byte |
| 12 | `plans-real-host` | 96 s | below |
| 8 | fuzz `system-plan` | 60 s | 2.38 M runs, 5,881 edges, 3,604 in the corpus, nothing found |

Phase 6's `serv-host` passed on the same image with its plan step now
done (66 s).

Fixed in the product by the suites and tests: a switch's uplink, a
partition's physical device and its tape drive named by the plan's
host's location codes rather than the target's; a partition's own
device on its own host taken as held; a real machine's type and model
(`Standard PC (Q35 + ICH9, 2009)`) refused for its spaces.

### On a real host

Suite 12, `plans-real-host`, on the QS/VS test disk at the pin under
QEMU (`QSMC_QSVS_DISK`): pool `MCPOOL` on a blank disk, switch
`PLANSW` (VLANs 1, 20), virtual disks `PLAND1` (thick) to `PLAND3` and
partitions `PLANP1` to `PLANP3` made from the console's own pages;
captured; the partitions with their disks and the switch deleted from
their own pages; the plan deployed back in 10 steps (`QMC6002`). Every
definition and profile read back equal to the plan, MAC addresses and
serial numbers included: QS/VS makes a partition's serial number from
the host's and its ID, and keeps a profile's MAC address. What could
not be put back the same: nothing the plan holds. A partition's
creation time is the host's (QS/VS's open question 44) and a plan does
not hold it. The activations were asked and taken, and no partition
stayed activated: their load sources are empty disks, so their IPL from
side A has nothing to start. That is for a real machine, below.

### Outbound and listeners

A deployment, a capture and a validation reach a host only through
`mgmtd`'s session; the scheduler opens no socket but its own and
`mgmtd`'s. No listener was added: Phase 3's `no-secrets` and
`web-hostile` hold TCP on 443 and 80 only.

## Acceptance

| # | Check | Evidence |
| --- | --- | --- |
| 1 | Suites 1 to 11 pass | The suites, above; every earlier phase's on GitHub (`qemu.yml`) |
| 2 | Suite 12 against a real host, or what cannot run listed; what could not be put back the same recorded | On a real host, above; Not done |
| 3 | Every page of steps 21 and 22 live and equal to its golden file; the count recorded | The pages: 86 of 90 |
| 4 | A host's install saves its plan first | `plans-update`, `schedd`'s `install` test |
| 5 | Two clean builds byte-identical; `build deny` and `build doctor` pass | The gate and two builds |
| 6 | A deployment never deletes, changes or takes what it did not make; 443 and 80 on `open` only | `plans-deploy`, `schedd`'s `deploy` test; Outbound and listeners |

## Not done

- NEEDS-CODY: a plan of the Dell's host deployed to a second host, or
  back to itself after a reinstall, and its partitions activated with
  QS/400 on their load sources (suite 12 under QEMU has empty disks).
