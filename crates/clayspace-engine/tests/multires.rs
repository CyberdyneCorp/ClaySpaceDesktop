//! A subdivision hierarchy as a subtool: held, sculpted, saved and taken back.
//!
//! The tier's whole claim is one sentence, and
//! `detail_survives_a_change_to_the_form_beneath_it` is that sentence measured
//! rather than read: sculpt fine detail at a high level, move the form
//! underneath it at a low one, and the detail is still there and still
//! oriented the way the form now sits. That is what a hierarchy is *for*, and
//! it is the one property a mesh layer cannot offer at any price — a mesh
//! moved at the cage's scale smears its wrinkles, because there is nothing
//! storing them in a frame that travels.
//!
//! Everything else here guards a seam the representation only has because of
//! how it is owned. A `clay_multires` is a free-standing owning handle that
//! `clay_document_save` has never heard of, so:
//!
//!   * the sculpt travels in a file beside the `.clayspace`, and a document
//!     that opens without it comes back as the cage it demonstrably holds;
//!   * the undo history holds the hierarchy's own bytes, because the ABI
//!     carries no delta record for a hierarchy gesture and says so twice;
//!   * and a level is added build-then-publish, priced first and refused over
//!     budget rather than attempted.

use clayspace_engine::{BackendPolicy, ClayDocument};
use clayspace_model::{
    BrushSettings, CageFault, ConversionSettings, Direction, DocumentModel, ExchangeModel,
    ExportSettings, GestureSample, ImportSettings, LayerKey, ModelError, MultiresLevelOp, Refusal,
    Representation, SceneModel, SculptModel, ToolKind,
};

// -- fixtures ---------------------------------------------------------------

