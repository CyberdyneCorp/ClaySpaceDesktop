# Move the engine pin to ClayCore v0.78.0

## Why

The pinned engine is v0.73.0. v0.78.0 is five minor versions ahead in one tag —
0.74.0 through 0.78.0 — and its theme is the one this application has been
running out of room in: *a surface is something a tablet can hold*. It brings a
third representation the engine did not have, one transport for asking what
changed since the last frame, a memory vocabulary that says which part of a
document a sculptor is allowed to let go of, and a between-strokes seam for the
work that makes the next stroke cheaper. Two of the things it fixes are
behaviours this repository had measured and written down as standing facts, and
one of those was held open by a test that fired the moment the pin moved.

**The pin moves cleanly, and that is a measurement rather than a hope.** The C
ABI gained 146 entry points with nothing removed, no signature changed and no
struct re-laid out. Three descriptors grew — `clay_mesh_brush_desc` by
`stamp_azimuth` and `seed_revision`, `clay_mesh_hit` by `seed_revision`, and
`clay_memory_report` by nine fields for the surface tier and its roll-ups — all
by appending behind the `struct_size` they already negotiate, and this workspace
writes that size from `size_of` of the compiled type rather than by hand, so the
growth is absorbed without a line changing. The whole workspace compiled against
v0.78.0 with **no source change at all**: the pin move is one commit touching one
submodule pointer, and `EXPECTED_ABI` moves by hand in the commit after it
because deriving it from the linked engine would make the check assert that a
number equals itself.

**Two formats do move, and only one of them is reachable from here.** The scene
and `.clayspace` formats go to minor 16 for a layer's per-axis scale; the brush
preset format goes to version 2 for the stamp azimuth it was one field short of.
What this build writes, and why, is below.

**A mask, a rebuild and now a hierarchy.** The headline of this pin is that
*Multirresolução* is a fourth representation in this application: a Catmull-Clark
cage, deterministic subdivision over it, and per-level detail stored as
coefficients in a transported local frame, so a change to the form underneath
leaves the wrinkles on it and attached. That is the largest thing here by a wide
margin, it is specified by three changes of its own, and its integration cost is
a decision this proposal has to record rather than leave to be found: **a
`.clayspace` does not carry a hierarchy**, and the release notes say so in their
own known-limits section.

**A tripwire fired, and it was the right one.** See *Which tripwires fired*.

### The formats, and what this build decided to write

**`.clayspace` and the scene go to minor 16, and this build writes 16.** Item 1
of the upgrade notes tells a host that exchanges documents with an older build to
write at minor 15 instead, where the per-axis field degrades to the identity
triple and the document opens rather than being refused. **That advice is not
reachable across this ABI.** The `minor` parameter exists on the C++
`scene::serialize_document`; it is not on `io::save_clayspace`, which has no such
parameter at all, and it is not on `clay_document_save`, which takes a path and
nothing else. So a document written through this ABI is written at the engine's
current minor whatever the host would have preferred — which was already true on
the pin-move commit, before a line of this branch's code was written.

It is also the choice this workspace would have made. It has followed the
engine's minor through 7, 8, 11, 14 and 15, each the same shape, and it exchanges
documents with no older build: the format is the engine's, the engine is vendored
and pinned here, and a `.clayspace` this application writes is opened by this
application. What minor 16 costs is that a document written now is *refused* by a
build predating v0.78.0 rather than misread, which is the direction the format
was designed to fail in. The consequence is made visible rather than left
implicit: `claycore::Document::FORMAT` carries the number with the reasoning,
`Document::format_of` reads the eight-byte container header so the constant is
checked against a file this build actually wrote, a ratchet parses
`kClaySpaceMinor` and `kSceneMinor` out of the pinned engine's own headers and
fails if either moves past it, and the minor reaches the diagnostics report so
that "it will not open on the other machine" has a figure to quote.

**Brush presets go to version 2, and this repository has none.** Nothing here
binds `clay_brush_preset_serialize`, `_deserialize` or `_version`; the session
directory holds layout, favourites, references, recents and locale, and no brush.
So item 2 of the upgrade notes — a shared preset library needing both ends
upgraded, and any preset saved with a turned chisel between the azimuth landing
in the ABI and this release needing a re-check — costs this repository nothing.
What this repository does have is the field the version bump exists for, and it
is now a control: see *The grain*.

### What was taken up, and against which numbered item

The release's own upgrade checklist is seven items. Six of them are answered
here, and the answer to two of them is *nothing to do, and here is the evidence*.

