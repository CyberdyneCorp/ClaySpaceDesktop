# Tasks

## 1. The filter

- [x] 1.1 A column at the shelf's leading edge rather than a row above it: the
      region is one swatch tall and a row would take a swatch's worth of height
      from it
- [x] 1.2 Keep the filter in egui's memory, beside the section folds
- [x] 1.3 Mark the chosen row as the active layer is marked — accent rail over
      a raised surface — so the shelf and the stack share one grammar
- [x] 1.4 Two new strings in three locales, and into `all()`

## 2. Browsing

- [x] 2.1 List the chosen representation's tools from the same declared table
- [x] 2.2 Draw a brush the active layer has no verb for dim, and give it hover
      sense rather than click sense
- [x] 2.3 Say why on hover, instead of describing a stroke that will not happen

## 3. Tests

- [x] 3.1 With no filter the shelf is exactly what it was: every tool that
      exists on the active representation, and no other
- [x] 3.2 Browsing draws the other representation's brushes and selects none
- [x] 3.3 Choosing a filter emits no command and does choose
- [x] 3.4 Mutation-check the refusal. Removing either guard alone leaves it
      shut — the sense and the click condition are two locks on one door — so
      the check had to remove both before the test would fail

## 4. Verification

- [x] 4.1 Look at the shelf
- [x] 4.2 `just check` — fmt, layering, clippy, spec, packaging and the
      workspace suite all pass
