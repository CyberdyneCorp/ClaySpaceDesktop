# A hierarchy the domain can describe

## Why

ClayCore 0.78.0 ships a third way to hold a surface: a cage, a subdivision
hierarchy over it, and detail stored per level **in a frame carried up from the
level below**. That last clause is the whole of it. Wrinkles cut at level 4 and
a jaw moved at level 1 are two edits to two different arrays, and moving the jaw
moves the frames the wrinkles are stored in, so the wrinkles ride on it instead
of being smeared or re-projected. A mesh cannot express that — a mesh has one
level and nothing under it to move — which is why this is a representation and
not a mode.

`crates/claycore` wraps all of it. Nothing above that knew it existed:
`Representation` had three members, `Verbs` had three fields, and every table,
locale array and exhaustive match in the workspace was sized for three.

The decision this change exists to make is **not** "wire the hierarchy up". It
is *which tools reach one, and why not the others* — and that decision belongs
in `clayspace-model`, in the table, before an adapter exists to disagree with
it. `tools.rs` says so in its own opening paragraph: the rule the table replaced
said every tool on a mesh layer is unavailable because "mesh layers are carried,
not sculpted", which was true when it was written and stopped being true without
anything noticing. A `match` arm can only be read. A table can be checked
against the engine's own vocabulary, and this change adds the fourth column and
the count that ratchets it.

## What Changes

- **`Representation::Multires`, and `ALL` is four.** Not `CREATABLE`: a
  hierarchy is built from a cage and there is no call that makes an empty one.
- **A fourth column in the verb table.** Fifteen of the twenty-one tools name a
  verb on it — the sixteen fixed-topology mesh brushes less Pintar and Borrar,
  plus Máscara.
- **The two colour brushes are absent, and the refusal says why** rather than
  only where they do apply. `Unavailable::NoVerbHere` carries an optional
  `ToolNote` for the one absence that reads as an oversight rather than as a
  boundary.
- **Suavizar carries a caveat there.** A smooth on a hierarchy picks a
  frequency — the form, the detail alone, or the form with the detail carried
  through unchanged — and the third is impossible on a flat mesh.
- **The two levels, as two numbers.** `MultiresLevels` carries where the brush
  writes and what the viewport draws, independently, and every transition moves
  one of them or says which.
- **The pass stack, addressed by id.** `MultiresSculptLayer`,
  `MultiresSculptLayerId`, `MultiresSculptLayerOp` — a second stack sharing a
  noun with the voxel one and sharing no addressing with it.
- **Two crossings.** `MeshToMultires` takes the mesh as a cage; `MultiresToMesh`
  bakes a level. They are the only two crossings that sample nothing, so their
  cost is a refusal rather than a tolerance, and `Refusal` grows the vocabulary
  to state one.
- **Four silent sites answered deliberately**: the mask extrude, the alpha, the
  deformation cage's division ceiling, and the row that carries no geometry yet.
- **The shelf's filter column stops being sized for five rows.** Six rows of
  fifteen pixels is ninety inside a region that is eighty-four, and nothing
  would have errored.
- **Nothing sculpts a hierarchy.** `clayspace-engine` refuses the crossing, the
  stroke and the boolean operand in words, and the benchmark harness reports
  `NoReferenceScene` rather than quietly taking no figure.

## Decisions worth stating

**The two levels do not collapse into one.** A single "current level" can offer
"sculpt coarse and look coarse" or "sculpt fine and look fine", which are the
two things a plain mesh already does. What it cannot offer is moving the broad
form while watching the pores, which is the workflow the representation exists
for. The sharpest consequence, and the one an interface has to be built on:
**changing where you sculpt redraws nothing.**

**The pass stack keeps ClayCore's noun and refuses its addressing.** Upstream
spends the word `sculpt_layer` on the hierarchy deliberately — the artist's
statement is identical to the voxel stack's, a named pass you keep as against
undo, which is a stack you pop — and its own header gates a new entry point that
says `layer` without saying `sculpt_layer`. So the noun is shared here too, and
the *addressing* is split, because that is where the defect would be:

| | `SculptLayer`, a grid's | `MultiresSculptLayer`, a hierarchy's |
|---|---|---|
| addressed by | `index: usize` | an id, minted once |
| a reorder | renumbers every position at or below it | renumbers nothing |
| order | replays cell writes, so it **is** the result | additive, so it changes organisation and not geometry |
| opened by | begin-recording / end-recording | an active pass and a write domain |

