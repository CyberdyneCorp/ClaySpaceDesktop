# Tasks

## 1. Establish what was there

- [x] 1.1 Confirm `layout` has no consumer: the module is exported and called
      by nothing, and the regions use `exact_width`
- [x] 1.2 Find the preference store rather than assuming there is none —
      `SessionStore` has been keeping the recent list and the locale all along

## 2. Storage

- [x] 2.1 `load_layout` and `save_layout` on `SessionStore`, beside the locale
- [x] 2.2 Tests: a round trip; nothing stored is the design's arrangement; a
      corrupt line costs the arrangement and never the start-up

## 3. The regions

- [x] 3.1 Draw the three resizable, clamped to the bounds `layout` declares
- [x] 3.2 Skip a collapsed region entirely rather than drawing it narrow
- [x] 3.3 Collect a splitter drag and store it, comparing first so the file is
      not rewritten every frame

## 4. The menu

- [x] 4.1 Fill the Janela menu: the three regions, and a reset
- [x] 4.2 Tick means *shown*, which is what a person reads a tick as
- [x] 4.3 Read the request out of memory and remove it, so one click is one
      toggle
- [x] 4.4 Drop egui's remembered `PanelState` on a reset, or the stored sizes
      move and the panels do not

## 5. Tests

- [x] 5.1 The menu offers all three, distinctly named, and a reset; and emits
      no command
- [x] 5.2 A collapsed region is not ticked
- [x] 5.3 A region put away stops drawing its sections, and the central region
      is no narrower for it
- [x] 5.4 Keep the capture harness in step with the composition root on
      collapse, while leaving its widths exact
- [x] 5.5 `window_smoke` still presents real frames

## 6. Verification

- [ ] 6.1 `just check`
