## 1. State the budget in the domain

- [x] 1.1 Add `FieldBudget` — samples per brick axis, world units per sample, bytes the cache may spend — and price a region in bricks against it, the way `Cost::within` prices a grid in cells.
- [x] 1.2 Give it a `DEFAULT` for the static parameter tables, which are read before any document exists, and hold that default against `BRICK_CONFIG` from the engine side so the two cannot drift.
- [x] 1.3 Refuse a region that is not a finite number. `as u64` reads NaN as zero, which would make the most dangerous input look like the cheapest.
- [x] 1.4 Name the refusal in its own vocabulary — `FieldRefusal` — rather than borrowing the conversion's, which sends a sculptor to a resolution control that is not on the shape picker.

## 2. Bound the shape parameters by it

- [x] 2.1 Derive `LARGEST` from `FieldBudget::DEFAULT` instead of the fixed 10.0, and say in the comment what the old number stood for and why it did not hold.
- [x] 2.2 Report a clamp: `ShapeParameter::clamp_reported` and `Shape::sanitised_reported`, reporting only the numbers the caller actually supplied.
- [x] 2.3 Carry the report to the sculptor on the remark channel rather than the refusal one, so the agent door does not answer a placement that happened as an error.

## 3. Price the placements

- [x] 3.1 Measure a shape's box per axis, in the adapter where `primitive_of` already resolves the engine's ordering and axes.
- [x] 3.2 Price it before anything is placed, in `place_object`, `insert_shape_subtool` and `set_object_shape` alike, so no route is a way round another.

## 4. Price the curve's thickness

- [x] 4.1 Work out the radii the change would produce, price the box they fill, and only then write them — so a refusal leaves the guide as it was.
- [x] 4.2 Take a curve's extent from the balls its points carry, and answer nothing for a guide with no points rather than the infinity an unguarded union answers.

## 5. Hold it

- [x] 5.1 `a_shape_parameter_is_bounded_by_the_layer`, `a_clamped_parameter_is_reported` in `clayspace-model`.
- [x] 5.2 `an_oversized_insert_is_refused_and_changes_nothing`, `a_long_thin_form_is_still_placed` in `clayspace-engine`.
- [x] 5.3 `an_oversized_curve_radius_is_refused`, `a_curve_over_the_brick_limit_can_be_removed` in `clayspace-engine`.
- [x] 5.4 `a_budget_describes_the_cache_it_prices`, so re-tuning the cache and leaving the parameter table behind fails the build rather than the session.

## 6. Left for later

- [ ] 6.1 Price a curve that grows by having points added far apart, which reaches the same region by a path this change does not cover.
- [ ] 6.2 Price a stroke's region the same way, once there is a measurement saying it needs it.
