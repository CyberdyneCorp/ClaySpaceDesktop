# Tasks

## 1. Draw the line the tube follows

- [x] 1.1 `CurveState::path` tessellates per join — the control points for
      `Corners`, Catmull-Rom for `Through`, a uniform cubic B-spline for
      `Rounded` — with `path_edges` beside it; verify
      `a_curve_through_its_points_visits_every_one_of_them`,
      `a_rounded_guide_sits_inside_its_own_points` and
      `a_cornered_guide_is_exactly_its_control_points`
- [x] 1.2 The guide agrees with the engine's sweep, measured rather than
      assumed; verify `the_guide_lies_inside_the_tube_it_describes` over all
      three joins
- [x] 1.2a That test asserts **depth, not membership**, and the distinction was
      forced by checking: the first version asserted only that samples fell
      inside the tube and passed when fed the control polygon, which is the
      line this change exists to stop drawing. Re-checked against the polygon
      after strengthening, where it now fails at +0.135
- [x] 1.2b The strengthened test then failed on `Rounded` at one sample of
      thirty-seven — the last — because the first draft pushed the final
      *control point*, which Catmull-Rom reaches and a B-spline does not. The
      end of the last span is evaluated through the same basis as everything
      before it, and the special case is gone
- [x] 1.3 `LatticeView::guide` carries the polyline, drawn brighter than the
      control polygon; the curve passes no `edges`, because two lines through
      the same points that agree only where the curve is straight is worse than
      one

## 2. Make it legible

- [x] 2.1 The surface is ghosted while a curve is up, as it is for a cage;
      verify `the_guide_is_visible_through_the_tube_it_runs_inside`
- [x] 2.2 That test measures **contrast and not pixel count**, and this was
      also forced by checking: with the surface left opaque the guide still
      moves 232 pixels, because the scaffold shader fades the line rather than
      hiding it. Measured 55.0 ghosted against 17.8 opaque, and the assertion
      sits between them
- [x] 2.3 The captures either side of the assertion differ **only** by the
      guide — same ghosting, same handles. The first version varied the
      ghosting too, and ghosting alone repaints the whole tube: 13,151 pixels
      of difference before a line is drawn, which passed and would have passed
      with no guide at all

## 3. Let a curve be refined where it bends

- [x] 3.1 `CurveModel::insert_curve_point` puts a point into the list and
      selects it; verify `a_point_inserted_into_a_curve_splits_the_span_it_names`,
      which also re-checks the guide against the field afterwards
- [x] 3.2 `CurveState::insertion_for_sample` maps a place on the tessellated
      guide to the control-point index that splits its span, clamped so a
      click on the last sample refines rather than extends; verify
      `a_place_on_the_guide_names_the_span_it_splits` and
      `a_cornered_guide_names_its_own_points`
- [x] 3.3 `Command::InsertCurvePoint`, separate from `AddCurvePoint` because
      appending and splitting are different acts from different gestures
- [x] 3.4 `App::is_double_press` holds the rule, ours rather than the window
      system's so it can be tested without a window; verify the four cases in
      `double_press`
- [x] 3.5 The double-click is tested against the **guide** and before the
      control points, since the second press of a double lands within a
      handle's reach of the point the first one selected

## 4. Two the first pass missed, reported from a session

- [x] 4.1 A press on the guide is **consumed**, not appended. It fell through
      to the append, which put a point at the far end of the curve and selected
      it — so the line jumped somewhere nobody clicked, and the double-click
      below could never fire because the first press had already added a stray
      point. This is the defect the first pass shipped
- [x] 4.2 The press order is decided in `App::curve_press_action`, returning a
      `CurvePress` rather than acting inside the ray arithmetic — the *order*
      of the questions was the whole bug, and an order worth getting right is
      worth testing without a window, a GPU or a camera; verify the five cases
      in `curve_press`
