# Design: capability bindings

## Single authority

Continue evolving `clayspace-model::tools::ToolKind::verbs()` in place. Each offered
`(ToolKind, Representation)` pair has a `Binding` with a stable artist intent,
an `ExecutionFamily` (field operation, voxel gesture, mesh stamp, hierarchy
pass, or a documented recipe), an exact callable engine symbol and a fidelity
grade supported by a
behavioural test. Absence is represented by no binding, not by a disabled
placeholder. A `ToolNote` explains where an absent tool applies.

The dispatch layer resolves this binding to the actual call. Its test observes
the resolved call and parameters, rather than accepting any `clay_*` string.
The shelf, availability, agent catalogue and diagnostics consume the same
binding. Static metadata cannot independently claim a different verb or
fidelity. A build-time or test-time symbol inventory checks entry points
against the pinned ClayCore header and wrapper.

## Fidelity decisions

`Native` means the plain label reading, `Specialized` names a
representation-specific improvement, `Approximation` names the observable
difference from the artist intent, and `Recipe` lists the actual operations.
No grade is inferred from a label. Existing differences to prove
include SDF Inflate versus Standard, Clay buildup versus Layer ceiling,
Smooth versus Relax and Flatten versus Polish; hierarchy smooth's three
frequencies; the hierarchy pass eraser; voxel Smear and the narrow-erosion
Crease recipe. When no distinct effect exists, remove the binding and explain
the absence. Do not synthesize a conversion to make an unavailable tool work.

## Representation changes

Brush settings use `(tool, representation)` as their key. On a layer switch,
retain the tool when a binding exists; otherwise choose a bound substitute and
report the substitution and its fidelity to the user and agent. Selection of
a substitute does not mutate the layer's representation.

## Verification

Tests enumerate every binding, check its symbol, execute it on a suitable
fixture, and compare the actual family/parameters with the declaration.
Measured profile assertions distinguish tools sharing a representation.
The same enumeration checks shelf, availability, notes and diagnostics.
