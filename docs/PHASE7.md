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