| Item | Answer |
|---|---|
| 1 — write at minor 15 if you exchange documents | Not reachable across the C ABI; this build writes 16 and names it. |
| 2 — brush presets are version 2 | No preset persistence exists here. `stamp_azimuth` is adopted as a control instead. |
| 3 — automask on the adaptive sculptor now takes effect | No adaptive surface, and no automask factor was ever set. Named rather than left silent. |
| 4 — a short buffer from a dirty-chunk drain is `BUFFER_TOO_SMALL` | No drain loop here branched on the code. The retry is now *taken* rather than offered. |
| 5 — decide where a hierarchy's bytes live, and fill the ledger | Both done: a side-car beside the document, and a host-merged ledger. |
| 6 — the maintenance queue and deferred normals | Both wired, with the flush made structural. |
| 7 — pass the `seed_revision` beside a `seed_class` | Both halves carried at once, because the class alone is the defect. |

**Item 5 — the hierarchy's bytes.** A hierarchy is an owning handle beside the
document, not a layer in it, and `clay_multires_serialize` is the whole of the
seam. The document a `.clayspace` saves is the *cage*. So this application writes
`<path>.multires` beside it, in the shape `objects.rs` established for the object
table — a one-line version header, hand-rolled text, growth only at the tail —
and **breaks that precedent in one place deliberately**: the object table's write
failure is printed and swallowed, because losing bookkeeping is not losing the
work; here the hierarchy *is* the work, so a failed side-car write fails the save.
One file rather than the directory the survey specified, because
`clay_multires_serialize` was measured at 1.39 ms on the fixture and a
per-layer checksum skip would have saved a millisecond on a two-minute autosave
clock at the cost of a tree that Save-As must copy and every removal must prune.

Item 5's second half is the ledger. `clay_document_memory` reports the surface
tier as zero — correctly, since it cannot walk what it does not own — and
measured on the starting form crossed to a mesh and dabbed once, the plain
roll-up says **8,463,808 bytes** while the sculpting session beside it is another
**8,446,536**. A host stopping at the plain figure publishes half the truth on a
trivial document. So each held session is asked what it costs, the ledgers are
merged by the only party that knows which sessions belong to this document, and
`clay_document_memory_with_surfaces` is what answers.

**Item 6 — between strokes, and the normals.** The maintenance queue is drained
against an 8 ms budget at every gesture end, and what it services is a real decay
this repository had: `MeshSculptor::refresh` — the ray-tree rebuild — was wrapped
and called nowhere while `refit` was called at four sites, and
`clay_mesh_sculptor_quality`, the number that says a tree wants rebuilding, was
computed and thrown away. The deferred-normals half is wired on both switches,
because they cover different verbs: fifteen of the sixteen mesh verbs go through
the stroke resolver, which carries its own deferral and settles at the end of the
call it drove, while Mover is a bare stamp per mirror and reads the sculptor's own
flag, whose flush is the host's. Nothing flushes on its own, so the flush is
structural rather than written at the end of each path that ends a stroke: the
record the stamps are noted into and the sculptor that owes them are held as one
value whose disposal recomputes. Nine cases, one per exit — committed, cancelled,
tool changed mid-drag, subtool changed mid-drag, undone mid-drag, document dropped
mid-drag, and the same under a cage preview.

**Item 7 — the seed.** The premise of the upgrade note is that a host is already
passing a `seed_class`. This one was not: `MeshStamp::as_raw` wrote
`CLAY_MESH_NO_CLASS` and every stamp paid the linear class scan the header calls
"the wrong thing to do per stamp on a large mesh". So both halves are carried at
once, which is the only safe order — the class alone is precisely the defect the
token exists to catch. A gate the notes do not mention was added on top of it and
is load-bearing: `geodesic_region` opens with `if (seed_d > radius) return;`, so a
perfectly valid seed handed to a stamp that has travelled past its own radius
loses the dab exactly the way a stale one does. The seed is therefore refused for
a mirrored copy, for a path that leaves the picked point's reach, and for any
preset whose stamps can shrink below the radius it was measured against.

### The grain

`stamp_azimuth` is the field the preset format bumped for, and this application
had a live symptom of its absence: every alpha stamp on a mesh was handed a fixed
world-X tangent, so a directional alpha landed the same way whichever way the
stroke ran — the chisel was literally unturned. It is now **Grão**, a brush
setting in degrees beside Ruído, taking the same path every other brush parameter
takes. A whole turn comes back to none rather than stopping at the end of its
travel, because an angle has no ends, and a NaN or an infinity becomes zero
because the engine builds a rotation basis out of it.

## Which tripwires fired

