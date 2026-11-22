# The web server

QS/MC's management surface: `web/webd`, run as the service `webd`
(binary `lic/qsmcwebd`). It is built on QS/400's `qsweb` (HTTP/1.1,
sessions and CSRF, the frame, the partials, the shared pages, golden
comparison), `x509kit` for the certificate and rustls with the
graviola provider for TLS. None of them is written or copied here
(`docs/PHASE3.md`).

## The service

`webd` is at 30 in PID 1's table (`lic/qsmcinit/src/host/table.rs`),
after the network service and the local console. It keeps no
capability: it binds nothing, and is handed its listening sockets by
PID 1. On a processor without what the cryptography needs (AVX2, ADX,
AES, PCLMULQDQ) PID 1 uses a table without it: it is not started, the
status screen's Processor line and `A3001015` say why, and the local
console works as before.

It is ready, and a slot on trial is confirmed (`slot::rule`), when it
has answered a request over TLS: after the first set of listeners
arrives, a client pinned to the certificate in use reads the
stylesheet over a socket pair, through the same TLS configuration,
certificate and pages a browser gets. A socket pair because the
console keeps no loopback interface: a connection to its own address
would go nowhere. That the listeners answer from the network is what
the conformance suites show (`docs/PHASE3.md`). On
a processor without the web server the rule is the base's readiness,
as in Phases 1 and 2.

The release profile aborts on a panic, so `qsweb`'s per-request guard
does not apply: a panic ends `webd`, and supervision starts it again.

## The listeners

PID 1 owns them (`host/web.rs`). `webd` connects to
`/run/qsmcweb.sock`; PID 1 binds TCP 443 and 80 on the `open`
interface's IPv4 address, from the network service's status line, and
sends them on that connection: a line `port address` each and an empty
line, then the sockets on one byte (`base/fdpass`). When the network
service notes another address PID 1 binds again and sends the new set;
`webd` stops accepting on the old ones. A bind that fails is said in
the log and tried again at the next change of address or start of
`webd`. Nothing listens on the `private` interface, on IPv6 or on
loopback. The listeners follow the address; the connections already
open on the old one are left to end.

## TLS and the certificate