fn scratch(name: &str, extension: &str) -> std::path::PathBuf {
    let path = std::env::temp_dir().join(format!(
        "clayspace-multires-{name}-{}.{extension}",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&path);
    path
}

/// A flat grid of quads, which is what a Catmull-Clark cage is supposed to be.
///
/// Written as a file and imported, because that is the only route a mesh layer
/// has into a document and a fixture taking another one would test a path no
/// sculptor reaches. The reader triangulates quads on the way in — the header
/// says so — which is fine for a cage, since the subdivision rule is defined
/// over faces of any arity.
fn cage_obj(path: &std::path::Path, divisions: usize, half: f32) {
    let mut text = String::new();
    let step = 2.0 * half / divisions as f32;
    for z in 0..=divisions {
        for x in 0..=divisions {
            text.push_str(&format!(
                "v {} 0 {}\n",
                -half + step * x as f32,
                -half + step * z as f32
            ));
        }
    }
    let stride = divisions + 1;
    for z in 0..divisions {
        for x in 0..divisions {
            // Wound so the sheet faces +y, which makes a Draw stamp read as a
            // bump rather than a dent.
            let a = z * stride + x + 1;
            text.push_str(&format!(
                "f {} {} {} {}\n",
                a,
                a + stride,
                a + stride + 1,
                a + 1
            ));
        }
    }
    std::fs::write(path, text).expect("write the cage");
}

/// A document whose only layer is a hierarchy, `levels` deep over a flat cage.
fn with_a_hierarchy(who: &str, levels: u32) -> (ClayDocument, LayerKey) {
    let policy = BackendPolicy::discover(None).expect("discover backends");
    let mut document = ClayDocument::new(policy).expect("a document");
    let path = scratch(who, "obj");
    cage_obj(&path, 4, 2.0);
    document
        .import_mesh(&path, ImportSettings::default())
        .expect("import the cage");
    let _ = std::fs::remove_file(&path);

    let mesh = document
        .scene()
        .layers
        .iter()
        .find(|layer| layer.representation == Representation::Mesh)
        .map(|layer| layer.key)
        .expect("the cage is a mesh layer");
    document.set_active_layer(mesh).expect("activate the cage");

    let settings = ConversionSettings::default();
    let key = document
        .convert_layer_in_place(Direction::MeshToMultires, settings.cell_size, settings.blur)
        .expect("a flat quad grid is a cage");
    for _ in 0..levels {
        document
            .apply_multires_level_op(MultiresLevelOp::AddLevel)
            .expect("subdivide");
    }
    (document, key)
}

/// One dab at the level the brush is bound to, centred on `at`.
fn dab(document: &mut ClayDocument, at: [f32; 3], size: f32) -> bool {
    document.begin_gesture();
    let outcome = document.apply_stroke(
        ToolKind::Padrao,
        BrushSettings {
            size,
            intensity: 1.0,
            ..BrushSettings::default()
        },
        &[GestureSample {
            position: at,
            pressure: 1.0,
            time: 0.0,
        }],
        // Unmirrored: a mirrored dab is two stamps and this measures where
        // one landed.
        [false; 3],
    );
    document.end_gesture();
    outcome.expect("the dab is applied").changed
}

/// The triangles the viewport is handed for the whole document.
fn drawn(document: &mut ClayDocument) -> Vec<[f32; 3]> {
    document.visible_mesh_geometry().0
}

/// How far the drawn surface stands off the flat sheet it was subdivided from.
fn relief(document: &mut ClayDocument) -> f32 {
    drawn(document)
        .iter()
        .map(|point| point[1].abs())
        .fold(0.0f32, f32::max)
}

/// Where the drawn surface stands highest, and how high.
fn peak(document: &mut ClayDocument) -> ([f32; 3], f32) {
    drawn(document)
        .into_iter()
        .fold(([0.0; 3], 0.0f32), |(at, best), point| {
            if point[1].abs() > best {
                (point, point[1].abs())
            } else {
                (at, best)
            }
        })
}

fn levels(document: &ClayDocument, key: LayerKey) -> clayspace_model::MultiresLevels {
    document
        .scene()
        .layer(key)
        .and_then(|layer| layer.multires.as_ref())
        .map(|state| state.levels)
        .expect("the row is a hierarchy")
}

// -- the property the tier exists for ---------------------------------------

/// Sculpt fine, move the form underneath, and the detail rides on it.
///
/// The measurement is a *difference between two hierarchies*, because that is
/// the only way to ask the question without also measuring the cage edit. Two
/// identical hierarchies; one gets a wrinkle at the finest level and the other
/// does not; then **both** get the same dab on the cage at level 0. What is
/// left between them is the wrinkle alone, carried through whatever the cage
/// edit did to the form.
///
/// Three things are asserted about that difference and each rules out a
/// different failure. Its **height** is unchanged, so the wrinkle was not
/// scaled or flattened. Its **vertex** is the same one, so it did not slide to
/// another part of the sheet. And its **direction has turned**, which is the
/// assertion that actually distinguishes this representation from a mesh: a
/// displacement stored in world space would come back pointing exactly where
/// it was, lying flat across a form that has rolled underneath it. A frame
/// carried up from the cage turns with the cage.
#[test]
fn detail_survives_a_change_to_the_form_beneath_it() {
    let (mut wrinkled, key) = with_a_hierarchy("survives-a", 3);
    let (mut plain, _) = with_a_hierarchy("survives-b", 3);

    // The wrinkle: a small dab at the finest level, off to one side so the
    // cage edit below rolls the form under it rather than lifting it squarely.
    assert!(
        dab(&mut wrinkled, [0.8, 0.0, 0.0], 0.6),
        "the wrinkle moved something"
    );
    let (_, wrinkle_height) = peak(&mut wrinkled);
    assert!(
        wrinkle_height > 0.0,
        "the fixture actually wrinkled the surface"
    );

    let before: Vec<[f32; 3]> = drawn(&mut wrinkled);
    let flat: Vec<[f32; 3]> = drawn(&mut plain);
    assert_eq!(
        before.len(),
        flat.len(),
        "the two hierarchies are the same subject, so they are the same size"
    );
    let (peak_vertex, height_before, direction_before) = tallest_difference(&before, &flat);

    // Now the form underneath, at the cage. A broad dab centred elsewhere, so
    // the sheet under the wrinkle tilts rather than merely rising.
    for document in [&mut wrinkled, &mut plain] {
        document
            .apply_multires_level_op(MultiresLevelOp::SetSculptLevel(0))
            .expect("drop to the cage");
        // Five, and the number is the measurement rather than a ritual. One
        // dab tilts the sheet under the wrinkle by about thirteen degrees,
        // which turns the stored displacement to cos 0.973 — real, but close
        // enough to 1.0 that the assertion below would not be saying much.
        // Five take it to cos 0.737, a turn of forty-two degrees, while the
        // wrinkle's own height stays put to seven significant figures.
        for _ in 0..5 {
            assert!(
                dab(document, [-1.0, 0.0, 0.0], 3.0),
                "the cage moved something"
            );
        }
    }
    assert_eq!(
        levels(&wrinkled, key).display,
        3,
        "sculpting the cage did not move what is drawn — which is the whole \
         point of two levels rather than one"
    );

    let after: Vec<[f32; 3]> = drawn(&mut wrinkled);
    let moved_flat: Vec<[f32; 3]> = drawn(&mut plain);
    let (peak_after, height_after, direction_after) = tallest_difference(&after, &moved_flat);

    assert_eq!(
        peak_vertex, peak_after,
        "the wrinkle is on the same vertex it was on: it rode the form rather \
         than being smeared to another part of the sheet"
    );
    assert!(
        (height_after - height_before).abs() < height_before * 1e-3,
        "and it is the same height — {height_before} before, {height_after} \
         after — rather than scaled or flattened by the edit underneath it. \
         A thousandth is a wide tolerance for what is measured: the two agree \
         to seven significant figures"
    );
    let cosine = dot(direction_before, direction_after);
    assert!(
        cosine < 0.8,
        "and it is pointing somewhere else ({cosine}). A displacement stored \
         in world space comes back at exactly cos 1.0, lying flat across a \
         form that has rolled underneath it; a frame carried up from the cage \
         turns with the cage, which is the entire claim of this tier"
    );
}

/// The tallest per-vertex difference between two same-sized point sets, as
/// (index, length, unit direction).
fn tallest_difference(a: &[[f32; 3]], b: &[[f32; 3]]) -> (usize, f32, [f32; 3]) {
    let mut best = (0usize, 0.0f32, [0.0f32; 3]);
    for (index, (here, there)) in a.iter().zip(b).enumerate() {
        let delta: [f32; 3] = std::array::from_fn(|axis| here[axis] - there[axis]);
        let length = delta.iter().map(|axis| axis * axis).sum::<f32>().sqrt();
        if length > best.1 {
            best = (
                index,
                length,
                std::array::from_fn(|axis| delta[axis] / length),
            );
        }
    }
    best
}

fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

// -- the side-car -----------------------------------------------------------

/// A save and a reopen reproduce the sculpt, not the cage under it.
#[test]
fn a_hierarchy_comes_back_from_a_file_with_its_sculpt_on_it() {
    let (mut document, key) = with_a_hierarchy("round-trip", 3);
    assert!(dab(&mut document, [0.3, 0.0, 0.2], 0.5), "sculpt something");
    document
        .apply_multires_level_op(MultiresLevelOp::SetSculptLevel(1))
        .expect("a sculpt level worth round-tripping");

    let standing = relief(&mut document);
    let count = drawn(&mut document).len();
    assert!(standing > 0.0, "the fixture sculpted something");

    let path = scratch("round-trip", "clayspace");
    document.save(&path).expect("save");
    assert!(
        clayspace_engine::multires::sidecar_for(&path).exists(),
        "the sculpt is written beside the document, because the document does \
         not carry it"
    );

    let policy = BackendPolicy::discover(None).expect("backends");
    let mut reopened = ClayDocument::new(policy).expect("a document");
    reopened.open(&path).expect("reopen");

    let row = reopened
        .scene()
        .layers
        .last()
        .map(|layer| (layer.key, layer.representation))
        .expect("the row the crossing left");
    assert_eq!(
        row.1,
        Representation::Multires,
        "the row came back a hierarchy. Nothing in the .clayspace says so — \
         the engine reports its layer as a mesh layer, because the layer holds \
         the cage — so this is the side-car being load-bearing rather than \
         decorative"
    );
    let held = levels(&document, key);
    assert_ne!(
        held.sculpt, held.display,
        "the fixture left the two apart, so this round trip is asking about \
         both rather than about one number twice"
    );
    assert_eq!(
        levels(&reopened, row.0),
        held,
        "with the level count and both of the numbers on it"
    );
    assert_eq!(
        drawn(&mut reopened).len(),
        count,
        "and the same surface drawn from it"
    );
    let back = relief(&mut reopened);
    assert!(
        (back - standing).abs() < 1e-5,
        "and the sculpt itself: {standing} before the save, {back} after"
    );
    let _ = std::fs::remove_file(&path);
    let _ = std::fs::remove_file(clayspace_engine::multires::sidecar_for(&path));
}

/// A document whose side-car has gone opens as the cage it holds.
///
/// The three answers available were refusing the file, opening it and going on
/// calling the row a hierarchy, and opening it and calling the row what is
/// actually there. The first throws away a cage over a file this application
/// wrote beside the document and the sculptor may never have copied. The
/// second is the worst: a hierarchy that has silently lost every level is
/// indistinguishable from one that never had any, and the sculptor finds out
/// by subdividing on top of nothing.
#[test]
fn a_missing_side_car_opens_the_cage_rather_than_refusing_the_document() {
    let (mut document, _) = with_a_hierarchy("no-side-car", 2);
    assert!(dab(&mut document, [0.0, 0.0, 0.0], 0.5), "sculpt something");

    let path = scratch("no-side-car", "clayspace");
    document.save(&path).expect("save");
    std::fs::remove_file(clayspace_engine::multires::sidecar_for(&path))
        .expect("take the side-car away, as a copy that moved one file would");

    let policy = BackendPolicy::discover(None).expect("backends");
    let mut reopened = ClayDocument::new(policy).expect("a document");
    reopened.open(&path).expect(
        "the document still opens: the cage is real work and refusing \
                 the file would throw it away",
    );

    let row = reopened
        .scene()
        .layers
        .last()
        .expect("the row the crossing left")
        .clone();
    assert_eq!(
        row.representation,
        Representation::Mesh,
        "and it is a mesh layer, which is what it now is. The layer stack, the \
         workspace bar and the inspector all draw that differently, which is \
         how the loss is seen rather than discovered"
    );
    assert!(
        row.multires.is_none(),
        "with no hierarchy state claiming otherwise"
    );
    assert!(
        relief(&mut reopened) < 1e-6,
        "the surface is the flat cage, which is what was saved"
    );
    let _ = std::fs::remove_file(&path);
}

/// A record the engine will not reconstruct costs its row and is named.
#[test]
fn a_damaged_record_costs_one_row_and_says_which() {
    let (mut document, _) = with_a_hierarchy("damaged", 2);
    assert!(dab(&mut document, [0.0, 0.0, 0.0], 0.5), "sculpt something");

    let path = scratch("damaged", "clayspace");
    document.save(&path).expect("save");
    let sidecar = clayspace_engine::multires::sidecar_for(&path);
    // The header and the record line survive; the blob does not. A truncated
    // tail would be dropped before it was ever handed to the engine, so this
    // corrupts the body in place instead — which is the case that reaches
    // `deserialize` and has to be refused there.
    let mut bytes = std::fs::read(&sidecar).expect("read the side-car");
    let length = bytes.len();
    for byte in &mut bytes[length - 64..] {
        *byte ^= 0xff;
    }
    std::fs::write(&sidecar, bytes).expect("write it back damaged");

    let policy = BackendPolicy::discover(None).expect("backends");
    let mut reopened = ClayDocument::new(policy).expect("a document");
    reopened.open(&path).expect("the document still opens");
    assert_eq!(
        reopened
            .scene()
            .layers
            .last()
            .expect("the row the crossing left")
            .representation,
        Representation::Mesh,
        "the row is the cage"
    );
    let report = reopened.multires_diagnostics();
    assert_eq!(report.held, 0, "and no hierarchy was recovered");
    assert_eq!(
        report.lost.len(),
        1,
        "and the row is named as lost, which is what a sculptor pastes when \
         they ask why their sculpt came back flat: {report:?}"
    );
    let _ = std::fs::remove_file(&path);
    let _ = std::fs::remove_file(&sidecar);
}

// -- the revision -----------------------------------------------------------

/// A dab lands after the caches between it and the last one were released.
///
/// The case ClayCore v0.78.0 fixed, and the release notes are emphatic that it
/// is not a crash: a stale bind wrote into released storage, the level was
/// rebuilt from the authoritative detail before it was read back, and *the dab
/// simply was not there* — with the stamp still reporting the weld-class count
/// it believed it had moved. With a memory warning after every dab, which is
/// what an operating system does under pressure, every second dab vanished.
///
/// So this asserts what a sculptor would notice, twice over: the surface moved
/// again, and the number the viewport watches moved with it. The second half
/// is this application's own half of the hazard — a host that cached the level
/// mesh and did not compare would go on drawing the surface as it stood before
/// the dab, which looks exactly the same from the sculptor's chair.
#[test]
fn a_dab_lands_after_the_caches_under_it_were_released() {
    let (mut document, _) = with_a_hierarchy("released-caches", 3);

    assert!(dab(&mut document, [-0.4, 0.0, 0.0], 0.5), "the first dab");
    let after_one = relief(&mut document);
    let watched_one = document.mesh_revision();
    assert!(after_one > 0.0, "the first dab reached the surface");

    document
        .release_hierarchy_caches()
        .expect("release what is rebuildable");

    assert!(
        dab(&mut document, [0.7, 0.0, 0.0], 0.5),
        "the second dab reports that it moved something"
    );
    let after_two = relief(&mut document);
    assert!(
        after_two > 0.0,
        "and it is actually on the surface: {after_one} then {after_two}"
    );
    assert_ne!(
        document.mesh_revision(),
        watched_one,
        "and the viewport was told to look again, which is the half a report \
         of moved vertices cannot cover"
    );

    // Two dabs, two bumps: the count is what catches a second dab that landed
    // exactly where the first one did rather than where it was aimed.
    let bumps = drawn(&mut document)
        .iter()
        .filter(|point| point[1].abs() > after_one * 0.5)
        .fold(([0.0f32; 2], 0usize), |(mut sides, _), point| {
            sides[usize::from(point[0] > 0.0)] += 1.0;
            (sides, 0)
        })
        .0;
    assert!(
        bumps[0] > 0.0 && bumps[1] > 0.0,
        "one bump each side of the origin, where the two dabs were aimed: {bumps:?}"
    );
}

// -- undo -------------------------------------------------------------------

/// A gesture on a hierarchy is one undo, and it puts the form back exactly.
///
/// The ABI carries no delta record for one — clay.h says so twice, unprompted,
/// of the resolved stroke and of the layered transaction alike — so what the
/// history holds is the hierarchy's own serialized bytes. Exact is the whole
/// reason that is worth its size, so exactness is what is asserted: every
/// vertex back where it was, not a tolerance.
#[test]
fn a_gesture_on_a_hierarchy_is_one_undo_and_it_is_exact() {
    let (mut document, _) = with_a_hierarchy("one-undo", 3);
    let flat = drawn(&mut document);

    assert!(
        dab(&mut document, [0.2, 0.0, -0.3], 0.6),
        "sculpt something"
    );
    let sculpted = drawn(&mut document);
    assert_ne!(flat, sculpted, "the dab moved the surface");
    let watched = document.mesh_revision();

    assert!(document.undo().expect("undo"), "there is something to undo");
    assert_eq!(
        drawn(&mut document),
        flat,
        "and the form is back where it was, vertex for vertex"
    );
    assert_ne!(
        document.mesh_revision(),
        watched,
        "with the viewport told to look again"
    );

    assert!(document.redo().expect("redo"), "and it can be put back");
    assert_eq!(
        drawn(&mut document),
        sculpted,
        "exactly as it was, which is what makes the bytes worth holding"
    );
    assert_ne!(
        document.mesh_revision(),
        watched,
        "and the redo is seen too. The engine's own counters restart at one \
         whenever a hierarchy is rebuilt from bytes, so an undo and the redo \
         after it leave the same number over two different surfaces — this is \
         the case that needs a generation of this application's own"
    );
}

/// Two gestures are two undos, taken back newest first.
#[test]
fn two_gestures_are_two_undos() {
    let (mut document, _) = with_a_hierarchy("two-undos", 2);
    let flat = drawn(&mut document);
    assert!(dab(&mut document, [-0.6, 0.0, 0.0], 0.5), "the first");
    let after_one = drawn(&mut document);
    assert!(dab(&mut document, [0.6, 0.0, 0.0], 0.5), "the second");

    assert!(
        document.undo().expect("undo"),
        "the second comes back first"
    );
    assert_eq!(drawn(&mut document), after_one);
    assert!(document.undo().expect("undo"), "then the first");
    assert_eq!(drawn(&mut document), flat);
}

// -- the preflight ----------------------------------------------------------

/// A level that will not fit is refused, and the hierarchy is as deep as it was.
///
/// `clay_multires_add_level` is build-then-publish: it prices the level
/// against the budget the hierarchy was built with and refuses over it without
/// touching what is there. What is asserted here is both halves — the sentence
/// a sculptor reads, and that the refusal cost them nothing.
#[test]
fn a_level_that_does_not_fit_is_refused_and_costs_nothing() {
    // Deep enough that the next one cannot fit: each level multiplies faces by
    // four, so the ceiling is reached by a factor rather than by a margin.
    let (mut document, key) = with_a_hierarchy("over-budget", 0);
    let mut deepest = 0;
    let refusal = loop {
        match document.apply_multires_level_op(MultiresLevelOp::AddLevel) {
            Ok(()) => {
                deepest += 1;
                assert!(
                    deepest < 12,
                    "the budget refused nothing in twelve levels, so this \
                     fixture is not testing what it says it is"
                );
            }
            Err(e) => break e,
        }
    };
    let ModelError::Conversion(refusal) = refusal else {
        panic!("a refusal a sculptor can act on, not an engine result code: {refusal}");
    };
    let (peak_bytes, budget_bytes) = match refusal {
        Refusal::LevelOverBudget {
            peak_bytes,
            budget_bytes,
        } => (peak_bytes, budget_bytes),
        other => panic!("the budget is what refused, and it says so: {other}"),
    };
    assert!(
        peak_bytes > budget_bytes,
        "and it is the *peak* during the build that is stated rather than what \
         would remain after it, because on a constrained machine the \
         high-water mark is what ends the session: {peak_bytes} against \
         {budget_bytes}"
    );

    let held = levels(&document, key);
    assert_eq!(
        held.count,
        deepest + 1,
        "and the hierarchy is exactly as deep as it was before the refusal — \
         build-then-publish, so nothing half-built is left standing"
    );
    // And the surface is still there and still drawable.
    assert!(!drawn(&mut document).is_empty());

    // What the interface is shown before the button is pressed.
    let priced = document
        .subdivision_cost()
        .expect("the engine prices the level that would come next");
    assert_eq!(
        priced.level, held.count,
        "the level that would come into being"
    );
    assert!(
        priced.within(budget_bytes).is_err(),
        "and the same refusal is available before anything is attempted, which \
         is what lets a control be greyed rather than pressed and refused"
    );
}

// -- crossing in and out ----------------------------------------------------

/// The engine refuses a cage rather than repairing it, and names the fault.
///
/// Measured on the geometry this application actually produces: marched output
/// from the starting form. It is refused, and that is the honest answer — a
/// conversion that quietly welded a face would change retopology somebody paid
/// for without saying so, and a cage is precisely the thing whose topology is
/// the work.
#[test]
fn a_marched_mesh_is_not_a_cage_and_the_refusal_says_which_fault() {
    let policy = BackendPolicy::discover(None).expect("backends");
    let mut document = ClayDocument::new(policy)
        .and_then(ClayDocument::with_starting_form)
        .expect("a document with a starting form");
    let path = scratch("not-a-cage", "obj");
    document
        .export_mesh(&path, ExportSettings::default())
        .expect("export the marched form");
    document
        .import_mesh(&path, ImportSettings::default())
        .expect("import it back");
    let _ = std::fs::remove_file(&path);

    let mesh = document
        .scene()
        .layers
        .iter()
        .find(|layer| layer.representation == Representation::Mesh)
        .map(|layer| layer.key)
        .expect("the import is a mesh layer");
    document.set_active_layer(mesh).expect("activate it");

    let settings = ConversionSettings::default();
    let refused = document
        .convert_layer(Direction::MeshToMultires, settings.cell_size, settings.blur)
        .expect_err("marched output is not a subdivision cage");
    let ModelError::Conversion(Refusal::NotACage { fault }) = refused else {
        panic!("named as the mesh's own fault, not as an engine code: {refused}");
    };
    assert_eq!(
        fault,
        CageFault::DegenerateFace,
        "and it says which fault, so the sculptor goes back to the mesh rather \
         than looking for a setting"
    );
    assert_eq!(
        document
            .scene()
            .layer(mesh)
            .expect("the row")
            .representation,
        Representation::Mesh,
        "and the source is untouched"
    );
}

/// Baking a level out gives an ordinary mesh with the sculpt on it.
#[test]
fn a_level_bakes_out_as_a_mesh_that_keeps_the_sculpt() {
    let (mut document, _) = with_a_hierarchy("bake-out", 2);
    assert!(dab(&mut document, [0.1, 0.0, 0.1], 0.6), "sculpt something");
    let standing = relief(&mut document);
    let count = drawn(&mut document).len();

    let settings = ConversionSettings::default();
    let baked = document
        .convert_layer_in_place(Direction::MultiresToMesh, settings.cell_size, settings.blur)
        .expect("a level is a mesh");
    assert_eq!(
        document
            .scene()
            .layer(baked)
            .expect("the row")
            .representation,
        Representation::Mesh
    );
    assert_eq!(
        drawn(&mut document).len(),
        count,
        "the level's own vertices, not the cage's"
    );
    assert!(
        (relief(&mut document) - standing).abs() < 1e-5,
        "with the sculpt where it was — nothing is resampled by this crossing"
    );
    assert!(
        document.multires_diagnostics().held == 0,
        "and nothing is holding a hierarchy any more"
    );
}

// -- the pass stack ---------------------------------------------------------
//
// A pass is a stroke you can dial back afterwards, and "afterwards" is the
// whole of it. These measure that: a pass takes the stroke, and its slider
// still moves the surface long after the pointer came up. Everything else here
// guards the four ways this stack differs from a grid's — it is addressed by
// id, a reorder moves nothing, a lock refuses coefficients and permits every
// property, and a composition change waits for the stroke to close.

use clayspace_model::{MultiresSculptLayerId, MultiresSculptLayerOp as PassOp};

/// The stack as the layer row would draw it.
fn passes(document: &ClayDocument, key: LayerKey) -> Vec<clayspace_model::MultiresSculptLayer> {
    document
        .scene()
        .layer(key)
        .and_then(|layer| layer.multires.as_ref())
        .map(|state| state.sculpt_layers.clone())
        .expect("the row is a hierarchy")
}

fn active_pass(document: &ClayDocument, key: LayerKey) -> MultiresSculptLayerId {
    document
        .scene()
        .layer(key)
        .and_then(|layer| layer.multires.as_ref())
        .map(|state| state.active_sculpt_layer)
        .expect("the row is a hierarchy")
}

/// Adds a pass and answers its id.
fn add_pass(document: &mut ClayDocument, key: LayerKey, name: &str) -> MultiresSculptLayerId {
    document
        .apply_multires_sculpt_layer_op(PassOp::Add {
            name: name.to_string(),
        })
        .expect("a hierarchy takes a pass");
    passes(document, key)
        .last()
        .expect("the pass that was just added")
        .id
}

/// A dab into a pass is still there to dial an hour later.
///
/// The property the whole stack exists for, and the one an interface would
/// otherwise quietly lose: a strength that only worked during the stroke that
/// made the pass would be a stroke modifier wearing a layer's clothes. So the
/// dab is made, the gesture is closed, and only *then* is the slider moved —
/// twice, in both directions, with the surface measured each time.
///
/// Zero is asserted as a return to the flat sheet rather than as "less": a
/// pass at zero strength contributes exactly nothing, which is what makes
/// hiding one and comparing an exact test rather than an approximate one.
#[test]
fn a_pass_is_still_dialable_long_after_the_stroke_that_filled_it() {
    let (mut document, key) = with_a_hierarchy("dialable", 2);
    let flat = relief(&mut document);

    let pass = add_pass(&mut document, key, "Rugas");
    assert_eq!(
        active_pass(&document, key),
        pass,
        "a new pass takes the stroke, or a sculptor has to find the control \
         that says so before anything they do lands anywhere"
    );

    assert!(dab(&mut document, [0.0, 0.0, 0.0], 0.9), "sculpt something");
    let standing = relief(&mut document);
    assert!(
        standing > flat + 1e-4,
        "the dab reached the surface: {standing} against {flat}"
    );

    // The gesture is over. Everything below is the slider alone.
    document
        .apply_multires_sculpt_layer_op(PassOp::SetStrength {
            id: pass,
            strength: 0.0,
        })
        .expect("dial it back");
    let dialled_out = relief(&mut document);
    assert!(
        (dialled_out - flat).abs() < 1e-5,
        "a pass at zero contributes exactly nothing, and this left {dialled_out} \
         where the untouched sheet stands at {flat}"
    );

    document
        .apply_multires_sculpt_layer_op(PassOp::SetStrength {
            id: pass,
            strength: 1.0,
        })
        .expect("dial it back in");
    assert!(
        (relief(&mut document) - standing).abs() < 1e-5,
        "and dialling it back in restores exactly what was there, with no \
         stroke replayed"
    );

    // Visibility is the same statement made with one click instead of a drag.
    document
        .apply_multires_sculpt_layer_op(PassOp::SetVisible {
            id: pass,
            visible: false,
        })
        .expect("hide it");
    assert!(
        (relief(&mut document) - flat).abs() < 1e-5,
        "hiding a pass removes its contribution bit for bit"
    );
}

/// With no pass made, the stroke goes into the form, as it always did.
///
/// The compatibility half: a sculptor who never opens the stack must not have
/// to. `MultiresSculptLayerId::BASE` is what an empty stack reads as, and the
/// stroke that follows takes the plain sculptor's path.
#[test]
fn a_stroke_lands_in_the_form_where_no_pass_has_been_made() {
    let (mut document, key) = with_a_hierarchy("no-pass", 2);
    assert!(
        active_pass(&document, key).is_base(),
        "a fresh hierarchy has no pass, so the stroke has nowhere else to go"
    );
    assert!(passes(&document, key).is_empty());

    assert!(dab(&mut document, [0.0, 0.0, 0.0], 0.9), "sculpt something");
    assert!(
        relief(&mut document) > 1e-4,
        "and it reached the form under the passes"
    );
    assert!(
        passes(&document, key).is_empty(),
        "without minting a pass nobody asked for"
    );
}

/// Selecting the form under the passes sends the next stroke back into it.
///
/// This is the whole of the write domain as this application expresses it:
/// there is no three-way control, there is a row for the form and a row per
/// pass, and which one is selected is the answer. So a sculptor fixing the
/// anatomy under a set of wrinkles selects the form, and the wrinkles are left
/// exactly where they were.
#[test]
fn selecting_the_form_leaves_the_passes_untouched() {
    let (mut document, key) = with_a_hierarchy("the-form", 2);
    let pass = add_pass(&mut document, key, "Rugas");
    assert!(dab(&mut document, [0.6, 0.0, 0.6], 0.5), "fill the pass");
    let with_the_pass = relief(&mut document);

    document
        .apply_multires_sculpt_layer_op(PassOp::SetActive {
            id: MultiresSculptLayerId::BASE,
        })
        .expect("select the form");
    assert!(
        dab(&mut document, [-0.6, 0.0, -0.6], 0.5),
        "sculpt the form"
    );

    let coverage = passes(&document, key)
        .iter()
        .find(|row| row.id == pass)
        .expect("the pass is still there")
        .coverage_vertices;
    document
        .apply_multires_sculpt_layer_op(PassOp::SetVisible {
            id: pass,
            visible: false,
        })
        .expect("hide the pass");
    let form_alone = relief(&mut document);
    assert!(
        form_alone > 1e-4,
        "the second dab went into the form, so hiding the pass leaves it: \
         {form_alone}"
    );
    assert!(
        with_the_pass > 1e-4 && coverage > 0,
        "and the first dab is still the pass's own: {with_the_pass}, over \
         {coverage} vertices"
    );
}

/// A reorder is organisation and never geometry.
///
/// The load-bearing difference from a grid's stack, and the one an interface
/// is most likely to get wrong: passes here are additive and therefore
/// commute, so a list drag must not re-evaluate a surface millions of vertices
/// wide. Measured vertex by vertex rather than asserted.
#[test]
fn sliding_a_pass_through_the_stack_moves_no_vertex() {
    let (mut document, key) = with_a_hierarchy("reorder", 2);
    let lower = add_pass(&mut document, key, "Baixo");
    assert!(dab(&mut document, [0.5, 0.0, 0.5], 0.5), "fill the lower");
    let upper = add_pass(&mut document, key, "Alto");
    assert!(dab(&mut document, [-0.5, 0.0, -0.5], 0.5), "fill the upper");

    let before = drawn(&mut document);
    document
        .apply_multires_sculpt_layer_op(PassOp::Move { id: upper, to: 0 })
        .expect("slide it to the bottom");
    let after = drawn(&mut document);

    assert_eq!(before.len(), after.len(), "and the same vertices");
    assert!(
        before == after,
        "a reorder moved the surface, which an additive stack cannot do — the \
         interface is now paying for a re-evaluation on a list drag"
    );
    let order: Vec<_> = passes(&document, key).iter().map(|row| row.id).collect();
    assert_eq!(
        order,
        vec![upper, lower],
        "while the stack itself did reorder, or nothing was measured"
    );
}

/// A lock refuses a stroke and permits every property change.
///
/// Both halves, because a lock that also froze the name and the slider would
/// mean "hide from the interface", which is a different feature — and because
/// a lock a sculptor could not undo from the row that shows it would be a
/// trap.
#[test]
fn a_locked_pass_refuses_the_stroke_and_takes_every_other_change() {
    let (mut document, key) = with_a_hierarchy("locked", 2);
    let pass = add_pass(&mut document, key, "Rugas");
    document
        .apply_multires_sculpt_layer_op(PassOp::SetLocked {
            id: pass,
            locked: true,
        })
        .expect("lock it");

    document.begin_gesture();
    let refused = document.apply_stroke(
        ToolKind::Padrao,
        BrushSettings {
            size: 0.9,
            intensity: 1.0,
            ..BrushSettings::default()
        },
        &[GestureSample {
            position: [0.0; 3],
            pressure: 1.0,
            time: 0.0,
        }],
        [false; 3],
    );
    document.end_gesture();
    assert!(
        refused.is_err(),
        "a locked pass took a stroke, so the lock is decoration"
    );

    for op in [
        PassOp::Rename {
            id: pass,
            name: "Rugas finas".to_string(),
        },
        PassOp::SetStrength {
            id: pass,
            strength: 0.5,
        },
        PassOp::SetVisible {
            id: pass,
            visible: false,
        },
        PassOp::SetLocked {
            id: pass,
            locked: false,
        },
    ] {
        let label = op.label();
        document
            .apply_multires_sculpt_layer_op(op)
            .unwrap_or_else(|e| panic!("a lock refused {label}, which it guards no part of: {e}"));
    }
}

/// A composition change waits for the pointer to come up.
///
/// The engine refuses it, and the refusal is right rather than a limitation: a
/// stamp reads the evaluated surface, so a slider moved between two stamps
/// would author one gesture against two different surfaces. The three that
/// move no vertex go through.
#[test]
fn the_composition_is_held_while_a_gesture_is_open() {
    let (mut document, key) = with_a_hierarchy("held", 2);
    let pass = add_pass(&mut document, key, "Rugas");

    document.begin_gesture();
    let _ = document.apply_stroke(
        ToolKind::Padrao,
        BrushSettings {
            size: 0.9,
            intensity: 1.0,
            ..BrushSettings::default()
        },
        &[GestureSample {
            position: [0.0; 3],
            pressure: 1.0,
            time: 0.0,
        }],
        [false; 3],
    );

    let refused = document.apply_multires_sculpt_layer_op(PassOp::SetStrength {
        id: pass,
        strength: 0.25,
    });
    assert!(
        refused.is_err(),
        "a slider moved mid-gesture, so one stroke is being authored against \
         two different surfaces"
    );
    document
        .apply_multires_sculpt_layer_op(PassOp::Rename {
            id: pass,
            name: "Rugas finas".to_string(),
        })
        .expect("a rename moves no vertex and is allowed through");

    document.end_gesture();
    document
        .apply_multires_sculpt_layer_op(PassOp::SetStrength {
            id: pass,
            strength: 0.25,
        })
        .expect("and the slider works again once the pointer is up");
}

/// A layer that is not a hierarchy says what a pass belongs to.
///
/// Named rather than generic, exactly as the level operations are: a sculptor
/// on a field or a mesh needs to know that passes are a hierarchy's, not that
/// "this failed".
#[test]
fn a_field_layer_says_where_passes_live() {
    let policy = BackendPolicy::discover(None).expect("discover backends");
    let mut document = ClayDocument::new(policy).expect("a document");
    let refusal = document
        .apply_multires_sculpt_layer_op(PassOp::Add {
            name: "Rugas".to_string(),
        })
        .expect_err("a field has no pass stack");
    let said = refusal.to_string();
    assert!(
        said.to_lowercase().contains("multires"),
        "the refusal has to name the representation a pass belongs to: {said}"
    );
}
