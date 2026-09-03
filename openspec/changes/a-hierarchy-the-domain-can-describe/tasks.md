# Tasks

## 1. The representation

- [x] 1.1 `Representation::Multires`, `ALL` at four, and the label — and
      `CREATABLE` left at two, because a hierarchy comes from a cage
- [x] 1.2 A name, a phrase, an icon of its own shape and a three-letter tag, in
      all three languages
- [x] 1.3 The shelf's filter column stops being sized for five rows, with the
      overflow held by a test measured off the rows the column draws

## 2. The verb table

- [x] 2.1 A fourth column on `Verbs`, forcing every literal in the table to
      answer
- [x] 2.2 The fifteen tools that reach a hierarchy, asserted as *the mesh
      column less the two colour brushes* rather than as a list of names
- [x] 2.3 The coverage ratchet gains a fourth count
- [x] 2.4 `Unavailable::NoVerbHere` carries an optional `ToolNote`, and
      `note_on` answers for a pair whether or not the tool is offered
- [x] 2.5 `ToolNote::MultiresStoresNoColour` and
      `ToolNote::MultiresSmoothChoosesAFrequency`, worded in three languages
- [x] 2.6 A row that carries no geometry yet says "cage" where a mesh row says
      "mesh"

## 3. The silent sites

- [x] 3.1 `can_extrude` refuses a hierarchy — no field to extrude from, the
      mesh's own reason
- [x] 3.2 `AlphaSupport::of` accepts one, with the reason written rather than
      inherited by an early return
- [x] 3.3 `division_limit` answers `None`, with the reason a hierarchy that
      plainly has a cage still has no lattice
- [x] 3.4 `holds_the_whole_gesture` and `replays_from_the_anchor` answer the
      mesh's way, because the layered gesture's cancel is exact
- [x] 3.5 `needs_colour_attribute` is pinned against the verb table so the two
      answers cannot drift

## 4. The levels

- [x] 4.1 `MultiresLevels`: the count, the sculpt level and the display level,
      with every transition moving one of them or saying which
- [x] 4.2 Subdividing moves both; removing the top brings both back inside
- [x] 4.3 `MultiresLevelOp`, and `changes_what_is_drawn` answering `false` for
      a change of sculpt level
- [x] 4.4 `SubdivisionCost`, refusing on the peak, with saturating face
      arithmetic
- [x] 4.5 `detail.rs` says what it is not, since the two share the word

## 5. The pass stack

- [x] 5.1 `MultiresSculptLayerId`, with no way to build one from a position
- [x] 5.2 `MultiresSculptLayer`, and `index` documented as draw order
- [x] 5.3 `MultiresSculptLayerOp`, with `changes_the_surface`,
      `is_destructive`, `needs_the_stroke_closed` and `refused_by_a_lock` as
      four separate questions
- [x] 5.4 `MultiresState::composition` in id order, and `reordered` proving a
      reorder moves nothing
- [x] 5.5 `WriteDomain`, refusing the one combination with no answer
- [x] 5.6 `MultiresSculptLayerCost`, with "a stroke is open" where the grid's
      has "recording"
- [x] 5.7 `LayerSummary::multires`, and three provided `SceneModel` methods

## 6. The crossings

- [x] 6.1 `Direction::MeshToMultires` and `Direction::MultiresToMesh`
- [x] 6.2 `Direction::is_exact`, and neither crossing choosing a resolution
- [x] 6.3 `CageFault`, `Refusal::NotACage`, `Refusal::LevelOverBudget` and
      `Refusal::DepthLimit`, each with its own sentence

## 7. The rest of the workspace

- [x] 7.1 Every exhaustive match answered — the stroke dispatch, the crossing,
      the mask extrude and the boolean operand refuse in words
- [x] 7.2 `add_layer` refuses a hierarchy explicitly rather than falling through
      its `_` arm to `add_sdf_layer` and recording the row as one, which is the
      dead row the mesh case beside it already exists to prevent
- [x] 7.3 `Scene::for_representation` answers `Option`, and the bench groups
      report `Skip::NoReferenceScene` rather than taking no figure
- [x] 7.4 `reference_suite` asserts which representations have no member, so a
      member arriving is a decision rather than a drift
- [x] 7.5 README and `docs/features.md` say what is described and what is not
      built
