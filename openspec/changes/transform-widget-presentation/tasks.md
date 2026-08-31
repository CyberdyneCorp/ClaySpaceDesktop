# Tasks

## 1. The cage's size

- [x] 1.1 Establish the inconsistency: `object_gizmo_reach` is a share of the
      camera's distance and the cage's reach is a share of the cage
- [x] 1.2 Floor the cage's reach with the same screen-constant share, letting
      it grow past that on a large cage

## 2. The readout

- [x] 2.1 Draw it over the viewport, in the lower-leading corner, translucent
- [x] 2.2 Position, rotation, the axis it turns about, and scale — which is
      what the engine's transforms actually take
- [x] 2.3 Show it for a placed object alone
- [x] 2.4 Put the unit on the heading rather than after each of three numbers:
      "0.1 mm 0.0 mm -0.0 mm" ran off the card, and three copies of one word
      is not three pieces of information
- [x] 2.5 Suppress a signed zero — a position of -0.0032 mm read as "-0.0"

## 3. Keeping the two frames in step

- [x] 3.1 Draw the readout in the visual harness's copy of the frame as well
- [x] 3.2 Measure the viewport rect there the way the composition root measures
      it — after the viewport bar rather than before. Taken before, the rect
      included the bar's strip and the readout was drawn across the view presets

## 4. The ratchet

- [x] 4.1 The readout needs the unit symbol, which is a domain `label()`. Give
      the status bar and the readout one helper to share rather than a second
      call site — an SI symbol has no translation to move into `Strings`, and
      the count stays at nine

## 5. Tests

- [x] 5.1 The readout is drawn for a placed object, inside the viewport, and
      not for a whole layer or with no manipulator up

## 6. Verification

- [x] 6.1 Look at it
- [x] 6.2 `just check` — fmt, layering, clippy, spec, packaging and the
      workspace suite all pass
