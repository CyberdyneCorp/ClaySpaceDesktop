# Tasks

- [x] Wrap DynamicSurface lifecycle, editing, conversion preflight, serialization, revisions and chunk copying in `claycore`, with ownership and refusal tests. (#206)
- [ ] Add `Representation::Dynamic` to the model, persistence, shell and capability table; document deliberately absent Layer. (#207)
- [ ] Implement explicit Mesh ↔ Dynamic commands with preflight, cost/loss report, selection reconciliation and one undo entry. (#208)
- [ ] Track topology, geometry and attribute revisions independently and upload only dirty chunks; compare with full rebuild and measure bytes. (#209)
- [ ] Integrate topology-changing edits with the existing sequenced history so undo and redo restore connectivity. (#210; depends on #151)
- [ ] Pin Dynamic locality with a behavioural test: stamp once and assert topology changes only inside brush support, with unchanged connectivity outside. (#216; after #207–#210)
- [ ] Update `README.md` and `docs/features.md`; run `openspec validate --all --strict` and the relevant workspace tests.
