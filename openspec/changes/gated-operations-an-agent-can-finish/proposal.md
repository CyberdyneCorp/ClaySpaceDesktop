# Gated operations an agent can finish

## Why

Every document operation behind a consent gate was unreachable for an agent,
and the modals involved blocked the application while they were open (#192):

- `exchange.run_export` and `exchange.run_import` took no path, and `SaveAs`
  was not offered, so after consent a native file panel opened on the person's
  screen and the call could not be completed by the caller that made it.
- The consent wait was 20 s against a 10 s call bound. A person agreeing late in
  the wait agreed to a call its client had already given up on.
- `layer.select` with a dragged cage standing opened a native three-way prompt;
  `document.open` and `document.quit` over unsaved work opened the native
  discard prompt. Each held the interface thread, which also serves the door.
- The crash-recovery offer was a native alert raised before the first frame, so
  an agent connecting to a session that had recovered work timed out on every
  call until somebody dismissed it. The startup banner said
  "agent: not built with a door" and printed the door's address on the next
  line.

As a result save, open, new, import and export were never exercised by the
audit, and persistence was never verified end to end.

## What changes

- `document.save_as {path}`, `exchange.run_import {path}` and
  `exchange.run_export {path}` take the path the panel would have asked for and
  open no panel. They are gated as before (`sobrescrever`, `abrir`,
  `exportar`), and the person is shown the path in the ask. The panel commands
  (`SaveAs`, `RunImport`, `RunExport`) stay on the pointer path and are listed
  as not offered.
- `document.save` of a never-saved document, `document.open` and
  `document.quit` over unsaved work, and a plain `layer.select` away from a
  dragged cage are refused with the call that says the answer up front instead
  of opening a dialog. `layer.select` takes `cage: "apply" | "discard"`.
- The consent wait is held inside the call bound (8 s against 10 s). The ask
  stays up when the wait ends, and the timed-out refusal says so; the retry
  picks up an answer given in between.
- The recovery offer is a window in the shell, answered with
  `AnswerRecovery`; no autosave is written over the offered file while it
  stands, and an unanswered offer survives a clean quit.
- The startup banner is printed after the door opens and states it once.
- Import and export failures are stated on the refusal channel, so the door
  answers them as errors.

## Impact

`exchange.run_export` and `exchange.run_import` now require `path`; a call
without one is refused as a bad argument. New commands: `SaveTo`, `ImportFrom`,
`ExportTo`, `SelectLayerSettlingCage`, `AnswerRecovery`. No file format change.
