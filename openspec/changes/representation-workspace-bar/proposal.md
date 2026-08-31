# The three representations stand above the viewport, as equals

## Why

ClaySpace's differentiator is that SDF, voxel and mesh are three
representations of one workflow. The interface said so in two places, both of
them small: a three-letter tag on a layer row, and a line of text at the far
right of the viewport bar reading `Representação: SDF`. A sculptor could not
see, without opening a panel, what the other two even were — let alone that
crossing between them was possible or what it would cost.

The line in the viewport bar had a second problem. It drew
`Representation::label`, the engine's own word, under a translated prefix, so
`Representação: voxel` was half translated on every locale.

## What Changes

- **A representation bar above the viewport**, inside the central region so it
  spans the viewport it labels rather than running behind the inspectors. Three
  cards, one per representation, each an icon and a name and a phrase saying
  what it is. The active one is raised and railed, in the same grammar the
  active layer row wears.
- **The cards are a statement, not a control.** Clicking one converts nothing.
  Crossing between representations costs something and is not always
  reversible, so it stays behind the conversion panel where the cost is shown
  and confirmed.
- **The crossings are a row beside them**, derived from
  `Direction::from_representation` rather than listed, so a crossing the domain
  gains is offered and one it loses stops being. Clicking one aims the
  conversion panel and opens it. It does not convert.
- **Three icons**, told apart by shape and never by hue: nested contours for a
  field, cells for a grid, a subdivided triangle for a mesh.
- **The bar sheds its parts in a stated order** as the window narrows — the
  phrases first, into the tooltip; the heading second; never the crossings.
- **The viewport bar's representation line is gone**, and with it one of the
  ten untranslated domain labels the shell still drew. The ratchet is nine.

## Out of scope, and why

- **A viewport HUD.** The guide asks for a status card in the viewport's
  corner. With the bar sitting directly above the viewport, a third place
  naming the representation would be repetition, and the concept the bar is
  drawn from has no HUD in it. Worth revisiting when the HUD has something to
  say that the bar does not — a field's health, a mesh's triangle count.
- **Cards of icon alone.** The bar scrolls below roughly five hundred pixels of
  central region rather than dropping the names. A representation has to be
  told by icon *and* text; a shape on its own is what the design's own tests
  refuse to let state depend on.
- **Remesh, and the other operations the concept shows beside `Convert To`.**
  The domain has crossings between representations and this bar offers exactly
  those. Remeshing is a different operation and inventing an entry point for it
  here would be drawing a button for a verb that does not exist.
- **The contextual SDF/voxel/mesh inspector.** The guide's PR 3.
