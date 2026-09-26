## Why

An audit (#196) found refusals that gave the wrong reason and requests that did
nothing and said nothing:

- A hidden layer was refused as **locked**. The engine's "editable" answer
  folded visibility in, was asked first, and the engine never answered the
  visibility question on its own — so the hidden refusal was unreachable.
- `layer/optimize` on a grid answered "nothing to consolidate", which reads as
  "already done" rather than "this is a field's action".
- Placing an object on a grid said "applies to SDF layers; this one is SDF" —
  the ViewModel built the refusal with a hard-coded representation.
- `lattice/drag` with none or several control points selected moved nothing and
  reported success; so did turning or scaling a single control point about its
  own middle, and pointing the manipulator at an object or layer that is not in
  the document.

Two display defects from the same audit ride along, because each already had a
requirement it was breaking: the transform readout showed a uniform scale as
three numbers, and the crossing panel told a sculptor leaving a hierarchy that
"the parametric history" would not come back, which is a field's sentence.

## What Changes

- The engine answers `active_layer_visible` itself, and `active_layer_editable`
  is protection alone. A hidden layer is refused as hidden.
- `consolidate_layer` refuses any representation but a field with
  `Unavailable::NoVerbHere`, naming the layer's representation.
- The object ViewModel's grid refusal names the active representation, through
  a new `clayspace_model::no_objects_on(representation)`.
- `drag_lattice_point` refuses with no cage up and with a selection other than
  one point; a cage `drag_gizmo` in rotate or scale mode refuses a selection of
  fewer than two points.
- `SetGizmoTarget` refuses a target the model cannot place (a missing object,
  a missing layer, a curve with nothing selected), rather than holding it.
- The transform readout shows one scale factor where the three agree.
- The crossing panel names a hierarchy's levels as what leaving one loses.

## Capabilities

### Modified Capabilities
- `scene-and-layers`: a refusal names the reason and the representation that
  hold.
- `object-transform`: a manipulation that would move nothing is refused.

## Impact

- `clayspace-engine` (`document.rs`), `clayspace-model` (`shape.rs`),
  `clayspace-vm` (`object_vm.rs`), `clayspace-view` (`shell/mod.rs`,
  `shell/windows.rs`, `strings.rs`), the MCP `lattice/drag` summary.
- No data model or file-format change.
