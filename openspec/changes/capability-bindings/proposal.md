# Capability bindings that tell the truth

## Why

The typed binding model has now landed through #199–#205, but the remaining
SDF duplicates and missing behavioural coverage leave some fidelity claims
unproven. This change records the completed foundation and the requirements
the remaining work must meet. See #198, #203 and #216.

## What changes

- Keep the existing typed bindings as the single authority for artist intent,
  execution family, exact engine entry point and evidence-backed fidelity.
- Use that table for the shelf, availability, substitution explanation,
  diagnostics and behavioural tests. Retire overlapping capability lists.
- Bind existing engine verbs and distinguish or withdraw tools whose claimed
  effect cannot be demonstrated. Keep layer operations separate from brushes.
- Preserve settings per tool and representation when the active layer changes.

## Scope

This change governs capability truth for the four existing representations.
Dynamic adds a fifth column in `a-representation-that-adapts`. Engine algorithm
development and automatic representation conversion are outside this change.

## Dependencies

The existing `representation-modes` and `sculpting-tools` specs are the base.
The changes in #199–#202 and #204–#205 have landed. A remaining fidelity claim
is not considered proven until its behavioural fixture passes.
