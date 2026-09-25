# Design

## Catalogue entries

Add `cut`, `retopo`, `uv`, `conform` and `bake` to `GROUPS`, then add an
`ActionSpec` for each of their 16 `Home::In` routes. Reuse the same tagged
choices for both the builder and the schema, including retopology methods.
Add the six existing brush controls and `curve.insert_point`, which the same
contract check finds missing. Route insertion to `InsertCurvePoint` with the
arguments used by the other curve point commands.

## Drift check

The test extracts literal `(group, action)` pairs from `Home::In` and the
builder's match arms, then compares both sets with the catalogue rows. It
also checks for duplicate rows and rows without a declared group. Existing
table-driven tests exercise every row's example and argument contract.

## Boundary

Keep `RunBake` deliberately unoffered because it opens a destination file
panel. Other actions retain their model-level refusals and job lifecycle.
