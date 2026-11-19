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

## Evidence so far

- Section 1 (`be7ab6f`), built from empty targets: every suite of
  Phases 0 to 2 passed, 32 steps on the disk and the ISO step, with
  `sockets` and `no-secrets` showing TCP listening only on 443 and 80
  of the `open` address (`10.0.2.15:443`, `10.0.2.15:80`) and
  `slot-confirm` confirming on the web server's first answer.

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

## Prototype choices answered with a message

Where a prototype page offers what V1 does not do, the page is drawn
as the prototype has it and the choice answers with a `QMC` message:
remote-host backup, the NTP server, the DHCP server and SSH on the
private network, languages other than English, and keeping terminal
windows open (`docs/OPEN_QUESTIONS.md`, item 19).

## Not done

- Section 3, pages 6 to 12: Date and time, Certificates, Backup and
  restore, Shut down or restart, Language and locale, Console version,
  the components page under `/dev/`.
- Section 4: the local console's option 4.
- Section 5: the suites that reach the web interface from outside.
- `BLOCKED(phase3)`: a change to the `open` interface's address from
  the web interface waits for `base/netd` to keep a change only when it
  is confirmed (`docs/OPEN_QUESTIONS.md`, item 24).
- The documents `docs/WEBUSERS.md`, `docs/BACKUP.md` and
  `docs/MESSAGES.md`; `docs/WEB.md` has the server, the listeners and
  the golden corrections.
- `NEEDS-CODY`: every page looked at in a browser beside the
  prototype.
