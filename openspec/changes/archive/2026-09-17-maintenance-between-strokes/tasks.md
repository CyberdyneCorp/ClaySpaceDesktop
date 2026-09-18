# Tasks

## 1. A gate that outlives a Rust scope

- [x] 1.1 Add `MaintenanceQueue::into_stroke` and `StrokeScope` to `claycore`,
      moving the queue in rather than borrowing it, and shutting the stroke on
      `Drop` as well as on `end`
- [x] 1.2 Factor `begin_stroke`/`end_stroke` into private helpers so both the
      borrowing guard and the owning scope shut a stroke the same way
- [x] 1.3 Two tests: a drag's worth of requests folded behind a gesture that
      hands them out only once it ends, and a gesture held in a field across an
      unwind that is still gated afterwards and still drainable when it ends

## 2. One door for "a gesture is open"

- [x] 2.1 Route all five writes of `ClayDocument::previewing` through
      `set_previewing`, which is also where the gate and the pin move
- [x] 2.2 Hold the gate with a `StrokeScope` for the length of a gesture, and
      make opening one idempotent — a press arriving over an unfinished drag is
      a lost pointer release, not a second gesture
- [x] 2.3 Take the memory pin at the same moment and give it back at the same
      moment, balanced on the transition rather than on the call

## 3. The request, and the decision

- [x] 3.1 Ask for an index rebuild from every path that writes through a mesh
      sculptor: a stroke segment, a deformer operation, a cage bend
- [x] 3.2 Record what a tree scores when it is built, because that is the only
      figure a later reading of it means anything against
- [x] 3.3 Decide at the drain rather than at the request: read `quality` once
      per gesture instead of once per segment, and rebuild only where the tree
      has drifted half again past what it started at
- [x] 3.4 Time the rebuild and file the measured figure on every request after
      it, so the budget weighs a measurement rather than a guess

## 4. The budget loop

- [x] 4.1 Drain from `end_gesture`, `apply_lattice` and `cancel_lattice` — every
      way a gesture ends, including the cage's own lifecycle
- [x] 4.2 Take, do, complete; start an item only where what is left of the
      budget covers its estimate; stop rather than step over one that does not
      fit, because `take_next` hands out the head of the queue
- [x] 4.3 State the budget as a named constant against what it was chosen from:
      half the 16 ms the specification allows an engine operation to hold the
      interface thread
- [x] 4.4 Complete an item of a kind nothing here produces rather than leaving
      it, so it cannot block what is queued behind it

## 5. Measure it

- [x] 5.1 The gate: nothing drained while a gesture is open, everything after
      it, and a gesture reopened over an open one still ending drainable
- [x] 5.2 The budget: a queue holding more than the moment can afford leaves
      the rest where it was, and a moment with no room at all services nothing
      and loses nothing
- [x] 5.3 The pin: taken for a gesture and given back on the pointer coming up,
      on a cage applied and on a cage abandoned
- [x] 5.4 Check each of them fails when the mechanism it names is removed,
      rather than trusting that a passing test measured anything

## 6. Say what changed

- [x] 6.1 Correct `mesh_quality`'s documentation, and the test that repeated it
- [x] 6.2 `docs/features.md`: what happens between two strokes, why it is a
      request rather than a rebuild, and what the pin is for
