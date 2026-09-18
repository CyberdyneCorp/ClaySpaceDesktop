# Tasks

## 1. Fold the completed changes in

- [x] 1.1 Archive the 35 complete changes in the order they were made, so the
      last text written for a requirement is the one that survives
- [x] 1.2 Archive `add-clayspace-desktop` and `render-quality-and-performance`
      with them. They are the base twenty-one of the 35 modify, and a MODIFIED
      delta against a requirement that is not in the living spec is refused
- [x] 1.3 Repair the deltas that could not fold as written: a requirement
      renamed with a MODIFIED rather than a RENAMED, and a MODIFIED that
      dropped a scenario it meant to withdraw. A modification may not drop a
      scenario — a supersession is a REMOVED and an ADDED, and saying which it
      is, is the point

## 2. Resolve the three contradictions

- [x] 2.1 **Subtool scale is per axis.** The engine's layer transform has taken
      a factor per axis since ClayCore ABI 0.74.0, so "a whole subtool scales
      uniformly" describes an engine that no longer exists. The requirement is
      restated as "Scale is per axis, on a placed object and on a whole subtool
      alike", which is a name the old reading cannot be got out of
- [x] 2.2 **A mesh layer is an operand of a resolved boolean and not of a live
      one.** The two deltas were describing two different operations and
      neither said which. `scene-and-layers` now draws the line at *live*
      composition — an entry in another layer's edit list — and points at
      `subtool-booleans` for the resolved boolean, which samples each operand
      into a volume of its own and prices the crossing
- [x] 2.3 **A second field subtool is composed into the preview, not a reason
      to fall back.** `a-preview-that-holds-the-whole-scene` reverses
      `live-field-brushes` rather than refining it, so the requirement is
      withdrawn and restated as "A live gesture is shown while it is being
      made, over the whole scene"

## 3. The gate

- [x] 3.1 `tools/check_specs.py`: a requirement in two capabilities, and a
      placeholder Purpose
- [x] 3.2 Run it from `just spec` and from the OpenSpec job in CI, beside
      `openspec validate --all --strict` rather than instead of it
- [x] 3.3 Tests in `tools/test_tools.py`, including one that breaks a copy of
      the specification and asserts the check fails on it. A guard nobody has
      seen fail is a guard nobody knows works

## 4. Write the Purposes

- [x] 4.1 Seventeen capabilities carried the placeholder the archive wrote.
      Each says what it covers now
- [x] 4.2 `representation-modes` said "each of SDF, voxel and mesh". There are
      four representations

## 5. Reconcile `docs/features.md`

- [x] 5.1 The mask section: "It did not, and the reason was never the engine"
      had drifted two paragraphs from the claim it answers, and read as a
      denial of the falloff
- [x] 5.2 Scale: two paragraphs still said a scale is uniform and that the axis
      boxes come back when ClayCore #373 lands. It landed in 0.74.0, and the
      same file says so 1,200 lines further down
- [x] 5.3 The voxel display: one paragraph said a grid is drawn "as the boxes
      it is" and that the rounded surface is a conversion away; another says
      the smooth surface is the default, which `VoxelDisplay::default` agrees
      with
- [x] 5.4 Regional refinement was described as something a sculptor does. It is
      bound through the model and the document and reached only by a benchmark
- [x] 5.5 Layer protection: what a ghosted or locked layer does is true and
      reachable; setting one is not

## 6. Gates

- [x] 6.1 `openspec validate --all --strict`
- [x] 6.2 `python3 tools/check_specs.py` and `python3 tools/test_tools.py`
- [x] 6.3 `just fmt-check` and `just layering`
