# Pinch on a field is the surface magnify

One cell of the SDF column was wrong.

**`Pinçar` had no field verb at all.** The engine's radial scale is
`CLAY_DEFORM_MAGNIFY`, and it is per *item* and in that item's local frame: put
one on a picked item of a form that is several smooth-unioned pieces and it
scales that piece's field and leaves the others, so the surface gathers on one
side of the blend and not the other. Nothing errors — it comes out wrong. Grab
had an assembled-surface resolver for exactly that reason and magnify did not,
so Pinch could not be a surface brush on a field (ClayCore #391), and the row
read `sdf: None`.

Nor is there a stroke op that gathers. Relief and incise move the surface along
its own normal, which is a different mark: shaping the profile of a ridge
cannot make it a pinch.

`clay_layer_magnify_surface` is the entry point that was missing, and it has
been in the pinned engine all along, wrapped nowhere and called nowhere. It
resolves the region against every item it reaches, maps it across every image a
layer mirror emits with the strength crossing untouched, is one undo step
however many items it warped, and its `strength` is **signed**: positive swells
away from the centre, negative gathers toward. Pinch is the negative half.

## Why Inflate stays on Relief

The positive half is not wanted, and ClayCore v0.120.0 is why. Its release
notes (#615 and #618 — documentation and one test, no ABI and no kernel change)
measure the two brush frames against the relief surface, as each
frame-isolated reference's distance in fractions of the amplitude `k`:

| fixture | inflate reference | draw reference |
|---|---:|---:|
| sphere, convex | 0.000 | 0.017 |
| torus inner equator, saddle | 0.000 | 0.077 |
| bowl, concave | 0.000 | 0.027 |
| thin fin, ridge | 0.001 | 0.568 |

> **Relief IS Inflate**, to the precision of the measurement. [...] Standard is
> only approximated by it.

Relief offsets the accumulated field, and offsetting a distance moves each
point of the isosurface along the field's own gradient — each along *its own*
normal, which is the Inflate frame. The engine's Draw preset displaces along
one averaged normal per stamp.

So on a field `Inflar` is already the faithful tool and `Padrão` is the
approximation. Moving `Inflar` onto a radial scale would replace a correct
Inflate with something that is not one, and the test suite said so four times
over while that binding stood — the last of them
`ordinary_releases_compact_exactly_without_meshing_again`, because a magnify is
a warp where an ordinary release compacts a stamp.

What is owed instead is that the approximation be *stated*: `Padrão` on a field
gets a tool note, and a test measures what the note claims.

## What changes

- **The wrapper binds it.** `magnify_surface` and `magnify_surface_preview` go
  into `crates/claycore`, beside `move_surface`, which is their exact
  counterpart and was already there. The positive strength is wrapped and
  tested there — it is the ABI's own shape, and a wrapper that could only
  subtract would be an odd thing to hand the next caller.
- **`Pinçar` on a field is that call at a negative strength.** Radius from the
  brush, easing from the drag falloff — a magnify is a region deformation
  rather than a stamp — and one dab per step of the brush's spacing along the
  stroke.
- **The region and the strength are measured rather than chosen**, against the
  brushes they stand beside: a region 1.35× the brush at 0.5 strength leaves
  the line under the stroke standing proud and the flanks falling away, a mark
  a twentieth of the ridge `Padrão` draws with the same brush. Pinch sharpens
  an edge that is there rather than making one.
- **The dab stands on the surface** where the gesture's raycast put it. A
  radial scale fixes its own centre, and a gather about a point *on* the
  surface draws the material toward the stroke — which is what pinching is.
  Sunk into the material it stops gathering and deflates uniformly instead.
- **The invert key spreads**, which is the gather's own opposite and the pair
  the grid's column already names for this tool.
- **The mask is honoured by the stroke.** `clay_magnify_params` carries no
  gate, so frozen samples are dropped from the path before any dab is placed —
  the same rule the snakehook already applies for the same reason.
- **The gesture is one undo step.** The engine makes each call one step; the
  stroke wraps its dabs in a group, so a pass across the form is one Cmd+Z.
- **`Inflar` and `Padrão` stay on Relief**, and `Padrão` on a field carries a
  note saying what that costs: each point moves along its own normal, so a
  feature narrower than the brush thickens instead of taking the mark — a few
  percent of the amplitude on a smooth form, the whole of it on a fin.

## What this deliberately does not do

**It does not give the gesture a live transaction.** Pinçar opens none, so the
whole stroke is delivered when the pointer comes up, exactly as Camada, Argila
and Vinco are. `clay_layer_magnify_surface` is already idempotent over a
gesture — it takes a total and replaces its own last frame — so a live preview
is reachable later without changing what is written.

**It does not draw the preview region in the viewport.**
`magnify_surface_preview` is wrapped and tested, which is the state
`move_surface_preview` has been in since it was bound; nothing in the interface
draws either yet, and drawing one alone would be an inconsistency rather than a
feature.

**It does not consolidate.** A dab is a warp, and a warp is evaluated per
sample for as long as it is in the edit list, so a pass of Pinçar costs the
layer for good — the same accumulation Move has and the same remedy,
`clay_layer_consolidate`.

**It does not build a faithful SDF Standard.** The engine measured one and does
not ship it: the exact draw frame is a warp of the accumulation, which is not
pointwise in the accumulated field and is what makes relief cheap. Spelled with
the verbs that exist it is one per-item warp per dab — 76.2 ms against relief's
8.8 ms over a 30-dab stroke on a 24-item blockout. The note says where the
approximation shows; the remedy it offers a sculptor is a smaller brush.
