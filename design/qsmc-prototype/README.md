# Implementation handoff: Quartz Systems/Management Console web UI

For Claude Code, in the `quartz-systems-mc` repository. This zip is the approved design for the QS/MC web UI, the only management surface of the console appliance besides its serial login. Make the console produce these pages from live state: its own settings, and the managed systems it reaches over QSI (the Quartz service-interface protocol, port 2300, Wire over mTLS).

## Where the design is

The web UI prototype is committed in this repository under `design/qsmc-prototype/`: the `qsmc/` pages, `shared/` (the stylesheet, the shared pages, the rendered 5250 screens), its `README.md` (this file) and `STYLE.md`. It is the specification of every page: each HTML file is a golden rendering, and the same page served live with the same data must produce the same DOM (whitespace and the CSRF token aside).

```
design/qsmc-prototype/
  README.md               this handoff
  STYLE.md                colours, type, table rules, verbs, message rules, what-not-to-do; the QS/MC section at the end
  index.html              opens qsmc/signon.html
  shared/qs400.css        the ONLY stylesheet, shared with the QS/400 web products. Ship it byte-for-byte.
  shared/img/             the 5250 screens rendered with the service console's 9x16 font (goldens for the terminal window)
  shared/components.html  every pattern once: frame, message area, list, properties, wizard, controls
  shared/confirm.html  shared/error.html  shared/help.html  shared/notauth.html   the generic pages, framed for QS/MC
  qsmc/                   81 pages
```

Pages of other products that QS/MC links to directly are included so the prototype clicks through; they are reference only:

- `nav/active-jobs.html` (from Navigator)
- `nav/end-job-confirm.html` (from Navigator)
- `nav/job-props-general.html` (from Navigator)
- `nav/ptfs.html` (from Navigator)

## Ground rules (do not negotiate these away)

1. **Server-rendered HTML, full-page reloads.** Forms are `method="post"`; lists sort and page with `?sort=` / `?page=` / `?rows=`; confirmation is a separate GET page that POSTs back; progress pages reload with `<meta http-equiv="refresh">`. The only JavaScript is the terminal window (WebSocket). Every other page works with script disabled, and all of them remain usable with CSS disabled.
2. **No new CSS.** The stylesheet is the QS/400 one, unchanged (177 lines). QS/MC's grouped action `<select>` is `<optgroup>`, its control panel strip is a one-row `table.list`. If something is truly unavoidable, append under a `/* QS/MC */` comment and record it in `STYLE.md`.
3. **The reference code is the most important field.** Courier, visible on every managed-system and partition row, always a link to `/refcode/<code>` (Meaning, Cause, Recovery, the partition it came from).
4. **Message IDs are links everywhere.** The console's own are `QMC` (`QMC1001` connected, `QMC2210` power off refused, `QMC3002` waiting for console, `QMC3110` activate failed, `QMC4001` console connection closed, `QMC5003` function accepted, `QMC6001` deploying, `QMC7001`/`QMC7010` updates); the managed system's messages (`CPF…`, `HTP…`) are shown as it sent them.
5. **Verbs**: Activate / Shut down / Restart for partitions; Power on / Power off / Restart for managed systems; Load / Unload for optical media; Deploy for plans; Load / Install for code. Power off, Delete partition, Deploy, Install update, Shut down console, and control panel functions 08 and 22 always go through the confirmation page listing exactly what is affected; none is reachable by GET.
6. **Hierarchy is host → partition.** The Partitions list across all hosts is the page operators live on; the per-host view is secondary.
7. **No other vendor's product or feature names, wording or marks.** Names in use: QS/MC, QS/VS, QS/400, Quartz Systems, and generic terms (managed system, partition, profile, reference code, serviceable event, control panel function).
8. **Commits**: per section, under Cody's git identity only (never as yourself); one-line conventional messages. Never push.

## Shared infrastructure

