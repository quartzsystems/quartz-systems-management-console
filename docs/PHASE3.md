# Phase 3: The web server and Console Management pages

What Phase 3 builds, how to run it, and the evidence for the acceptance
table. Phases 0 to 2 still hold (`docs/PHASE0.md`, `docs/PHASE1.md`,
`docs/PHASE2.md`); what changed is said here.

Phase 3 gives the console its management surface: the web server
(`web/webd`, the binary `lic/qsmcwebd`) on 443, with 80 answering only
with a redirect, on the `open` interface; web users with the
prototype's four roles; the frame; and every shared, Welcome, sign-on
and Console Management page of the prototype, live against the
console's own state. It manages itself and nothing else: QSI and
managed systems are Phase 4.

The design is `design/qsmc-prototype/`: each page is the golden its
served page must match under `qsweb`'s comparison. The server, the
frame, the partials, sessions and CSRF are QS/400's `qsweb`; the
certificates are QS/400's `x509kit`; TLS is rustls with the graviola
provider. Nothing of them is written or copied here. The stylesheet is
`qsweb::CSS`, served byte for byte as QS/400's `docs/WEB_STYLE.md` ("A
product outside QS/400") requires; the prototype's own copy is older
(177 lines) and is not served.

## Step 0: the pins and what they brought

| Commit | What |
| --- | --- |
| `6498c90` Move the QS/VS pin to 2fe7583 and QS/400 to 155aba2 | QS/VS from `f0ef0f15c216c4ac6203f05e141c92926aee5705` to `2fe7583b8fc0eaeab92db7f31f34d152094f2727` ("Find the fixture's base crates by the repository's URL"); QS/400 from `423c65dea0b8c9fab3d2c3efaba6e9f27e679c99` to `155aba213638b178cb730cff4a4113ef1d8fb525` (its `main` with handoff 22), which that commit pins. `Cargo.lock` started again from QS/VS's at its pin |
| `49b2fdf` Move the QS/VS pin to 3de567e | QS/VS to `3de567ea27d350ebe8897530cfd03da84c005440` ("Record the confirmed network change for QS/MC"), with `base/netd`'s `apply_confirmed` (`ff181be`, item 24); QS/400 stays at `155aba2`, which it pins |

What the new pins changed, and what it means here:

| Commit | What | Here |
| --- | --- | --- |
| QS/400 `ef63ae7`, `50c65cd` | `qsweb::brand::Brand`: the family's name, vendor, release, `Server` token, paths, cookie, sign-on and Not authorized lines, the sign-on notice's place and the User field's length | QS/MC's frame and sign-on page match the prototype with no copy of either |
| QS/400 `1e62887` | `x509kit`: certificates without the store | the web certificate: made, read, imported |
| QS/400 `987fcf8` | the fixture's second web product over TLS | the model of `webd`'s TLS and golden tests |
| QS/VS `0f1f8a5` | `base/fdpass` hands a hold's files over sixteen to a byte | nothing: QS/MC passes fewer |

`build doctor` (both pins agree, no shared crate twice), `build deny`,
the pin checks, the workspace tests and the three clippy runs pass at
the new pins. Built from empty targets (release and test images), every
suite of Phases 0 to 2 passed, 32 steps on the disk and the ISO step,
with `mirror` fixed (`474cc38`, below).

## What exists

| Where | What |
| --- | --- |
| `web/webd` | the server: PID 1's listeners received over `/run/qsmcweb.sock` and followed, rustls with graviola (TLS 1.2 and 1.3), the certificate made, renewed and imported and served to the next handshake, `qsweb`'s limits, connection limits, the headers, the stylesheet, the redirect on 80, the self-test that makes it ready (`docs/WEB.md`) |
| `lic/qsmcwebd` | the binary: `webd` with the pages, and the socket `/run/qsmcwebd.sock` the local console asks |
| `web/users` | web users, the four roles and the matrix, sign-on's lock-out and address limit, `web.toml`, the socket's protocol (`docs/WEBUSERS.md`) |
| `web/msgs` | the `QMC` catalogue, first- and second-level (`docs/MESSAGES.md`) |
| `web/pages` | every page from a view, its routes, the App (sessions, the task of each route, audit), the help topics |
| `web/backup` | the backup file: the payload, sealed under a passphrase (`docs/BACKUP.md`) |
| `lic/qsmcinit` | the listeners bound and handed over and following the `open` address; the slot rule on the web server's answer; the restore step at the IPL and its confirmation; option 4's requests while the web server is down; the `webd-fails` hook |
| `svc/console` | option 4, Work with the web interface (`docs/LOCALCONSOLE.md`) |
| `tools/conformance` | the browser (`src/web`), pinned to the certificate it expects, and the fourteen web suites (`src/suite/web`) |
| `tools/fuzz` | `qsmcfuzz` over QS/400's fuzzer, a workspace of its own: `backup-file`, `backup-payload`, `cert-import`; `build fuzz` |

