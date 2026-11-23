# Security

What a QS/MC console trusts and what it does not, the boundaries
between them, and for each what enforces it and which suite shows it.
Phase 8 reviewed every one (`docs/PHASE8.md`). QS/VS's
`docs/SECURITY.md` is the same for a host.

## What is trusted, and what is not

| Party | Trusted with | Not trusted | Enforced by | Shown by |
|---|---|---|---|---|
| A browser and its user, by role | what the user's role allows (`docs/WEBUSERS.md`'s matrix), after sign-on | every byte of a request; anything without a session; a state change without the form's token; another site's page | TLS on 443 only, 80 redirects to it and never off the console; the session cookie `HttpOnly; Secure; SameSite=Strict; Path=/qsmc`; the token checked on every POST in one place (`web/pages/src/app/routes.rs`); the task of every route; a body said two ways refused, with every request after it on its connection (`qsweb` would read on: `docs/OPEN_QUESTIONS.md` 49); no file served from a path; the terminal socket from the console's own page only; the limits of `docs/WEB.md` | `web-signon`, `web-roles`, `parts-roles`, `serv-roles`, `plans-roles`, `web-hostile`, `sec-web`, `fuzz --target web-request`, `--target terminal-input` |
| A web page | its own content | script: none but `qsweb`'s navigation script, served by the console, and the terminal window's; no inline script but the help window's Close window, admitted by its hash; framing, sniffing, a referrer, caching | every answer on 443 carries `X-Frame-Options: DENY`, `nosniff`, `Referrer-Policy: no-referrer`, `Cache-Control: no-store` and the policy `default-src 'none'; script-src 'self' 'unsafe-hashes' 'sha256-bPxKeIO3vE75rWhFps/iWImI78V11QxJ/ssbAigs2Vw='; connect-src 'self'; style-src 'self'; img-src 'self'; form-action 'self'; frame-ancestors 'none'; base-uri 'none'` (`web/webd/src/serve.rs`, a test holds the hash to the handler); the terminal window has its own policy (`docs/TERMINAL.md`) | `sec-web` (every page crawled), `web-pages` |
| A managed system | the replies and events of its own families, once paired | everything it sends: its hello, names, messages, codes, console frames, its pace | QSI's TLS 1.3 with both ends pinned (`docs/MGMT.md`); one thread a system, each request bounded in time; every value it names escaped on every page; a frame past its limits ends the session; the session's credit; `mgmtd` confined (below) | `mgmt-hostile`, `sec-systems`, `fuzz --target mgmt-peer`, `--target mgmt-host`, `--target mgmt-servicing` |
| A system not paired | answering a pairing with its one-time password | anything else | pairing pins the certificate it saw; any other certificate after is refused (`QMC7333`) | `mgmt-pair` |
| The network | carrying frames | anything it says | 443 and 80 on the `open` address only; nothing else listens; outbound only DHCP, DNS, NTP, QSI to paired systems and `fetchd` to the configured server; each service's network confined (below) | `sockets`, `web-listeners`, `mgmt-outbound` |
| The stream server | serving files | its bytes: an index or a file not signed and named as it should be | `fetchd`, unprivileged, its own user and no key, verifies each index against the stream's key built into the image and each file against its size and SHA-256 before anything is kept (`docs/UPDATES.md`) | `serv-streams`, `fuzz --target stream-index` |
| Update images | being installed, when a key the running image trusts signed their manifest | anything unsigned or signed otherwise; an older release; one that cannot read the console's state | `updd` verifies before a byte is written, and nothing turns that off (`docs/UPDATES.md`, `docs/RELEASING.md`) | `serv-refusals`, `sec-rotation`, `fuzz --target qsmc-update-image` |
| Imported files: a backup, a certificate or PKCS #12 file, a system plan | what they hold, once they read whole | their bytes | each reader is bounded and strict and refuses cleanly with a message: a backup sealed under its passphrase and checked whole before a restore (`docs/BACKUP.md`); a certificate's key checked against it and PKCS #12's iterations bounded; a plan read strictly, 1 MiB at most (`docs/PLANS.md`) | `web-backup`, `web-certs`, `plans-import`, `sec-imports` (every file of the fuzzers' corpora through its page), `fuzz --target backup-file`, `--target backup-payload`, `--target cert-import`, `--target system-plan` |
| Removable volumes (seed, update, dump) | a seed's settings once; an update image; a place to copy a dump | their files' contents | mounted read-only to read, never `dev`, `suid` or `exec`; a seed's answers checked by every rule of the state files before anything is applied | `refused-answers`, `serv-local`, `fuzz --target seed-answers` |
| Installation media | being installed by its own installer | another build's payload | the installer verifies the payload's manifest against the key in its own initramfs | `refused-media` |
| The local console's operator | the console itself: network, the web interface's reset, service functions | nothing it types is a command | `qsmccons` manages nothing but the console; sign-on as a service tools user with idle sign-off (`docs/LOCALCONSOLE.md`) | `sign-on`, `console-panels` |
| The firmware | starting the console's own UKIs | nothing after | Secure Boot, when the operator enrols `qsmc-db.cer` (`docs/INSTALL.md`) | `sec-secure-boot` |

