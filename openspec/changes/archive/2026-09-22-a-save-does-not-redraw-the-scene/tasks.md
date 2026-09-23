## 1. A save that writes a file and nothing else

- [x] 1.1 Lend the engine the sculptor's visibility for the length of the write and take it straight back, marking no layer for refill and draining nothing — through the borrowing bracket a bake already uses, rather than a second one of the save's own.
- [x] 1.2 File the borrowed flags as visibility gestures undo hops in one step, so a ⌘Z after a save reaches the sculptor's edit.
- [x] 1.3 Report a flag that will not come back: the engine would then be showing the file's pattern over a cache filled for the sculptor's, and a save that returned quietly would hide it.

## 2. An autosave clock that measures idle time

- [x] 2.1 Stamp the clock after the write rather than before it, whether the write succeeded or failed.
- [x] 2.2 Skip the tick while a gesture is open, without restarting the clock, and schedule no wake-up for it meanwhile.
- [x] 2.3 Put the ordering somewhere a test can reach, as the settle deferral already is: `App` lives in the binary.

## 3. Hold it

- [x] 3.1 `crates/clayspace-engine/tests/solo.rs`: a save while soloed refills nothing, leaves the scene and the solo where they were, adds no undo step, and still writes the sculptor's own pattern into the file.
- [x] 3.2 `crates/clayspace-app/src/main.rs`: a save longer than the interval is not due again when it finishes; a failed save waits out the interval; an open gesture skips the tick without restarting the clock.
- [x] 3.3 `crates/clayspace-model/src/session.rs`: the rule itself — an overdue autosave under an open gesture is not due.
