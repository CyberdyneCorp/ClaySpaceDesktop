# Tasks

## 1. The collision

- [x] 1.1 Establish it: `geometry_section` and `voxel_section` both draw
      `section_geometry`, and `heading` keys its fold and its recorded rect by
      the heading's own word
- [x] 1.2 Give each representation its own heading, which is what ends it
- [x] 1.3 Regression test: folding the geometry section leaves the voxel
      section open
- [x] 1.4 Confirm by mutation that the test fails when the two share a word
      again

## 2. The module

- [x] 2.1 `shell/inspector/` with `mod`, `sdf`, `voxel`, `mesh`
- [x] 2.2 Dispatch on the active representation from one fixed slot in the
      right region, so the panel's shape does not move under a sculptor
- [x] 2.3 Move the voxel display controls across verbatim

## 3. What each says

- [x] 3.1 Field: how many items the edit list holds, and whether it is
      collapsed
- [x] 3.2 Draw the field's section only where the engine has reported. A
      heading over nothing takes a section's height from a panel that already
      runs past its own bottom — it pushed the mask controls off, and a test
      that drags the mask slider caught it
- [x] 3.3 Mesh: the fixed-topology contract, which is a fact rather than a
      setting and the reason its brushes differ
- [x] 3.4 Voxel: display and blur, as before, under its own word
- [x] 3.5 Write down what was *not* built and why — every per-layer control the
      concept shows that the domain cannot express

## 4. The words

- [x] 4.1 Three section headings, the two field labels, yes/no and the mesh
      sentence, in all three locales
- [x] 4.2 In `all()`, so the locale-coverage tests see them

## 5. Verification

- [x] 5.1 Look at a grid layer's right panel, and see two sections where there
      was one
- [x] 5.2 `just check` — fmt, layering, clippy, spec, packaging and the
      workspace suite all pass
