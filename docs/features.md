# Features

What works today. Anything not listed here is not built yet — see
[roadmap.md](roadmap.md).

Four tools currently damage what they touch, and two controls do nothing.
Neither is a gap in this application: both are engine defects, filed, and
listed under *Known-degraded* at the end of this page. They are called out
there rather than quietly omitted, because a tool that is offered and does the
wrong thing is worse than one that is missing.

Every tool names the ClayCore entry point it invokes, so a binding can be
checked against the engine's own documentation without reading the
implementation. A tool with no engine counterpart is not offered.

## Sculpting tools

All fifteen are bound and each is covered by a before-and-after capture in
`target/visual/`.

| Tool | Engine verb | Layers | What it does |
|---|---|---|---|
| Padrão | `clay_layer_apply_stroke` with relief | both | Displaces the surface along its normal |
| Inflar | `clay_voxel_sculpt_inflate` / relief | both | Dilates; a negative amount erodes |
| Suavizar | `clay_item_volume_relax` / `clay_voxel_sculpt_smooth` | both | Relaxes the surface. Bakes on the field side |
| Mover | `clay_layer_move_surface` | SDF | Drags the assembled surface. Buds rather than stretches |
| Pinçar | `clay_voxel_sculpt_pinch` | voxel | Moves surface cells toward the brush centre |
| Raspar | `clay_voxel_sculpt_scrape` | voxel | Flattens and smooths from one snapshot |
| Planar | `clay_item_volume_flatten_from`, cut-only | SDF | Planes without filling, which keeps a facet crisp |
| Preencher | `clay_voxel_sculpt_fill_cavities` | voxel | Fills narrow pockets |
| Camada | `clay_layer_apply_stroke`, clamped | both | A stroke that does not build up on itself |
| Máscara | `clay_mask_apply_stroke` | both | Freezes a region against every verb. Invert, clear, expand, contract, smooth, bounded complement and extrude are in the Máscaras menu |
| Puxar | swept-sphere chain | SDF | Pulls a tendril out, tapering to its tip |
| Polir | `clay_item_volume_flatten_from`, cut-only | SDF | hPolish |
| Relaxar | `clay_item_volume_relax` | SDF | Relax as a brush |
| Nudge | `clay_voxel_sculpt_smudge` | voxel | Drags the surface skin, leaving the interior |
| Trim | `clay_cut_create` | SDF | A shape drawn on the frame, cutting through |

**Trim is not a stroke tool.** Its gesture is a shape drawn on the view frame,
not a drag across the surface, and the interface refuses a stroke for it rather
than doing something adjacent to what the label says.

A tool whose verb exists on one representation only reports itself
**unavailable with a reason** on layers that cannot accept it — it is never
offered and then silently inert.

## Brush controls

| Control | Maps to | Range |
|---|---|---|
| Intensidade | stroke strength | 0–1 |
| Tamanho | stroke radius, in document units | 0.005–1 |
| Fluxo | stamp spacing | 0.01–1 |
| Ruído | positional jitter | 0–1 |
| Borda | falloff: Dura, Linear, Suave, Gaussiana | — |
| Acumular | buildup against clamped accumulation | on/off |
| Suavização | lazy-mouse lag | 0–0.95 |

Settings are held **per tool**: switching away and back returns what you left,
not a default. Values are clamped to what the engine accepts rather than
producing an error you cannot act on.

Symmetry about X, Y and Z is applied through the layer's mirror, so both halves
belong to one operation and undo together.

## Viewport

- MatCap shading with five built-in materials, generated rather than shipped as
  assets. Vertex colours modulate the material where a mesh carries them.
- Orbit, pan, zoom, frame-all. Pitch is clamped short of the pole, where the
  view matrix degenerates.
- Four view presets — Perspectiva, Frontal, Lateral, Superior. The orthogonal
  three switch projection and **keep the framing**, so comparing front and side
  does not rezoom. Orbiting away from one stops claiming to be it.
- A ground grid and a symmetry-plane indicator, both excluded from export and
  both measurably dimmer than the sculpt.
- A navigation gizmo in the corner, at a fixed size, sharing the camera's
  rotation and nothing else.