QS/MC is its own appliance OS (Linux 6.18 LTS, all-Rust static-musl userland, built with the QS/400 toolchain) and its own repository; it depends on, or vendors, the same crates the QS/400 web products use: the HTTP server, the frame template and the five partials (`message_row`, `list_table`, `paging`, `props`, `wizard`), sessions and CSRF, help and second-level message routes. If those crates exist by the time this starts, use them; if not, build them here to the same spec (`design/handoff-http-server-administration.md` in the project has it) and keep them reusable.

**Frame.** Header band `Quartz Systems/Management Console` | `Console: QSHQMC1  User: qsadmin  Sign off  Help`; navigation tree Welcome · Systems Management (Servers, Partitions, Custom groups) · System Plans · Console Management (Console settings, Users and roles, Network settings, Date and time, Backup and restore, Certificates, Shut down or restart, Console version) · Service Management (Serviceable events, Reference code history, Manage dumps, Code updates, Connectivity) · Updates; footer `QS/MC V1R0M0 · Quartz Systems`. Route prefix `/qsmc/`.

**Sign-on.** Console users (not QS/400 profiles): ID, password, role Administrator / Operator / Viewer / Service, with the role → task matrix of `qsmc/console-roles.html`. The sign-on page carries the configurable notice `<p>`. Sessions: opaque cookie, idle timeout and maximum length from Language, locale and session settings; terminal windows may outlive the web session when that option is on.

**QSI client.** One connection per managed system (`qsmc/connectivity.html`), kept open, resumed quietly after a drop, with the service password age tracked; every page that touches a managed system is a QSI call on that connection, and a failed call renders the system's message in the message area, never a stack trace. The private management network (eth1, DHCP for service interfaces) and the public one (eth0, the web UI) are separate and configured on `qsmc/console-network.html`.

**Conventions.** URLs mirror the file names (`qsmc/partition-vadapters.html` → `GET /qsmc/partition/<host>/<id>/vadapters`). Default 25 rows per page. Dates `MM/DD/YY`, times `HH:MM:SS` in the console time zone.

## Build order

One step = one commit, ending with the page matching its golden file.

### Group 1 — console, sign-on, welcome, connectivity

| Step | Golden page(s) | Route(s) | Backend |
|---|---|---|---|
| 1 | `qsmc/signon.html` | `GET/POST /signon` | console user store, notice text, audit of sign-ons |
| 2 | `qsmc/console.html`, `console-users`, `console-roles`, `console-network`, `console-datetime`, `console-certs`, `console-backup`, `console-shutdown`, `console-locale`, `console-version` | `/console/...` | local settings: users and roles matrix, eth0/eth1 and DHCP, NTP (client and server to the private network), the console's TLS certificate and trusted CAs, backup to file or SFTP with passphrase and the backup history, Restart / Power off with the confirmation page, language/locale/session, version and hardware |
| 3 | `qsmc/connectivity.html`, `qsmc/add-system.html`, `qsmc/add-system-result.html` | `/connectivity`, `/servers/add`, `/servers/add/result` | QSI connect, Hello, type detection (standalone QS/400 or QS/VS host), Test and Reset connection, password age warning past 90 days |
| 4 | `qsmc/welcome.html` | `/welcome` | section links and the status strip: managed systems and partitions by state, open serviceable events, dumps, updates, console version and uptime |

### Group 2 — managed systems

