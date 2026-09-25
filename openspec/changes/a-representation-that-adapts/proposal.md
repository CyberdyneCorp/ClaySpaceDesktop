# A representation that adapts

## Why

ClayCore exposes a DynamicSurface with topology-changing strokes and chunked
geometry. The safe wrapper landed in #206, but the application still has no
first-class representation for it. Treating it as a mesh would conceal
conversion, persistence and history costs. See #198 and #207–#210.

## What changes

- Use the pinned engine's DynamicSurface lifecycle, editing, serialization,
  preflight and dirty-chunk calls now bound in `claycore`.
- Add `Representation::Dynamic` with its own persistence identity, capability
  bindings, viewport card, inspector and agent description.
- Expose Mesh → Dynamic and Dynamic → Mesh as explicit, preflighted commands.
- Record topology-changing edits so undo and redo restore connectivity as well
  as positions, in the document's existing ordered history.
- Upload only dirty chunks, keyed by independent topology, geometry and
  attribute revisions.

## Scope and dependencies

This is application and wrapper integration, not a new ClayCore algorithm.
It builds on the living representation, conversion, edit-history and viewport
specs. The ordered-history contract from #151 is reused rather than replaced.
Capability bindings follow `capability-bindings`. Dynamic deliberately has no
Layer brush: a stroke can create vertices that did not exist at stroke start,
so a per-vertex starting-surface ceiling has no defined meaning.
