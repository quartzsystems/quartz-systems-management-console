# Console reference codes

A QS/MC console posts reference codes the way a QS/400 system and a
QS/VS host post theirs: eight characters, a two-character family and
six digits. The table is `lic/refcode/src/lib.rs`; nothing else spells
a code, and a unit test (`refcode::tests::the_document_lists_the_table`)
holds this document to it.

A code is posted as one line, `REF <code> <text>`, sometimes with what
this posting adds after a colon:

```
REF C3001000 Console IPL started
REF A3001015 Processor lacks what the cryptography needs: AVX2, ADX
```

## The console's range

An operator reads a console's lines beside a QS/400 system's and a
QS/VS host's, so a console's code must never read as either's.

- QS/400's families (`lic/slic/src/src.rs` in its repository, at the
  pinned commit) are `C1` (LIC initialization), `C6` (OS
  initialization), `A6` (attention), `B1` (fatal) and `D1` (power
  down).
- QS/VS's (`lic/refcode/src/lib.rs` and `docs/REFCODES.md` in its
  repository, at the pinned commit) are `C2`, `A2`, `B2` and `D2`.

No family of either ends in `3`. The console takes the families whose
second character is `3`, and the first character keeps the meaning it
has in both:

| Family | Meaning |
|---|---|
| `C3` | console IPL and progress |
| `A3` | attention; the console continues |
| `B3` | fatal; the console parks |
| `D3` | power down |

A unit test (`refcode::tests::no_code_is_in_a_qs400_or_qsvs_family`)
fails if a code lands in a family of either. The word before the code
is `REF`, as a QS/VS host's is, not QS/400's `SRC`.

**NEEDS-CODY:** the `3` families are a proposal of Phase 0. Strings of
other families appear in QS/400's tree only as QSI samples of a host's
codes (`A2`, `C2`, `B2`, `D2` in `lic/qsi/src/samples.rs`) and a test
fixture of its guest agent (`C9002A00`); neither is a family QS/400
posts. `docs/OPEN_QUESTIONS.md` item 1.

## Posting

In Phase 0, PID 1 posts on the display and the serial port. The
history, a record for the log service, comes with that service in
Phase 1. The heartbeat is on the serial port only and will never be in
the history.

## The blocks

| Block | Whose |
|---|---|
| `C3001xxx`, `A3001xxx`, `B3001xxx`, `D3001xxx` | PID 1: the IPL in order, supervision, the slot, shutdown. An attention code has the number of the IPL step it belongs to |

Later phases take further blocks and record them here.

## The codes

| Code | Text | When |
|---|---|---|
| `C3001000` | Console IPL started | the first thing PID 1 posts, once the console is open |
| `C3001005` | Console running | the heartbeat, every 5 s, on the serial port only |
| `C3001010` | Kernel file systems mounted | proc, sysfs, devtmpfs, the tmpfs on `/run` and cgroup2 are mounted |
| `A3001010` | Kernel file systems not mounted | a kernel file system did not mount, with which and why; the IPL goes on |
| `A3001015` | Processor lacks what the cryptography needs | the processor lacks AVX2, ADX, AES or PCLMULQDQ, with which: the web server and QSI cannot run on it; the console still IPLs |
| `B3001000` | Console PID 1 failed | by PID 1's panic handler, with the panic message, before it parks |
