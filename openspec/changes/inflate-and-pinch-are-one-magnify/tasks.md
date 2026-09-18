## 1. The wrapper

- [x] 1.1 Bind `clay_layer_magnify_surface` and `clay_layer_magnify_surface_preview` in `crates/claycore/src/sculpt.rs`, beside `move_surface` and its preview.
- [x] 1.2 Give the descriptor a `MagnifyParams` that fills `struct_size` from the type compiled against, as every other descriptor does, and carry only the radius and the easing — a radial scale has no half-space to gate and no gesture id to fold on.

## 2. The two field verbs

- [x] 2.1 Route `Inflar` and `Pinçar` on an SDF layer to the magnify rather than to the stroke resolver, and point the layer's mirror rather than reflecting the gesture by hand.
- [x] 2.2 Take the sign from the tool and the magnitude from Intensidade, and turn the sign over on the invert key — which gives each tool its own opposite, the depth below being a property of the tool.
- [x] 2.3 Sink Inflar's dabs into the material along the field's own gradient, and leave Pinçar's on the surface.
- [x] 2.4 Lay one dab per step of the brush's spacing along the path, so a pointer resting still does not pile frames at one centre.
- [x] 2.5 Drop the frozen samples before any dab is placed, since the engine's descriptor has no gate.
- [x] 2.6 Wrap the gesture's dabs in one undo group.
- [x] 2.7 Invalidate the dab's own ball with no dilation, once per image the layer's symmetry makes of it.
- [x] 2.8 Take `Inflar` out of the SDF stamp recipes, along with the two constants that shaped relief into a swell.

## 3. Hold it

- [x] 3.1 `crates/claycore/tests/sculpting.rs`: a magnify swells every item of a blended form; the sign is magnify against pinch; the frames of one gesture fold into one warp; a preview names the nodes and touches nothing; a strength or a radius of zero is refused.
- [x] 3.2 `crates/clayspace-engine/tests/sdf_magnify.rs`: the swell is broader and lower than the ridge, measured in world units; the pinch gathers — the middle rises and the rim falls; inverting a brush gives its own opposite; a magnify across a blend moves both items; a gesture is one undo step; Pinçar is on the field's shelf.
- [x] 3.3 `crates/clayspace-engine/tests/sdf_brushes.rs`: Pinçar joins the field's surface brushes and its signed ones.
- [x] 3.4 `crates/clayspace-model`: the field vocabulary is fifteen tools rather than fourteen.
- [x] 3.5 The two places that used Pinçar as their example of a tool a field has no verb for — `crates/clayspace-engine/tests/voxel_tools.rs` and `describe_offers_only_the_tools_the_layer_has_a_verb_for` — name Apagar instead, whose only verb is the grid's. Both assertions stand; only the example moved.

## 4. Say so

- [x] 4.1 `docs/features.md`: the two rows, the count, the measurement behind the strength, what the invert key does, and the mask and warp-accumulation notes. Remove the "SDF Pinçar" gap, which is closed.
- [x] 4.2 `docs/roadmap.md`: ClayCore #391 is taken up rather than waiting upstream, and the gesture-region note names two callers rather than one.
- [x] 4.3 `docs/features.md`: the agent surface's example of what a field does not offer is `erase` rather than `pinch`.

## 5. Keep the file the same on both platforms

- [x] 5.1 Move the canonical fixture out of the binary into `clayspace_app::canonical`, and say there which verbs may stand in it: only the ones whose record is the ask itself, since a sunk dab centre and a baked volume are both floats the engine worked out and neither is the same across toolchains.
- [x] 5.2 Stamp `Argila` where the fixture stamped `Inflar`, so the document still crosses more than one verb.
- [x] 5.3 `crates/clayspace-app/tests/canonical_document.rs`: every position the fixture stamps is in the saved bytes verbatim — the matrix's property, asked on one machine.