Reusing `SculptLayerOp` would have compiled and addressed the wrong pass.
`MultiresSculptLayerId` has no way to be built from a `usize`, and the one
position that survives is `MultiresSculptLayer::index`, which says it is draw
order.

**A reorder is not a geometry edit, and the model is written so it cannot
become one.** `MultiresSculptLayerOp::Move` answers `false` to
`changes_the_surface`; `MultiresState::composition` answers in **id order**
rather than stack order, so a caller folding the terms is not one refactor away
from having built an ordering rule into a representation that does not have one;
and the reorder is tested by comparing the composition and every pass's own
fields before and after. An interface that treated a list drag as an edit would
re-evaluate and re-upload millions of vertices for it.

**The colour brushes are absent rather than disabled, and the absence is
explained.** A hierarchy stores where a vertex went — a displacement read in the
vertex's own transported frame — and `absorb_level_edit` is the one write path,
taking positions. A paint stamp moves no vertex, so the stamp reports zero moved
and the write-back is skipped; the colour it wrote lands in the level's cache,
which the engine releases under memory pressure. The brush would appear to work
and its work would evaporate. It is the one absence here that reads as an
oversight — every other mesh brush is on this shelf — so it is the one that
carries a note, and the note names the route that does work: the cage's colours
are subdivided all the way up.

**A hierarchy has no lattice cage, which looks wrong and is not.** A subdivision
surface plainly *has* a cage, and bending it at level 0 propagating up is the
representation working. But `division_limit`'s cage is a lattice the interface
makes up, applied through `clay_mesh_sculptor_lattice`, which takes a fixed
mesh; there is no `clay_multires_*_lattice`, because a level above the base is
derived and pushing its vertices through a point map writes nothing the next
evaluation would not overwrite. The hierarchy's own cage is dragged by sculpting
level 0, which is a stroke.

**The crossings are exact, and that is a different kind of cost.** Every other
crossing lays the source onto a lattice and pays half a cell of movement and a
vanishing feature size. `MeshToMultires` keeps the vertices it is given, exactly,
as level 0 — and `clay_multires_from_mesh` *refuses* rather than repairing, since
a cage is precisely the thing whose topology is somebody's work. So `Refusal`
grows `NotACage` with the fault named, and stating half a cell for a crossing
that copies vertices would be the invention `chooses_resolution`'s own comment
records having made once already.

**Subdividing is priced apart from crossing.** The first level costs nothing and
the fifth can cost twenty million faces, so `SubdivisionCost` is its own type
rather than a `Cost` filled in with zeroes and a cell count that means nothing.
It refuses on the **peak** allocation rather than on what remains, because the
high-water mark during the build is what ends a session on a constrained device,
and its face arithmetic saturates — the failure mode of an unchecked multiply
here is that the operation is *allowed*.

**A representation with no benchmark member says so on every run.** The brush
group derives itself from `Representation::ALL` crossed with the verb table, so
a fourth representation mints its whole family of figure names the day it lands.
`Scene::for_representation` answers `Option`, and `Skip::NoReferenceScene` is
reported rather than nothing — a member is a subject with a recorded size and a
revision, and adding one stops every committed baseline comparing, so it arrives
with the change that can measure it.

## Out of scope

- **Sculpting one.** No `Layer` holds a `clay_multires`, so the crossing, the
  stroke and the boolean operand all refuse in words. That is the adapter's
  change, and it is blocked on the paragraph below.
- **Undo.** Neither the hierarchy's stroke record nor its layered gesture
  crosses the C ABI, which `clay.h` states twice rather than leaving to be
  discovered, and this application's undo interleave works only because
  `claycore::MeshDeltas` is an exact replayable record. There is no equivalent.
  Whether a multiresolution gesture can enter the history at all is the question
  the adapter has to settle first.
- **A side-car.** `claycore::LayerRepresentation` has three values and maps
  anything unknown to `Sdf`, so a hierarchy cannot round-trip its own
  representation through the engine's layer record, and the hierarchy's bytes
  are the host's to place. `claycore/tests/multires_document.rs` already
  measures that seam.
- **An inspector with the two levels on it.** The panel states the contract and
  draws no control, because the numbers are per-layer state the shell does not
  carry — and a control drawn for a value nothing reads is that module's own
  stated worse-than-empty.
- **Two verbs with no tool.** `_erase` takes the active pass toward zero and
  `_restore` takes the level's own detail toward the pure subdivision. Both are
  gestures inside the layered stroke transaction, and both want the change that
  opens one.
