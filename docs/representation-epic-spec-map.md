# Representation epic: OpenSpec ownership

The living specifications in `openspec/specs/` are the baseline. The changes
below own the remaining work in #198. An issue appears in one change's task
list; a referenced dependency keeps its own change.

| Issues | Change | Contract |
| --- | --- | --- |
| #199–#205, #216 (all but Dynamic locality), #217 | `capability-bindings` | One typed binding per offered tool and representation, with dispatch and behavioural evidence |
| #206–#210, #216 (Dynamic locality) | `a-representation-that-adapts`, archived 2026-10-06 | Dynamic identity, persistence, explicit conversion, ordered topology history, chunked drawing and the locality contract, now in the living `representation-modes`, `representation-conversion`, `edit-history`, `viewport-rendering` and `performance-budgets` specs |
| #211–#214 | `retopology-uv-and-baking` | Job and accepted result, guides, density, UV preservation, hierarchy flow and bake out |
| #215 | These three OpenSpec changes | Proposal and task ownership |

The representation decisions are explicit:

- SDF Inflate and Pinch use the signed assembled-surface magnify binding.
  A surviving SDF shelf distinction needs a measured fixture; otherwise its
  binding is removed with a note. (#201, #203)
- Voxel Crease is a narrow-erosion recipe, with its steps declared. (#204)
- A hierarchy owns pass-specific smooth and erase verbs; it owns no colour
  brush. (#199, #200)
- Dynamic does not offer Layer: new vertices have no stroke-start surface from
  which to measure the Layer ceiling. (#207)
- Retopology guides and density are workflow data, not sculpt brushes. The
  accepted result is a new mesh subtool with optional preserved UVs. (#211–#213)

`capability-bindings` includes completed foundation tasks so the remaining
behavioural work has one contract. `a-representation-that-adapts` likewise
recorded the completed ClayCore wrapper; it is archived under
`openspec/changes/archive/2026-10-06-a-representation-that-adapts` and its
deltas live in the specs named above. What #209 leaves open is engine-side — a
coloured surface is drawn whole until `clay_dynamic_surface_copy_chunk` carries
an attribute — and is stated in `viewport-rendering` rather than held in a
change. Neither change reopens completed issues.
