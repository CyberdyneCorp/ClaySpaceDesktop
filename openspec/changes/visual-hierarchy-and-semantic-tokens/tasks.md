# Tasks

## 1. The surface ladder

- [x] 1.1 Add `VIEWPORT`, `PANEL` and `RAISED` to `palette.rs`, with the hex
      each was converted from recorded in `SOURCES` so the conversion test
      covers them
- [x] 1.2 Retune `GRID_MINOR` and `GRID_AXIS` to the viewport's ground, by the
      same amount the viewport drops below the shell
- [x] 1.3 Re-point `Tokens::panel` and `Tokens::raised` at the new constants
      and add `Tokens::viewport`, so the grid tones stop doubling as chrome
- [x] 1.4 Clear the renderer to `VIEWPORT` rather than `GROUND`, and draw the
      viewport bar's chips against it
- [x] 1.5 Tests: the four surfaces are ordered by luminance; the grid sits
      above the viewport, below the axis lines, and far below the foreground;
      primary text and the accent clear their floors on the viewport as well
- [x] 1.6 Assert the grid kept its step in **sRGB**, not in linear light — the
      two are not the same distance this far down the curve, and the first
      version of the test failed on tones that look identically spaced

## 2. The sculpt slider

- [x] 2.1 Add `sculpt_slider`: a track in the ground's tone, the traversed
      range filled in the accent, a knob restrained at rest and lifted under
      the pointer or during a drag, and the value monospaced and right-aligned
- [x] 2.2 Add `Tokens::control_track` and `Tokens::control_fill` so the widget
      names what it draws with rather than reaching for `ground` and `accent`
- [x] 2.3 Route `slider_named` through it, leaving the call sites and the
      `slider_id` recording unchanged
- [x] 2.4 Test that the fill follows the value: none at the bottom of the
      range, more at half, more again at the top, counted off the pixels
      inside the slider's own rect
- [x] 2.5 Give the arrow keys back. `egui::Slider` handled them; a hand-drawn
      track silently dropped them, and `Sense::click_and_drag` is focusable —
      so the control stayed reachable by keyboard and became inert once
      reached, which is worse than not being reachable
- [x] 2.6 Lock the horizontal arrows to the focused slider, as `egui::Slider`
      does, or they move focus to the next control instead of the value
- [x] 2.7 Size a press from the slider's own precision: one displayed unit, or
      a hundredth of the range where that is coarser. A fiftieth of the
      one-to-sixteen mask range was 0.3, which rendered as the same integer
- [x] 2.8 Hand out `slider_widget_id` so a test can put focus on a slider.
      Focus arrives by Tab and not by clicking, and the first version of the
      keyboard test clicked — so it passed with the arrow handling deleted,
      because a click on a slider sets the value to where it landed
- [x] 2.9 Make the suite's slider drags a fraction of the control's width
      rather than a fixed pixel delta. The tracks now span their columns, so
      the forty pixels that pushed the cage from three divisions to four became
      four tenths of one — a fixed delta is the same
      coordinate-off-a-screenshot mistake `slider_rect` exists to avoid,
      measured sideways

## 3. One selection grammar

- [x] 3.1 Add `Tokens::selection` and `Tokens::selection_soft`, and a
      `selection_rail` helper that draws the 2 px leading rail
- [x] 3.2 Rail the active layer row, over the raised surface it already has,
      and set its name in primary text
- [x] 3.3 Look at it, then take the rail back off the active brush: the swatch
      already carries an accent ring, a raised card and an accent label, and a
      rail made a fourth mark on one sixty-pixel card
- [x] 3.4 Test that the active row is railed and the others are not

## 4. The shelf's swatch

- [x] 4.1 Restore the shelf swatch to `size::SWATCH`, which #64 changed to
      `size::COLOUR_CHIP`
- [x] 4.2 Record each swatch's rect under `brush_swatch_id`, and assert from it
      that the shelf draws at the scale's swatch size — a token name in a call
      proves nothing about what reached the screen
- [x] 4.3 Confirm by mutation that the test fails when #64 is reintroduced
- [x] 4.4 Add a scale-hygiene test: every size the scale names is drawn with by
      something, comments stripped so prose naming a token does not count as a
      control using it
- [x] 4.5 Record what that test **cannot** do. It does not catch #64, because
      the material preview draws with `SWATCH` too and kept it "used" while the
      shelf did not. A shared size cannot be policed by counting references
- [x] 4.6 Give the layer row's name-width minimum its own `LAYER_NAME_MIN`
      rather than borrowing `SWATCH`: a name in a list is not a shaded ball,
      and the two cannot be moved independently while they share a token

## 5. Section rhythm

- [x] 5.1 Establish what a section break actually measures today, before
      changing it — it is `SNUG` above the rule and `ROOMY` below, which is
      already one `space::SECTION`
- [x] 5.2 Write it as the section step less the group step, so the rhythm
      follows the scale rather than two constants that coincide with it
- [x] 5.3 Confirm no pixel moved, since the right panel already overflows its
      region and any added space costs visible content

## 6. Verification

- [x] 6.1 Refresh the shell captures and look at them
- [x] 6.2 `just check` — fmt, layering, clippy, spec, packaging and the
      workspace suite all pass
- [x] 6.3 `gesture_end::neither_end_of_a_gesture_leaves_the_frame` is a
      machine-speed gate that trips on this box either side of its 16.7 ms
      budget — 17.7, 19.9, 24.8 ms across runs. Verified on a stashed tree that
      it fails on `main` too, at 20.1 ms, so it is not this change. Nothing
      here touches the sculpt path: the additions are rectangle fills in the
      shell