Sessions and CSRF are `qsweb`'s (`Sessions`, `csrf_ok`); the
handoff's `web/session` is not a crate here, as nothing of it is
QS/MC's own.

## How to run

```
cargo run -p build -- all && cargo run -p build -- --test-image all
cargo run -p build -- test                  every suite of Phases 0 to 3
cargo run -p build -- test --suite web-backup
cargo run -p build -- fuzz                  each target 60 s with coverage
cargo test -p webpages --test golden        every live page against its prototype file
```

A browser reaches a console booted with a NIC at `https://<open
address>/qsmc/`; the first Administrator is the installation's user,
its ID in lower case.

## The pages

17 of the prototype's 90 files are live, each equal to its file under
`qsweb`'s comparison with the corrections in `docs/WEB.md`:

| Prototype | Route |
| --- | --- |
| `qsmc/signon.html` | `/qsmc/signon`, `/qsmc/signoff` |
| `qsmc/welcome.html` | `/qsmc/welcome` |
| `qsmc/console.html` | `/qsmc/console` |
| `qsmc/console-users.html`, `console-roles.html` | `/qsmc/console/users`, `/qsmc/console/roles`, `/qsmc/confirm/user-delete` |
| `qsmc/console-network.html` | `/qsmc/console/network` |
| `qsmc/console-datetime.html` | `/qsmc/console/datetime` |
| `qsmc/console-certs.html` | `/qsmc/console/certs`, `/qsmc/confirm/cert-new`, `/qsmc/confirm/ca-remove` |
| `qsmc/console-backup.html` | `/qsmc/console/backup` (Backup, Restore, History), `/qsmc/confirm/restore` |
| `qsmc/console-shutdown.html` | `/qsmc/console/shutdown` |
| `qsmc/console-locale.html` | `/qsmc/console/locale` |
| `qsmc/console-version.html` | `/qsmc/console/version` |
| `shared/error.html`, `notauth.html`, `confirm.html`, `help.html` | a page not found or refused, Not authorized, every confirmation, `/qsmc/help/` and `/qsmc/msg/` |
| `shared/components.html` | `/qsmc/dev/components`, a test image only |

The other 69 files of `qsmc/` are the pages of later phases' functions
(systems, partitions, plans, service management, updates, the
terminal); the tree's ten routes to them answer that the function
arrives later (`QMC9030`). `nav/`'s four are QS/400's samples.

## The suites

| Suite | Handoff | What it shows |
| --- | --- | --- |
| `web-pages` | 2 | the stylesheet byte for byte; every link from Welcome answers; no page with a script; 80 only redirects |
| `web-first` | 3 | the installation's user signs on as the first Administrator; the certificate names the host and its address |
| `web-roles` | 4 | 22 routes for each of the four roles, page or Not authorized as the matrix says; no route without a task |
| `web-signon` | 5 | the cookie's attributes; a form without its token; the lock-out; the address limit; the idle timeout; sign off |
| `web-users` | 6 | the last enabled Administrator kept; a disabled user's session ends |
| `web-network` | 7 | a change of the browser's interface kept when reached again and taken back when not; `private` added and removed |
| `web-certs` | 8 | a new self-signed certificate to the next connection; a PEM chain and key imported; a wrong key refused |
| `web-backup` | 9 | a backup restored on a second installation: users, network, time, the QSI identity and the web certificate; the refusals |
| `web-slot` | 10 | a slot on trial whose web server fails is not confirmed; the next IPL is on the old slot |
| `web-listeners` | 11 | 443 and 80 on `open` only, nothing on `private`; following a change of the address and its taking back |
| `web-option4` | 12 | the local console's reset; the web interface signs on with it (the golden screens are `console-panels`') |
| `web-hostile` | 13 | not TLS, an oversized head, a malformed request, a body past the limit, a silent connection, too many connections; the session goes on |
| `web-secrets` | 14 | no password, passphrase or key in any log; each kind of change audited |
| `web-processor` | 15 | no web server on a processor without what the cryptography needs; `A3001015`; the local console works |

Suite 1 is Phases 0 to 2's, and 13's fuzz targets are `build fuzz`'s.
`web-network` changes the browser's interface from DHCP to the same
address static and back: QEMU's user network forwards to one guest
address, so the browser cannot follow an address that moves.
`web-listeners` moves it to `10.0.2.16` and watches the listeners from
inside.

## Evidence so far

