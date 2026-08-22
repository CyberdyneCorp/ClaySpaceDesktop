## Context

See `proposal.md` — Why.

Three constraints shape everything below.

**The layering is enforced, not conventional.** `tools/check_layering.py` asserts
that `clayspace-view` cannot reach the engine and `clayspace-vm` cannot reach
`egui`, `wgpu` or `winit`. A representation's capabilities are engine facts and
the shelf is a View, so the table that connects them has to live in the Model
and travel outward as ViewModel state.

**Sculpting today is one path.** `SculptModel::apply_stroke(tool, brush, samples,
symmetry)` is the whole sculpting surface, and `ClayDocument` dispatches on
`ToolKind` internally. It reaches SDF and voxel layers because both consume a
resolved stroke. A mesh layer does too — `apply_to_mesh` is the engine's fourth
stroke consumer — but the things that are *not* strokes (deformers, the lattice
cage, repair, sculpt-layer strength) have no route through it at all.

**A mesh sculptor is a stateful object.** `clay_mesh_sculptor_create` builds
vertex adjacency once and the session keeps it; brushes then cost what the
falloff reaches rather than what the mesh holds. That is unlike the SDF and
voxel paths, where the document is the only state. Something has to own it and
know when it is stale.

## Goals / Non-Goals

**Goals:**
- One declared table saying which verb exists on which representation, with the
  shelf, the availability rules and the tests all reading it.
- A sculpting path that carries operations which are not strokes, without
  turning `apply_stroke` into a variant type.
- Conversions that are ordinary commands on the existing undo history.
- Each phase leaves the application shippable.

**Non-Goals:**
- Composing with a mesh layer. The engine's own position is that a mesh becomes
  an operand only by conversion, and paying that spends the retopology the
  import was for. The specs draw that line; this design does not try to move it.
- Retopology, remeshing or dynamic tessellation. The mesh brushes stretch what
  is there, which is the artist's signal, and the engine stops there too.
- A node graph for the SDF edit list. Deformers and combine operations become
  reachable; presenting the edit list as an editable graph is its own change.
- Changing the stroke engine's semantics. Spacing, pressure, jitter and taper
  are the engine's and stay shared across all four consumers.

## Decisions

### The capability table is data in the Model, not a match arm

`Representation` gains a declared table of the verbs it supports, and
`ToolKind::availability` is rewritten to consult it. The alternative — a `match`
per tool, as `tools.rs:240` does today with its blanket `Unavailable::MeshLayer`
— is what produced a refusal that outlived the fact it described. A table can be
asserted against the engine's own enums in a test; a match arm can only be read.

The table also answers "which tools does the shelf show", so the shelf and the
disabled state cannot disagree — they are the same lookup.

**Alternative considered:** asking the engine at runtime what a layer supports.
Rejected: the C ABI exposes no capability query, so it would mean trying an
operation to see whether it is refused, and a probe with side effects is not a
question.

### Sculpting grows a second verb, not a wider stroke

`SculptModel` gains `apply_operation(op)` beside `apply_stroke`, where an
operation is something a gesture cannot express: a deformer with its parameters,
a lattice edit, a repair pass, a sculpt-layer strength change, a conversion.

**Alternative considered:** widening `apply_stroke`'s tool parameter into an
enum carrying parameters. Rejected: it would make every caller and every double
handle cases that are not strokes, and the stroke path is the latency-critical
one — `gesture_end.rs` and `sculpt_latency.rs` measure it. Keeping it narrow
keeps those measurements about strokes.

### The mesh sculptor is owned by the layer and invalidated by the engine

`ClayDocument` holds a mesh sculptor per mesh layer, created on first use and
dropped when the layer's geometry is replaced from outside (import, undo across
a conversion, a reload). Its `refresh` and `refit` entry points are the cheap
paths; recreating is the expensive one and happens only when the vertex count
changes.

**Alternative considered:** creating a sculptor per stroke. Rejected on the
engine's own statement that adjacency is an O(vertices) build the session pays
once — per stroke it would be paid per stroke, which is precisely the "cost
follows the model rather than the edit" shape `performance-budgets` forbids.

### A conversion is a command that adds a layer

Conversions go through `Command` like everything else, so they land on the
existing undo history and the existing dispatch. Each produces a new layer.

