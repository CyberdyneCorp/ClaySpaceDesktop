# Close agent catalogue gaps

## Why

`measure` can run command routes without checking the catalogue's published
groups. Long-running work it starts is absent from `state.jobs` and `wait`, so
the session can claim to be quiet while a retopology or UV job is active.
`describe.not_offered` omits two commands, and several action summaries name
an effect different from the command's actual parameters or representation.

## What changes

- Validate `measure.group` against the same groups as `tools/list`.
- Include active retopology, UV, conform and bake jobs in the outstanding work
  reported by `state.jobs` and `wait`.
- Carry worker progress into the observable job state as it changes.
- Explain bake execution and profile export in `describe.not_offered`.
- Correct summaries for brush flow and smoothing, transform snapping, lattice
  dragging, layer combine and voxel hole repair.

## Relationship to other work

The missing groups and routes, brush controls and curve insertion are covered
by `offer-agent-workflows` for issue #146. This change follows that work and
does not duplicate its route contract test. Job lifecycle and result-layer
semantics remain in the retopology workflow work.
