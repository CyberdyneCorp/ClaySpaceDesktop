# Tasks

## 1. Bind the cut, where the unsafe is allowed to live

- [x] 1.1 `claycore::Cut` wrapping `clay_cut_create`, owning the returned
      `clay_item*` and freeing it with `clay_item_destroy`, so a refused
      placement cannot leak one; verify a test that creates and drops a cut
      without placing it
- [x] 1.2 `clay_cut_polygon_from_open_curve` and `clay_cut_polygon_from_curve`
      wrapped with the **size query held inside** — call with a null out
      pointer for the count, allocate, call again — so nothing above the bridge
      can size a buffer from a stale count; verify both against a known outline
- [x] 1.3 A frame that is not orthonormal is reported rather than corrected;
      verify `a_frame_that_is_not_orthonormal_is_refused`, which must fail if
      the wrapper normalises before the call
- [x] 1.4 `clay_cut_desc` is built with `struct_size` set and both extents at
      zero, so the engine derives the sweep from the region; verify a cut on a
      large subtool passes entirely through it

## 2. The crossing: what was drawn to what is cut

- [x] 2.1 A `CutFrame` in `clayspace-model` — origin, right, up, forward — and
      the arithmetic that turns viewport points into world units on it. In the
      domain crate, with no camera type: it takes a basis and a scale, so it is
      testable without a viewport
- [x] 2.2 The frame is the camera's basis with its origin outside the region
      along `forward`, **not** through the picked point; verify the same
      gesture from one camera position twice produces the same cut whatever lay
      under the pointer
- [ ] 2.3 The world-per-pixel scale is taken at the region's centre depth, and
      the consequence is written where it is chosen: under perspective a shape
      over a deep form cuts slightly wider at the front than the outline
      showed. Verify the error is zero in an orthographic view. **Not done**:
      the frame comes from `outline_frame`, which the mask already uses and
      which anchors on the subtool — the scale is therefore the mask's and
      inherits whatever that does. Measuring the perspective error is a
      separate piece of work and it is not measured yet, so the claim is not
      made
- [x] 2.4 A degenerate gesture — fewer than three points, or a shape of no area
      — is refused before it reaches the bridge, by **one** gate rather than
      two. `apply_cut` asked `is_drawn` and then `place_drawn_cut` refused each
      gesture again on its own terms, so those three refusals could never fire
      and the refusal test passed whichever of the pair was doing the work —
      a guard that is never exercised reads at review as protection and is not.
      The gesture's own arithmetic decides now, at the point where what it
      decides is used, and `each_gesture_is_refused_in_its_own_terms` holds
      that all three arms are reachable. Proven by putting the outer gate back:
      all three then report "o gesto é pequeno demais para cortar" and the test
      goes red

## 3. Route the tool through the gesture it already has

- [x] 3.1 `ToolKind::Trim` takes the drawn-outline path rather than the stroke
      path; `is_stroke_tool` already says so and nothing yet reads it for this
- [x] 3.2 An open-stroke gesture beside `MaskGesture::Lasso` and
      `MaskGesture::Rectangle`, and each of the three maps to exactly one entry
      point — open curve, closed curve, and the engine's own rectangle
- [x] 3.3 The rectangle uses `CLAY_CUT_RECT` rather than a four-point polygon,
      since the engine can express it exactly
- [x] 3.4 `operation()` stops returning `None` for Trim, and the brush ring
      stays off — `shows_the_brush_ring` already refuses a drawn gesture and
      Trim is one

## 4. Which half

- [x] 4.1 The side an open stroke covers is inferred from the direction it was
      drawn; verify both directions over one form take opposite halves
- [ ] 4.2 The inversion modifier flips the **operation** and not the side;
      verify a held modifier keeps the covered half and swaps what survives.
      **Not done**: the direction rule covers the same ground for a line and
      the winding does for a lasso, so the modifier is not yet needed to reach
      either half. It is worth adding only if a sculptor asks for the half a
      redraw would give them without the redraw
- [x] 4.3 A closed lasso takes no side; verify `clay_trim_side` does not enter
      the closed path

## 5. Hold it

- [x] 5.1 A visual test: a form, a shape drawn across it, and the surface
      afterwards. Read the **rendered** result rather than a pick — a pick is
      answered from a path that stays correct whether or not the cache was
      refilled, which is what made three guards useless on the tube.
      `clayspace-app/tests/cut_cache_freshness.rs` renders the cut made on a
      meshed form against the same cut made on a second document *before* its
      first mesh, which is ground truth because nothing narrow runs on that
      path. Proven by breaking it: without the cut's `refill_bound`, 2594
      pixels differ against none, and the **triangle counts in that run were
      identical** — 281,362 either way — so a count is exactly the measurement
      that could not have seen it