- Section 1 (`be7ab6f`), built from empty targets: every suite of
  Phases 0 to 2 passed, 32 steps on the disk and the ISO step, with
  `sockets` and `no-secrets` showing TCP listening only on 443 and 80
  of the `open` address (`10.0.2.15:443`, `10.0.2.15:80`) and
  `slot-confirm` confirming on the web server's first answer.
- The QS/VS pin at `3de567e` (`49b2fdf`), built from empty targets:
  the same 32 disk steps and the ISO step passed, `mirror` among them;
  `sockets` and `no-secrets` again listed only `10.0.2.15:443` and
  `10.0.2.15:80`.

## Found while testing

- Phase 1's `mirror` failed two runs in three on the new pins, as it
  had once in Phase 2: "`Resynchronization started: unit 2, ` did not
  appear in time". Phase 2 put it down to the kill part; it was the
  part before. The returned unit's resynchronization is posted by the
  storage service, which starts first, and it can come before "Base
  ready"; the suite looked for it only after that line. It now looks
  for the whole line anywhere in the boot's log (`suite/base/mirror.rs`).
  Four runs in four passed after.
- `webd`'s first answer to itself was first asked over TCP of its own
  address; it timed out whenever DHCP was quick, as the console keeps
  no loopback interface. It is asked over a socket pair. One `time`
  failure in 45 runs after that was not seen again.
- `webd` asked for HTTP headers by their capitalized names; `qsweb`
  keeps them lower case, so the redirect's host and the stylesheet's
  `304` would not have worked. Found by the pages' tests.
- Setting the clock by hand ended the operator's session: sessions,
  the lock-out and the network window were timed by the wall clock.
  They are timed by the uptime (`docs/WEB.md`, Time). Found by Date
  and time's test. The network page's host name went to PID 1 but
  not to the network service, which reads it only on a reload; the
  web server now asks for one after each host change, as the local
  console does.
- The web interface stopped answering after a change of the `open`
  interface that kept its address (a name server, DHCP to static): the
  interface went down and up, PID 1 handed the web server an empty set
  of listeners when the address went, and when it came back could not
  bind 443 again while the web server's last listeners were closing;
  it recorded the set as sent and never tried again. PID 1 now keeps
  the listeners while the address is gone (a socket bound to it answers
  again when it is back), sends an empty set only as the first, and
  sends no set it could not bind whole, trying again at the next
  heartbeat (`lic/qsmcinit/src/host/web.rs`). Found by `web-network`
  and `web-backup`.
- The web server is ready before DHCP has given the console its
  address: it answers its own first request over a socket pair, and
  listens once the address comes. The suites wait for 443 to listen
  before the browser's first request.
- The certificate the first start made named only the host when DHCP
  came after it, as `web-first` found on its second run: the `open`
  address was missing. A certificate the console made is now made
  again with the addresses of a set of listeners that it does not name,
  before that set listens (`docs/WEB.md`).
- Once, the evidence run's first boot of the release image (Phase 0's
  `ipl-slot-a`) saw its first heartbeat only at the 60 s limit, with the
  storage service still finding the disks; run again on the same
  images it passed in 12 s, twice. Not seen since.
- The harness's TLS client resumed its sessions, and a resumed session
  skips the certificate check: a browser pinned to the certificate
  replaced went on. It resumes none now (`tools/conformance/src/web`).
- `qsweb` checks a request head's length while it waits for the head's
  end: a head that arrives whole within one read may pass 16 KiB by up
  to that read (`docs/WEB.md`). Bounded, so not a hole; `web-hostile`
  sends four times the limit.
- QS/400's `httpc` cannot be pinned to one certificate (item 26), and
  `x509kit`'s PKCS #12 reader takes any iteration count (item 27).

## Prototype choices answered with a message

Where a prototype page offers what V1 does not do, the page is drawn
as the prototype has it and the choice answers with a `QMC` message:
remote-host backup, scheduled backups, the reference code history and
the media library in a backup, the NTP server, the DHCP server and SSH
on the private network, a DNS domain, languages other than English, and
keeping terminal windows open (`docs/OPEN_QUESTIONS.md`, item 19). What
the prototype links and does not draw (the users' forms, the Restore
tab, a certificate's View) is `qsweb`'s properties under the page's
own (item 23).

## Not done

- `NEEDS-CODY`: every page looked at in a browser beside the
  prototype.
- The web interface does not listen on `private` when `web.toml` says
  it may (`listen_private`): no page sets it, and PID 1 hands over the
  `open` address's listeners only (item 25).
- A silent connection closes at 60 s through QEMU's user network
  against the 30 s read timeout; why is not yet known (`docs/WEB.md`).
- What QS/400 and QS/VS would change: `qsweb`'s help window's inline
  handler (item 22), `httpc`'s pinning (item 26), PKCS #12's iteration
  counts (item 27), `netd`'s line for a removed role (item 28).
