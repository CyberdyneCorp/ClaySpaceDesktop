# Tasks

## 1. Move the pin

- [x] 1.1 Point `vendor/ClayCore` at v0.120.0 and move `EXPECTED_ABI` to 0.120
      by hand, which `version_is_the_pinned_engine` holds against the linked
      engine. It is deliberately not derived from that engine, which would make
      the check assert that a number equals itself
- [x] 1.2 Confirm the retopology pin is unaffected: `vendor/CyberRemesherAndUV`
      stays at v0.9.0, and `tools/check_engine_pin.py` answers for both
- [x] 1.3 Confirm the container minor does not move, so `Document::FORMAT`
      stays where it is and a document this build writes is readable by a build
      on the previous pin
- [x] 1.4 Build the whole workspace against the new engine before touching a
      line of it. Thirteen symbols added, zero removed, no struct re-laid out

## 2. Measure what moves under us, rather than scaling it

- [x] 2.1 Establish that the mesh grab change does not reach this application:
      both `stroke_mesh` and the hierarchy's stroke make one stamp at the
      anchor rather than driving a stroke consumer, and the release is explicit
      that `stamp` is unaffected unless a carried region is opened. Held by
      `mesh_move.rs`, whose drag still arrives within 0.05 of
      `travel * intensity`
- [x] 2.2 Measure the voxel drag either side of the pin on one fixture rather
      than dividing by a factor. Brush 0.4 on a 0.05 grid: the rim rises 5
      cells against the centre's 6 where it rose 1 against 4, and the drag
      reaches the whole ask up to the ball's radius
- [x] 2.3 Run the three tripwires the survey expected to fire —
      `multires_document.rs`, `mesh_automask.rs`, `visual_voxel_sculpt.rs` —
      and record that none of them did, with the limit each still pins

## 3. Decide what the voxel drag asks for

- [x] 3.1 Ask for the taper by name on an unmasked grid drag.
      `dragging_footprint` keeps `solid_footprint`'s solid coverage and its
      bite-scaled radius, and keeps the curve, because a drag is the one grid
      verb whose falloff shapes the pull rather than the coverage
- [x] 3.2 Leave the sculptor's `Borda` honest on a masked grid layer, where the
      engine's own weighting is kept so the mask goes on gating, and say in
      `docs/features.md` that the name now means the curve it names
- [x] 3.3 Keep `voxel_grab_taper.rs`'s margins as they are rather than loosening
      them, and record in the file that it was armed, that it fired, and what
      answered it

## 4. Hold the decision with a test

- [x] 4.1 `a_drag_as_long_as_its_brush_arrives_short_of_the_ask`: a second
      reading of the taper at the centre instead of the rim, where the two
      pulls are furthest apart and neither sits on a bound — 4 cells of the 8
      asked with the taper, 8 of 8 without it
- [x] 4.2 Anchor `voxel_brushes.rs`'s drag where a press lands. Buried half a
      ball inside the slab it measured nothing, and read as a working drag only
      because the old cast's support reached a shade wider

## 5. Write it down

- [x] 5.1 `README.md`: the pin, the symbol diff, and the two host-visible
      changes it carries — which of them reaches this application and why the
      other does not
- [x] 5.2 `docs/features.md`: the grid drag's reach table, re-measured, and
      what `Borda` now means to a drag on a masked layer
- [x] 5.3 This change, and `openspec validate --all --strict`