- [x] 5.2 A cut is one undo entry and leaves an item, not a bake; verify the
      history depth moves by one and the item is in the layer afterwards —
      `a_cut_is_one_undo_entry_and_gives_the_material_back` asserts the depth
      moves by exactly one, that one undo puts the removed material back, and
      that a redo takes it again. The depth is what the undo bracket is for:
      placing the item and pointing it are two engine edits and a sculptor
      asked for one thing, so without the bracket the depth moves by two and
      the first undo leaves an item standing that nobody placed
- [x] 5.3 Trim greys out on a mesh, a grid and a hierarchy, and the reason
      names the representation; verify against the tool table rather than a
      hard-coded list —
      `trim_is_offered_on_a_field_and_greyed_out_everywhere_else` asks
      `for_representation`, which is what the shelf itself calls, and
      `asking_for_a_cut_on_anything_else_names_the_representation` asks
      `availability` for the refusal. Both walk `Representation::ALL`, so a
      fifth representation is covered by default
- [ ] 5.4 `just check` — formatting, clippy, the workspace suite, the
      specification
- [x] 5.5 `docs/features.md` gains the tool: the three gestures, why a line and
      a lasso are two entry points, and which half survives — with the
      rectangle's own row in the table, since it is the one shape with no
      travel to read. Two rules the defects taught are written where the
      behaviour is described rather than left in a commit message: a cut is not
      reflected by the layer's mirror, and a line and a rectangle keep two
      points rather than every sample the pointer sent. `README.md` gains the
      section and its contents entry

## 6. Say what is not in it

- [x] 6.1 The benchmark baseline's `brush.sdf.trim` skip — "no gesture this
      harness can synthesise" — is either measured or left with its reason
      updated. **Measured**, and the skip stays exactly as it is: read again,
      it is emitted from the *brush* group's `!tool.is_stroke_tool()`, where it
      is true and stays true — that harness synthesises strokes and a cut is
      not one. The gap was not a wrong reason but a missing figure, and it sat
      where `mask.outline` sits: `groups/cut.rs` measures `cut.line` and
      `cut.lasso` on the reference form, for the same reason the mask's outline
      is measured away from the brushes — the cost follows the form's size
      rather than the brush's. Two figures rather than the mask's one, because
      the open and closed curves are different engine entry points. Each sample
      is undone off the clock, since a cut is destructive and repeating one
      would measure a smaller form every time. First run, on a **busy** box and
      so indicative only: line 106 ms mean / 112 p95, lasso 108 / 117. No
      baseline value is recorded here — a figure absent from the baseline
      prints as `new` and does not gate, and recording one is a job for a quiet
      machine
- [x] 6.2 Trimming a mesh by crossing into a field and back is **stated as out
      of scope** in the proposal rather than left as a gap someone fills
      silently: it is a representation change with its own cost and its own
      undo entry, and a tool that says it removes material must not perform one

## 7. What a session found that the fixtures did not

Reported from use: "after some subsequent Line cuts it stops working, it
doesn't cut anymore". Three defects, each with a guard proven by reverting the
fix and watching the guard fail.

- [x] 7.1 The polygon closing an open curve reached `span * 4.0 + 1.0` either
      side of the line, so a form 2.0 across placed an item 18 across. Sized
      from the region's own corners projected onto the frame instead, which is
      exact for any camera angle where a multiple of the longest axis is wrong
      by the shape of the box and by how the frame is turned against it. The
      margin came down from `BRICK_MARGIN` — sixteen voxels — to four, because
      the polygon's extent is the item's bound and that is what a placed cut
      asks the brick cache to refill; verify
      `successive_cuts_keep_cutting`, which reports `spans 19.920 against
      2.000` when the multiple is put back
- [x] 7.2 A cut is an item, so the layer's bounds grow to hold it, and the next
      cut read those bounds as the region it had to clear — a feedback loop
      that ran away at eightfold a cut: 2.0, 18, 146, 1170, 9362. Framed
      against the **surface's** extent instead, which a subtract cannot
      enlarge. Verify `a_turned_frame_does_not_grow_what_the_next_cut_must_clear`
- [x] 7.3 **And that guard exists because the first one could not see it.**
      `successive_cuts_keep_cutting` walks a frame squared up with the world,
      where a box's projection is its own width, so it stayed green with 7.2
      reverted — reverting a real fix confirmed what the test could see and
      nearly had it deleted as unnecessary. At 45 degrees the projection is the
      diagonal: 2.000, then 4.83, 7.66, 10.49 at a steady 2.83 a cut, refused
      from the tenth. The guard asserts **stability** rather than a threshold,
      since the size it settles at is a property of the margin and the fixture
      while settling at all is the property under test
- [x] 7.4 The shape was positioned relative to the region's centre rather than
      the frame's own origin. `OutlineFrame::at` defines a drawn point as
      `origin + right*x + up*y`, and the viewport builds that origin by casting
      the view-centre ray onto a plane through the subtool — so `(0, 0)` is the
      middle of the screen. The two agree exactly while the form is symmetric
      about what the camera is framed on, which is every fixture we had and the
      first cut of any session; verify `successive_cuts_keep_cutting`, whose
      third line cut reports its probe "already outside the form" when the
      anchor is put back
