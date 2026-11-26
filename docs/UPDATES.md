# Code updates

How the console learns of new levels, brings one to a host, and updates
itself (`upd/fetchd`, `lic/qsmcfetchd`, `lic/qsmcupdd`,
`sched/schedd/src/update.rs`, `web/pages/src/updates/`). The update
engine, the stream's index and its reader are QS/VS's `base/update` and
`base/stream` and QS/400's `updcat` and `httpc`; nothing of them is
written here.

## Product streams

Three, under one server address the operator can change (Code updates'
settings, `updates.toml` in `/data/fetch`):

| Stream | What | Read |
| --- | --- | --- |
| `qsmc` | this console | `<server>/qsmc/v1/index`, `base/stream` |
| `qsvs` | QS/VS hosts | `<server>/qsvs/v1/index`, `base/stream` |
| `qs400` | the level partitions and standalone systems are compared with | `<server>/qs400/v1/catalog`, QS/400's `updcat` |

A file of a stream is `<server>/<stream>/v1/file/<name>`. The default
server is `https://updates.quartzsystems.net` (QS/400's
`docs/UPDATE_PROTOCOL.md` still names `updates.quartz.systems`); `http://` is taken for a local mirror
or a test. An index or catalog is used only when its signature verifies
against a key of its stream built into the image
(`/qsmc/keys/streams/<stream>.pub`), and its sequence is never less than
the one kept; otherwise it is refused, `QMC7201` or `QMC7202` with
`A3007100`, and what is kept does not change.

