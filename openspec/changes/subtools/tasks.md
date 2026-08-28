## 1. Activation: one path from click to sculpt target

- [x] 1.1 Make `SceneModel::set_active_layer` the single mutation both
      `Command::SelectLayer` and viewport picking reach; `select_at` activates
      rather than only setting `selected`
- [x] 1.2 Route viewport clicks in `clayspace-app/src/main.rs` through
      `ObjectViewModel::pick_at`: an object hit selects the object *and*
      activates its layer; a plain surface hit activates the hit layer; empty
      space clears the selection
- [x] 1.3 Regression tests: a click on a second subtool's geometry makes the
      next dab land there; a ghosted subtool passes activation through; a
      locked subtool activates but refuses the dab with its stated reason

## 2. Whole-subtool manipulator from the UI

- [x] 2.1 Push `SetGizmoTarget(GizmoTarget::Layer(key))` from the view when a
      subtool is the selection and a manipulator mode is on
- [x] 2.2 Regression test: selecting a subtool and dragging the manipulator
      moves, turns and uniformly scales the whole subtool; one undo reverts it
- [x] 2.3 Visual capture: the manipulator on a whole subtool's middle

## 3. Solo, and the hide/restore machinery the boolean shares

- [x] 3.1 Solo: snapshot visibility, flip through engine visibility, restore
      on release; record the engine depths solo created
      - `SceneModel::set_solo(Option<LayerKey>)` — the state to be in rather
        than a toggle, so the row and the document cannot disagree about
        whether one is engaged. `Scene::soloed` says which
- [x] 3.2 Undo/redo hop solo-created depths, reusing the existing depth
      interleaving; regression test — sculpt, solo, release, ⌘Z reverts the
      sculpt, not visibility
      - `VisibilityGesture` records the depths a batch produced and what was
        soloed on either side of it; `hop_visibility_back`/`_forward` run at
        the head of `undo`/`redo` beside the `mesh_undo` and `crossing_undo`
        checks. `history()` reports the depth without them. Hopping a solo's
        own commands releases the solo, since the scene it described has gone
      - Found and fixed alongside: undoing a *hand-made* hide left the eye in
        the stack where the command put it — the engine reverts the flag and
        cannot say that it has. `reconcile_layers` re-reads it now;
        `undoing_a_hand_made_hide_brings_the_eye_back` is the regression
- [x] 3.3 Extract hide-all-but-one / restore as the reusable primitive the
      boolean bake needs, restoring on every exit path including errors
      - `ClayDocument::with_only_visible(&[keys], body)` over
        `with_visibility`: the closure owns the restore, so there is no exit —
        error, early return, refusal before anything ran — that leaves the
        document showing what the operation wanted
- [x] 3.4 Regression tests: solo round-trips a mixed visible/hidden pattern
      exactly; a failed operation inside the hidden window restores visibility
      - `crates/clayspace-engine/tests/solo.rs`, fourteen of them
- [x] 3.5 Save-while-soloed writes the real visibility pattern, saves, then
      re-applies the solo; regression test over save and reopen
      - `DocumentModel::save` writes through `with_visibility` over the solo's
        own snapshot, so the recovery copy is right too

## 4. Inserting a form as a subtool