TLS 1.2 and 1.3 with the graviola provider's default suites. The
certificate and its key are `/data/web/certificate.pem` (the chain,
leaf first) and `/data/web/key.pem` (PKCS #8, mode 600). At the first
start there are none: `webd` makes an ECDSA P-256 key and a self-signed
certificate valid for 365 days, its common name the host name and its
alternative names the host name and the `open` address, and says its
SHA-256 in the log. A certificate replaced (made anew, or imported as
a PEM chain and key or a PKCS #12 file) is served to the next
handshake; sessions already open keep theirs. A key that does not
belong to the certificate is refused and the certificate in use stays.

The first start can come before DHCP has given the console its
address. A certificate the console made itself (marked by
`/data/web/made-here`) that does not name each address of a set of
listeners is made again with them before that set listens, and said in
the log. An imported certificate is the operator's and is never made
again; importing one removes the mark, and a new self-signed one sets
it.

The web certificate is not the console's QSI identity
(`/data/state/identity.crt`), which is unchanged.

## Limits and headers

| What | Limit |
| --- | --- |
| A request's head | 16 KiB (`qsweb`; 431), checked while its end has not come: a head that arrives whole within one read may pass it by up to that read |
| A body read whole | 8 MiB (`qsweb`; 413) |
| Requests a connection | 200 (`qsweb`) |
| A connection silent | 30 s, then closed (`web-hostile`: closed at 30 s) |
| Connections at once | 64, of them 16 from one address; one more is closed at once |

Every answer on 443 carries `Content-Security-Policy: default-src
'none'; style-src 'self'; img-src 'self'; form-action 'self';
frame-ancestors 'none'; base-uri 'none'` but the terminal window, whose
policy also allows its script, its socket and its style attributes
(`docs/TERMINAL.md`); `X-Frame-Options: DENY`,
`X-Content-Type-Options: nosniff`, `Referrer-Policy: no-referrer`, and
`Cache-Control: no-store` unless it says otherwise. There is no
`Strict-Transport-Security`: with a self-signed certificate it would
leave a browser no way past its warning. `Server` is `QSMC/V1R0M0`.

Port 80 answers every request with `301` to the same path and query on
`https://` and the host the request named (or the address it came
in on), and nothing else.

## Static files

The stylesheet is `qsweb::CSS`, served byte for byte at
`/qsmc/shared/qs400.css` with an entity tag (its SHA-256's first 16
hex digits) and `max-age=3600`; a matching `If-None-Match` is answered
`304`. The one script is QS/400's terminal script,
`/qsmc/terminal/script.js` (`termlook::script::text`), on the terminal
window only; `qsweb`'s navigation script is not served.

## Pages and their golden files

Each page is a function of a view (`web/pages`): the console's state
when served, the prototype's sample data in `web/pages/tests/golden.rs`,
which compares the result with the prototype's file under `qsweb`'s
golden comparison (whitespace, entity forms, attribute order and the
CSRF field do not count; served URLs are mapped back to file names by
`webpages::url::proto`). A page is added by writing its view and its
rendering, a route with its task (`app/routes.rs`), and its golden
test with the prototype's data.

Golden corrections, each the only difference allowed:

| File | Correction | Why |
| --- | --- | --- |
| `qsmc/signon.html` | the form's `action` is `signon.html`, not `welcome.html` | the form posts back to the sign-on route |
| `shared/error.html`, `notauth.html`, `confirm.html`, `components.html` | the frame is QS/MC's, as on its other pages: the wordmark and title suffix `QS/MC Management Console`, the header (`Console: QSHQMC1`, `User: qsadmin`) and the footer; the breadcrumb's root is `QS/MC` | the prototype's shared pages carry QS/400's frame (`docs/OPEN_QUESTIONS.md`, item 20) |
| `shared/error.html` | no `Job log` button; `Technical details` for `Technical description` | QS/MC has no job log; `qsweb`'s heading |
| `shared/notauth.html` | the closing paragraph is QS/MC's (`Brand::notauth_note`) | the prototype's names QS/400's security officer |
| `shared/help.html` | the window's wordmark, title and footer are QS/MC's; Contents and Index lead to the help pages; Close window is a plain link to the console's start | as above; `qsweb` closes by script only for a product that serves script (item 22) |
| `shared/components.html` | the job actions (`../nav/active-jobs.html` and the two beside it) lead to the page itself | QS/MC has no job pages; the page is served under `/dev/components` in a test image only, with its error page example at `/dev/error` |
| `qsmc/connectivity.html` | Reset connection and Change password lead to the add form (`add-system.html`) | both pair the system again with a new password; QSI has no service password to change |
| `qsmc/add-system.html` | Verify TLS certificate: grey text, the system's own certificate pinned when paired, not a box for a certificate authority | QSI trusts a system by the certificate pinned at pairing (`docs/MGMT.md`) |
| `qsmc/add-system-result.html` | the form carries the system's name (`ms`); a Certificate SHA-256 row after Reference code | Add and Cancel act on that system; the operator compares the fingerprint with the one the system shows |
| `qsmc/servers-empty.html`, `-error.html`, `-paged.html` | Systems Management drawn open, as on `servers.html` | the console opens the group on every state of the page |
| `qsmc/servers-paged.html` | the boxes are `s1` to `s25`, not `s0` to `s24`; `Page 1 of 2`, not `of 3` | the other states number from 1; 27 entries at 25 a page are two pages |
| `qsmc/confirm-poweroff.html`, `confirm-poweron.html` | the form begins before the choice, with the system's name | as drawn, the radios and selects are outside the form and would not be sent |
| `qsmc/terminal.html`, `terminal-disconnected.html` | the picture has `id="term"`; the window's `<script>` element is the server's (dropped from the comparison by its source) | the script replaces the picture by its id |
| `qsmc/terminal-disconnected.html` | the status line names `QSTEST01`, not `QSDEV01` | the window is of `QSTEST01` |
| `qsmc/terminal-config.html` | one form, not one inside another; the strip's box named `strip`, not `cp`; Open and Cancel named | a form in a form is not valid HTML; the code page is `cp` already |
| `qsmc/console-shutdown.html` | the form begins before the Restart / Power off choice and the message, not after them | as drawn, the choice and the message would not be sent; `qsweb`'s confirmation has them in its form |
| `qsmc/vr-storage.html`, `vr-tape.html` | the buttons named (`vdisk`, `pool`; `vtape`, `ptape`) | the page tells which was pressed |
| `qsmc/vr-optical.html` | Read-only checked and `disabled`; the address and Fetch `form="fetch"`, a form of their own after the upload's; Upload named | an upload streams with its length said first, so after the file only Upload's part may come (`web/pages/src/vres/upload.rs`); a host's images are read-only to partitions, QSI has no other |
| `qsmc/partitions-paged.html` | the boxes are `p1` to `p25`, not `p0` to `p24` | as Servers' paged state |
| `qsmc/partition-io.html`, `manage-profiles.html`, `create-6.html` | the buttons named (`add`, `remove`; `create`, `save`, `default`; `add`) | the page tells which was pressed |
| `qsmc/confirm-activate.html`, `confirm-shutdown.html` | the form carries the choices of the page before as hidden fields | as drawn, the confirmation's OK would send nothing of what was chosen |
| `qsmc/schedule.html` | the buttons named (`add`, `refresh`) | the page tells which was pressed |
| `qsmc/event-detail.html` | a row `Managed system's number` after Problem number | the console numbers the fleet's problems; the system's own number is shown beside it (`docs/SERVICE.md`) |
| `qsmc/dumps.html` | the form begins before the list, not after it; each box has the dump's key as its value; the buttons named (`download`, `copy`, `delete`) | as drawn, the boxes are outside the form and would not be sent |
| `qsmc/code-updates.html` | the note under the hosts says what activating a QS/VS update does: the host restarts into its other slot and its running partitions are shut down first | the design's note (the management layer restarts, partitions keep running) is not what QS/VS does (`docs/UPDATES.md`) |
| `qsmc/code-update-progress.html` | View log named (`log`); the Save system plan step's plan linked (`plan-view.html`) | the page tells which was pressed; the plan saved first is linked (`docs/PLANS.md`) |
| `qsmc/plans.html` | Create plan and Import named (`create`, `import`) | one form holds both; the page tells which was pressed |
| `qsmc/plan-view.html` | an Edit link (the description, Save as template) before Export; Export leads to the document (where it lands is `plans.html`), not `#` | the prototype has neither page; Export downloads the plan |
| `qsmc/plan-deploy-progress.html` | Cancel deployment named (`stop`) | the page tells it from Refresh and from the wizard's Cancel |

What a live page adds to its golden file's data, its golden test
unchanged: Add scheduled operation lists `the console` last among the
targets, Install code update's alone (`docs/SCHED.md`), and Code
updates' Schedule opens it with that operation and its row's target
chosen; Code updates shows a download that stopped with its message
(`QMC7204`, `QMC7205`) until it is asked for again.

What System plans' live pages add, their golden tests unchanged: the
validation (`plan-deploy-2.html`) has a row for every item of the plan
where the prototype's lists eight, and a partition deployed to another
host is a Warning for its new MAC addresses and serial number
(`docs/PLANS.md`, "Against the prototype"); step 3 says what a
partition's Warning is about the same way, `Warning (POOL2, C3)`. Step
1 lists every QS/VS host, the plan's own last.

## Time

Sessions, the sign-on lock-out and the window of an `open` address
change are timed by the console's uptime, which setting the clock does
not move; what is shown and kept (a user's last sign-on, the backups'
dates) is the wall clock. Language, locale and session keeps the date, time
and number formats every page shows; the display offset is on Date and
time, where the prototype has it. Date and time's time zone is the display
offset `host.toml` keeps (`UTC`, `UTC+hh:mm`): the page offers each
half hour from UTC-12:00 to UTC+14:00 and the three quarter-hour
offsets, with no summer time. Its NTP status is PID 1's `synced` fact:
when and with which server the network service last posted `C3002030`.

## Certificates

The page shows the web certificate in use; View adds its every field
(names, serial, dates, both fingerprints) under the buttons. Generate
self-signed... goes through the confirmation page and makes a new
ECDSA P-256 certificate for the host name and the `open` address,
served to the next connection. The import form takes a PEM file with
the chain and its key, or PKCS #12 with its password (`x509kit`); a key
that is not the certificate's is refused (`QMC8302`) and nothing
changes. A certificate imported without a key that is an authority's is
added to the trusted authorities, `trusted.toml` (32 at most), which
nothing uses before the update server's connection in Phase 6: managed
systems are trusted by their pinned QSI certificates. Download CA
certificate gives the chain's last certificate, which is the
certificate itself while it is self-signed. The prototype has no
certificate request, and there is none.

## The managed systems' pages

Phase 4's pages read the managed-systems service (`docs/MGMT.md`) over
its socket: a list from what it keeps, never waiting on a managed
system; an action as a live request, whose refusal shows the console's
message for it and then every message the managed system gave, its own
identifiers first-level and, for a QS/VS host's `QVS` identifiers, their
second-level text from the catalogue (`/qsmc/msg/<id>`). The routes
mirror the prototype's files:

| Prototype | Route |
| --- | --- |
| `connectivity.html` | `/qsmc/connectivity` (`?test=`, `POST` Test all) |
| `add-system.html`, `add-system-result.html` | `/qsmc/servers/add`, `/qsmc/servers/add/result`, `/qsmc/servers/reset?ms=`; Remove `/qsmc/confirm/system-remove?ms=` |
| `servers.html` and its states | `/qsmc/servers?t=&s=&sort=&page=&rows=` |
| `system-general.html` and its six tabs | `/qsmc/server/<name>/<tab>` |
| `confirm-poweroff.html`, `confirm-poweron.html` | `/qsmc/server/<name>/confirm?op=poweroff\|restart\|poweron` |
| `refcode-history.html`, `refcode.html` | `/qsmc/refcodes?ms=&p=&f=&t=&page=`, `/qsmc/refcode/<code>?ms=&p=&seq=` |
| `terminal-config.html`, `terminal.html`, `terminal-disconnected.html` | `/qsmc/terminal`, `/qsmc/terminal/<host>/<id>` (`docs/TERMINAL.md`) |
| `control-panel.html` | `/qsmc/partition/<host>/<id>/panel`; 08 and 22 `/qsmc/confirm/panel` |

Welcome's strip counts the managed systems and partitions by state from
what is kept; its serviceable events, dumps and updates stay as Phase 3
left them until Phase 6.

What a page offers and QSI v1 or this release does not do answers with
a message: a host's name (`QMC1008`), its IPL side, keylock and boot
mode (`QMC2220`), Power on (`QMC2201`), Servers' actions of later phases
(Manage system profiles, Manage dumps: `QMC9030`), and the terminal
toolbar's Copy, Paste, Keypad and Keyboard Map (`QMC8700`). A function
a managed system's hello does not offer answers `QMC1114`, naming what
it lacks (`mgmtproto::ask::lacks`).

A host's code is explained from the catalogue its QS/VS release writes,
built into the image at `/qsmc/catalogue.json`; a partition's QS/400
code from `srcdata`; the console's own from `refcode`. A code none of
them knows is shown with its text as posted and `QMC1201`.