**The keys.** A test image trusts the harness's test keys
(`tools/conformance/src/streams/keys.rs`). A release image trusts the
keys in the folder `QSMC_STREAM_KEYS` names (`qsmc.pub`, `qsvs.pub`,
`qs400.pub`, the three streams' release keys); the build refuses an
image signed with a release key (`QSMC_SIGNING_KEY`) without them. A
development build without either trusts development keys made from its
own development signing key, and says so
(`tools/build/src/product/keys.rs`).

QS/MC's own release keys are made once, on a trusted machine, into a
folder outside the checkout, by `base/buildaid` as QS/VS's are:

```
cargo run -p build -- keys /path/to/qsmc-keys
```

It writes `qsmc-release.ed25519` (the signing key's 32-byte seed in
hexadecimal) and its public half `qsmc-release.pub`, and the Secure Boot
db pair `qsmc-db.key`, `qsmc-db.cer` and `qsmc-db.esl`; it never writes
over a key that is there. The folder `QSMC_STREAM_KEYS` names is made
beside them, `streams/`, of public halves only: `qsmc.pub` (a copy of
`qsmc-release.pub`), `qsvs.pub` (QS/VS's `qsvs-release.pub`) and
`qs400.pub` (QS/400's release key's public half). Only the private
files are secrets: `qsmc-release.ed25519` and `qsmc-db.key`.

## The download service

`fetchd` (`lic/qsmcfetchd`, at 47 in PID 1's table) is the one process
that connects to the stream server, and the only outbound traffic of
the console beyond DHCP, DNS, NTP and QSI. It binds its socket as root,
makes its folders, then runs as user 23991 for good with the
managed-systems group as its supplementary one (so `mgmtd`, which brings
an image to a host, can read what it downloaded), with `CHOWN`, `SETUID`
and `SETGID` only until then. It holds no key of the console's. Its
requests on `/run/qsmcfetchd.sock` (`upd/fetchd/src/proto.rs`): status,
configure, check, download, remove, and the backup of its settings.

- **Check now** reads the three streams at once (`QMC7001`, `C3007100`).
  A server that answers nothing or is slower than 30 s is `QMC7204`
  with `A3007105`; nothing kept changes. Checking automatically
  (`Never`, the default; `Daily 04:00`; `Weekly`) and downloading
  automatically are settings, off until an operator turns them on: the
  console calls nowhere it was not told to.
- **Download** writes `/data/updates/.partial/<stream>/<name>`, at most
  4 GiB in all; a download that stops keeps what it has and the next
  resumes it with a range request. When the file is whole its size and
  SHA-256 are checked against the index, and only then is it moved to
  `/data/updates/<stream>/<name>`, readable by `mgmtd` and `updd`. A
  file that fails its check is removed: `QMC7205`, `A3007110`, which
  opens the console's problem. Code updates shows a stopped download's
  message until it is asked for again.
- Each release's notes are fetched once after a check and served at
  `/qsmc/updates/notes/<stream>/<file>` as text.

## A host's update

From Code updates' Install, its confirmation (`/qsmc/confirm/
update-install?t=<host>`), or a scheduled operation, with the same
checks (`schedd::update::host_plan`, `install_host`):

1. The stream offers the host a release newer than it runs whose
   `updates_from` it meets, and the image is downloaded (`QMC7003`
   otherwise).
2. The confirmation lists the running partitions it shuts down, each
   given the seconds typed; without that leave an install while
   partitions run is refused (`QMC7004`).
3. The image is streamed to the host over QSI's `update` family
   (`Stage`), QSI's credit the back-pressure, nothing held whole. The
   host verifies it against its own keys (`Staged`, `Verified`).
4. A system plan of the host is saved (`docs/PLANS.md`), by the
   scheduler, which owns the plans: named for the host and the date,
   described by the update, its automatic plans kept to ten. The
   install stops when none can be saved (`QMC7013`).
5. It is installed into the host's other slot (`Install`,
   `Installing`, `Installed`) without disturbing its partitions.
6. The partitions are shut down, and the host activated (`Activate`):
   it restarts into the slot and confirms it or rolls back by itself.
   The console reports which (`Confirmed`: `QMC7011`; `RolledBack` with
   the host's reason: `QMC7012`).

The progress page (`/qsmc/updates/code/<host>/progress`) has the
steps Download, Transfer, Verify signature, Save system plan (done,
the plan linked), Install on host, Activate, Restart and reconnect,
Confirm; it reloads by Refresh, and View log shows the host's update log.
The console's own install has no plan step: a plan is of a host.

## The console's own update

`updd` (`lic/qsmcupdd`, at 35) is `base/update` with QS/MC's values: its
image format (magic `QSMCUPDT`, head `QSMC-UPDATE 1`, fixes `QM`), the
staging folder `/data/update`, and the codes `C3007000` to `C3007075`,
`A3007010` and `A3007030`. Its image is `build update`'s: both slots'
UKIs and a manifest of the product, release, build, fixes and the state
schemas the release reads (`qsmcupdd::schemas`), signed with the build's
key, which the image trusts as `/qsmc/keys/system.pub`.

1. **Staged** from a downloaded file (Code updates, a scheduled
   operation) or a removable volume (the local console's option 8, 4).
2. **Verified**: signed by a key the release trusts, whole, this
   product's, not older than what runs (unless allowed, which the
   console's own pages never do), and able to read every state file of
   the console at its schema. A refusal is `A3007010` with its reason
   and nothing is kept.
3. **Installed** into the other slot on every load-source disk, read
   back and compared, then marked pending by PID 1. A power loss before
   that leaves nothing pending.
4. **Activated** by a restart after its confirmation page. Before the
   pending slot's first IPL PID 1 keeps a copy of the state
   (`update::state_copy`, `C3007065`), and of `mgmt`, `web`, `fetch` and `plans`
   beside it (`lic/qsmcinit/src/ipl/carry.rs`).
5. **Confirmed** when the web server answers (`C3001095`, `C3007060`).
   A slot that does not confirm within 15 minutes is followed by the
   old slot with its state put back (`C3007070`), `A3001035` opening the
   console's problem, and a console dump.
6. **Migrated** only after the confirmation: each service writes the
   state an older release wrote in this release's form
   (`release::trial::migrate_after`, `C3007075`), never while the slot
   is on trial, so a roll back always finds the state the old release
   wrote.

## What each failure leaves

| Failure | What is left |
| --- | --- |
| An index of the wrong signature or an older sequence | the kept index as it was; `QMC7201`, `QMC7202` |
| The server absent or too slow | nothing changed; `QMC7204` |
| A file of the wrong size or hash | the file removed; `QMC7205`, the console's problem |
| A download cut short | what it has, resumed by the next |
| A host's image refused at the host | nothing installed; the host's reason |
| A host that rolls back | the host on its old slot; `QMC7012` with its reason |
| A console image refused | nothing staged; the reason at the page or the panel |
| Power lost during the console's install | the slot that ran, nothing pending |
| Power lost during the pending slot's first IPL | the pending slot tried again, or the old slot with its state put back |
| A pending slot that does not confirm | the old slot, its state as it was, the problem open, a dump |

## The tests

`tools/conformance` runs a stream server of its own
(`src/streams/server.rs`: signed indexes and a catalog under the test
keys, images, notes; slow, cut short, wrong or absent on request) that
the console is pointed at. The console's own update installs the test
image's variants, built with `cargo run -p build -- --test-image
--variant next all` (`QSMC_RELEASE=V1R1M3 QSMC_NEXT_SCHEMA=1`, whose
`web.toml` is of schema 2) and `--variant broken all`
(`QSMC_RELEASE=V1R1M3 QSMC_TEST_WEBD_BROKEN=1`, whose web server cannot
start), under `test-image/next/` and `test-image/broken/`, signed with
the test image's own key.
