# Tasks

## 1. Move the pin

- [x] 1.1 Point `vendor/ClayCore` at v0.126.0 and move `EXPECTED_ABI` to 0.126
      by hand, which `version_is_the_pinned_engine` holds against the linked
      engine
- [x] 1.2 Move `Document::FORMAT` to minor 20 by hand, and say beside it why: a
      node record gained one byte, the item's own mirror axes, and the ABI
      still writes at the engine's current minor and no other
- [x] 1.3 Confirm the retopology pin is unaffected: `vendor/CyberRemesherAndUV`
      stays at v0.10.0, and `tools/check_engine_pin.py` answers for both
- [x] 1.4 Build the whole workspace against the new engine before touching a
      line of it. Thirty-two symbols added, zero removed, no struct re-laid out

## 2. Run what the release moves under us

- [x] 2.1 Run the whole suite in release and record which tests moved and why.
      One did, `a_mesh_cage_evaluation_is_priced_by_every_point_the_cage_holds`,
      armed for ClayCore#655: 3³ 0.026 ms against 32³ 0.224 ms over 2,048
      evaluations, 9x, where the old sum put the pair past 1,000x
- [x] 2.2 Keep the tripwire a tripwire: it is now
      `a_mesh_cage_evaluation_is_priced_by_its_dragged_points`, asserting the
      ratio stays under the same 20 from the other side

## 3. Say what is adopted and what is not

- [x] 3.1 `README.md`: the pin, the symbol diff, the format minor, and which of
      the release's host-visible changes reach this application without a
      source change
- [x] 3.2 `docs/roadmap.md`: the pin line, and the v0.126.0 entry points this
      application does not call yet — the stroke session, the multires delta,
      the item's own mirror axes, the voxel grid clone, the mesh from arrays —
      each with the open issue or change that will take it up
- [x] 3.3 This change, and `openspec validate --all --strict`
