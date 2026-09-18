## 1. The row a stroke would land in, as part of what a tool is asked about

- [x] 1.1 Put the selected row on `LayerState`, so a tool that depends on it is answered by the same value every other condition is answered by rather than by a rule written into one call site.
- [x] 1.2 Answer it from the document through the engine's own active pass, and forward it on the shared document so the running application does not inherit a double's default.
- [x] 1.3 Name the refusal `NeedsAPass`, apart from a missing attribute: nothing is absent from the layer, the stack may be full and the selected row is simply the form.

## 2. Bind the verb the wrapper already exposes

- [x] 2.1 Give `Apagar` its hierarchy row in the capability table, and say in the row why the mesh column stays empty between the two that are filled.
- [x] 2.2 Route the stroke through `SculptLayerStroke::erase` with the write domain named rather than left to `Automatic`, since `Automatic` with no pass resolves to the form.
- [x] 2.3 Keep the gesture one undo step, by the path every other hierarchy gesture already takes.

## 3. What a sculptor is told

- [x] 3.1 Add the tool note that says erasing a hierarchy takes the selected pass toward zero, in all three locales.
- [x] 3.2 Refuse the form with a sentence that names the pass to select, rather than with a list of where the tool applies.

## 4. Measured, not argued

- [x] 4.1 Erase one pass of two over a base and assert, vertex for vertex, that hiding the erased pass leaves exactly what was there before — the ABI has no per-pass checksum, and hiding is exact.
- [x] 4.2 Assert the other half in the same test: the erase did lower the pass, so the equality above cannot be satisfied by a stroke that did nothing.
- [x] 4.3 Assert the refusal on the form, that it names a pass, that the surface did not move, and that selecting a pass is all it takes.
- [x] 4.4 Assert one gesture is one undo step, against the four segments a drag arrives in.
- [x] 4.5 Prove the note by the calls: the same tool reaches the grid's erase on a grid and the layered stroke's erase on a hierarchy.
- [x] 4.6 Assert in the ViewModel that the eraser is on a hierarchy's shelf, that the status line carries the reason where the form is selected, and that a grid's eraser asks about no pass.

## 5. Documentation

- [x] 5.1 Give `Apagar` its hierarchy column in the tool table in `docs/features.md`.
- [x] 5.2 Say in the hierarchy section what erasing means there and why it is refused on the form.
