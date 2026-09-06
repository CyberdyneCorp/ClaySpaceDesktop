# Tasks

## 1. Anchor the taper

- [x] 1.1 `ClayDocument::tendril_radius` derives a control point's radius from
      the distance travelled from the anchor, and `snakehook_stroke` calls it
      per sample instead of computing `index / (len - 1)` inline; verify
      `a_radius_depends_on_the_distance_and_not_on_the_stroke`
- [x] 1.2 `TAPER_SPAN` carries why it is five and not eight — a fatter tendril
      renders with specks of background through it, measured through
      `visual_holes` at spans 100, 8, 5 and 3 — so the next reader meets the
      artifact rather than a magic number
- [x] 1.2a The first reading of that artifact was **wrong and is corrected in
      place**: specks in the engine's own mesh as well as in ours was read as
      "then it is the mesher's", and reported upstream as such. All three
      pictures go through one rasteriser, so agreement across them rules out
      our incremental store and nothing below it. Measured topologically, the
      same document is watertight, 2-manifold and Euler characteristic 2 at
      resolutions 96, 128 and 192 — a sphere, where a pinhole is a tunnel and
      would give 0. What it carries is sub-pixel slivers, 150 at 96 rising to
      1342 at 192, which a rasteriser drops. The comment in `visual_holes` that
      licensed the bad inference is corrected too
- [x] 1.3 The radius floor is documented as a **precaution and not a measured
      fix**: it was added against the hypothesis that thin tendrils pinholed,
      the test failed again unchanged because at that stroke length the floor
      never engaged, and the measurement went the other way; verify
      `a_tendril_never_thins_past_what_the_grid_can_carry` and
      `a_brush_finer_than_the_grid_does_not_taper_at_all`
- [x] 1.4 The behavioural regression, measured through `clay_eval_points`
      rather than a pick, because a pick is answered by a marcher whose own
      behaviour changes between pins; verify
      `a_point_already_pulled_keeps_its_thickness`, confirmed to fail at
      0.02580 against a 5e-3 band with the old taper

## 2. Dirty what the segment changed

- [x] 2.1 `LiveHook` replaces the `(LayerId, NodeId)` pair and carries how many
      control points the engine was last given, which is what lets a segment
      name the part of the curve that moved
- [x] 2.2 `tendril_tail_bounds` returns the box the newest samples changed,
      three control points back because a Catmull-Rom segment is governed by
      four, grown by each point's radius and the stroke's blend
- [x] 2.3 `tendril_tail_regions` returns that box and every image the mirror
      places it at, **each as its own region** — a union spans the untouched
      middle, which measured 350 bricks against the 300 two separate images
      need; verify `a_mirrored_pull_dirties_both_halves_while_it_is_drawn`,
      confirmed to fail at 150/150 against the unioned version
- [x] 2.4 `world_bounds` crosses from the item's own space into world through
      all eight corners, since a rotated transform re-aligns the box; the
      dependence on the stroke node's transform being identity is recorded as
      **our** invariant rather than the engine's guarantee
- [x] 2.5 The grow path reports `self.dirty.len()` where it returned the
      constant `1`; verify the `grown > 1` arm of
      `growing_a_pull_dirties_the_new_end_and_not_the_whole_tendril`
- [x] 2.6 A segment that cannot name its changed region falls back to the
      node's own bound; reached when a mask freezes the newest samples

## 3. Hold it

- [x] 3.1 `growing_a_pull_dirties_the_new_end_and_not_the_whole_tendril`
      baselines against **the whole tendril** and not against the authoring
      call, which carries the starting form's own bricks and is large enough to
      hide the defect — the first draft of this test passed against a half-fix
      that kept the wide refill, and was corrected after that was checked
- [x] 3.2 `just check` — formatting, clippy, the workspace suite, the
      specification
- [x] 3.3 `docs/features.md` gains the taper rule and why the pull used to slow
      down, in the section that already explains why one gesture grows one curve

## 4. Say what is not ours and not taken

- [x] 4.1 The residual — mirrored time climbing while its brick count does not
      — is reported upstream with the measurements, and confirmed there as
      `ctape_stroke_dist` walking every segment per sample
- [x] 4.2 The chained-items workaround is **declined** rather than deferred: it
      became possible only once the taper was anchored, and it trades a few
      milliseconds against the brush's whole appearance
