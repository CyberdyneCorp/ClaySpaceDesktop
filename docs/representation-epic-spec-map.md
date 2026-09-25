# Representation epic: OpenSpec ownership

The living specifications in `openspec/specs/` are the baseline. The changes
below own the remaining work in #198. An issue appears in one change's task
list; a referenced dependency keeps its own change.

| Issues | Change | Contract |
| --- | --- | --- |
| #199–#205, #216–#217 | `capability-bindings` | One typed binding per offered tool and representation, with dispatch and behavioural evidence |
| #206–#210 | `a-representation-that-adapts` | Dynamic identity, persistence, explicit conversion, ordered topology history and chunked drawing |
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
records the completed ClayCore wrapper. Neither reopens completed issues.
