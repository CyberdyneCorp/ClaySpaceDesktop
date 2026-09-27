# Stroke controls that act

## Why

The audit behind #178 found three brush controls that a sculptor or an agent
could set and see nothing change:

- **Borda on a field.** Every falloff produced the identical affected area. A
  field stroke stamps an item, which has no falloff curve, and the setting was
  read and dropped on the way.
- **Suavização.** The engine runs its lazy mouse per call, from the call's first
  sample. A live stroke is sent in segments of about three stamps, so the
  trailing point restarted at every joint and had no path to act on.
- **Agent strokes came out beaded.** A segment began at its own first new
  sample, leaving the stretch since the last stamp unstamped. A pointer's
  samples are dense enough to hide it; an agent's few far-apart samples each
  sent a segment of one sample, and the stroke became one blob per sample.

## What changes

- A field stamp's rim width follows the falloff (Dura 0.25, Linear 0.5, Suave
  1.0 — unchanged — and Gaussiana 1.5 times the region).
- The sculpt ViewModel steadies the gesture itself, once, with the engine's own
  lag, and sends the brush with smoothing zeroed. Drags are not steadied.
- Each stamping segment after the first starts where the next stamp is owed,
  measured with the gap the document reports through the new
  `SculptModel::stamp_gap` (a named field brush scales its spacing).

## Out of scope

Mask extrude thickness is capped by how far the mask reaches off the surface —
an engine property, filed as CyberdyneCorp/ClayCore#660. `mask/apply steps`
already reaches the operation on main (#251).

## Impact

`clayspace-model` (trait method, `BrushSettings::spacing`), `clayspace-engine`
(`stamp_gap`, `field_rim`), `clayspace-vm` (`stroke_path`), `clayspace-app`
(forwarding). No file-format change.
