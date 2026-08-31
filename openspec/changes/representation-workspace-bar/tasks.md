# Tasks

## 1. Groundwork

- [x] 1.1 Split `shell.rs` into one module a region before adding another one
      to it — 5,056 lines, and the bar is a whole new region
- [x] 1.2 Make the tests that read the crate's own source walk it, since two
      were listing a flat `src/` and one of them went quietly blind rather than
      failing

## 2. The icons

- [x] 2.1 Add `FieldRepresentation`, `VoxelRepresentation` and
      `MeshRepresentation` to the one drawn icon set
- [x] 2.2 Look at them, and redraw the mesh icon: a triangle with a line
      dropped from its apex is a warning sign, and it read as one beside two
      icons that are plainly objects. Subdivided by its edge midpoints instead

## 3. The words

- [x] 3.1 Add `representation_sentences`, `section_representation` and the two
      card hints, in all three locales
- [x] 3.2 Put the three scalars in `all()` so the locale-coverage tests see
      them, and give the array its own test as the other vocabularies have

## 4. The bar

- [x] 4.1 `workspace.rs`: the bar, the cards, and the crossings row
- [x] 4.2 Derive the crossings from `Direction::from_representation` rather
      than listing them
- [x] 4.3 Aim the conversion panel and open it; never run the conversion
- [x] 4.4 Open it only when it is shut — `ToggleConvert` is a toggle, and a
      sculptor who clicked Malha would have watched the panel they were aiming
      disappear
- [x] 4.5 Draw it inside the central region, so it spans the viewport rather
      than running behind the inspectors
- [x] 4.6 Remove the viewport bar's representation line, and lower the
      untranslated-label ratchet to nine

## 5. Fitting

- [x] 5.1 Size cards from their own text: a width that fits "Signed Distance
      Field" leaves "Malla" floating, and one chosen for Portuguese clips the
      Spanish
- [x] 5.2 Shed the phrases, then the heading, never the crossings
- [x] 5.3 Count egui's own `item_spacing` in the arithmetic — leaving it out
      overran the inspector's edge by eight pixels while believing it had fitted
- [x] 5.4 Record the floor honestly: below roughly five hundred pixels the bar
      scrolls, because going further means icon-only cards and a representation
      must be told by icon *and* text

## 6. Tests

- [x] 6.1 Every card is drawn, and each in its own place
- [x] 6.2 A crossing aims the panel, opens it, and does **not** emit
      `RunConversion`
- [x] 6.3 The cards shed their phrases as the window narrows, and the crossings
      are drawn at both widths
- [x] 6.4 Guard the duplicated composition: `build_shell` in the visual tests
      is a second copy of the composition root's frame, and the bar was wired
      into the application and drew nothing in any capture with nothing failing
- [x] 6.5 Fix the harness's central panel, which painted the *shell's* ground:
      every capture understated the viewport boundary after the two tones were
      separated

## 7. Verification

- [x] 7.1 Look at the bar, in a capture
- [x] 7.2 `just check` — fmt, layering, clippy, spec, packaging and the
      workspace suite all pass
