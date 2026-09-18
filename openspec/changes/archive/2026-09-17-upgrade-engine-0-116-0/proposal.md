# Move the engine pin to ClayCore v0.116.0 and the retopology pin to v0.9.0

## Why

Both pins moved on main — ClayCore v0.113.0 → v0.116.0 in #140, CyberRemesher
v0.8.0 → v0.9.0 in #147 — and neither move had a change of its own. Every
earlier pin move in this repository does: `upgrade-engine-0-52-2` through
`upgrade-engine-0-113-0`. This change is that record, written after the fact,
so the pin the workspace stands on is described in the same place as the six
before it rather than only in a merge commit.

**Nothing had to change to build against either.** The ClayCore symbol diff
from v0.113.0 is one addition and zero removals, and every struct that grew did
so behind the `struct_size` this workspace already writes. The retopology
library's 121 commits likewise forced nothing.

**Two ClayCore defects it carries were ours to feel rather than to fix.**

- A **topological move discarded the volume's feather**, so the hard
  `CLAY_OP_REPLACE` edge showed as a lattice in every drag the move made on a
  field. The feather is the whole of ClayCore #67: with one, the inside is the
  volume, the outside is what was there, and the band between them crosses over.
- The **live Move door dropped `gesture_id`**. `clay_sdf_move_begin` did not
  copy the name into the transaction, so a live drag went unnamed however this
  application labelled it — and it always did label it — and folded by centre
  and radius like any other grab. Two separate presses at one anchor at one
  brush size compared equal, the second replaced the first, and **the first
  pull was lost**. ClayCore #604 fixed it, and the tripwire in
  `move_gesture_identity.rs` fired on the pin move, which is what it was
  written to do.

**The retopology library finally declares an ABI number.** Two guards in this
workspace carried the same note: the engine's SOVERSION was its project major,
still 0, so `libcyber_capi.so.0` named v0.7.0 and v0.8.0 alike and a mismatched
library linked without complaint. Both said "the day one exists, assert on it
here instead." v0.9.0 is that day — the headers carry
`CYBER_ABI_VERSION_MAJOR`/`_MINOR`, and `cyber_abi_check` applies the
compatibility rule rather than leaving a caller to compare two numbers by hand.
The release guard stays beside it: the ABI check says the library can serve
this build, the release number says the submodule is the build we pin, and a
library can satisfy the first and still be the wrong build.

## What Changes

- **`EXPECTED_ABI` to 0.116**, by hand, held against the linked engine by
  `version_is_the_pinned_engine`. The container minor does not move.
- **`clay_move_params.gesture_id` is now carried by both Move doors**, so a
  named grab folds only into a grab of the same name and a second press at one
  anchor is a second pull. `move_gesture_identity.rs` asserts the property both
  doors share rather than holding one of them behind a pin.
- **The topological move keeps its feather**, so a drag it makes on a field
  leaves no lattice. `visual_field_stroke_quality.rs` is the record of it.
- **`crates/cyberremesh/src/version.rs` asserts the declared ABI** through
  `cyber_abi_check`, beside the release assertion it already made, and
  `tests/guards.rs` loses the note that said no ABI number existed.

## What this change does *not* claim

It does not take up what the release enables and nobody has adopted. The
five retopology groups that are routed and dispatched in
`catalogue/actions.rs` and never declared in `GROUPS` — so no tool is offered
and the dispatch is unreachable — are #146, not this. Against a library of 235
entry points this workspace reaches 33.
