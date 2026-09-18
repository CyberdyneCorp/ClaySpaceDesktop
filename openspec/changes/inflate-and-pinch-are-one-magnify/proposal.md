# Inflate and Pinch on a field are one magnify

Two cells of the SDF column were wrong.

**`Pinçar` had no field verb at all.** The engine's radial scale is
`CLAY_DEFORM_MAGNIFY`, and it is per *item* and in that item's local frame: put
one on a picked item of a form that is several smooth-unioned pieces and it
scales that piece's field and leaves the others, so the surface gathers on one
side of the blend and not the other. Nothing errors — it comes out wrong. Grab
had an assembled-surface resolver for exactly that reason and magnify did not,
so Pinch could not be a surface brush on a field (ClayCore #391), and the row
read `sdf: None`.

**`Inflar` shared `Padrão`'s verb.** Both applied `clay_layer_apply_stroke` in
the Relief family — relief moves the accumulated surface along its own normal,
which is what Standard *is* — so the two brushes were one verb wearing two
profiles: a region and rim 1.35× the brush at 0.32 of the lift, against the
standard clay mapping. Shaping a ridge cannot make it a swell, and the audit
measured the consequence: Inflar left a **taller** mark than Padrão, 75 px
against 50 over six reproductions (#179).

`clay_layer_magnify_surface` answers both, and has all along — it is in the
pinned engine, wrapped nowhere and called nowhere. It resolves the region
against every item it reaches, maps it across every image a layer mirror emits
with the strength crossing untouched, is one undo step however many items it
warped, and its `strength` is **signed**: positive swells away from the centre,
negative gathers toward. One entry point covers both verbs.

## What changes

- **The wrapper binds it.** `magnify_surface` and `magnify_surface_preview` go
  into `crates/claycore`, beside `move_surface`, which is their exact
  counterpart and was already there.
- **`Inflar` on a field is a positive strength and `Pinçar` a negative one.**
  Radius from the brush, easing from the drag falloff — a magnify is a region
  deformation like a drag rather than a stamp — and one dab per step of the
  brush's spacing along the stroke. `Padrão`'s relief binding is left where it
  is.
- **The region and the strength are measured rather than chosen**, against the
  brush they stand beside: a region 1.35× the brush at 0.5 strength peaks two
  fifths lower than Padrão's ridge and is still moving clay where the ridge has
  been flat for two readings. At the brush's own radius it is lower and no
  broader, which is a weaker brush; at 1.0 strength it is taller as well as
  wider, which is #179 again in the new binding.
- **Inflar's dabs are sunk half a radius into the material** along the field's
  own gradient, and Pinçar's are not. A radial scale fixes its own centre, and
  a gesture's samples are raycast hits — so a dab left where it lands is
  centred exactly where the verb has least to say. A swell wants clay all round
  its centre; a gather wants to stand on the surface and pull it in. Sink a
  pinch and it deflates instead of gathering.
- **The invert key gives each tool its own opposite**, not the other tool: an
  inverted Inflar deflates and an inverted Pinçar spreads, the depth being a
  property of the tool rather than of the sign. The same pair the grid's column
  already names for these two.
- **The mask is honoured by the stroke.** `clay_magnify_params` carries no
  gate, so frozen samples are dropped from the path before any dab is placed —
  the same rule the snakehook already applies for the same reason.
- **The gesture is one undo step.** The engine makes each call one step; the
  stroke wraps its dabs in a group, so a pass across the form is one Cmd+Z.
- **`Padrão` stays on Relief.** The point of the change is that Standard and
  Inflate stop being the same verb.
- **The canonical document stops stamping Inflar.** Sinking a dab means the
  file records a point the *field* answered rather than the point the gesture
  asked for, and the engine "pins no FP flags for its own translation units and
  makes no cross-build promise" — so that point is not the same float on
  arm64-macOS as on x86_64-Linux. The byte-identical check authored an Inflar
  and its digests parted. `Argila` stamps in its place — a second stamping
  verb, at the same asymmetric position — `clayspace_app::canonical` says which
  verbs the fixture may hold and why, and a test asserts every position the
  fixture stamps survives into the bytes verbatim, so the next verb that
  records what it computed is caught on one machine rather than by two legs of
  a matrix disagreeing.

## What this deliberately does not do

**It does not give the gesture a live transaction.** Neither brush opens one,
so the whole stroke is delivered when the pointer comes up, exactly as Camada,
Argila and Vinco are. `clay_layer_magnify_surface` is already idempotent over a
gesture — it takes a total and replaces its own last frame — so a live preview
is reachable later without changing what is written.

**It does not draw the preview region in the viewport.**
`magnify_surface_preview` is wrapped and tested, which is the state
`move_surface_preview` has been in since it was bound; nothing in the interface
draws either yet, and drawing one alone would be an inconsistency rather than a
feature.

**It does not consolidate.** A dab is a warp, and a warp is evaluated per
sample for as long as it is in the edit list, so a pass of Inflar costs the
layer for good — the same accumulation Move has and the same remedy,
`clay_layer_consolidate`.
