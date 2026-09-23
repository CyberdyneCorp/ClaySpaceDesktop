# Carry what a binding *is* in the capability table, not in the prose beside it

## Why

`ToolKind::verbs` held four `Option<&'static str>` — one entry-point name per
representation — and everything else about a binding lived in comments.
Whether a row is the representation's natural verb, a strength the
representation alone has, a useful stand-in, or several verbs composed was
recorded where it could not drive the shelf, could not reach the diagnostics
line, and could not fail a test when the code stopped matching it.

The audit found the consequences spread across the shelf, and each one is the
same failure one layer down:

- two tools naming one call on one representation while the interface presents
  them as different tools (#203). Under bare strings, the correct case — Padrão
  and Inflar are both relief on a field, and relief is the field's Inflate —
  was indistinguishable from a row copied by mistake;
- a caveat describing a mode the code never selected (#199). The caveat and the
  row it hangs off had no relation either could be held to;
- a row naming an entry point the code never calls (#202), which is now checked
  against the linked engine but only as a *name*.

ClayCore v0.120.0 measured the last of these and settled which way round it
goes: relief is the faithful Inflate and the engine's own Standard preset is
the approximation. That ordering is the reason Padrão carries
`ToolNote::SdfStandardIsAnInflate`, and until now there was nowhere in the
table to write it down.

## What Changes

- Each column of `Verbs` becomes a `Binding`: the entry point, the
  `SemanticIntent` the tool means by it, the `ExecutionFamily` the call belongs
  to, and the `Fidelity` with which it keeps the label's promise.
- The same lookups stay the only ones — `verb_on`, `exists_on`,
  `for_representation`, `availability` — with `binding_on` beside them for the
  callers that want to say something about a binding rather than name it.
- Recipes become expressible: a binding may be several verbs standing in for
  one the engine has not, and is marked as such rather than left out with the
  reason in a comment.
- The claims become checkable. A tool means one thing wherever it is offered. A
  caveat never hangs off a row that does exactly what its label says. Two tools
  whose bindings are identical in every part are one verb under two words, and
  the two that really are are named with their reason.
- The diagnostics report carries the active tool's binding, intent, family and
  fidelity, so a report about a brush that surprised somebody says which of the
  four calls behind that one word ran.
- `tools/check_layering.py` holds the one-table rule that was a comment: no
  View, ViewModel, engine adapter or agent-facing crate may decide anything per
  (tool, representation).

Behaviour on the shelf is unchanged: the same tools reach the same
representations through the same calls. What changes is what the table says
about them and what can be asserted from it.

## Impact

- `sculpting-tools`: a requirement for the typed row and for the invariants it
  makes checkable.
- `diagnostics`: a requirement for the tool line.
- `crates/clayspace-model`: `tools.rs` — the types, the rewritten table and its
  tests; `diagnostics.rs` — the section and its line; `shape.rs` — the object
  row, written the same way.
- `crates/clayspace-engine`, `crates/clayspace-app`: the `Verbs` literals that
  state where a structural operation applies, and the composition root that
  fills the new section.
- `tools/check_layering.py`, `docs/features.md`, `docs/architecture.md`.
