# Web users, roles and sign-on

The web interface has its own users, apart from the local console's
service tools users. Both are kept on the data file system; neither
list is ever the other's after the first start.

## The list

`/data/state/webusers.toml`, through `base/state`, written by the web
server (`web/users`, `webusers::list`) and, while the web server is
down, by PID 1 for the local console's option 4. Each user has:

| Field | What |
| --- | --- |
| `id` | 1 to 32 letters, digits, `.`, `_` and `-`, a letter first; kept in lower case |
| `description` | free text |
| `role` | Administrator, Operator, Viewer or Service |
| `hash` | the password's Argon2id hash (`base/users`); the password is never kept |
| `enabled` | whether it may sign on |
| `last_sign_on`, `last_from` | when and from which address it last signed on |
| `changed` | when its password last changed |

Passwords follow `base/users`' rules, stated on every page and panel
that takes one: 8 to 128 characters, at least one letter and one digit,
not containing the user ID.

The first Administrator is made at the web server's first start from
the first service tools user of `/data/state/users.db`: the
installation's user, its ID in lower case and the same password hash.
After that the lists are separate.

The last enabled Administrator cannot be deleted, disabled or given
another role (`QMC8010`). An Administrator who is locked out is reset
from the local console, option 4.

## Roles and the matrix

The prototype's Roles page changes the matrix, so it is state: the
`[matrix]` of `webusers.toml`. Administrator has every task and is not
changed. The default is the prototype's:

| Task | Administrator | Operator | Viewer | Service |
| --- | --- | --- | --- | --- |
| View systems and partitions | yes | yes | yes | yes |
| Open terminal window | yes | yes | | yes |
| Activate / shut down partitions | yes | yes | | |
| Power on / off managed systems | yes | yes | | |
| Create / delete partitions | yes | | | |
| Virtual resources | yes | | | |
| System plans | yes | | | |
| Serviceable events | yes | yes | yes | yes |
| Code updates | yes | | | yes |
| Console settings | yes | | | |
| Users and roles | yes | | | |
| Control panel functions 50-70 | yes | | | yes |

Every route names its task (`webpages::app::routes::task_of`); a test
holds that no route of the navigation tree lacks one. Users and roles
and the delete confirmation need Users and roles; Console version and
Welcome, View; every other Console Management page and confirmation,
Console settings. The managed systems' pages of Phase 4:

| Route | Task |
| --- | --- |
| Servers, a system's properties, Open terminal window, a terminal window and its picture, the control panel | View |
| Saving a system's General or Power-on parameters tab | Console settings |
| Add managed system, Reset connection, Remove | Console settings |
| Power off, Restart, Power on (the confirmation and its POST) | Power on / off managed systems |
| Connectivity and Test connection, Reference code history, an explanation | Serviceable events |
| A key typed in a terminal window, control panel functions 01 to 49, the confirmation of 08 and 22 | Open terminal window |
| Control panel functions 50 to 70 | Control panel functions 50-70 |

A Viewer sees a terminal window and cannot type (`docs/TERMINAL.md`).
The later phases' routes need the task their pages will: View for
partitions and groups, System plans, Serviceable events for events and
dumps, Code updates for updates. A route the role lacks answers Not authorized
(403, `QMC9010`), and the attempt is audited. A change of the matrix
applies at each user's next sign-on.

## Sign-on

| Rule | Value |
| --- | --- |
| Wrong passwords before an ID is locked | 5 in a row (`base/users`) |
| Lock | 5 minutes (`QMC9002`); setting a new password ends it |
| Attempts from one address | 20 a minute (`QMC9004`) |
| A missing ID | answers as a wrong password, after the same work |
| Idle timeout | 30 minutes by default, 1 to 480 (Language, locale and session) |
| Longest session | 12 hours by default, 1 to 72 |
| Cookie | `QSMCSID`, `HttpOnly`, `Secure`, `SameSite=Strict`, `Path=/qsmc` |
| Forms | each carries the session's token; one without it is refused (403, `QMC9011`) |

Sessions, the lock and the address limit are timed by the console's
uptime, which setting the clock does not move. A disabled or deleted
user's sessions end at their next request (`QMC9006`); signing off
ends the session at once. The lock and the address limit are kept in
memory: a restart of the web server forgets them.

## Audit

Every change made through the web interface is a line of the system
log, `audit: user <id> from <address>: <what>: <outcome>`: users and
roles, every setting, the certificate, a backup and a restore, a power
down, a sign-on and its refusal, a page refused. Never a password, a
passphrase or a key. The local console's option 4 is audited by the web
server as `from the local console`, with the session's user.

Phase 4's actions are each audited with the managed system's answer
(`paired with QSHQVS1`, `refused: QVS1011 ...`): adding, pairing again,
testing and removing a managed system, its description and power-on
parameters, power off and restart and power on, a control panel
function, a terminal window opened or disconnected and its socket,
Print Screen. Every request to a managed system also carries the
signed-on user's ID, for the managed system's own log. The pairing
password is in no line.
