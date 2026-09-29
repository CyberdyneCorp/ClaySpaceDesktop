# Tasks

- [x] Wrap DynamicSurface lifecycle, editing, conversion preflight, serialization, revisions and chunk copying in `claycore`, with ownership and refusal tests. (#206)
- [x] Add `Representation::Dynamic` to the model, persistence, shell and capability table; document deliberately absent Layer. (#207 — also the basic Mesh ↔ Dynamic crossings through the existing conversion path, a `.dynamic` side-car, and snapshot undo in the ordered history; the preflight cost/loss report, the chunked drawing and the reversible topology delta remain with #208–#210)
- [x] Implement explicit Mesh ↔ Dynamic commands with preflight, cost/loss report, selection reconciliation and one undo entry. (#208 — engine-priced crossings in both directions refused past the surface budget with estimate and limit; the panel shows peak, budget and quad loss; each crossing is one undo that restores the source representation; transform and visibility carried across, and a freeze in place takes the name back; round trip within 1e-5; no tool converts a layer)
- [ ] Track topology, geometry and attribute revisions independently and upload only dirty chunks; compare with full rebuild and measure bytes. (#209)
- [ ] Integrate topology-changing edits with the existing sequenced history so undo and redo restore connectivity. (#210; depends on #151)
- [ ] Pin Dynamic locality with a behavioural test: stamp once and assert topology changes only inside brush support, with unchanged connectivity outside. (#216; after #207–#210)
- [ ] Update `README.md` and `docs/features.md`; run `openspec validate --all --strict` and the relevant workspace tests.
