# Tasks

## 1. The format minor

- [x] 1.1 Name the container version this build writes, with the reasoning for
      16 over 15 and the fact that 15 is not reachable across this ABI
- [x] 1.2 `Document::format_of`, so the constant is checked against a file this
      build actually wrote rather than asserted
- [x] 1.3 A ratchet against `kClaySpaceMinor` and `kSceneMinor` in the pinned
      engine's own headers, skipped where the vendored source is absent
- [x] 1.4 The number in the diagnostics report, so a refusal elsewhere has an
      answer a person can quote

## 2. The frame a layer places

- [x] 2.1 `Transform::into_world` / `into_local` stretch per axis
- [x] 2.2 Scale composes innermost, as the engine composes a layer — which the
      uniform case could not tell apart from the other order
- [x] 2.3 `Transform::largest_scale` for a world length carried inward, with
      the engine's dual rule as the reason

## 3. The engine half

- [x] 3.1 `write_layer_transform` uses the per-axis call for every transform
- [x] 3.2 `stand_subtool_at` too, so an insertion cannot unsquash a layer
- [x] 3.3 The five brush-radius divisions take the largest factor
- [x] 3.4 The cage's offset conversion divides component by component
- [x] 3.5 A boolean's mesh operand carries its layer's placement onto a node
      through the per-axis node setter
- [x] 3.6 A stretched subtool is refused the cage in words

## 4. Where a layer stands

- [x] 4.1 `from_file` reads each layer's placement back — the defect this fixes
      is that every reopened subtool was assumed to be at the origin
- [x] 4.2 `resync_layer_transforms` reads the engine rather than a snapshot
- [x] 4.3 `layer_states`, `remember_layers` and its six call sites go
- [x] 4.4 `Layer::transform` stays, as a cache, and says which it is

## 5. The interface

- [x] 5.1 `per_axis_scale()` is true on a layer target
- [x] 5.2 The visual capture of the subtool manipulator draws the boxes
- [x] 5.3 The object panel's scale hint stopped saying "scale is uniform" over
      three factors and a widget with three boxes on it

## 6. Tests

- [x] 6.1 A scale box on one axis stretches that axis of a whole subtool, and
      reaches the field
- [x] 6.2 The centre handle still takes all three
- [x] 6.3 A move does not unsquash what it moves
- [x] 6.4 A squashed subtool reopens squashed **and where it stood**, and the
      file says the minor this build claims to write
- [x] 6.5 Undo and redo of a stretch, with no host-side snapshot behind them
- [x] 6.6 The cage refuses a stretched subtool and names the stretch
- [x] 6.7 A squashed frame carries a point in and out of itself, and a stretch
      is along the frame's own axis rather than the world's

## 7. The record

- [x] 7.1 README and `docs/features.md` stop saying a whole subtool scales
      uniformly
- [x] 7.2 The specification's scenario is turned around rather than deleted