- [x] 4.1 `SceneModel`/`ObjectModel` gain insert-as-subtool: create the layer
      and place the primitive in it as one undo gesture, selected on arrival,
      with a collision-free default name
      - `ObjectModel::insert_shape_subtool`. `ClayDocument::insert_subtool`
        is the bracket all three sources share — `begin_undo_group` around
        the layer *and* what fills it, closed on the failing path too, and
        the layer adopted through the one activation call so it arrives
        selected. The subtool's *layer* stands where the sculptor pointed and
        the form sits at its middle, so the whole-subtool manipulator lands on
        the form rather than at the origin
      - `unique_layer_name` derives the name; `add_layer` and the mesh import
        go through it as well, since a collision made after the fact shadows
        a voxel grid just as surely (upstream ask ClayCore #365)
- [x] 4.2 Import a mesh as a subtool through the existing `add_mesh_layer`;
      copy an existing subtool via the bake path from task 5.1
      - The import already made a layer of its own and did not activate it;
        `attach_reference` now routes through `set_active_layer`, so an
        imported mesh arrives selected as the spec asks
      - Task 5.1's bake is built here as `ClayDocument::bake_subtool`, over
        `with_only_visible` and `volume_from_region` — stage 5 uses it for
        both operands. `copy_subtool` is that bake into a fresh subtool: an
        honest copy, which is why the interface says "copiar" (#364)
- [x] 4.3 `Command::AddLayer` carries a `Representation` (default SDF); new
      commands for the three insertion sources
      - `InsertShape` (honouring `SetInsertAs`), `InsertMesh` and
        `CopySubtool(key)`. `PlaceShape` became `InsertShape` rather than
        gaining a sibling: two commands for "put the form I picked into the
        scene" is the two-writers-of-one-idea shape decision 1 rejects
- [x] 4.4 View: an insert control offering the fourteen primitives, mesh
      import and copy-a-subtool, plus the choice between "as a new subtool"
      (default) and "into the active subtool"
      - The shapes window used to draw nothing at all on a grid or a mesh,
        which took the subtool insertion away with the object one. The
        destination chips are drawn first and the reason is stated beside
        them, so the subtool destination stays reachable — which is what the
        spec's refusal scenario asks for
- [x] 4.5 Regression tests: a sphere inserted as a subtool takes the next dab
      and leaves the previously active subtool untouched; a copy is
      independent of its original; each insertion is one undo step; placing
      into a voxel layer is still refused while inserting as a subtool is not
      - `crates/clayspace-engine/tests/insertion.rs`, sixteen of them;
        `crates/clayspace-vm/tests/objects.rs` for the destination default
        and the copy control; `visual_shell.rs` for the chips being wired
      - Found and fixed alongside: `SharedDocument` inherited three *provided*
        `ObjectModel` methods instead of forwarding them, so `mesh_operands`
        answered with the empty default and the shapes picker in the real
        application listed no imported model to place. The same class of
        defect the file's own note about `set_combine` describes

## 5. Booleans between subtools

- [x] 5.1 Bake one subtool alone to a volume item: hide the others (task 3.3),
      region from the union of `clay_layer_bounds` padded by the band,
      `clay_item_volume_from_document`, restore
      - `bake_subtool_over` is task 4.2's bake given the caller's region, and
        `bake_operand` is the three routes an operand can take. A *grid* is not
        one of them: `clay_item_volume_from_document` refuses a document whose
        only shown layer is a voxel one — "invalid argument (empty document)" —
        so a grid is read through `clay_item_volume_from_voxels` at index 0,
        which is `clay_voxel_to_layer` without the layer, and a mesh takes the
        crossing `place_mesh_object` already pays
      - Both operands are sampled over **one** region, which is what 5.1 asks
        for. Not because a volume item goes wrong outside its lattice — it
        reads as *outside* there, measured in
        `an_intersection_with_a_grid_keeps_only_what_both_hold`, which is what
        lets a grid operand take part in an intersection at all — but so the
        two halves of the result sit on the same lattice and meet cell-for-cell
        at the join, and so the cost stated beforehand prices what was done
      - `claycore`'s `Item::volume_from_voxels` widened from `&VoxelGrid` to
        `&VoxelField` — the one thin-wrapper change this needed, because a grid
        *borrowed from a document* is a `VoxelGridRef` and the two share only
        that; every existing caller coerces
      - Found and fixed alongside: a rasterized grid reported no extent at all
        until the first dab landed on it. `voxel_bounds` is the only account of
        a voxel layer's box there is — `clay_layer_bounds` answers for a layer's
        SDF content — and it was refreshed by a stroke and by opening a file but
        not by the crossing that makes the grid. Frame All framed the default
        box; a boolean naming one as an operand refused it as empty.
        `after_conversion` refreshes it now
- [x] 5.2 The operation: new SDF layer, operand A with `CLAY_OP_ADD`, operand
      B with the chosen op, all inside one undo gesture; operands hidden by
      default or removed on request; result selected
      - `ObjectModel::run_boolean`, over task 4.1's `insert_subtool` bracket.
        The bakes happen *before* the group, as a copy's do — the
        hide-and-restore writes visibility commands of its own and one step
        back has to reach the boolean rather than the flags it borrowed — and
        retiring the operands happens *inside* it, so hiding or consuming them
        is part of the one thing the sculptor asked for
- [x] 5.3 Cost: default resolution from the operands' detail, presented with
      the same `Cost` vocabulary the conversion crossings use, recomputed when
      the sculptor changes the resolution; nothing runs unconfirmed
      - `boolean_cost` is `Cost::of` over the pair's region, and `boolean_cell`
        is the finer of the two operands' own detail — a grid says what it is
        worked at, a field and a mesh are worked at the brick cache's cell. The
        ViewModel re-derives it when the *pair* changes and leaves it alone
        otherwise, so the default follows the operands and the number stays the
        sculptor's
- [x] 5.4 Refusals with named causes: an empty operand, a protected operand, a
      non-overlapping intersection, a pair over budget — scene left unchanged
      - `ModelError::Boolean(BooleanRefusal)` rather than a formatted string:
        the interface has to be able to say *which* of the two subtools is the
        problem, and a sentence the adapter built cannot be asked that
        afterwards. The overlap test is on the boxes, which is what can be
        answered before anything is sampled and is the case the spec names
- [x] 5.5 View: the boolean panel — pick two subtools, name which is cut and
      which cuts, choose the operation and resolution, show the cost, confirm
      - `shell::boolean_window` over `BooleanViewModel`. The two operands are
        labelled by the role each plays, and a subtraction prints the sentence
        it reads as; the confirm button is disabled until there is a pair, and
        the panel says that the result is resolved rather than live and what
        becomes of the operands
- [x] 5.6 Regression tests, one per spec scenario: cylinder bores a sphere;
      the result sculpts, moves and re-operands; one undo reverses the whole
      boolean with operands visible again; operands consumed only on request;
      an SDF/voxel/mesh operand each work without a manual conversion; a
      boolean over a *sculpted* operand, not only over primitives
      - `crates/clayspace-engine/tests/booleans.rs`, twenty-four of them;
        `crates/clayspace-vm/tests/booleans.rs` for the panel's own rules
        against a double; `shared_forwarding.rs` for the four provided methods
        the shared document has to answer for itself
- [x] 5.7 Visual captures: union, subtraction and intersection of two subtools
      - `crates/clayspace-app/tests/visual_booleans.rs` — the three over one
        pair from one camera, plus what the viewport shows before and after —
        and `visual_shell.rs` captures the panel itself

## 6. Per-subtool sculpting state

- [x] 6.1 Move `symmetry` into `Layer`; toggles read/write the active
      subtool's axes; new subtools start with X on. Regression test —
      symmetry off on A survives a visit to B that has it on
- [ ] 6.2 Replace the standalone document mask with the engine's per-layer
      `add_mask(layer, cell_size)`; mask verbs act on the active subtool's
      mask
      - The mask moved onto `Layer` and the verbs act on the active
        subtool's; the standalone `clay_mask_create` stayed. A document-owned
        mask is lent *out* of the document and every masked verb in the
        wrapper takes the document and the mask together — `apply_stroke`,
        `relax_region`, `flatten_region`, `mask_extrude`, and a voxel grid
        borrowed from the same document — so it cannot be handed back in from
        safe Rust. Reaching it means giving those five a form that takes the
        mask's *layer*, which is a redesign of `claycore`'s masking surface
        rather than the thin wrapper this change allows. Recorded in
        docs/features.md and measured in `claycore_mask_persistence.rs`
- [x] 6.3 Regression tests: two subtools keep independent masks; a mask gates
      only its own subtool's edits; the existing mask suites still pass
- [x] 6.4 Confirm per-layer masks ride the document's save path; if the engine
      does not serialize them, keep app-side persistence per layer and record
      that in docs/features.md
      - They do: measured in `claycore_mask_persistence.rs`. Unreachable for
        now for the reason under 6.2, so masks stay per subtool and per
        session; docs/features.md says so
- [x] 6.5 Move `armature` and its bounds into `Layer`; `ArmatureModel` answers
      for the active subtool. Regression test — two rigs pose independently
      and both survive a save and reopen
- [x] 6.6 Resolve a standing deformation cage when the active subtool changes
      (apply or drop, the sculptor's choice); regression test — the cage does
      not follow the switch

## 7. Active-subtool cue in the viewport

- [x] 7.1 `visible_mesh_geometry` returns per-layer index ranges alongside the
      concatenated buffer
      - `CarriedSpan` comes out of the walk that already rebases the indices,
        so nothing has to reconstruct the seams of a concatenation that has
        forgotten them. A layer that contributed no triangle gets no span: an
        empty range is an empty draw call, and a cue with an exception in it
      - The two sources a layer can be drawn from — a voxel grid's chunks or
        its smooth surface, and a mesh layer's triangles — used to spell the
        rebasing out separately. `CarriedBuffer::append` is the one copy of it
- [x] 7.2 Renderer draws carried geometry one range at a time with a per-draw
      tint uniform; active subtool tinted, others plain
      - A second material buffer and a second bind group rather than one buffer
        written twice: `Queue::write_buffer` is ordered against the submission
        and not against the draws inside it, so writing the tint between two
        draw calls gives both of them the last value written
      - `MeshSpan` arrives with the buffer it describes and the active key
        arrives on its own — activation is a click, and folding it into the
        buffer's staleness check would make choosing a subtool re-walk and
        re-upload every visible grid
      - The tint is the accent's *hue* at full value, mixed 0.45 toward white:
        the accent as stored took two thirds of the value out of the clay,
        which reads as a shadow rather than as a cue
- [x] 7.3 Active SDF subtool cued by its bounds outline via
      `clay_layer_bounds`, drawn like the selected object's box
      - `LatticeView::subtool_outline`, through `outline_box` — which the
        selected object's own box now goes through too, since the corner
        arithmetic is the part that is easy to get subtly wrong twice. Dimmed
        against the object outline: which subtool is active is standing state,
        and the box a sculptor just put an object into is the more urgent
      - Nothing is cued while one layer is visible on its own. The requirement
        is to be distinguishable *from the other visible layers*, and with none
        to be distinguished from a tint says only that the clay changed colour
- [x] 7.4 Visual captures: active voxel subtool tinted among plain ones;
      active SDF subtool outlined; export test — exported geometry carries no
      trace of the cue
      - `crates/clayspace-app/tests/visual_active_subtool.rs`, four of them.
        The tint test asserts that the two activations mark *disjoint* pixels,
        which is the claim a count cannot make: one material for every span —
        the concatenated draw as it was — would move the same pixels both times
      - The export test compares whole `v` lines and not their first three
        numbers: OBJ writes vertex colour as three more numbers on the same
        line, which is exactly where a tint drawn into the clay rather than
        into the material would end up
- [x] 7.5 Review the SDF outline capture against the design's open question;
      fall back to dimming the union surface if the box reads badly
      - The outline stays; design.md's Open Questions records the reading and
        the reason. In short: the box is clear with both forms in frame and
        degenerates to two lines in the corners when the active subtool fills
        the viewport — but that is the view where no other subtool is on screen
        to be confused with, and the fallback dims every visible SDF layer
        together, which says nothing in the case the spec names

## 8. Performance, documentation, and the gate

- [x] 8.1 Benchmark subtool switching (activation including
      `arm_mesh_sculptor` on a heavy mesh subtool), solo round-trip including
      undo hops, and the boolean bake; add to the reference suite with budgets
      set from measurement
      - `bench/groups/subtool.rs`, nine figures, all measured on the reference
        suite and merged into `benchmarks/baseline-linux-x86_64.json`:
        `subtool.activate.mesh` 159.7 ms mean / 169.6 p95, `subtool.activate.sdf`
        0.00 ms, `subtool.solo` 14.4 / 21.5, `subtool.solo_undo` 203.3 ms,
        `subtool.copy` 4349.5 ms, `subtool.boolean` 10198.2 ms
      - Activation is measured between **two** mesh subtools rather than
        between a mesh one and a field one. The sculptor is cached one layer at
        a time, so a fixture that went mesh → field → mesh finds it already
        built and measures nothing; alternating between two evicts it on every
        switch, which is the worst case and a real one
      - The screen is outside the clock for activation and only for activation:
        choosing a subtool moves no geometry — the cue is a per-draw tint over
        buffers that did not change — so a refresh timed there would price a
        re-upload the application does not perform
      - `subtool.copy` is here so `subtool.boolean` can be read: a copy is one
        bake, a boolean is two over a larger region plus the layer that holds
        them, and without the pair a change in the sampling cannot be told from
        a change in everything around it
      - Only activation carries a budget, and it is the specification's rather
        than one invented here: "no engine operation SHALL block the interface
        thread for more than 16 ms". Solo is a re-mesh priced like an edit and
        the copy and the boolean are bakes behind a stated cost and a confirm,
        which is the class every unbudgeted figure in this suite belongs to
      - The nine figures were **merged into** the Linux baseline rather than
        re-recording it. The whole run compared clean against the existing
        baseline — no regression in any of the other 125 figures, the largest
        move being `brush.mesh.nudge.p95` at +20 % against a 2.0 tolerance — so
        re-recording would have moved 125 recorded values to hide nothing.
        README says so beside the conditions
- [x] 8.2 If activation misses the interaction budget on mesh subtools, arm
      the sculptor lazily on the first dab
      - **Fixed, though not by the mitigation this task names.** Activation on
        a mesh subtool was 159.7 ms mean / 169.6 p95 against the 16 ms
        `performance-budgets` allows an engine operation to hold the interface
        thread, and this change made that operation fire on a viewport click as
        well as on a stack-row click. It is **0.00 ms** now, measured on the
        same fixture at the same load
      - What made it a *repeated* cost was the document holding one sculptor:
        a second carried mesh evicted the first, so alternating between two
        mesh subtools paid the weld and the adjacency pass every time. The
        document holds several now — `clayspace-engine/src/sculptors.rs`, an
        LRU bounded by count, with the reasoning for a count rather than bytes
        stated there — and a switch onto a mesh welded once is a lookup
      - The lazy-arming fallback this task names was tried and does not work,
        which is why it is not what shipped: a pick against a mesh layer is
        answered by the sculptor's own raycast, the interface sends no stroke
        where the pick reported nothing, so with no sculptor the "first dab"
        can never arrive. `the_pointer_finds_an_imported_mesh` and
        `the_mesh_reports_what_its_queries_cost` in
        `clayspace-engine/tests/mesh_sculpting.rs` fail the moment the arming
        comes out
      - Holding several introduces one bug class in exchange — a sculptor
        outliving the mesh it was built over, which the engine answers with a
        refusal rather than a read of freed storage. Dropped on removal and on
        the reconciliation that finds a layer gone or brought back;
        `clayspace-engine/tests/mesh_sculptor_cache.rs` holds the outcomes, and
        six unit tests in `sculptors.rs` hold the eviction policy itself. The
        eviction on the *restore* path is a guard rather than a fix and says so
        in both places: measured, the restored layer keeps the same mesh behind
        its handle, so the test passes with that call taken out
      - Found alongside: `discard_cage_preview` reverted its offsets into
        whichever sculptor was held rather than the layer the preview was laid
        on. Reachable only with more than one mesh subtool, which is what this
        change made ordinary; it is keyed by layer now
      - **What is left is the first weld of a given mesh** — once per mesh per
        session, about 165 ms, and after a reopen it is paid on the first click
        on each mesh subtool. It cannot move to the first dab (above), and not
        to document open either: open costs 44.8 ms and warming two sculptors
        there would make it about 395 ms, paid whether or not they are used.
        Where it could go is a worker thread, which is an engine question —
        asked upstream as
        [#368](https://github.com/CyberdyneCorp/ClayCore/issues/368)
- [x] 8.3 Update README "What it does today", docs/features.md and
      docs/roadmap.md (upstream asks now ClayCore #321, #210, #364, #365);
      cognitive-complexity pass over touched functions against the frontend
      8–12 / backend 15 targets
      - README gains the subtool paragraph, the boolean paragraph and two rows
        in the input table — a click on a form activates its subtool, and a
        click on a placed shape does both. `Sculpt → Shapes` was corrected to
        `File → Shapes`, which is the menu the control has always been in, and
        so was *Escultura → Formas* in features.md
      - docs/roadmap.md: six open upstream findings, four numbered — #321 as
        what a live boolean waits on, #210 with what solo's hops now add to it,
        #364 as what a cheap duplicate waits on, #365 as why insertion derives
        unique names. **#317 verified against the pin**: closed upstream on
        2026-08-26 and still true here, because `clay_layer_node_transform`,
        `_params` and `_op_blend` are absent from
        `vendor/ClayCore/bindings/c/clay.h` at 0.52.2 — so the sidecar table
        stays and the tripwire stays a test. A new *What is slow and why*
        section carries the six figures and why activation misses its bound
      - features.md: **Mesh-surface booleans stays** under *Deliberately
        absent*. A mesh subtool is a legal operand now, but through the same
        sampling — what changed is who performs the crossing and when, not
        whether one is performed
      - Complexity, measured with `clippy::cognitive_complexity` at a threshold
        of 12 over the whole workspace, against the same measurement on `main`:
        the one regression was `App::apply_now`, 16 on main and 17 here, split
        into `dispatch_to_models` / `apply_app_effects` / `settle_after` and now
        under 12. `shell::menu_bar`'s File closure went to 13 when the boolean
        entry was added; the seven panel toggles are one `panel_items` call now.
        Nothing else this change touched moved: `Renderer::render` (14) and
        `shell::right_panel` (14) read the same as on main and are flagged
        rather than mangled to satisfy a number
- [x] 8.4 Full suite green: `cargo test`, `cargo clippy`, `just bench-compare`
      against the Linux baseline
      - `just check` end to end: fmt-check, layering, lint, test, spec,
        packaging

## 9. What three reviews found in the above

Each of these is a defect in work already ticked, so the fix is recorded here
rather than by unticking the task it belongs to. Every one carries a test that
fails without it.

- [x] 9.1 A mesh subtool could not be moved with the manipulator (task 4.2's
      scenario, "carries its geometry, and can be moved with the manipulator")
      - `GizmoTarget::Layer` resolves to the engine's layer transform, and a
        mesh layer is *carried* rather than evaluated — the tape has no item to
        move, so the transform reached nothing. Measured: a mesh subtool moved
        five units along X drew its first vertex where it drew it before, and
        `layer_bounds` answered `None` for every mesh layer, so `gizmo_reach`
        fell back to the object default on all of them
      - `ClayDocument::carried_placement` and the two conversions beside it are
        the one crossing between world space and a carried mesh's own vertices.
        Everything that crosses goes through them: what is drawn, what is
        picked, the box the manipulator sizes itself to, the cage, and a mesh
        operand of a boolean. Identity is the common case and skips the work,
        which is why the 1170 existing tests are untouched
      - `Layer::mesh_bounds` is the box in the mesh's own coordinates —
        `clay_layer_bounds` answers a layer's *SDF* extent, so a carried mesh
        had no account of its extent at all, which is also why `MeshToVoxel`
        had been refusing every source as unbounded
      - `an_imported_mesh_arrives_as_the_active_subtool` asserted that the layer
        was a target the manipulator could address, which was true of a subtool
        the manipulator could not move. It asserts the triangles now, and
        `a_mesh_subtool_moves_with_the_manipulator` holds the third clause
      - Found alongside: `mesh_to_voxels` takes its region from `bounds()`,
        which is the active layer's box — so **every** mesh-to-voxel crossing
        was refused as an unbounded region, and the panel priced it as nothing.
        `a_carried_mesh_can_be_crossed_to_a_grid`
- [x] 9.2 Undo hopped visibility *before* asking whether a mesh gesture was
      newer (task 3.2)
      - A mesh gesture records the engine's depth without raising it, so a solo
        engaged before a stroke ends at exactly that depth and both answered
        "newest". Measured: one dab on a soloed mesh subtool, one undo, and the
        solo was released *and* the engine undid the entry beneath it — the
        import that made the layer — while the stroke stayed and its gesture
        was stranded at a depth the engine would never return to. Redo had the
        mirror of it, and lost the stroke for good
      - Both directions ask the mesh history first and again after the hop.
        `a_mesh_stroke_made_under_a_solo_is_what_one_undo_takes_back` and
        `a_mesh_stroke_undone_under_a_solo_comes_back`
- [x] 9.3 `visibility_redo` outlived the history it described (task 3.2)
      - Cleared only by `write_visibility`, so a gesture left on the redo side
        went on matching `depths.first() == engine_undo_depth() + 1` whenever
        the depth returned to that value and silently spent a redo the sculptor
        meant for their own work — putting an edit back through the *hop*, which
        resyncs neither the object table nor the layer transforms, and engaging
        a solo nobody asked for. `ClayDocument::redo_room` reads the engine's
        redo stack on both sides of every step: a stack the engine truncated is
        the only word there is that an edit landed.
        `a_redo_after_a_new_edit_does_not_re_engage_a_released_solo`
- [x] 9.4 Undoing a consuming boolean re-keyed its operands (task 5.2)
      - `reconcile_layers` could not match a restored layer to anything, so it
        rebuilt each through `..Layer::new(..)` with a fresh `LayerKey` and a
        default of everything the host keeps. Measured: new keys, the
        manipulator reading `position: [0, 0, 0]` while the engine held the
        real transform, the painted mask and the symmetry axes gone, and no
        object rows at all — the table is resynced by depth and was still filed
        under the key that had left. `ClayDocument::retired` keeps a removed
        layer whole until history brings it back or the session ends, which
        also gives a restored grid its chunks and its extent back.
        `one_undo_gives_the_consumed_operands_back_as_they_were`
- [x] 9.5 `remove_layer` moved the sculpt target and stranded a solo (tasks 3.1
      and 6.1)
      - It clamped the active *index* instead of following the layer it pointed
        at, so removing a row above the active one moved the sculpt target — and
        with it the mask, the mirror and the rig — to an unrelated subtool. And
        it never looked at `self.solo`, so removing the soloed subtool left the
        field naming a key the document no longer had and the layers it hid
        hidden, with no row anywhere to release it from.
        `removing_a_subtool_above_the_active_one_keeps_the_sculpt_target`,
        `removing_the_soloed_subtool_brings_the_scene_back`,
        `removing_another_subtool_leaves_the_solo_engaged`
- [x] 9.6 The crossing did not derive a unique name (task 4.1)
      - `unique_layer_name` was wired into `add_layer`, the import, insertion,
        the copy and the boolean, and not into `convert_layer` — which is the
        path that actually creates voxel layers, and so the one the ClayCore
        #365 collision it exists to prevent is most reachable from. Two rows
        called "Forma · voxel" shadow one another's grid. `extrude_from_grid`
        had the same literal name. `crossing_the_same_source_twice_names_the_grids_apart`
- [x] 9.7 Copy Subtool was offered for grids and always refused (task 4.2)
      - `bake_subtool` went straight to `clay_item_volume_from_document`, which
        refuses a document whose only shown layer is a grid — the refusal
        `bake_operand`'s voxel branch exists for and documents. The copy takes
        the operand's three routes now. `a_grid_subtool_can_be_copied`
- [x] 9.8 The new-layer list offered a representation the document cannot make
      (tasks 4.3 and 4.4)
      - `add_layer` matched only `Voxel` and sent everything else to
        `add_sdf_layer`, then recorded the row as a mesh: a row labelled "Malha"
        over a field layer nothing could ever put a triangle into, offering the
        mesh vocabulary over nothing, active on arrival. The specification
        qualifies the offer — "SDF, voxel and mesh *where a mesh source is at
        hand*" — and when a layer is made out of nothing there is none, so
        `Representation::CREATABLE` is what the list draws and the document
        refuses the third by name. `a_new_layer_cannot_be_asked_for_as_a_mesh`
- [x] 9.9 `set_solo` had no restore on a half-written batch (task 3.3)
      - `write_visibility` states that a batch which failed halfway is one whose
        caller is about to restore; `with_visibility` honours that and
        `set_solo` did not, so a failing flag left the scene part hidden with
        `self.solo` still `None` — no solo shown as engaged, and nothing offered
        that would put the rest back. The engine refuses a visibility write
        naming a *locked* layer, which is what lets
        `a_solo_refused_halfway_puts_the_scene_back` force the half-written
        batch rather than assert about one nobody can produce
      - `with_visibility`'s own doc claim is narrowed to what it can keep: a
        panic unwinding out of `body` is not covered and cannot be from there,
        since the restore needs the `&mut self` the body is holding
- [x] 9.10 Coverage the scenarios did not have
      - "A voxel subtool is created directly" and "the default stays what it
        was": every `AddLayer` in the tree passed `Sdf` and the one test over
        the command inspected only names. `a_voxel_subtool_is_created_directly`
        and `the_default_stays_what_it_was` in `clayspace-vm/tests/scene.rs`,
        and `the_new_layer_control_makes_a_field_layer_by_default_and_offers_a_grid`
        in `visual_shell.rs` — the whole control was unreachable from a test,
        which is how the dead mesh entry survived
      - "Clicking empty space clears the selection": the test that had held it
        was rewritten into one that asserts only what the raycast answered, and
        the arm that does the clearing sat in the event loop. It is
        `input::selection_after` now, beside `activation` and for the same
        reason, with `a_press_on_nothing_clears_the_selection`
- [x] 9.11 A second attributed raycast per press (task 1.2)
      - `pick_object_at` threw away the layer `pick_item` had already
        attributed and asked `SceneModel::layer_at` for it again, on every
        primary press including every stroke start — against a doc comment
        saying the answer both need is the one raycast already paid for.
        `pick_item` answers `(ItemKind, LayerKey)` now and `Picked` carries the
        layer, so the second raycast is paid only where the first met nothing
