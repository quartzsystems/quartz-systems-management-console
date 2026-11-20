| `qsmc/console-shutdown.html` | the form begins before the Restart / Power off choice and the message, not after them | as drawn, the choice and the message would not be sent; `qsweb`'s confirmation has them in its form |
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

The web certificate is not the console's QSI identity
(`/data/state/identity.crt`), which is unchanged.

## Limits and headers

| What | Limit |
| --- | --- |
| A request's head | 16 KiB (`qsweb`; 431), checked while its end has not come: a head that arrives whole within one read may pass it by up to that read |
| A body read whole | 8 MiB (`qsweb`; 413) |
| Requests a connection | 200 (`qsweb`) |
| A connection silent | 30 s, then closed |
| Connections at once | 64, of them 16 from one address; one more is closed at once |

Every answer on 443 carries `Content-Security-Policy: default-src
'none'; style-src 'self'; img-src 'self'; form-action 'self';
frame-ancestors 'none'; base-uri 'none'`, `X-Frame-Options: DENY`,
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
`304`. No page carries a script in this phase, so the navigation
script is not served.

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
| `shared/help.html` | the window's wordmark, title and footer are QS/MC's; Contents and Index lead to the help pages; Close window carries `qsweb`'s inline handler | as above; the handler is `qsweb`'s (item 22) |
| `shared/components.html` | the job actions (`../nav/active-jobs.html` and the two beside it) lead to the page itself | QS/MC has no job pages; the page is served under `/dev/components` in a test image only, with its error page example at `/dev/error` |

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