**One, and it is the one that should have.**
`claycore/tests/placed_objects.rs::an_intersecting_object_has_no_finite_bound`
asserted that an ordinary cube placed with `Op::Intersect` answers
`Influence::Everything`, which the wrapper's own documentation called "a normal
path rather than an edge case". On v0.78.0 it returns a finite box —
`[-1,-1,-1]..[1,1,1]`, the *layer's* own box and not the cube's. It was the only
failing test in the workspace on the pin move; `clayspace-engine` passed in full.
That is #319, and the test is turned around in place following `mask_gate.rs`:
the history stays in the doc comment, the issue is named, and the assertion now
holds the finite bound.

The sharper consequence is one the release notes do not state and this repository
can: `engine_op` maps all fourteen `Combine` values and emits no
`Op::Transition*`, all fourteen `Shape` variants are bounded, and there is no
infinite grid repeat here — so **after #319 no operation this application can
produce reaches `Influence::Everything` any more**, and the `Everything` arms in
`node_bound` and `refill_what_a_step_reached` are dead in practice. They stay,
because they also absorb an error, but the prose that claimed intersect reached
them is corrected in all four places that said it.

**Nine were re-checked and left standing**, each two ways: the issue number does
not appear anywhere in the release notes, and no entry point answering it is among
the 146 added. #392 (a stroke does not resolve its template's deformer chain into
each stamp), the mesh-layer geometry revision sitting still through history, #379
(a smooth commit consolidates the layer), #67 (`clay_item_volume_relax` through a
corrugating round trip), #321 (no live boolean between subtools), #364 (no
instancing), the `MAX_JITTER` ceiling, a mip level refusing gradient normals, and
a mesh having no field to extrude a mask from. Three of those needed more than a
date stamp and now say why in their own comments:

- **The release's alpha bullet is a different defect from #392.** "A stroke still
  duplicates its alpha's samples per stamp" is about memory, on a stroke that
  carries its alpha correctly. #392 is a stroke that does not carry it at all.
  Letting the two collapse into one sentence is how a fix gets claimed for an open
  limit.