| Step | Golden page(s) | Route(s) | Backend |
|---|---|---|---|
| 5 | `qsmc/servers.html` (+ `-empty`, `-error`, `-paged`) | `/servers?t=&s=&sort=&page=` | QSI inventory per system: type, state, available processors and memory, type-model, serial, reference code, partition count; grouped action `<select>` with `<optgroup>` Operations / Configuration / Connections / Serviceability |
| 6 | `qsmc/system-general.html`, `-processors`, `-memory`, `-io`, `-network`, `-power`, `-capabilities`, `-refcode`; a standalone QS/400's `-status`, `-dumps` | `/server/<name>/{general,processors,memory,io,network,power,capabilities,refcode}`; a standalone's `/server/<name>/{general,status,dumps,capabilities,refcode}` | QSI managed-system properties; I/O with each device's type, vendor, use and whether a partition can be given it; a host's Network interfaces; Power-on parameters (IPL side, keylock, boot mode, auto-start) written back. A standalone QS/400 has its own tabs: General with its fix level, Status (processors, main storage, the system ASP, jobs, as its WRKSYSSTS counts them; QSI base's `Resources`) and its own dumps |
| 7 | `qsmc/confirm-poweron.html`, `qsmc/confirm-poweroff.html` | `/server/<name>/confirm?op=poweron\|poweroff\|restart` → POST | lists every partition affected; power off delayed (shut partitions down first, `PWRDWNSYS *CNTRLD` through QSI) or immediate; `QMC2210` when refused |
| 8 | `qsmc/refcode-history.html`, `qsmc/refcode.html` | `/refcodes?ms=&p=&f=&t=&page=`, `/refcode/<code>` | QSI reference code log with words 2–9; explanation pages for the C1/C2/C6/A6/B6/D6 families with Meaning, Cause, Recovery and links to the partition's terminal window and control panel |
| 9 | `qsmc/vr-ethernet.html`, `vr-storage`, `vr-optical`, `vr-tape`, `vr-pools`, `qsmc/create-vdisk.html` | `/server/<host>/virtual/{ethernet,storage,optical,tape,pools}`, `/server/<host>/virtual/storage/create` | QS/VS host resources: virtual switches and VLANs, storage pools and virtual disks (thin/thick, assign, resize, unassign), media library (upload or fetch an ISO, Load / Unload per partition), virtual and pass-through FC tape drives, shared processor pools |

### Group 3 — partitions

| Step | Golden page(s) | Route(s) | Backend |
|---|---|---|---|
| 10 | `qsmc/partitions.html` (+ `-empty`, `-error`, `-paged`) | `/partitions?ms=&s=&n=&sort=&page=` | every partition across every host plus standalone systems as one row with no ID; grouped action `<select>` |
| 11 | `qsmc/partition-general.html`, `-processors`, `-memory`, `-io`, `-vadapters`, `-settings` | `/partition/<host>/<id>/{general,processors,memory,io,vadapters,settings,other}` | profile editing (shared/dedicated, min/desired/max, pools, weight; memory; physical slots; virtual adapters Create / Edit / Remove; auto start, boot mode, IPL source, keylock, console device) |
| 12 | `qsmc/activate.html`, `qsmc/confirm-activate.html` | `/partition/<host>/<id>/activate` → confirm → POST | profile choice, open a terminal window on activation, Advanced keylock / IPL source / IPL type; `QMC3110` when a required resource is missing |
| 13 | `qsmc/shutdown.html`, `qsmc/confirm-shutdown.html` | `/partition/<host>/<id>/shutdown` → confirm → POST | Delayed (`PWRDWNSYS *CNTRLD` with delay), Operating system (`*IMMED` through QSI), Immediate; optional restart |
| 14 | `qsmc/create-1.html` … `create-9.html` | `/partitions/create/step/1..9` | wizard state in the session; step 9 shows the full profile; Finish creates the partition Not activated |
| 15 | `qsmc/manage-profiles.html` | `/partition/<host>/<id>/profiles` | Activate / Edit / Copy / Delete / Set default; Save current configuration as profile |
| 16 | `qsmc/schedule.html`, `qsmc/schedule-add.html` | `/schedule`, `/schedule/add` | scheduled operations (once / daily / weekly / monthly) for activate, shut down, restart, power on/off, backup, code update; Run now |
| 17 | `qsmc/groups.html`, `qsmc/group-create.html`, `qsmc/group-view.html` | `/groups`, `/groups/create`, `/group/<name>` | named sets of systems and partitions; the group page is the Partitions list filtered to its members |

### Group 4 — terminal window and control panel (the script page)

| Step | Golden page(s) | Route(s) | Backend |
|---|---|---|---|
| 18 | `qsmc/terminal-config.html` | `GET/POST /terminal` | partition choice, screen size, code page, colours, control panel strip on/off, take over |
| 19 | `qsmc/terminal.html`, `qsmc/terminal-disconnected.html` | `/terminal/<host>/<id>?menu=&scale=`, `/terminal/<host>/<id>/screen.png`, `wss://…/terminal/<host>/<id>/ws`, `POST .../key` | the multiplexed console stream from the QS/VS host (or the standalone system's service interface) over QSI; the grid drawn exactly as Access for Web and Access for Desktop draw it: the service console's 9×16 font and palette, integer scaling, OIA row, the desktop's keypad at the right (F1–F24, Enter Reset Attn, SysRq PgUp PgDn, FldExit Field+ Field−, Help Print Clear), menu bar, toolbar and status bar; the control panel strip above the grid (reference code, meaning, 03 / 08 / 21 / 22 / 34 and a function field); `render()` on the server for the no-script PNG and Print screen PDF; `shared/img/5250-24x80.png` and `5250-24x80-off.png` are the goldens |
| 20 | `qsmc/control-panel.html` | `/partition/<host>/<id>/panel` (+ POST function) | functions 01–70 with their texts; 25 then 26 unlock 50–70; 08 and 22 via the confirmation page; result pre with the reference code before and after |

### Group 5 — system plans, service management, updates

| Step | Golden page(s) | Route(s) | Backend |
|---|---|---|---|
| 21 | `qsmc/plans.html`, `qsmc/plan-view.html` | `/plans`, `/plan/<name>` | capture a managed system's configuration (host, switches, pools, every partition profile) as a plan; View (printable nested tables), Export, Import, Delete |
| 22 | `qsmc/plan-deploy-1.html` … `-3.html`, `qsmc/plan-deploy-progress.html` | `/plan/<name>/deploy/step/1..3`, `/plan/<name>/deploy/progress` | target host, validation table (OK / Warning / Error per item), partitions to create, Deploy via confirmation, progress page with Pending / In progress / Complete per step and Cancel |
| 23 | `qsmc/events.html`, `qsmc/event-detail.html` | `/events?s=&f=&t=&ms=&page=`, `/event/<n>` | serviceable events from QSI: description, reference code with explanation, FRU table (virtual FRUs named as resources), analysis log; Close, Repair, Add note, Export |
| 24 | `qsmc/dumps.html` | `/dumps` | partition and service processor dumps: Download, Copy to remote host, Delete; dump space |
| 25 | `qsmc/code-updates.html`, `qsmc/code-update-progress.html` | `/updates/code`, `/updates/code/<host>/progress` | console and QS/VS host levels against `updates.quartzsystems.net`, Download / Install (confirmation, a system plan saved first) / Schedule; progress page |
| 26 | `qsmc/updates.html` | `/updates` | fleet overview: console, hosts and partitions with current and latest levels; partitions link to Navigator's PTF page on that partition |

## Fixture data

Console `QSHQMC1`; managed systems `QSHQ400` (standalone) and `QSHQVS1` (QS/VS host, 8 processors, 131,072 MB, pools POOL1/POOL2, switches ETHSW0/ETHSW1); partitions `QSDEV01` (1, running), `QSTEST01` (2, not activated, `A6005008`), `QSPROD01` (3, running); 25 branch hosts `QSBR01VS1`…`QSBR25VS1` for the paged states; serviceable event 1042 (`B6000100` on VD04); system plan `qshqvs1-baseline`; users qsadmin / operator1 / viewer / qsservice.

## Product-specific acceptance

- Every golden page diffs clean with the fixture data; every link and form action on every served page resolves; script off and CSS off, every page except the terminal grid is fully operable.
- `partitions` shows 25 rows at 1024×768 without scrolling the page body; the reference code column is never empty (`00000000` for a healthy system).
- Power off, Delete partition, Deploy, Install, Shut down console, functions 08 and 22 are reachable only through the confirmation page and a POST with the CSRF token.
- The terminal window's server-rendered PNG of the fixture screen is byte-identical to `shared/img/5250-24x80.png`.
- No other vendor's names or marks anywhere in the repository (grep in CI).
