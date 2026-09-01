# Tasks

## 1. Move the pin

- [x] 1.1 Point the submodule at v0.73.0 and move `EXPECTED_ABI`, which the
      `version_is_the_pinned_engine` test holds to the submodule
- [x] 1.2 Build the whole workspace against the new engine before changing a
      line of it, so that what the upgrade forces is separable from what it
      enables — it forces nothing: 52 added entry points, none removed, no
      signature changed, no struct re-laid out
- [x] 1.3 Run the whole suite and triage every failure against the release
      notes rather than assuming staleness — two failures, both tripwires
      written to fail on exactly this

## 2. A mask protects a region from an operation

- [x] 2.1 Establish which of the two the gate defect was: a threshold and width
      that were never right, or a placement. The release notes say placement
      (#394, ABI 0.67.0) and the header now states the world-space rule
- [x] 2.2 Put the `set_gate` call back into `stroke_sdf`, on the stroke
      template — correct for every stamp only because the gate does not travel
      with the item, which is the opposite of the alpha's rule
- [x] 2.3 Turn both tripwire files around to hold the protection, keeping the
      threshold and width sweep: it was the evidence that no tuning was the
      answer, and held the other way it says the protection is not balanced on
      one lucky pair
- [x] 2.4 Measure it through the application rather than only at the boundary:
      1.0 against an unmasked 0.825, from a start of 1.0
- [x] 2.5 Check the half that already worked did not go with it — a mask still
      keeps a brush from depositing, and an unmasked document strokes as it did

## 3. Rebuild a mesh layer's topology

- [x] 3.1 Bind `clay_mesh_voxel_remesh`, its estimate, its report and the
      document-level `clay_document_voxel_remesh_layer` in `claycore`, taking
      the parameter defaults from `clay_mesh_voxel_remesh_defaults` rather than
      transcribing them
- [x] 3.2 Offer four controls rather than fourteen parameters, and state the
      two that are decisions rather than pass-throughs: colours always carried,
      an open surface closed rather than refused
- [x] 3.3 Carry the report through to the interface as an outcome. The
      operation destroys vertex and polygon identity and drops UVs every time,
      and this is the only place a sculptor is told
- [x] 3.4 Draw it beside the layer stack where the field layer's collapse row
      is, and say why it is always shown where that one waits for advice
- [x] 3.5 Three languages, as every string in this application is

## 4. Keep the sculptor honest across a rebuild

- [x] 4.1 Drop the mesh sculptor before the rebuild rather than after: it holds
      an adjacency and a BVH over triangles about to be replaced
- [x] 4.2 Measure whether `clay_document_mesh_layer_revision` covers undo. It
      does not — 1, 2, 2, 2 across attach, rebuild, undo, redo, with the
      triangle count going 119,100 / 37,752 / 119,100 / 37,752
- [x] 4.3 Record the engine depth each rebuild sits at, as this file already
      records a crossing, and drop the sculptor when history stands on either
      side of one. Bounded by there having been a rebuild at all, so an
      ordinary undo does not put the 160 ms weld back on the interface thread
- [x] 4.4 Prove the regression test bites: with the record disabled, a stroke
      after undoing a rebuild is refused with "the mesh changed its vertex or
      index count under this sculptor"
- [x] 4.5 Hold the engine's silence as an equality in `claycore`, so the day it
      is fixed the record here is reported as dead weight rather than kept

## 5. Say what moved

- [ ] 5.1 Compare the benchmark suite against the recorded baseline, in full
      runs rather than filtered ones
- [x] 5.2 Update the documentation that states the engine version, and the
      roadmap's account of what the engine gets wrong — two of its entries are
      closed by this pin
