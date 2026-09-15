# Tasks

## 1. Move the pin

- [x] 1.1 Point the submodule at v0.113.0 and move `EXPECTED_ABI` by hand, which
      `version_is_the_pinned_engine` holds against the linked engine. It is
      deliberately not derived from that engine, which would make the check
      assert that a number equals itself
- [x] 1.2 Build the whole workspace against the new engine *before* changing a
      line of it, so what the upgrade forces is separable from what it enables.
      It forces nothing: nothing removed from the C ABI across 29 minors, no
      signature changed, every struct that grew did so behind `struct_size`
- [x] 1.3 Move `Document::FORMAT` to container minor 19, and record beside it
      that the minor moved in two steps *inside* v0.97.0 — 18 at ABI 0.87.0 and
      19 at 0.88.0 — so "no format change" being true of the v0.113.0 span and
      false of this jump is met as reasoning rather than reconstructed
- [x] 1.4 Add `CLAY_ERROR_SNAPSHOT_MISMATCH` to the error vocabulary as a named
      kind rather than `Unknown(10)`, with what separates it from the other
      replay refusals: it is the one that leaves the document byte-identical,
      because the snapshot's identity is in the journal header

## 2. Take the improvements our own tests demanded

Two assertions in this workspace were written to **fail** the day an upstream
defect was fixed. Both fired on the pin and both name the workaround to delete.

- [x] 2.1 `clay_document_mesh_layer_revision` now moves when history replaces a
      layer's triangles. Verify across the full cycle before trusting it —
      attach 1, rebuild 2, undo 3, redo 4, undo 5, monotone in both directions —
      rather than across the single transition the test exercised
- [x] 2.2 Delete the `Rebuild` record it existed for: the struct, the field,
      both push sites, and the depth-crossing half of
      `settle_geometry_revisions`. The revision is now the whole signal
