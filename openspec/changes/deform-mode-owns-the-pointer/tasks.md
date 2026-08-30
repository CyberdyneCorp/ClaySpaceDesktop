# Tasks

## 1. Move the picking where a test can reach it
- [x] 1.1 `clayspace_model::ray_hits_sphere` and `ray_hits_segment` — one piece
  of arithmetic for a control point, a curve point and a manipulator handle,
  with the capsule's clamp and its behind-the-eye rejection asserted
- [x] 1.2 `input::handle_under`, `input::toward_eye` and `GIZMO_GRAB` out of
  `main.rs`, which has no tests and can have none

## 2. The three faults
- [x] 2.1 The arrow is a capsule from the pivot to its tip, tested last so the
  centre, the scale box and the crossing rings keep their own presses
- [x] 2.2 `input::shows_the_brush_ring` — one rule for both modes that take the
  press away from the brush, and `App::cursors` asks it
- [x] 2.3 The selection box: `Camera::screen_through` as the exact inverse of
  `ray_through`, `input::screen_at`, `points_within`, `is_a_marquee` and
  `selection_from_marquee`
- [x] 2.4 `Command::SelectLatticePoints` and `LatticeModel::select_lattice_points`
  — a set at once, sorted and deduped, rather than a loop over the one-point
  call or the toggle
- [x] 2.5 The hover highlight: `App::gizmo_target` shared by the press and the
  highlight, `hovered_handle` per frame with the pointer up, and the drag's own
  handle kept lit while it runs
- [x] 2.6 `Drag::Marquee` through the press-order chain, before the object pick
  and the surface; `shell::selection_box` draws the band from the interface's
  own tokens

## 3. Hold it there
- [x] 3.1 `gizmo.rs` — the shaft is grabbable along its length, a press past
  the arrowhead is not, the hit reports how far along the ray it was, and
  nothing behind the eye is grabbable
- [x] 3.2 `input.rs` — an arrow grabbed through the ray the application builds,
  the particular handles keeping their presses, the ring off while a cage is
  up, and the box's catch, its slop and its add rule
- [x] 3.3 `camera.rs` — a screen position is the ray that goes back through it,
  on both projections, and nothing behind a perspective camera is on screen
- [x] 3.4 `visual_lattice.rs` — a hovered arrow is drawn differently from a
  cold one, which is what makes the highlight a highlight
- [x] 3.5 `clayspace-engine/tests/lattice.rs` — a set replaces what was held,
  ignores an index the cage does not have, and gives the manipulator a face
- [x] 3.6 `docs/features.md`
