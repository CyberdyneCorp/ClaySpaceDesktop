# Tasks

## 1. Move the pin

- [x] 1.1 Point the submodule at v0.52.2 and move `EXPECTED_ABI`, which the
      `version_is_the_pinned_engine` test holds to the submodule
- [x] 1.2 Run the whole suite against the new engine and triage every failure
      against the release notes rather than assuming staleness

## 2. Answer the undo change

- [x] 2.1 Measure the change rather than infer it: same document, same single
      undo, on both pins — v0.39.0 leaves 3,952 vertices, v0.52.2 leaves 0
- [x] 2.2 Take the crossing back whole, carrying the layer in the host while
      the engine carries the filling
- [x] 2.3 Establish why the layer is hidden rather than removed, by trying the
      removal and measuring what it does — a second undo resurrects the emptied
      layer and a redo builds a third one beside it
- [x] 2.4 Drop an undone crossing's layer at save, since a file has no redo
      stack to restore its content from
- [x] 2.5 Rewrite `a_crossing_is_taken_back_by_undo` to cover undo, undoing
      past it, and redo, and verify it by mutation rather than by watching it
      pass
- [x] 2.6 Replace the `representation-conversion` requirement that says a
      conversion is not undoable, keeping the record of why it used to be true

## 3. Record what moved

- [x] 3.1 Re-record the Linux baseline against v0.52.2, on a quiet machine (0.13 load per core, stamped in the file)
- [ ] 3.2 Re-record the macOS baseline, on a macOS machine
- [x] 3.3 Report the `brush.sdf.mover` regression upstream with the measured
      localisation (CyberdyneCorp/ClayCore#335)
- [x] 3.4 Report the half-undo across a crossing upstream (CyberdyneCorp/ClayCore#341)

## 4. Say what moved

- [x] 4.1 Carry the new pin and the re-recorded figures into the documentation
      that quotes them — the README stat block and its sample `diagnostics`
      output, and the roadmap's header and baseline paragraph. Moving a pin
      without this is how the README came to claim 0.39.0 and a 25.7 ms
      startup across three re-recordings