- [x] 2.3 Level 1 answers `CLAY_NORMAL_GRADIENT` (#550). Flip the assertion and
      let the coarse path ask for gradients, so a coarse surface is no longer
      face-shaded by construction
- [x] 2.4 Pass the document wherever gradients are asked for, not at level 0
      only — the engine refuses gradients without one, which is what the first
      run after flipping 2.3 reported. Keep the live-gesture exclusion, which is
      a different rule: the preview's lattice is not the document's field
- [x] 2.5 Flip both assertions rather than deleting them, so each now guards the
      capability the code has started relying on

## 3. Stop meshing the field twice per stroke

- [x] 3.1 Make `settle` rebuild from bricks instead of calling
      `clay_document_mesh`, now that #549 has removed the slivers the detour hid
- [x] 3.2 Remove `clean_override` and the second rebuild it scheduled. A
      whole-document mesh cannot be patched incrementally, so it made the *next*
      sync throw the mesh away and rebuild every brick anyway
- [x] 3.3 Take the surface epoch in `settle`, so the next sync does not run a
      whole rebuild to reach the state it is already in
- [x] 3.4 Drop the empty-field special case, which existed because
      `clay_document_mesh` refuses an empty document rather than returning an
      empty mesh. `rebuild_at` clears first and meshes per key, so that case is
      unreachable rather than handled
- [x] 3.5 Rename `SettleRoute::Document` to `SettleRoute::Bricks` and update
      what `settle_attribution` asserts

## 4. Ask the engine what it knows

- [x] 4.1 Bind `clay_layer_move_surface_regions` and use the reported boxes
      instead of reconstructing one from brush size and distance travelled.
      Size the buffer from the layer's symmetry and re-ask on refusal, which is
      safe because a short buffer is refused before the first edit is recorded
- [x] 4.2 Bind `clay_document_layer_mirror` and replace forget-and-guess-safe
      with ask-and-compare in `point_the_mirror` and `mirror_for_dirtying`
- [x] 4.3 Correct both issues in the open where the measurement contradicts what
      they claimed: the slab case never existed here because `mirrors()` already
      returns one entry per image, and the mirror readback removes no
      over-invalidation that can be reached from a plain stroke

## 5. Say so when an export is not manifold

- [x] 5.1 Validate the mesh between `mesh_combined` and `save`, and carry the
      verdict out of `export_mesh` rather than discarding it
- [x] 5.2 Use `clay_mesh_validation_report`, not `clay_mesh_validate`: the
      two-bit call drops the nine other quantities the same pass computed, and
      the counts are what separate a shippable file from a ruined one
- [x] 5.3 Keep the predicted and the observed halves apart —
      `ExportWarning::for_export` speaks from the format and the settings before
      the write, `for_written_mesh` from the bytes after it — and assert that no
      message is produced by both
- [x] 5.4 Hold the finding on the application and leave the export panel open
      when one is raised, since closing it is how the application says the write
      went fine

## 6. Show a field's Move while it is dragged

- [x] 6.1 Ask whether the gesture replays *before* asking the representation in
      `stamps_between_segments`. The threshold is right for a stamping segment,
      which costs a re-mesh of what it touched, and wrong for a replayed one,
      which lays the whole drag down from its anchor every time
- [x] 6.2 Pin both counts as measured rather than as inequalities: on a
      0.08-unit drag, `Mover` goes 0 → 8 segments before pointer-up and `Padrão`
      stays at 1 — the press's own dab, which is not a segment

## 7. Check the tests, not only the code

- [x] 7.1 Revert each change under its own new test and confirm the test fails.
      This caught three: one decorative (pinned the ABI rather than our adoption
      of it), one actively wrong (a ratio bound that passed on the unfixed
      code), and one long-standing — `visual_subtools` asserted a settled
      surface matched `clay_document_mesh` within 0.01 and passed only because
      `settle` *called* `clay_document_mesh`, comparing an output against itself
- [x] 7.2 Fix that last one by measuring the baseline rather than widening the
      bound: two meshers of one field render 0.0157 apart on an undisturbed
      sphere and 0.0333 on the two-sphere fixture, with zero dark specks in any
      of them
- [ ] 7.3 Resolve the four Linux CI failures, which do not reproduce on macOS.
      The likeliest candidate is the export test asserting that ratio 0.5 is
      non-manifold — a coordinate that ClayCore have shown is chaotic under tiny
      input perturbations, and therefore not a property a platform-independent
      test may assert

## 8. Documentation

- [x] 8.1 Move the engine version, the container minor and the export section
      forward in `README.md` and `docs/features.md`
- [x] 8.2 Correct the passage in `docs/features.md` describing the whole-field
      path as current, which 3.1 removed
- [x] 8.3 Correct the README's "618 PNGs" line, which described the visual
      captures as if they were golden images. They are written to `target/visual/`
      for looking at and nothing compares them; that line is where the belief
      came from that a pin move owed a golden refresh
- [x] 8.4 Give `docs/architecture.md` the two paths it never described — how a
      gesture reaches the model and when, including the replay-versus-stamping
      distinction whose ordering was the Move defect, and what settling does now
      that it rebuilds from bricks
- [ ] 8.5 Record in `docs/roadmap.md` that coarse-during-drag is unblocked
      rather than refused, and why it is not therefore scheduled

## 9. Name every Move gesture (#122)

- [x] 9.1 Give `claycore::MoveParams` the `gesture_id` field and pass it through
      `to_raw`, with a wrapper test in each direction: a named drag whose radius
      changes stays one warp, two differently named drags stay two
- [x] 9.2 Issue a fresh id in `begin_gesture`, clear it in `end_gesture`, and send
      it from both Move doors through one `move_params`
- [x] 9.3 Hold the held door to it in `move_gesture_identity.rs`: a second drag
      from the same press adds to the first rather than replacing it — red
      before 9.2, with the surface at 1.1460 after one drag and after two
- [x] 9.7 Restate `the_unpreviewed_drag_coalesces_to_one_grab_per_image`: its
      re-anchored arms asserted one grab per segment inside `begin_gesture`,
      which naming ends. They now run unnamed, and named arms assert one grab
      per image however the segments are anchored
- [x] 9.8 Guard that naming does not cost a mirror its far side: a named
      mirrored held drag leaves both sides where the unnamed one does
- [x] 9.5 Keep a tripwire on the live door, which cannot carry the name on this
      pin because `clay_sdf_move_begin` drops `gesture_id`
- [ ] 9.6 Report the dropped `gesture_id` to ClayCore, and turn the tripwire into
      the held door's assertion on the pin that fixes it
- [x] 9.4 Correct the proposal's reason for leaving the id unset, and describe
      the naming in `docs/features.md`
