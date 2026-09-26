# A thickness per rig

## Why

Skin thickness was one document-wide multiplier (#180). The engine is handed
each rig's radii with the thickness already applied, and every history step
reads every rig back by dividing by it — so a thickness chosen for one rig
became the divisor for all of them. A rig written at 1 and read at 2 came back
with its authored radii halved, and the next edit wrote that out for good. The
multiplier was also not saved: a reopened rig came back at the default with the
old thickness baked into what it reported as authored.

Two smaller holes in the same capability: `armature/add` accepted a negative
radius, which the engine skins as an inverted fan, and `armature/insert` over
MCP was not mirrored although the pointer's insert is.

## What changes

- Thickness is stored on the layer that holds the rig. `set_skin` rewrites the
  active rig alone, is refused on a subtool without one, and records nothing
  when the value does not change. A thickness change remains one undo step and
  now names the layer it belongs to.
- Every rig is read back through its own thickness, after a history step and on
  open.
- Each non-default thickness is saved in a `.rigs` side-car keyed by stack
  position, like the hierarchy side-car, and read back before the rigs are
  recovered. A missing or malformed side-car reopens rigs at the default.
- `ArmatureViewModel::add` refuses a radius that is not positive and floors a
  tiny one where `resize` does; `insert` mirrors through the same path the drag
  uses.

## Already in place

The round trip at a non-default thickness, the thickness as an undo step, the
refusal of edits on missing spheres, and the rig mirror in `state.tool` landed
earlier (#235, #252); this change keeps them and adds the tests from the issue.

## Impact

Documents saved before this change have no `.rigs` side-car and open exactly as
before, at the default thickness.
