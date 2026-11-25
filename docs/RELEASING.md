# Releasing QS/MC

How a release of QS/MC is signed, built and published, after QS/400's
`docs/releasing.md` and QS/VS's `docs/RELEASING.md` with QS/MC's own
keys and names. `VERSION` holds the version (`1.0.0` is V1R0M0);
`lic/release` reads it at build time.

## The keys

A release is signed with two keys of QS/MC's, and trusts three keys of
the product streams. None is in the repository or in a log. The keys
themselves are Cody's to make and keep (`NEEDS-CODY`,
`docs/QUALIFICATION.md`).

| Key | Variable | Signs or is trusted for | If it leaks / is lost |
|---|---|---|---|
| Release signing key, ed25519: a file of 64 hexadecimal characters (the 32-byte seed) | `QSMC_SIGNING_KEY` | the image's manifest, the installation media's payload, the update image, `SHA256SUMS` and the `qsmc` stream's index; every image trusts its public key | anyone can make update images and media that consoles accept / no update can reach installed consoles until a later release, trusting a new key, is installed from media |
| Secure Boot db key, RSA-2048, PKCS#8, and its X.509 certificate | `QSMC_SB_KEY`, `QSMC_SB_CERT` | the three UKIs (Authenticode, QS/400's `pesign`); `qsmc-db.cer` is on the media and in the release for enrolment | anyone can boot code on a machine that enrolled it / every machine that enrolled it must enrol a new certificate |
| The streams' public keys: `qsmc.pub`, `qsvs.pub`, `qs400.pub` in one folder | `QSMC_STREAM_KEYS` | built into the image (`/qsmc/keys/streams/`): the `qsmc`, `qsvs` and `qs400` indexes are used only when they verify against them (`docs/UPDATES.md`) | public halves: they cannot leak; see below for replacing one |

Make QS/MC's two once, on a trusted machine, into a directory outside
the checkout (an encrypted volume):

```sh
cargo run -p build -- keys /media/keys
```