- A brush cursor drawn *in the scene* rather than as a screen circle, so it
  shows the footprint the brush will cover. It clears when the pointer leaves
  the surface rather than hanging at an arbitrary depth.
- Device loss is recovered rather than fatal: the surface, renderer and buffers
  are rebuilt against the same window and nothing authored is lost.

## Interface

The regions from the design: a menu bar, a tool options bar, a left region with
the scene tree, layer stack and sculpt settings, a central viewport, a right
region with material, geometry and brush inspectors, a brush shelf, and a
status area with a memory meter, the active backend and the working unit.

The **accent marks the active brush and nothing else**. Layer selection is
indicated by surface tone and weight; a test asserts the accent's coverage
stays about constant as the active tool changes, so it cannot quietly spread.

Contrast floors are enforced as tests, not intentions: 4.5:1 for text, 3:1 for
indicators that carry state. Where the quiet-until-addressed rule would fall
below a floor, the floor wins.

Every user-facing string is externalised. Brazilian Portuguese is the design's
own language and the default; English is carried alongside, and an untranslated
system tag falls back rather than showing keys.

## Acceleration

Backends are discovered at runtime and ranked per platform:

| Platform | Order |
|---|---|
| macOS | `metal` → `cpu` |
| Linux | `cuda` → `vulkan` → `opencl` → `cpu` |

The CPU backend is always compiled in, so selection never fails for want of a
candidate. Where a backend declines an operation — OpenCL implements neither
raycast nor device meshing — the fallback is **per operation**: the selected
backend stays active for everything it does support, and the fallback is
recorded once rather than once per call.

Backend choice affects speed, never results.

## History

- Undo and redo over the engine's own vocabulary.
- A stroke of any length is **one** history entry, mirrored halves included.
- An edit that changed nothing adds no entry and does not mark the document
  modified. This matters because the engine documents several verbs as
  legitimately able to change nothing — a sub-cell drag, a stamp that misses
  every cell — so a successful call is not evidence that anything happened.
- Changing symmetry costs its own entry, because a layer's mirror is document
  state. It is written only when it actually differs, so a run of strokes at
  one setting costs nothing extra.

## Not built yet

Documents cannot be saved or opened from the interface, there is no export
dialog, panels cannot be resized or collapsed, shortcuts are fixed, and the
scene tree and layer stack read a fixture rather than the live document. See
[roadmap.md](roadmap.md).

## Deliberately absent

**Soft-body dynamics.** The design's *Dinâmica* panel shows gravity, rigidity
and damping. ClayCore has no solver and none is planned, so the panel ships as
voxel size and resolution levels — which is what "Dinâmica: Ligada" means
operationally, closer to ZBrush's Sculptris Pro than to a simulation. The
physics controls are not shipped disabled; they are not shipped.

**Mesh-surface brushes.** ClayCore sculpts fields and voxels. Mesh layers are
carried, saved and exported, never sculpted.

**Windows.** Out of scope for this change.


## Known-degraded

Offered, and not doing what they should. Each is an engine defect with a filed
issue and a test that fails when it is fixed.

| What | Effect | Issue |
|---|---|---|
| Suavizar, Relaxar, Planar, Polir | Corrugate the region they act on. The bake-and-replace round trip damages the surface before any verb is applied | [#67](https://github.com/CyberdyneCorp/ClayCore/issues/67) |
| Ruído | Inert. Clamped to zero, because jitter interacts badly with the cache's narrow band | [#67](https://github.com/CyberdyneCorp/ClayCore/issues/67) |
| Intensidade, on Add-based tools | No effect — a stroke at 0 deposits as much as one at 1. **Fixed upstream, awaiting a release** | [#61](https://github.com/CyberdyneCorp/ClayCore/issues/61) |
| Simetria | Off by default. The layer mirror had no observable effect. **Fixed upstream, awaiting a release** | [#60](https://github.com/CyberdyneCorp/ClayCore/issues/60) |
| Layer names after reopening | Lost, along with visibility and stack order. Ids are recovered by probing | [#69](https://github.com/CyberdyneCorp/ClayCore/issues/69) |
| Seams while dragging | A stroke leaves faint slivers until the pointer comes up, when a full re-mesh clears them. **Fixed upstream, awaiting a release** | [#66](https://github.com/CyberdyneCorp/ClayCore/issues/66) |
