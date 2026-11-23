# The terminal window

A partition's console in a browser: Open terminal window
(`/qsmc/terminal`), the window (`/qsmc/terminal/<host>/<id>`), and the
control panel (`/qsmc/partition/<host>/<id>/panel`). The console's
cells come from the QS/VS host over QSI (the console family's stream,
`qsipart` frames), through the managed-systems service
(`docs/MGMT.md`); they are drawn by QS/400's look (`termlook`) and, with
script, by QS/400's terminal script. Nothing of either is written here
(`web/pages/src/terminal`).

## Viewers

A host allows one viewer of a partition's console. `mgmtd` is that
viewer: it attaches when the first window opens, keeps the latest whole
screen (a window that opens later is sent it at once), sends every frame
and update to each window that has it open, and detaches when the last
window closes. Keys are taken from any window whose user may type and
dropped from one whose user may not. Take over (Open terminal window)
takes the console from another console, not from another window of this
one: windows of one console share it.

A window that cannot attach (the partition not activated, another
console viewing it, the system not connected) is the disconnected page
with `QMC4001` and the system's own message.

## The window

The menu, the toolbar, the control panel strip (the partition's current
code, its meaning, functions 03, 08, 21, 22 and 34, and the function
field), the screen with the keypad beside it, the status line: as the
prototype draws them (`terminal.html`), without the navigation column.

| Route | What |
| --- | --- |
| `GET /terminal/<host>/<id>` | the window; it opens the window this web session holds when there is none (`?scale=2`, `?strip=0`, `?c=w`, `?take=1`) |
| `POST` the same | a key of the keypad (`k=F3`), the toolbar (`tb`: Connect, Disconnect, Print Screen; the others answer that they are not in this release), a function of the strip (`f`, `fn`) |
| `GET .../screen.png` | the picture: the held window's latest whole screen, or one asked of the host (`console` `Screen`) |
| `GET .../screen-off.png` | the same greyed, as a disconnected window shows it |
| `GET .../print` | Print Screen: `termlook`'s PDF, its producer `QS/MC Management Console` |
| `GET .../ws` | the script's WebSocket |
| `GET /terminal/script.js` | QS/400's script: `termlook::script::text` with its keypad |

Without script the picture is the server's and the keypad posts; the
page answers a key with the window again a moment later, showing the
system's answer.

With script the page's one `<script>` element is QS/400's, with
`data-socket` (the window's `ws`), `data-image` (the picture's id,
`term`), `data-scale` and, for black on white, `data-scheme="bw"`. The
socket speaks `termlook::exchange`: `screen` (the whole screen first),
`delta`, `closed` with why, and `message` out; keys by name and typed
text in, turned into `qsipart` keys by `Input::console_keys`.

## Script and policy

The terminal window is the only page with a script of its own (every
other framed page loads only `qsweb`'s navigation script, which the
window does not), under a policy of its own:

```
default-src 'none'; script-src 'self'; connect-src 'self'; style-src 'self';
style-src-attr 'unsafe-inline'; img-src 'self'; form-action 'self';
frame-ancestors 'none'; base-uri 'none'
```

The script is served from the console; the socket is the console's,
and is opened only from the console's own page: its `Origin` must be
`https://` and the `Host` the request names, or it is refused with 403
and audited (Phase 8, `sec-web`).
`style-src-attr 'unsafe-inline'` is for the style attributes the window
is drawn with, as the prototype draws it; no other page's policy
changes (`webd::serve::secured` keeps a page's own policy and gives
every other answer the server's). A window that is not connected
carries no script, and its policy is the same without `script-src` and
`connect-src`.

## Roles

| Who | May |
| --- | --- |
| a role with View | see Open terminal window, the window, the picture, Print Screen and the control panel |
| a role with Open terminal window | type (the keypad, the socket) and use control panel functions 01 to 49 |
| a role with Control panel functions 50-70 | functions 50 to 70 |

By the default matrix a Viewer sees the window with its keypad
disabled; its posted key is Not authorized and its socket's keys are
dropped by `mgmtd`.

Functions 08 and 22 go only through their confirmation page
(`/qsmc/confirm/panel`) and its POST with the form's token.

## The web session

A window ends with the web session it was opened in (signing off, the
idle timeout, the session's end) unless Language, locale and session's
Keep terminal window sessions open is on (`web.toml`, `keep_terminals`).
A socket also ends after the session's idle time without a key while
the option is off.

## The picture

The prototype's `shared/img/5250-24x80.png` is Access for Web's Work
with Active Jobs; QS/400's handoff 23 draws it byte for byte from that
5250 screen. The simulator's recorded screen
(`tools/sim/screens/wrkactjob.screen`) is the same screen as console
cells: each cell's character, colour and underline were read from the
prototype's PNG once, against the console font. Drawn through
`termlook::screen::grid` with the status row naming `QSHQ400`, it is the
prototype's picture byte for byte: `sim`'s
`the_recorded_screen_draws_the_prototype_picture` and the
`mgmt-terminal` suite, through a running console, both compare the
bytes.

## A standalone system

From Phase 5 a standalone QS/400 answers QSI itself, and its console is
at ID 0 (`/qsmc/terminal/<name>/0`): Open terminal window lists it as
`<name> (standalone)`, its window and control panel are a partition's,
the system's own code on the strip and its keylock from its state. Its
local display shows the same screen and takes keys too. A system whose
hello does not offer the console family is refused with `QMC1114`.

## Not in this release

Copy, Paste, Keypad and Keyboard Map of the toolbar (the
script has the keyboard and the keypad); a 27 x 132 screen chosen from
the console (the size is the system's).
