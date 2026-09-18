# Tasks

## 1. The binding

- [x] 1.1 Bind `clay_layer_set_transform_nonuniform` in `claycore`, with what
      the engine says it costs
- [x] 1.2 Use it for *every* object transform, since the ABI does no partial
      updates and the uniform call collapses a stretch

## 2. The domain

- [x] 2.1 `Transform::scale` and `SceneObject::scale` become three
- [x] 2.2 A scale drag applies to the axis whose box was grabbed, and to all
      three from the centre
- [x] 2.3 `uniform_scale` for the callers that still want one number, and
      `is_uniformly_scaled` for the ones that need to know

## 3. What stays uniform

- [x] 3.1 A layer's transform: the engine's layer call takes one factor
- [x] 3.2 The geometric call sites that convert between a layer's frame and the
      world take `uniform_scale`, which is exact there because a layer's three
      components never diverge

## 4. The side-car

- [x] 4.1 Append the two extra components after the counted run of parameters
- [x] 4.2 Read them optionally, so a document written before this opens
- [x] 4.3 Tests: a stretch round-trips; a row in the previous format reads as
      uniform; and no field a previous reader walks by position has moved

## 5. The interface

- [x] 5.1 Offer the boxes on a placed object and not on a whole subtool
- [x] 5.2 Ask the ViewModel which, rather than deciding it in the composition
      root
- [x] 5.3 Show three factors where they differ and one where they do not

## 6. The stale answers

- [x] 6.1 Remove `GizmoHandle::all_for_transform`, which existed to state the
      belief this change disproves and had no caller outside its tests
- [x] 6.2 Replace the ViewModel's `handles()` with `per_axis_scale()`: the
      manipulator stopped being mode-specific when it became one widget
      carrying every operation

## 7. Tests

- [x] 7.1 An axis box stretches one axis and leaves the other two alone
- [x] 7.2 The centre handle stays uniform
- [x] 7.3 A stretch reaches the *field*: a sphere doubled in x is inside at a
      point along x where an unstretched one is outside, and still outside
      along y
- [x] 7.4 A stretched object reopens stretched

## 8. Verification

- [x] 8.1 `just check` — fmt, layering, clippy, spec, packaging and the
      workspace suite all pass