- **The device-gradient work (#426/#243) is not what `claycore_lod.rs` holds.**
  That is `clay_eval_gradients` answering on the selected backend; this is a mip
  level *refusing* to compute gradient normals, through a mesher that takes no
  backend argument at all.
- **`a_placed_node_reports_its_primitive_and_nothing_else` is a tripwire that
  cannot fire**, and the re-check established that rather than assuming it.
  `clay_layer_node_transform` and its three siblings are declared in the pinned
  header and generated into `claycore-sys`; they landed at the 0.60.0 pin and are
  simply not wrapped. The object side-car beside every saved document is a wrapper
  nobody wrote, not an ABI that cannot answer. #373 adds *layer* readback, which
  is a different granularity and does not retire it.

**Two new tripwires were written for limits the release states about itself**, so
that the next release is measured against them rather than against a paragraph:
`multires_document.rs` holds that a dab on a hierarchy does not reach the document
its cage came from — the same document saved either side of the dab is **812 bytes
both times, byte for byte identical**, while the dab takes the finest level's
relief from 0.000 to 0.883 and the hierarchy's own blob to 13,128 bytes — and
`mesh_automask.rs` holds that two of the five automask factors are declared and
inert: **62,576 vertices reached with no automask, 62,576 with cavity at full
strength, 62,576 with surface-group, 62,576 with both, every position identical to
the last bit**, with three controls beside them so the equality cannot pass by
automasking having stopped working altogether.

## What changes

- The submodule pin, `EXPECTED_ABI`, and the documentation that states the engine
  version.
- **The wrapper crate grows by five modules and about twelve thousand lines**:
  `multires.rs`, `memory.rs`, `surface_view.rs`, `maintenance.rs`, and the
  additions to `mesh_sculpt.rs`, `document.rs`, `brick.rs`, `remesh.rs` and
  `error.rs`. **131 of the 146 new entry points are called from it.** The fifteen
  that are not are named in the module doc they belong to: ten take a
  `clay_dynamic_sculptor` or `clay_dynamic_surface`, which this workspace does not
  hold; three are mesh-sculptor telemetry nothing reads; two are
  `clay_multires_project`. Every wrapper is executed by a test, `abi_surface.rs`
  being where the ones nothing above calls yet are run.
- **Every result code the header declares arrives as a kind that means it.**
  `CLAY_ERROR_CANCELLED` was unmapped and came back as `Unknown(9)`. A ratchet
  parses `typedef enum clay_result` out of the pinned header and fails when it
  declares a code the table does not name.
- **A fourth representation**, specified by `a-hierarchy-the-domain-can-describe`,
  `a-hierarchy-that-is-sculpted-and-saved` and `a-stack-of-passes-on-a-hierarchy`.
- **A live smooth is drawn beside the rest of the scene** (#378), specified by
  `a-preview-that-holds-the-whole-scene`, and **the memory report says which
  part**, specified by `memory-that-says-which-part`.
- **A whole subtool stretches per axis** (#373), specified by
  `a-subtool-stretches-per-axis`, which also names the format minor this build
  writes.
- **The maintenance queue is drained between two strokes**, specified by
  `maintenance-between-strokes`.
- **A mesh segment recomputes its normals once**, a stamp is told which class
  space its pick was made in, and **Grão** turns a stamp about its own facing —
  the three items in this proposal's own delta specs, together with the two
  diagnostics figures the seed needs to be watchable and the benchmark harness
  recording the spread a figure was reduced from.

## What does not change

**The adaptive surface is still not adopted.** Ten of the fifteen unwrapped entry
points take one, four of the five preflights price operations on one, the memory
pins hold one resident, and `clay_surface_view_from_dynamic` transports one. That
is the same deferral `upgrade-engine-0-73-0` made, for the same reason: it is its
own change with its own measurements.

**`clay_multires_project` is not wrapped**, and the sculpt-layer stack does not
carry a stroke into the *base* detail — a stroke goes into the form under the
passes. **Exporting a hierarchy exports its cage**, because `mesh_combined`
reaches the layer and the layer holds the cage; keeping them in step means a
wholesale replacement per gesture or per save. **A hierarchy is refused as a
boolean operand** rather than routed through the mesh arm and quietly composing
against a form nobody can see.

**The benchmark baseline is deliberately not re-recorded.** It stands at engine
0.52.2, which is the wrong A side for this pin as well as the previous one, and
replacing it hides whatever regresses next. Twenty-three new figure keys landed
with this work — fourteen `multires.*`, five `normals.*`, four `maintenance.*` —
and every one of them exists on the B side only, because the A-side binary is
pinned and must not be rebuilt. They will read as `new` and cannot be A/B'd. The
A/B itself is a measurement phase of its own and is the one task in this change
left open.

**The work-class and QoS work (#428, #431) is unreachable.** `parallel::WorkClass`
does not cross the C ABI: there is no work, class, QoS or priority symbol among
the 146, and no `CLAY_WORK_*` in the header. An unclassified dispatch stays
UserInitiated, so nothing existing is wrong today — it is a missing capability
rather than a defect.

**The device-gradient work (#426, #243) is unobservable here.**
`clay_eval_gradients` takes a backend and the only gradient this application asks
for is `BrickMeshParams::gradient_normals` fed to the brick-cache mesher, which
takes none. The Linux baseline's active backend is CUDA, so the fixed path is not
even reached. Stated plainly, the way the 0.73.0 proposal stated plainly that the
alpha tripwire did not fire.

**#441 and #442 — the two cheaper culls — land below this repository's
measurement floor.** They are per-item constants measured upstream at 50,000
items; the reference scene is 8 strokes and the deepest fixture in the suite is
`tape`'s 96-edit document, both around a hundred items. Upstream's own device gate
found a median of 0.9949x across 298 shape-matched points. Seeing them would take
a deep-edit-list scene that no product surface corresponds to, and a new scene
member changes `conditions.scenes`, which is the first thing `compare::unlike`
refuses on.

## What we found that upstream has not

**`clay_multires_preflight_encode`'s `persistent_bytes` is not a ceiling on the
blob.** The header calls the preflight figures ceilings. Measured on the pinned
engine, the encode preflight reports **21,392 against a serialized blob of
25,448** on one fixture and **1,589,696 against 1,460,304** on another — under on
one and over on the other, with `authoritative_bytes: 0` in both. It is a budget
verdict and nothing may be sized from it. `clay_multires_serialize` sizes itself,
and the wrapper says so where a caller would reach for the other.

**Deferring a stroke's normals cost about fourteen per cent more than it saved,
in every regime tried.** On a 66,049-vertex sheet, one resolved stroke, the
deferred arm was slower at every stamp spacing from nine stamps per call to
sixty-three, and the ratio did not move: 1.14, 1.15, 1.14, 1.14. The
de-duplication is real and large — sixty-three stamps reduce to 8,522 unique
classes, so the flush recomputes a fifteenth of what the per-stamp path does — but
the *difference* grew at about 0.07 ms per stamp, which is the signature of a cost
paid per stamp rather than at the flush: the deferred list accumulates one entry
per class per stamp and is sorted at the end, so sixty-three stamps sort some
hundred and twenty thousand entries to arrive at eight thousand. One machine, one
mesh shape. It is recorded rather than smoothed over, and it means **this
release's item 6 cannot be claimed here as a win without a figure**: what the seam
is worth on this application is the structural flush, which is a correctness
mechanism, not the deferral, which on this measurement is not one.

**A hierarchy's seed token renumbers on every bind, not only on a rebuild.**
Measured, it reads 1, then 3 after one dab, then 4, 5, 6 and 7 as caches are
dropped, trimmed and levels rebound. This application binds a fresh sculptor per
segment, so no numbering outlives a pick and passing no seed is the correct answer
rather than a stub — but a host that caches one across frames has a hazard the
release notes describe for the *mesh* sculptor and not for this one.

**A hierarchy put back from its own bytes is a new handle whose revisions restart
at 1.** A dab, an undo and a redo walk the evaluated counter 1 → 3 → 1 → 1 — the
same number over two different surfaces — so a viewport keyed on the engine's
counter alone would show the redo as not having happened. What is held beside it
is a generation of this application's own.

**A hierarchy level's chunk table comes into being when the level is first
viewed.** A stamp made before that marks nothing, so a host that sculpts a level
it has never drawn sees an empty dirty set. Correct, and surprising; it cost four
failing tests to find and is now recorded in `surface_view.rs`'s module doc.

**A `MemoryLedger` must be accumulated onto the first surface's answer, not onto a
default one.** `merge` takes the minimum of the two category counts, so folding
into a zeroed ledger reports every category as unfilled however many surfaces went
in.

**`clay_multires_stroke_restore` does not refuse at level 0**, where clay.h says
it is refused because the cage has no pure subdivision to return to. This build
returns `Ok` with `moved_vertices: 0` — the same outcome by a different route.
Worth an upstream note; the test pins the observed behaviour at level 2 rather
than asserting a refusal that does not happen.

## The limits that stay

Stated here rather than discovered later. Four of these the release names as
unchanged from v0.73.0, and this repository can confirm three of them against its
own tests:

- **A `.clayspace` does not carry a hierarchy.** The release notes call this the
  largest integration cost in the release and they are right. The side-car above
  is this application's answer, and it has consequences: a document whose side-car
  is absent opens as the cage it demonstrably holds rather than as a hierarchy
  that has silently lost every level, and a side-car that is present and damaged is
  named in the diagnostics report — while one that is *absent altogether* names
  nothing and cannot, because nothing in a `.clayspace` distinguishes a document
  that never held a hierarchy.
- **The surface tier's memory figures are zero unless the host fills the ledger.**
  Answered here, and the count of sessions asked is reported beside the figure so
  that a zero surface tier reads as *there are none* rather than as *nobody asked*.
- **Two automask factors still do not cross the ABI.** Cavity and surface-group
  need callbacks a descriptor cannot carry. Unchanged from v0.73.0, and now held by
  `mesh_automask.rs` with the numbers above.
- **A stroke still duplicates its alpha's samples per stamp** — roughly 800 MB of
  blob for a 200-stamp stroke carrying a 1024×1024 alpha. Filed, not fixed there.
  Unchanged from v0.73.0. Not the same defect as #392, which is also still open.
- **`voxel_remesh` sharp mode is still experimental and not manifold at
  longest-axis 96.** Unchanged from v0.73.0, and the warning strings this
  application shows stay word for word.
- **`DynamicSurface::from_mesh` still refuses a raw marched mesh at its default
  weld epsilon.** This is why `Direction::MeshToMultires` is offered against a
  named fault rather than as a repair: `clay_multires_from_mesh` refuses rather
  than repairs, and most meshes this application can make today come from imports
  and from `voxel_remesh`, which is the textbook thing a Catmull-Clark cage should
  not be built from.
- **The multires stroke record does not cross the C ABI.** clay.h says it twice,
  unprompted, of `clay_multires_sculptor_apply_stroke` and of the layered stroke
  transaction; `commit` reports an entry count and nothing more. So a hierarchy's
  undo holds the surface's own serialized bytes — 1.39 ms to take and 8.15 ms to
  put back at level 4 over a 16×16 cage, bounded at 256 MB with the oldest dropped
  first. That is where `an-undo-costs-the-edit` is bent, and
  `a-hierarchy-that-is-sculpted-and-saved` says so in its own words.
- **`clay_layer_lattice_gizmo` returns no warps for a layer carrying a per-axis
  scale.** A stretched subtool is refused the deformation cage, in words, rather
  than being handed an empty one.
- **`clay_list_backends` reporting metal on a real iPad has still not been
  observed by upstream**, and CUDA-enabled wheels are still not shipped. Neither
  reaches this application, which builds the engine from source.