**Alternative considered:** converting in place, which is what a "mode switch"
would imply. Rejected: the crossing is lossy in one direction and irreversible
in the other, and an in-place conversion makes undo the only way back — which
works until the session ends. A new layer makes the original the way back.

### The cost of a conversion is computed, not written down

The dialog states surface movement as half the chosen cell size, and the feature
size that vanishes as one cell, both recomputed as the user changes resolution.
Writing the numbers into strings would make them wrong the first time someone
changed the default.

### Phase order is shell, conversions, vocabulary

The shell is first because the other two have nowhere to appear without it: a
conversion needs somewhere to say what the active representation is, and the
vocabulary needs a shelf that can hold a representation's own verbs. Conversions
come before the vocabulary because mesh sculpting is worth much less without the
round trip that produces a mesh to sculpt.

### An alpha is a PNG, decoded in the engine bridge

The engine decodes no images — `clay_item_add_alpha` and
`clay_voxel_sculpt_carve_alpha` take samples — so the host decodes. **PNG only.**

The decoder lives in `clayspace-engine`, beside `import_mesh`, which already has
exactly this shape: a path in, an engine call out, the format checked before the
engine is asked. That keeps image decoding out of the View, out of the
ViewModels, and out of the pure domain crate, none of which should grow a file
format. `png` is currently a dev-dependency of `clayspace-app` used by the visual
harness to write captures; it becomes a real dependency of `clayspace-engine` to
read them.

**Alternative considered:** the image formats the exchange path already links.
Rejected — it links none. Mesh import hands a path to `clay_mesh_load` and the
engine reads the file, so there is no existing image decoder to reuse and every
additional format is a dependency bought for a brush. PNG carries 8- and
16-bit greyscale, which is what an alpha is; a sculptor with a JPEG can convert
it, and a second format can be added later without changing anything here.

### The sculpt layer stack lives in the existing layer panel

A voxel layer's sculpt layers are shown inside the layer panel, under the layer
they belong to, rather than in a panel of their own.

They are not a second kind of document object competing for space — they belong
to one voxel layer and are meaningless without it, so nesting them under it says
that and a separate panel would not. It also means the panel is already open
whenever they are relevant, which a panel of its own would not be.

**Alternative considered:** a panel of its own, as ZBrush gives them. Rejected
for this shell: `region::LEFT` and `region::RIGHT` are fixed widths and the panels
cannot yet be resized or collapsed, so a fifth panel would take space from the
four that are always needed to serve one that matters only on voxel layers.

## Risks / Trade-offs

**The shelf's contents change under the pointer when the active layer changes** →
The active tool is kept where the new representation has it and replaced with a
stated substitution where it does not, rather than silently resetting. Covered
by two scenarios in `representation-modes`.

**A mesh sculptor per layer is memory the document did not previously hold** →
Created on first sculpt rather than on import, dropped with the layer, and its
size reported in the layer cost panel that already exists for this purpose.

**Phase 3 is large enough to stall** → It is a list of independent verbs, not a
structure. Each verb is its own task against a table entry the shell already
reads, so the phase can be delivered a verb at a time and stopped anywhere.

**The engine's vocabulary will keep growing** → The capability table is asserted
against the engine's enums in a test, so a verb ClayCore adds and this
application has not taken up is visible as a failing count rather than as
silence. That test is the mechanism that would have caught the mesh refusal.

**Sculpting a mesh stretches it, and the application cannot fix that** → Stated
rather than prevented, per `mesh-sculpting`. The engine reports a quality figure
and the application surfaces it; retopology is out of scope by the engine's own
boundary.

## Migration Plan

Each phase ships behind nothing — there is no flag, because each phase is
coherent on its own.

Phase 1 changes what the shelf shows. The existing 15 tools keep their verbs and
their representations, so an SDF or voxel session is unchanged; what changes is
that a mesh layer stops showing 15 disabled entries.

Phase 2 adds commands and a panel. Nothing existing changes behaviour.

Phase 3 adds verbs. The one behavioural change outside it is that mesh layers
become pickable, which alters what a press on a mesh layer does: it sculpts
where it previously orbited. That is the point of the phase, and it arrives with
the brushes rather than before them.

Rollback for any phase is the commit. No document written by a later phase is
unreadable by an earlier one except a voxel layer carrying sculpt layers, which
is why the document format's handling of those is a task in phase 3 rather than
an assumption.
