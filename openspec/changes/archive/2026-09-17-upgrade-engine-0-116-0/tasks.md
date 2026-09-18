# Tasks

Every task below landed on main before this change was written — in #140 for the
engine pin and #147 for the retopology pin. The change records them rather than
proposing them, because the convention this repository follows is that a pin
move has a change, and these two did not.

## 1. Move the ClayCore pin

- [x] 1.1 Point `vendor/ClayCore` at v0.116.0 and move `EXPECTED_ABI` to 0.116
      by hand, which `version_is_the_pinned_engine` holds against the linked
      engine. It is deliberately not derived from that engine, which would make
      the check assert that a number equals itself
- [x] 1.2 Confirm the container minor does not move, so `Document::FORMAT`
      stays where it is and a document this build writes is readable by a build
      on the previous pin
- [x] 1.3 Build the whole workspace against the new engine before touching a
      line of it. The symbol diff is one addition and zero removals, and every
      struct that grew did so behind `struct_size`

## 2. Take up the two fixes that were ours to feel

- [x] 2.1 Retire the tripwire in `move_gesture_identity.rs` that held the live
      door behind a pin, and assert instead on the property both doors now
      share: a second press at one anchor at one brush size is a second grab,
      and the first pull is still there
- [x] 2.2 Record the feather's return in `visual_field_stroke_quality.rs`,
      which asserts on the property — no lattice in a drag on a field — rather
      than against a golden image, as every visual test in this workspace does

## 3. Move the retopology pin

- [x] 3.1 Point `vendor/CyberRemesherAndUV` at v0.9.0
- [x] 3.2 Assert the declared ABI in `crates/cyberremesh/src/version.rs`
      through `cyber_abi_check`, which applies the compatibility rule rather
      than comparing two numbers — the question is whether the library can
      serve a client compiled against *these* headers, not whether two numbers
      match
- [x] 3.3 Keep the release assertion beside it. The two answer different
      questions: the ABI check says the library can serve this build, the
      release number says the submodule is the build we pin
- [x] 3.4 Drop the note in `tests/guards.rs` that said no ABI number existed,
      which is no longer true of the engine

## 4. Write it down

- [x] 4.1 `README.md`: the pin, the symbol diff, and the two defects it carries
      that this workspace felt
- [x] 4.2 `docs/features.md`: the gesture-identity section, which said the live
      door could not carry the name
- [x] 4.3 This change, and `openspec validate --all --strict`