It writes `qsmc-release.ed25519` (mode 0600) and its public half
`qsmc-release.pub` (what `QSMC_TRUSTED_KEYS` lists, and the `qsmc.pub`
of the streams' folder), `qsmc-db.key` (0600), `qsmc-db.cer` and
`qsmc-db.esl` (the certificate as an EFI signature list), prints the
release key's public half and the certificate's SHA-256, and never
writes over a key that is there. Keep two offline copies of the
directory. A key that exists in one place only is lost with that place,
and with the release key lost no installed console can be updated
again. `qsvs.pub` and `qs400.pub` are the public halves of QS/VS's and
QS/400's release keys, taken from their releases.

### Development keys

Without `QSMC_SIGNING_KEY`, a build signs with a development key it
makes under `<target>/keys/`, and says so. An image signed with one
says so too: a line after the banner at every IPL ("This image is signed
with a development key: it is not a release…"), the status screen's
Signature row on the serial port and the local console
(`development key, Secure Boot off`), and the build on the Console
version page (`<build> (development key)`). Without `QSMC_STREAM_KEYS`
it trusts development keys of the three streams made from its own.

`build release` refuses a development key: without `QSMC_SIGNING_KEY`,
with it naming the development key, without both `QSMC_SB_KEY` and
`QSMC_SB_CERT`, with QS/400's development certificate, or without
`QSMC_STREAM_KEYS`, it stops before it builds.

### The trusted keys and rotating them

An image trusts a list of keys for its own updates
(`/qsmc/keys/system.pub` in its initramfs): the key that signed it,
then each key `QSMC_TRUSTED_KEYS` names (a file of
64-hexadecimal-character lines, `#` comments). The update service and
the installer accept an update image or media signed by any of them,
and nothing else (`docs/UPDATES.md`).

To retire key K1 for K2:

1. Make K2 (`build keys`), and keep it offline beside K1.
2. Release N, signed by K1, with `QSMC_TRUSTED_KEYS` naming K2's
   public half: consoles that install N trust K1 and K2.
3. Release N+1 and later are signed by K2 (and name K3 when it exists).
   Their stream entries say they update from N or later
   (`updates_from`).
4. K1 is kept until no console older than N is supported, then
   destroyed.

A console older than N trusts K1 alone: it refuses N+1 with `QMC7101`
(not signed by a key this console trusts) and `A3007010`, and is
updated to N first, or installed from N+1's media. `sec-rotation`
shows both (`docs/PHASE8.md`).

### A stream's key replaced

The three streams' keys are in the image, not in its state: a console
trusts a stream's new key only once it runs a release that carries it.
A key file may hold several lines, each a key, so a stream's key is
replaced the same way as QS/MC's own:

1. The stream's owner makes the new key and gives its public half.
2. A QS/MC release is built with both keys in that stream's file
   (`QSMC_STREAM_KEYS`'s `qsvs.pub`, say, of two lines), and installed
   on every console.
3. Only then does the stream sign its index with the new key. A console
   that did not install that release refuses the new index with
   `QMC7201` and keeps what it had; it takes nothing from the stream
   until it is updated, from a removable volume if need be.
4. A later release drops the old key's line.

## Cutting a release

From a clean tree at the commit to be released, with the release keys:

```sh
QSMC_SIGNING_KEY=/media/keys/qsmc-release.ed25519 \
QSMC_SB_KEY=/media/keys/qsmc-db.key QSMC_SB_CERT=/media/keys/qsmc-db.cer \
QSMC_STREAM_KEYS=/media/keys/streams \
QSMC_TRUSTED_KEYS=/media/keys/next.pub \
  cargo run -p build -- release 1.0.0
```

`build release VERSION` refuses a version that is not `VERSION`'s, a
tree with any change or untracked file, and the keys above; it needs
`docs/RELEASE_NOTES_V<x>R<y>M<z>.md`. It then builds the release image
with the UKIs signed for Secure Boot before the disk and the ISO are
made of them, the ISO (with the certificate, the notices and the
release notes at its root), the update image and the manifest, and
writes `target/release/V1R0M0/`:

| File | What |
|---|---|
| `QSMC-V1R0M0-<hash>.iso` | the installation media; the name is `buildkit`'s (open question 8) |
| `QSMC-V1R0M0-update-<hash>.upd` | the update image |
| `MANIFEST.json`, `MANIFEST.sig` | every artifact's SHA-256, signed |
| `NOTICES.txt` | third-party notices |
| `RELEASE_NOTES_V1R0M0.md` | the release notes |
| `qsmc-db.cer` | the Secure Boot db certificate |
| `catalogue.json`, `catalogue.schema.json` | every reference code of the console with its meaning, cause and recovery, every `QMC` message with both texts, and the catalogue's form |
| `stream/index.toml` | the `qsmc` stream's index of this release |
| `SHA256SUMS`, `SHA256SUMS.sig` | the above, and their ed25519 signature |

Then it builds the whole release again from clean at the same target
path and compares every file byte for byte; a difference stops it
(`--no-compare` builds once, for a trial). The tag, `v1.0.0`, is
Cody's, and is pushed with the release's files published.

### By the workflow

`.github/workflows/release.yml`, started by hand, does the same on
GitHub with the keys held as the repository's secrets
(`QSMC_SIGNING_KEY` the seed's hexadecimal line, `QSMC_SB_KEY` the PEM,
`QSMC_SB_CERT_B64` the certificate in base64) and the streams' public
keys committed in `tools/build/keys/streams/`; it refuses a signing key
whose public half is not that folder's `qsmc.pub`. The release
directory and its stream are the run's artifact. It tags nothing.
Started on a tag already made (`gh workflow run release.yml --ref
v1.0.0 -f tag=v1.0.0`), it checks the tag is the commit it built and
the files against `SHA256SUMS`, then publishes the GitHub release on
the tag: the release directory's files, `SHA256SUMS` and its signature
as its assets, the release notes as its text. `v1.0.0` was published
by hand the same way.

## The stream

Releases of QS/MC are published as the `qsmc` stream: one directory
with every release's update image and ISO, and `index.toml`, the
signed index of them (`base/stream`), in the form QS/VS's
`docs/RELEASING.md` gives. A console reads it through `fetchd`, as it
reads `qsvs` and `qs400` (`docs/UPDATES.md`): pointed at a directory
served as the stream, it finds the release there (`sec-stream`).

```sh
cargo run -p build -- stream add target/release/V1R1M2 /srv/qsmc/v1
cargo run -p build -- stream verify /srv/qsmc/v1
```

`stream add` copies a release's update image and ISO into the stream's
directory and joins its entry to the index there, one sequence further,
signed again. `stream verify` checks the index against the release key
and `QSMC_TRUSTED_KEYS`, and every file it names against its size and
SHA-256. No service hosting the streams is part of QS/MC.

## When a key leaks or is lost

| | Leaked | Lost |
|---|---|---|
| Release signing key | Make a new key. Publish at once a release signed by the old key that trusts only the new one (`QSMC_TRUSTED_KEYS` names it, and the build is given the new key's file as `QSMC_SIGNING_KEY` only for the release after it); tell every operator to install it; take the old key out of the trusted list of every later release. Until a console has that release, an update signed by the leaked key would be accepted by it: say so to every operator, and that they install nothing that did not come from the stream. | No update can reach a console that trusts only the lost key. A new key is made; the next release is installed from its media on every console (its state kept: a backup first, `docs/BACKUP.md`). |
| Secure Boot db key | Make a new pair, sign the next release with it, and tell every operator who enrolled the old certificate to remove it from db (or add it to dbx) and enrol the new one. | Make a new pair; the next release's UKIs are signed with it, and every machine that enrolled the old certificate enrols the new one before installing it. |
| A stream's key | The stream's owner's to say; QS/MC releases without that key's line at once, as above, and the console takes nothing from that stream until it runs that release. | As a replacement, above. |

Each event is written into the release notes of the release that
follows it.
