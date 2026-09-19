# A layer switch is atomic

The first stroke after any switch was made with the previous subtool's
settings.

Measured three times in the voxel area and three more in the coverage pass. A
stroke made right after a switch used the layer-before's brush size, symmetry
and tool. In one session a new grid layer inherited **brush size 100**, so the
first dab on it was a metre across. `state.mask.present` stayed true on a
subtool that has no mask. A new armature layer inherited the previous subtool's
mirror, so strokes on it came out mirrored — which, since `add_zsphere` places
the reflected node itself, hung a second arm off the first.

## Why it happened

`Command::SelectLayer` reached the followers before it reached the scene.

The scene ViewModel is the only one that *moves* the active layer. The
sculpting ViewModel reads it — to decide what the shelf offers, which brush
settings belong and which mirror a stroke carries — and so do the cage, the
manipulator and the mask. The composition root dispatched to the sculptor
first, so each follower read the *old* active layer, set itself up for the
subtool being left, and only afterwards did the scene move. The next command
was the first one to see a consistent document, which is why the error was
always exactly one switch behind.

Two more instances of the same shape sat beside it:

- **A new layer arrives active.** `add_layer` activates what it made, through
  the same call a stack row click takes. The sculpting ViewModel ignored
  `AddLayer` and `RemoveLayer` outright, so `layer add {kind:'grid'}` — the
  repro's own switch — left the brush holding the field layer's settings even
  with the order corrected.
- **A rig layer is a switch nothing announces.** `begin_armature` adds the
  armature's own layer and turns that layer's mirror off, and no `SelectLayer`
  passes through the command path to say so.

## What changes

- **The scene ViewModel is dispatched to first**, ahead of every follower that
  reads the active layer. The order is stated where it is kept, and a test
  reads the composition root for it.
- **The sculpting ViewModel follows the layer on `AddLayer` and `RemoveLayer`**
  as well as on `SelectLayer`. All three move the sculpt target; a new layer
  arrives active and a removal hands the target to whatever is left, which may
  be a different representation.
- **The mask ViewModel does the same.** A mask belongs to a subtool, and none
  of these three commands touches the document — which is why the refresh that
  follows every edit never ran for a switch and `present` stayed true on a
  subtool that has no mask. It is read where the command arrives rather than
  from a list elsewhere that has to remember it.
- **Starting a rig refreshes the sculpting ViewModel**, so the new armature
  layer's own symmetry is what the options bar shows and what the first ZSphere
  is placed with.
- **`refresh_after_conversion` and `refresh_after_open` become one
  `refresh_for_active_layer`.** The two were the same call under two names, and
  a rig makes a third caller. What they have in common is the only thing the
  method does.

## What this deliberately does not do

**It does not make every follower refresh through one trait.** A
`refresh_for_active_layer()` implemented by four ViewModels and called in a row
would be a list to keep in step, which is the shape the mask defect already
took. Each ViewModel reads the document when the command reaches it, and the
one thing the composition root owes is the order.

**It does not widen what a switch settles.** The rig, the retopologiser and
the bake panel are re-read after a switch exactly as before. A new or removed
layer re-reads them through the document-touching path, as it always has.