- [x] 4.3 A control point answers before the guide, so a double-click on a
      point takes the point instead of inserting a second one coincident with
      it. The first pass had this the other way round and it was recorded as a
      deliberate trade; it was a defect
- [x] 4.4 A drag draws the curve freehand, laying a point each tube-width, and
      a click still lays one — told apart by distance travelled rather than by
      a mode, so one path serves both; verify the four cases in `curve_stroke`
- [x] 4.5 A freehand stroke stays on the plane its first point chose rather
      than re-picking the surface per point: by the second point the tube the
      stroke is drawing is under the pointer, so a surface pick lands on the
      stroke's own output and the curve climbs it

## 5. Make it feel like drawing

- [x] 5.1 An appended control point dirties the end it added and each image the
      layer mirror places it at, instead of the swept node's whole bound —
      2.9 ms at the thirtieth point of a stroke against 31.1, and 80 bricks
      against 880; verify
      `appending_a_point_dirties_the_end_and_not_the_whole_tube`
- [x] 5.2 Anything that is not an append falls back to the node's own bound,
      since a moved point, a radius, a join or a profile can move the whole
      tube
- [x] 5.3 The narrow region is held by a **staleness** test and not only by a
      cost one: measure the surface, refill the whole layer, measure again, and
      require no change; verify `a_curve_laid_point_by_point_is_not_left_stale`,
      confirmed to fail at 1.098 against a deliberately narrowed region
- [x] 5.4 What remains is the engine's and is not worked around: each brick's
      evaluation walks every segment of the curve, so the two costs compound —
      reported upstream, confirmed in `ctape_stroke_dist`, and not scheduled
- [x] 5.5 No brush ring while a curve is up. `shows_the_brush_ring` gains a
      fourth clause rather than the caller gaining a condition, because it is
      the rule and the other three live there; verify its test

## 6. Dragging a point costs what it disturbed

- [x] 6.1 A drag names the neighbourhood it disturbed — where the points were
      and where they now are — instead of the swept node's whole bound:
      **168 bricks against 1452, 5.1 ms against 13.4**; verify
      `a_dragged_control_point_leaves_no_stale_bricks`
- [x] 6.2 One box **per point** rather than one around the range, since a bent
      tube's enclosing box is mostly air: 168 bricks against the 256 a single
      box dirties
- [x] 6.3 `REACH` is two, not three: point `i` appears in the four-point window
      of the spans from `i-2` to `i+1`, so those windows reach `i-2` to `i+2`
- [x] 6.4 The guard reads the **rendered surface**, because the engine-side
      tests compare picks and a pick stays correct whether or not the brick
      cache was refilled. Proved by making it fail: a 0.15x margin leaves
      **2363 stale pixels**, naming the place the point came from
- [x] 6.5 The two margins are measured apart rather than made to agree — the
      drag takes 2.0 because its cliff is at 1.2 and widening costs nothing
      (the same 168 bricks), the append keeps 1.5 because its cliff is at 0.7
      and widening costs 164 bricks against 67 on the path a freehand stroke
      runs every tube-width
- [x] 6.6 Three wrong readings on the way to this, each corrected by
      measurement and each recorded where it happened: a margin sweep whose
      `sed` edited the drag path while the test exercised the append path; a
      "marks nothing" control that passed an empty vector and fell through
      `!is_empty()` into the node-wide arm, so it marked **everything** and
      returned exactly what a correct implementation would; and an upstream
      question answered accurately about the *tape* cache when it had been
      asked about the *brick* cache

## 7. Hold it

- [x] 7.1 `just check` — formatting, clippy, the workspace suite, the
      specification. Re-run after **every** edit to this file, which is the
      lesson of the numbering above: section 4 was inserted, the old section 4
      became 5, its tasks kept their 4.x numbers, and the change was reported
      as validating on a run taken before the edit
- [x] 7.2 `docs/features.md` — the *Pulling a tendril* neighbourhood gains what
      the guide is, why it is drawn tessellated, and how a drag draws one
