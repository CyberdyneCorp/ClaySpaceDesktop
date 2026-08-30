# Tasks

## 1. A brush colour, and the two brushes that read it

- [ ] 1.1 Add a brush colour to the sculpt session in `clayspace-model` —
      one current value plus a short recent list, shared across tools rather
      than stored in `BrushSettings`, which is per tool and per representation
- [ ] 1.2 Extend `SculptModel` with the accessor pair the combine settings
      already use, and add the commands and observable to `clayspace-vm`
- [ ] 1.3 Resolve the colour to a palette entry in `clayspace-engine`, reusing
      an existing entry within tolerance rather than adding a duplicate, and
      pass it to the voxel paint brush
- [ ] 1.4 Pass the same colour to the mesh paint stamp, which was left at its
      white default
- [ ] 1.5 Stop the composition root suppressing vertex colour, and confirm the
      field surface is unchanged by the switch
- [ ] 1.6 Offer the swatch in the brush options, shown for the tools that write
      colour and hidden for the rest
- [ ] 1.7 Tests: the palette entry changes, geometry does not move, a mask
      protects colour, undo and redo restore it, and a saved document reopens
      with the colours painted

## 2. Mover on voxel layers

- [ ] 2.1 Declare the row and give voxel Mover its gesture state: an anchor,
      what has been asked for, and what has been emitted
- [ ] 2.2 Emit only the part that has grown past a whole cell, quantised before
      it is emitted so the record matches what the grid did
- [ ] 2.3 Reflect the displacement as well as the centre under symmetry
- [ ] 2.4 Tests: sub-cell steps accumulate, the total matches a single drag,
      one gesture is one undo, and a drag is not a smudge

## 3. Planar on voxel layers

- [ ] 3.1 Declare the row and bind it to the grid's flatten verb, two-sided,
      with the plane normal voxel Scrape already uses
- [ ] 3.2 Say in the tooltip that the voxel flatten is two-sided where the
      other two are cut-only
- [ ] 3.3 Tests: height variance about the plane falls, the mask is honoured,
      symmetry reaches both sides, one gesture is one undo

## 4. Vinco and Argila on SDF layers

- [ ] 4.1 Let a tool carry its own operation and accumulation, overriding the
      Combinar panel where the tool *is* the operation, as Camada already does
      for accumulation
- [ ] 4.2 Vinco: incise, tight region, dense spacing, inverting to a relief
      ridge
- [ ] 4.3 Argila: relief with buildup and a clay profile, distinct from Padrão
      by accumulation and spacing and from Camada by the opposite of the same
      axis
- [ ] 4.4 Tests: the trough is narrow, no primitive is added, the inverse
      raises, buildup accumulates and Camada does not

## 5. Mover Topológico

- [ ] 5.1 Bind `clay_item_volume_move_topological` in `claycore` with its
      descriptor, and test it against a fixture close in space and far along
      the surface
- [ ] 5.2 Add the tool, on SDF layers only, through the baked-region path the
      relax and flatten strokes already use
- [ ] 5.3 Tests: the topological drag and the Euclidean one differ measurably
      on the same fixture, and symmetry reaches it

## 6. Document-owned masks

- [ ] 6.1 Give `claycore` a mask source that names a layer, and take it on the
      five masked entry points instead of a borrowed handle
- [ ] 6.2 Move `ClayDocument`'s mask geometry into the document, keeping only
      what the interface needs beside it
- [ ] 6.3 Point the renderer's mask sampling at the same document mask the
      verbs consult
- [ ] 6.4 Tests: a mask survives save and reopen, a document without one opens,
      each subtool keeps its own, and every representation is still gated

## 7. Hold the table to the code

- [ ] 7.1 Table-drive a test over every tool × representation pair the table
      declares, asserting each lands rather than refusing
- [ ] 7.2 Update `docs/features.md` and the roadmap with what is now bound and
      what remains upstream, with the measurement behind each refusal

## 8. Look at it

- [ ] 8.1 Visual captures for the new bindings — voxel paint, voxel grab, voxel
      planar, SDF crease, SDF clay, topological move, mask persistence
- [ ] 8.2 Run the whole suite and the layering and specification gates