## The services confined

Every service is confined in three layers (`lic/sandbox`):

1. **Capabilities.** PID 1 starts each with only the capabilities its
   row keeps, every other taken from its bounding set before `exec`, and
   no new privileges (`base/supervise`, `lic/qsmcinit/src/host/table.rs`).
   `mgmtd` and `fetchd` then become their own users for good.
2. **Landlock.** Before its first thread, a service restricts itself to
   the paths and access its policy names (`lic/sandbox/src/policy.rs`):
   files under any other path cannot be opened, listed, made or removed,
   whatever their owner and mode. TCP is restricted too: no service may
   bind a TCP port of its own (the web server's listeners are opened by
   PID 1 and handed over), and only `mgmtd` and `fetchd` may connect,
   each to the port its peer's setting names. Signals and abstract
   sockets outside its own domain are scoped away. A kernel without
   Landlock ABI 6 refuses to start a service at all, so none ever runs
   unconfined. A rule holds the folder that is there when the service
   confines itself: a folder it writes it makes first, and one it only
   reads that another service makes (`/data/updates`, `/data/log`) PID 1
   makes before any service starts (`sandbox::policy::read_folders`).
3. **A seccomp filter** (`base/confine`): the calls the service makes,
   some only with given arguments: `socket` with its families only (unix
   for every service; `inet` and `inet6` for `mgmtd`, `fetchd` and
   `netd`; netlink and packet for `netd`), `clone` with a thread's flags
   only (no `fork`), `prctl` to name a thread, `ioctl` to see a terminal
   or set a descriptor's mode (the storage, update, network and local
   console services take any, for their devices), `tgkill` to its own
   process. `execve`, `ptrace`, `mount` (but the local console's),
   `setuid`, `unshare`, `bpf`, `kexec_load` and the rest are refused to
   every service (a unit test holds each filter to that list). Any call
   not allowed ends the service with `SIGSYS`: PID 1 posts `A3001085`
   naming it, its problem opens (`QMC7342`), and supervision starts it
   again; no other service notices.

| Process | User | Capabilities, and why | Reads | Writes | Listens | Connects | Filter |
|---|---|---|---|---|---|---|---|
| `qsmcinit` (PID 1) | root | all: it mounts, writes the A/B record and the firmware's variables, opens the web server's listeners, starts and ends the services, powers off | everything | the record, `/data/state` at the IPL, `/data/log/panics`, the console's dumps | its control socket `/run/qsmcinit.sock`; 443 and 80 on the `open` address, handed to `webd` | nothing | none: PID 1 is the trust root |
| `webd` | root | none | `/data` (a backup reads its folders; files it may not read it passes over), `/qsmc/catalogue.json`, `/proc/uptime`, `/proc/cpuinfo`, `/proc/sys/kernel/osrelease`, `/dev/urandom` | `/data/state`, `/data/web`, `/data/restore`, `/data/dumps/console` (deletes), `/tmp` (a network form's check), `/run` (its socket for the local console) | the listeners PID 1 hands it, and `/run/qsmcwebd.sock` | unix sockets of PID 1, `mgmtd`, `schedd`, `fetchd`, `probd`, `updd`, `netd`; no TCP | 96 calls |
| `fetchd` | 23991, the download service's own | `CHOWN`, `DAC_OVERRIDE` until it owns its folders, `SETUID`, `SETGID` until it is its user; then none | `/data/state`, `/qsmc/keys`, the resolver's files, `/dev/urandom` | `/data/fetch`, `/data/updates` | `/run/qsmcfetchd.sock` (bound before) | the stream server's TCP port, DNS; PID 1's socket | 96 calls |
| `mgmtd` | 23990, the managed-systems service's own | `SETUID`, `SETGID` until it is its user; then none | `/data/state` (its QSI key is readable by its user alone), `/data/updates`, `/proc/sys/kernel/hostname`, the resolver's files, `/dev/urandom` | `/data/mgmt` | `/run/qsmcmgmtd.sock`, `/run/qsmcmgmtd-consoles.sock` (bound before, mode 0600) | each paired system's QSI port, DNS; PID 1's socket | 96 calls |
| `schedd` | root | none | `/proc/sys/kernel/hostname`, `/dev/urandom` | `/data/state` (`schedules.toml`), `/data/plans` | `/run/qsmcschedd.sock` (bound before) | `mgmtd`, `fetchd`, `updd`, PID 1 | 96 calls |
| `probd` | root | none | `/data/log` (the codes' history) | `/data/problems`, `/run` (its socket) | `/run/qsmcprobd.sock` | PID 1 | 96 calls |
| `updd` | root | `SYS_ADMIN`, `SYS_RAWIO`, `DAC_OVERRIDE`: the load-source disks' ESPs and the firmware's variables | `/data/state`, `/data/updates`, `/qsmc/keys`, `/sys`, `/dev` | `/data/update`, `/sys/firmware/efi/efivars`, the ESPs | `/run/qsmcupdd.sock` | PID 1 | 96 calls |
| `logd` | root | `SYSLOG`: the kernel's log, root's alone | `/dev/kmsg`, `/proc/sys/kernel/random/boot_id` | `/data/log`, `/run` (its socket) | `/run/logd.sock` | nothing | 96 calls |
| `netd` | root | `NET_ADMIN`, `NET_RAW`, `NET_BIND_SERVICE` (DHCP), `SYS_TIME` (NTP sets the clock) | `/sys` | `/data/state` (`network.toml`), `/etc` (the resolver's file), `/proc/sys/net`, `/dev/rtc0`, `/run` (its socket) | `/run/netd.sock`, UDP 68 | DHCP, DNS, NTP; netlink | 99 calls |
| `stgd` | root | `SYS_ADMIN` (ublk), `SYS_RAWIO`, `IPC_LOCK` and `SYS_RESOURCE` (io_uring's buffers), `DAC_OVERRIDE`, `FOWNER` | `/sys`, `/proc` | the load-source disks (`/dev`), `/sys/firmware/efi/efivars`, `/run` | `/run/qsmcstgd.sock` | PID 1 | 99 calls |
| `qsmccons` | root | `SYS_TTY_CONFIG` (the display's console), `SYS_ADMIN` (option 8's removable volume) | as root: not under Landlock, which would refuse option 8's `mount` | `/data/state/users.db`, the removable volume | nothing | the services' sockets | 99 calls |

Two keep more than the rest, and why it is accepted:

- **The local console has no Landlock.** Landlock refuses `mount` to a
  domain, and option 8 mounts a removable volume for an update image or
  a dump. It keeps its seccomp filter and its two capabilities, it reads
  nothing from the network, and its operator is at the machine. Asked of
  QS/VS: a volume mounted by PID 1 on the console's request
  (`docs/OPEN_QUESTIONS.md`, item 42), after which the console is under
  Landlock like the rest.
- **`mgmtd` and `fetchd` may connect to any TCP port.** Their peers'
  ports are settings (a system's address and port, the stream server's
  URL). Their seccomp filters give them `inet` sockets; Landlock gives
  them no listener and no path but their own.

A unix socket is reached by its path whatever Landlock says (ABI 6 does
not restrict it); each server checks its peer's credentials or the
session it names, as `docs/MGMT.md` and `docs/WEB.md` say.

The policies were derived in the learning mode (`confine-learn`: a
call the filter does not allow is reported on the service's standard
error, which PID 1 copies to the serial log in a test image, and fails
with `ENOSYS`) under the suites of every phase, then fixed
(`docs/PHASE8.md`). `sec-confined` shows each service as the kernel sees
it (seccomp mode 2, no new privileges, its user) and, at one IPL each,
each service made to call `ptrace` once confined (`confine-trip-<name>`):
ended, `A3001085` posted, started again, every other service ready with
no failure.

## The machine

| What | Why | Shown by |
|---|---|---|
| The kernel's hardening symbols, all of QS/VS's Phase 11 set: dmesg restricted, hardened user copies and fortified strings, the allocators' freelists randomized and checked, caches not merged, lists checked, corruption a panic, every allocation zeroed, the stack's offset random, used registers zeroed, an oops a panic, the text read-only, stacks guarded, KASLR, mitigations on, Yama | `kernel/config/qsmc.config`, each with its reason; the build checks every symbol of the final `.config` | `build kernel` |
| Landlock, first of the security modules | the services' paths and ports, above | `sec-confined` (a service on a kernel without it does not start) |
| What an appliance never uses, taken away: modules, kexec and crash kernels, hibernation, `/dev/mem`, `/dev/port`, `/proc/kcore`, the I/O port calls, 32-bit programs, vsyscall, `TIOCSTI`, line disciplines on demand, `binfmt_misc`, kprobes and uprobes, debugfs, the packet filter | the fragment's "taken away" group | `build kernel` |
| The command line `console=ttyS0,115200 console=tty0 panic=-1 quiet` and the slot's `qsmc.slot=`, baked into the UKI | a panic restarts the console; nothing on it can be changed from outside a signed UKI | `ipl-slot-a`, `ipl-slot-b` |
| `/data` mounted `nodev,nosuid,noexec`; `/proc`, `/sys`, `/run`, pstore and efivarfs `nodev,nosuid,noexec`; `/dev` `nosuid,noexec` | no device node, set-user-ID program or executable on a file system that can be written | `sec-machine` |
| The root read-only once the services that waited for `/data` start; `/etc`, `/tmp` and, with no data file system, `/data`, bound onto themselves `nodev,nosuid,noexec` (`lic/qsmcinit/src/ipl/mounts.rs`) | nothing on a writable file system is ever executed; every program is in the UKI's initramfs | `sec-machine` |
| The kernel's messages root's alone (`DMESG_RESTRICT`), and `logd` alone has `SYSLOG` | no other service reads the kernel's log | `sec-machine` |
| A kernel panic's records kept by pstore and posted at the next IPL (`A3001005`, a console dump) | Phase 6 | `serv-dumpfile` |
| Every file the console writes whole after a power loss: `base/state`'s files, `mgmtd`'s problems and history, the web certificate's pair and its mark, a restore confirmed (`docs/PHASE8.md`, Power pulls and hostile files) | a power pull is an operator's normal act | `power-pull`, `serv-power`, `sec-power` |

## Secrets

No password, hash, passphrase, key, pin or pairing password is written
to a log, a dump, a backup's clear part, a page or an audit record.
`web-secrets` (suite 14 of Phase 3), `no-secrets` (the installer and the
local console), `mgmt-audit` and `serv-audit` each search every log,
dump and page they reach for the values they used; `sec-secrets` does
the same once more on the final image across all of them, and a build
whose image holds one fails.

## Listening and connecting

The console listens on 443 and 80 of its `open` address and nothing
else; it connects only by DHCP, DNS, NTP, QSI to paired systems, and
`fetchd` to the configured stream server (`docs/ARCHITECTURE.md`).
`sockets` and `web-listeners` list every socket of every process on the
image; `mgmt-outbound` watches every connection made, and each service's
Landlock and filter keep it from making another (`sec-confined`).
