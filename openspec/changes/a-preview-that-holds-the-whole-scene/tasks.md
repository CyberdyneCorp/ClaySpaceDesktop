# Tasks

## 1. Reach the document from a lattice that is not the document's

- [x] 1.1 Add `BrickRequest::translated` in `claycore`, and say in the doc
      comment which fields an evaluation actually reads — origin, spacing,
      dims, band — so the claim that the key may be carried across untouched is
      checkable rather than asserted
- [x] 1.2 Test it by reading the same bricks twice, once where they sit and
      once translated onto a form two units away, and assert the keys did not
      move and the originals still submit

## 2. Compose the preview with the rest of the document

- [x] 2.1 Give `LiveSmooth` a `Rest`: the layer to exclude, the other visible
      field subtools' bounds, and the backend the pointer-down pass is routed
      to. `None` where there is nothing to compose, which is the measured case
- [x] 2.2 Widen the preview lattice over those bounds once at pointer-down and
      fill what it drains from the rest of the document, skipping any brick the
      transaction already covers — that one holds a composed sample, and the
      rest of the document alone would be that composition with the layer under
      the brush taken out of it
- [x] 2.3 Fold the rest into every batch the transaction produces, by a minimum
- [x] 2.4 Lift the gate in `live_smooth_is_possible` and say in its doc comment
      what used to be there and why it is gone

## 3. Hold it

- [x] 3.1 Invert `a_second_field_subtool_falls_back_to_the_gesture_being_held`:
      it now asserts the gesture opens *and* that the second subtool is still
      drawn while the first is smoothed, measured by how far the drawn surface
      reaches. Checked to fail with the composition disabled
- [x] 3.2 Write the regression the calls exist for:
      `composing_the_rest_of_the_document_does_not_spoil_the_gesture_it_is_inside`
      asserts the history depth does not move for the length of the gesture and
      that the commit still lands — the case the hide/sample/show route could
      not serve
- [x] 3.3 Cover the engine's asymmetry from above as well as at the ABI: a
      hidden second subtool is left out of the preview, an empty one neither
      refuses the gesture nor changes it. The refusal half — an unknown layer
      is `NotFound` — is not reachable from here, because the excluded layer is
      always the active one; it is held in `claycore`'s `abi_surface.rs`
- [x] 3.4 Correct the three places that state the limitation as a fact:
      `docs/features.md`, `docs/roadmap.md`, and the *What this does not do*
      section of `live-field-brushes`
