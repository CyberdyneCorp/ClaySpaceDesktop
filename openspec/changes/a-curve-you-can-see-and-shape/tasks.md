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

## 5. Hold it

- [x] 5.1 `just check` — formatting, clippy, the workspace suite, the
      specification. Re-run after **every** edit to this file, which is the
      lesson of the numbering above: section 4 was inserted, the old section 4
      became 5, its tasks kept their 4.x numbers, and the change was reported
      as validating on a run taken before the edit
- [x] 5.2 `docs/features.md` — the *Pulling a tendril* neighbourhood gains what
      the guide is, why it is drawn tessellated, and how a drag draws one
