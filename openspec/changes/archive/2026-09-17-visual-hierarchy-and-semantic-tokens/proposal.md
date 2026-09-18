# The viewport is darker than the shell, and state wears the accent

## Why

The design system was written for one surface tone and has been asked to carry
four. `GROUND` is simultaneously the application's ground *and* the viewport's
clear colour, so the sculpt sits on exactly the tone the panels around it are
built from; `GRID_MINOR` and `GRID_AXIS` are grid-line colours that the shell
also draws panels and raised rows with. A capture of the shell shows the
result: chrome and sculpt occupy one flat band of grey, and the eye has
nothing to tell it where to look.

The redesign guide states the target as a rule — the viewport should receive
70–80% of the user's visual attention — and the current tokens cannot express
it, because the viewport and the chrome are the same colour by construction.

Three further gaps come from the same root:

- **Sliders are invisible at rest.** `Intensity`, `Size` and `Flow` head the
  options bar and are drawn as a hairline track with a grey knob. Nothing
  carries how far along its range a value sits, so the bar reads as a row of
  numbers with decoration under them. The design-system spec asks for exactly
  this ("a thin track with a position marker … without a raised handle or a
  filled decorative bar"), and it was written before the options bar had a
  primary sculpting role.
- **The active layer is indicated by a tone step nobody sees.** `panel` to
  `raised` is a 3.5% luminance step. It is the only thing distinguishing the
  layer being sculpted from the three that are not.
- **The brush shelf lost its swatches.** `close-brush-integration-gaps` (#64)
  changed the shelf's swatch from `size::SWATCH` (54 px) to `size::COLOUR_CHIP`
  (16 px) — the size named for the recent-colour row that the same commit
  added. The brushes' marks are now unreadable, the accent ring around the
  active one is a 3 px halo, and the shelf leaves a 40 px empty band under
  itself. `size::SWATCH` is documented as "a brush swatch in the shelf" and is
  no longer used by one.

## What Changes

- **A four-step surface ladder.** `VIEWPORT` is added below `GROUND`, and
  `PANEL` and `RAISED` are named in their own right rather than borrowed from
  the grid. The renderer clears to `VIEWPORT`; the shell keeps `GROUND`. The
  ladder becomes viewport < ground < panel < raised, and a test asserts the
  ordering rather than trusting four hex values to stay sorted.
- **The grid is retuned to the ground it is drawn on.** `GRID_MINOR` and
  `GRID_AXIS` drop by the same amount `VIEWPORT` drops below `GROUND`, so the
  grid keeps the distance above its ground that it was tuned for instead of
  becoming more prominent as a side effect of a darker viewport.
- **A reusable sculpt slider.** One widget, drawn as a track in the ground's
  tone with the traversed range filled in the accent, a knob that is restrained
  at rest and lifts under the pointer, and its value set monospaced and
  right-aligned. Every slider in the shell goes through it, so the treatment is
  changed in one place rather than per control.
- **The accent means state, and marks at one weight.** The active layer row
  gains a 2 px accent rail at its leading edge, over the raised surface and the
  primary-weight name it already has. The active brush is left alone: it
  already carries the same gesture at the same weight — a thin accent stroke
  tracing the thing itself, which for a ball is a ring — and a rail would be a
  fourth mark on one card. The accent's remit widens from "the active brush" to
  "active state", which is what it was already doing on the brush's label and
  on the tool-status line.
- **The shelf's swatch is restored to `size::SWATCH`**, with a test that pins
  it to the shelf so the next find-and-replace across size tokens fails loudly
  instead of shrinking the brushes again.
- **Section rhythm, stated in terms of the scale.** A section break already
  measured one `space::SECTION`, but as two constants either side of the rule
  that happened to add up to it. It is now written as the section step less the
  group step. No pixel moves; what changes is that the rhythm follows the scale
  when the scale moves, instead of sitting on a number nobody would think to
  look for in `heading_rule`.

## Out of scope, and why

- **The representation bar, the contextual inspector, the viewport HUD and
  focus mode.** Each is a new region with its own state and its own strings in
  three locales. They are the guide's PR 2, PR 3 and PR 7 and belong in changes
  of their own; this one adds no new control, only a treatment for the ones
  that exist.
- **Enlarging the shelf beyond its documented size, previews rendered from the
  real brush behaviour, and the All/SDF/Voxel/Mesh filters.** The guide's PR 4.
  Restoring `size::SWATCH` here is a regression fix, not the visual shelf.
- **MatCap presets, AO and shadow tuning, grid distance fade.** The guide's
  PR 5. Retuning the grid's two tones is forced by moving the ground under it
  and stops there; a distance fade is a shader change with a frame-time cost
  that has to be measured.
- **The unified transform gizmo.** The guide's PR 6, and the largest single
  item in it.
- **A high-contrast theme.** The contrast floors already in the spec are
  enforced against the new surfaces; an alternate theme is a separate
  capability.
