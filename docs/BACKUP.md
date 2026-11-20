# Backup and restore

A backup is the console as it is kept on its data file system: what
it needs to come back as the same console on another installation. It
is made and restored from Console Management, Backup and restore, by a
user whose role has the Console settings task.

## What a backup holds

Every file under `/data/state` and `/data/web`, and the managed
systems' files under `/data/mgmt`:

| Folder | What |
| --- | --- |
| `state/` | the state files (`host.toml`, `network.toml`, `inventory.toml`, `webusers.toml`, `web.toml`, `trusted.toml`, `backups.toml`), the service tools users (`users.db`), and the console's QSI identity: its key and certificate |
| `web/` | the web certificate and its key |
| `mgmt/` | `systems.toml`, the managed systems as paired with their pins, and `mgmt.toml` (`docs/MGMT.md`) |

The QSI key and the `mgmt/` files are the managed-systems service's
user's alone: the web server cannot read them, and asks the service for
them (`backup_files`) as it makes a backup. They pass through the web
server only inside the backup being made or opened, under its
passphrase. A console restored from a backup has the identity its
managed systems pinned and its systems' pins, so its sessions resume
without pairing again; the next IPL gives the files back to the
service's user.

Not held: the logs and the reference code history under `/data/log`
(the log service's, not restored over a running one), the managed
systems' reference code histories under `/data/mgmt/refcodes`, and
anything of a later phase. A file being written (a name beginning `.` or ending
`.new`) is left out.

## The file

`<host>-<yyyymmdd>.qmcb`, made by `web/backup` (`qmcbackup`):

| Bytes | What |
| --- | --- |
| 8 | `QSMCBAK1` |
| 12 | Argon2id's memory in KiB, passes and lanes, big-endian `u32`s: 65536, 3, 1 as made |
| 16 | the salt, random |
| 12 | the nonce, random |
| the rest less 16 | the payload under AES-256-GCM, with the 48 bytes above as additional data |
| 16 | the tag |

The key is Argon2id of the passphrase and the salt, 32 bytes. The
passphrase is 12 to 128 characters and is typed for each backup; it is
never kept. Without it nothing of a backup is readable: the payload and
its manifest are both under the cipher, and a changed byte anywhere,
the header included, refuses the file. A file that asks for more than
256 MiB, ten passes or four lanes is refused before any work.

The payload is `QSMCPAY1`, the manifest as `key value` lines (`console`,
`release`, `build`, `made` in Unix seconds) ended by an empty line,
then each file: a big-endian `u16` path length, the path under `/data`
(`state/host.toml`), a `u32` mode, a `u32` length and the bytes. A path
is under `state/` or `web/`, at most four parts below it, each letters,
digits, `.`, `_` or `-` and never `.` or `..`; a payload is at most
7 MiB, under the web server's upload limit, and 4096 files.

## Making one

The Backup tab: Console data (always), Download as file, and the
passphrase; Back up now downloads the file and records it in
`backups.toml` (the last 20, the History tab). What the page offers and
V1 does not do answers with a message (`docs/OPEN_QUESTIONS.md`, item
19): a remote host (`QMC8405`), a schedule, the reference code history,
the media library (`QMC8700`). Each backup is audited.

## Restoring one

1. The Restore tab: the file and its passphrase, then Open. A wrong
   passphrase or a changed file is `QMC8402`, a file that is no backup
   `QMC8403`, a backup of a newer release than the console's `QMC8404`.
   The page shows what the backup holds: the console, its release and
   build, when it was made, how many files. The payload is kept as
   `/data/restore/uploaded` (mode 600).
2. Restore... and the confirmation page; OK makes it
   `/data/restore/pending` and restarts the console.
3. PID 1, at the IPL after `/data` is mounted and before anything reads
   state (`lic/qsmcinit/src/ipl/restore.rs`), writes the trial mark
   `/data/restore/trial`, moves `state/` and `web/` to
   `/data/restore/previous/`, writes the backup's files in their place,
   and posts `C3001045`. A payload it cannot read, or of a newer
   release, is `A3001046` and changes nothing.
4. When the web server answers on the restored state (the slot rule,
   `docs/WEB.md`), the folders before it are deleted: `C3001096`.
5. An IPL that finds the trial mark puts the folders before it back:
   `A3001045`. That is the restored console not starting its web server,
   or the restore cut short by a power loss at any point after the mark
   was written.

An upload never confirmed is deleted at the next IPL.
