# Move the engine pin to ClayCore v0.126.0

## Why

The pinned engine is v0.120.1. v0.126.0 covers 0.121.0 through 0.126.0 and
none of the six was tagged on its own. It carries the answers to five issues
this repository filed, each of which left an open issue here waiting on
exactly that engine change:

| ClayCore | answers here | what it fixes |
|---|---|---|
| #655 (a cage drag costs what was dragged) | #176 | a 32³ mesh cage preview frame, ~1.7 s → ~11 ms |
| #660 → #667, #691 (mask extrude reaches its thickness) | #178 | a grid's extruded wall is no longer capped at the paint depth |
| #663 → #669 (a Move on the mirror plane is applied once) | #170 | `a_move_on_the_plane_is_applied_once` can stop being ignored |
| #664 → #673 (an item carries its own mirror axes) | #170 | turning symmetry off no longer rewrites what was made under it |
| #666 → #676 (an intersect drag is padded by what follows its operand) | #282 | the intersect frame's delta box stops scaling with the layer |
| #658 → #682 (a voxel grid can be cloned and bulk-read) | #285 | grid-to-field can leave the interface thread without a per-cell FFI call |

**The pin moves with no source change to build.** Thirty-two symbols added,
zero removed, no existing struct re-laid out or appended to. The whole
workspace compiled against v0.126.0 before a line of this repository was
touched.

**Two constants move by hand.** `EXPECTED_ABI` to 0.126, held against the
linked engine by `version_is_the_pinned_engine`. And `Document::FORMAT` to
minor 20: #673 adds one byte per node record, the item's own mirror axes, and
a default save now writes minor 20. The ABI still writes at the engine's
current minor and no other — `clay_document_save` takes a path and nothing
else — so the constant follows it, as it has through 7, 8, 11, 14, 15, 16, 17
and 19. A document this build writes is refused by a build on the previous
pin rather than misread, which is the direction the format was designed to
fail in; a minor-19 document opens here, re-encoded once at load so a crash
journal can pair with it (#675), at 3.61 ms against 1.41 on a 1.58 MB grid.

## What moves under us, measured rather than assumed

**One tripwire fired, the one armed for this.**
`a_mesh_cage_evaluation_is_priced_by_every_point_the_cage_holds` asserted that
a 32³ cage with one corner dragged costs more than 20x a 3³ one, because
`clay_mesh_lattice_displacement` summed every control point. Against
v0.126.0 the pair reads 0.026 ms against 0.224 ms over 2,048 evaluations — 9x,
about the ratio of the divisions, which is the O(n) per-axis basis #655 builds.
It is flipped to assert the ratio stays *under* 20 and renamed for what it now
holds. The budget it was reported beside, `cage.drag_32`, is enforced by
`price-a-cage-drag-by-its-dragged-points`, not here.

**The rest of the suite passes unchanged**, which was run rather than
reasoned about. The release notes name six entry points that answer
differently to a caller who recompiles nothing, and each was checked against
how this application calls it:

- *Mask extrude reaches the requested thickness* (#667, #691). On a **field**
  layer this application already hands the engine the painted patch swept
  along the surface normal, so its wall was as thick as asked before the pin
  and `mask_extrude_thickness.rs` still measures 0.05, 0.1 and 0.6 within
  0.0001. On a **grid** the wall was capped and is documented so; the engine
  now grows it along estimated normals for the requested layers. Measuring
  the grid wall, and deciding whether the field sweep is still needed, is
  #178's remaining work.
- *A Move on a mirror plane is applied once* (#669). `move_mirror.rs` keeps
  `a_move_on_the_plane_is_applied_once` ignored under the engine issue's name;
  un-ignoring it is `symmetry-that-mirrors-what-you-make` task 5.2.
- *The mesh frame verbs move along an area-weighted normal* (#677): Draw,
  Flatten, Clay, Crease, Scrape and Layer on the fixed, hierarchy and adaptive
  surfaces. Nothing here pins a frame verb's direction on an asymmetric
  triangulation, and every mesh brush test passes unchanged.
- *A topological move longer than about half its reach is sub-stepped* (#681).
  This application's Mover Topológico tests pass unchanged; a one-slice drag
  is bit-identical by the engine's own claim.
- *Consolidating a voxel or mesh layer is `Unsupported`* (#684), where it was
  `InvalidArgument` "nothing to consolidate". The application already refuses
  Optimize on those representations with its own reason before the engine is
  asked, so the new kind is not reached from the interface.
- *`clay_voxel_to_layer` is one undo step* (#679), where one undo left an
  empty layer. `conversion.rs` passes unchanged.

## What Changes

- **`EXPECTED_ABI` to 0.126 and `Document::FORMAT` to minor 20**, both by hand.
- **The cage tripwire flips** to hold the engine to pricing a cage by its
  dragged points.
- **`README.md` and `docs/roadmap.md`** carry the pin, the symbol diff, the
  format minor, and the v0.126.0 surface this application does not call yet.

## What this change does *not* take up

Each of these is its own change, and the line between a pin move and what the
pin enables is kept so a bisect over the upgrade lands on the upgrade:

- **The stroke session** (`clay_stroke_tx_*`, ABI 0.126.0) and its six
  consumers: a gesture fed in pieces, bit-identical to the whole-path call,
  one undo step. This application resolves its own stroke and stamps per
  segment; adopting the session is a change to how a stroke is driven.
- **The multires delta** (`clay_multires_delta_*`, ABI 0.125.0): a hierarchy
  gesture in a host undo stack, in place of a whole-hierarchy snapshot per
  gesture.
- **An item's own mirror axes** (`clay_item_set_mirror_axes`,
  `clay_layer_set_node_mirror`, ABI 0.121.0): `symmetry-that-mirrors-what-you-make`
  task 5.1, #170.
- **The voxel grid clone and bulk read** (`clay_voxel_grid_clone`,
  `clay_voxel_get_occupied`, ABI 0.123.0): the grid-to-field crossing's
  per-cell read, #285.
- **A mesh from arrays** (`clay_mesh_from_arrays`, ABI 0.124.0): the
  vertex-aligned OBJ text `claycore::mesh` writes to keep a host's uvs.
- **`clay_item_volume_move_topological_from`** (ABI 0.122.0), which samples a
  document directly so an outward pull is not clipped.
