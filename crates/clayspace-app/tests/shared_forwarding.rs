//! The shared document answers for itself, method by method.
//!
//! `SharedDocument` is not a test double. Every trait it implements is
//! implemented by the one real document behind it, so a *provided* method left
//! unforwarded is not a partial implementation that says so — it is the
//! trait's default quietly answering on the document's behalf, and the default
//! is written to be inert. `shared.rs` carries a note about this beside
//! `SculptModel::set_combine`, which is the one that was found first: the
//! options bar dispatched, the ViewModel called the model, the default
//! discarded it, and fourteen combine operations drew the same picture.
//!
//! `ObjectModel` had the same hole in three places. `mesh_operands` answered
//! with the empty default, so the shapes picker in the running application
//! offered no imported model to place, and the placement behind it could only
//! ever have refused. These hold that the document's own answers get through.

use clayspace_app::SharedDocument;
use clayspace_engine::{BackendPolicy, ClayDocument};
use clayspace_model::{
    Combine, CombineSettings, ExchangeModel, ExportSettings, ImportAs, ImportSettings, ObjectModel,
    Representation, SceneModel, Shape,
};

fn shared() -> SharedDocument {
    let policy = BackendPolicy::discover(None).expect("discover backends");
    let document = ClayDocument::new(policy)
        .and_then(ClayDocument::with_starting_form)
        .expect("a document with a starting form");
    SharedDocument::new(document)
}

fn adding() -> CombineSettings {
    CombineSettings {
        op: Combine::Add,
        ..CombineSettings::default()
    }
}

/// A file to import, since a mesh layer is the one thing a fresh document has
/// none of.
fn a_mesh_on_disk(document: &mut SharedDocument, who: &str) -> std::path::PathBuf {
    let path = std::env::temp_dir().join(format!("clayspace-shared-{who}.obj"));
    let _ = std::fs::remove_file(&path);
    document
        .export_mesh(&path, ExportSettings::default())
        .expect("something to import");
    path
}

#[test]
fn the_shared_document_offers_the_mesh_layers_it_holds() {
    let mut document = shared();
    let path = a_mesh_on_disk(&mut document, "operands");
    document
        .import_mesh(
            &path,
            ImportSettings {
                becomes: ImportAs::Reference,
                ..Default::default()
            },
        )
        .expect("import a mesh");

    assert!(
        !document.mesh_operands().is_empty(),
        "the shared document answered with the trait's empty default, so the \
         picker offers no imported model to place"
    );
    let (from, _) = document.mesh_operands()[0];
    assert!(
        document.mesh_operand_cost(from, 0.02).is_some(),
        "the crossing's cost is not being stated, so consent is being asked \
         for something unstated"
    );

    // Back onto the field layer, which is the only place an object can live.
    let field = document
        .scene()
        .layers
        .iter()
        .find(|layer| layer.representation == Representation::Sdf)
        .map(|layer| layer.key)
        .expect("a field layer");
    document.set_active_layer(field).expect("work on the field");
    assert!(
        document
            .place_mesh_object(from, 0.02, [0.0; 3], adding())
            .is_ok(),
        "placing the chosen mesh reached the trait's refusing default"
    );
    let _ = std::fs::remove_file(&path);
}

#[test]
fn the_shared_document_inserts_and_copies_subtools() {
    let mut document = shared();
    let before = document.scene().layers.len();

    let inserted = document
        .insert_shape_subtool(Shape::Sphere, &[0.5], [3.0, 0.0, 0.0], adding())
        .expect("a sphere as its own subtool");
    assert_eq!(document.scene().layers.len(), before + 1);
    assert_eq!(document.scene().active, Some(inserted.layer));

    assert!(
        !document.copyable_subtools().is_empty(),
        "the copy control would be offered nothing to copy"
    );
    let copy = document
        .copy_subtool(inserted.layer, 0.02)
        .expect("a copy of the subtool just inserted");
    assert_eq!(document.scene().layers.len(), before + 2);
    assert_ne!(copy.layer, inserted.layer);
}

/// The four the boolean panel asks: what could take part, what a pair would be
/// sampled at, what it would cost, and the operation itself. Every one of them
/// is provided on the trait, so an unforwarded one would leave the panel with
/// an empty list, no price and a refusal.
#[test]
fn the_shared_document_answers_for_the_subtool_boolean() {
    use clayspace_model::{BooleanOp, BooleanSettings};

    let mut document = shared();
    let base = document.scene().active.expect("a starting layer");
    let tool = document
        .insert_shape_subtool(Shape::Sphere, &[0.8], [0.6, 0.0, 0.0], adding())
        .expect("a second subtool")
        .layer;

    let offered = document.boolean_operands();
    assert!(
        offered.iter().any(|(key, _)| *key == base) && offered.iter().any(|(key, _)| *key == tool),
        "the boolean panel would be offered nothing to combine"
    );
    let settings = BooleanSettings {
        base: Some(base),
        tool: Some(tool),
        op: BooleanOp::Subtract,
        cell_size: 0.04,
        consume: false,
    };
    assert!(
        document.boolean_cell(base, tool).is_some(),
        "the panel has no default resolution to start from"
    );
    let cost = document
        .boolean_cost(settings)
        .expect("the panel has no cost to state");
    assert!(cost.cells > 0);

    let before = document.scene().layers.len();
    let result = document.run_boolean(settings).expect("the boolean runs");
    assert_eq!(document.scene().layers.len(), before + 1);
    assert_eq!(document.scene().active, Some(result.layer));
}

/// A loaded alpha stamp reaches the document.
///
/// `set_alpha` and `alpha_name` are provided, and the shared document did not
/// forward either — so the interface loaded a stamp, the ViewModel handed it
/// over, the default swallowed it, and the options bar went on reporting that
/// no stamp was in use because the default answers `None`. Found by the
/// structural check below rather than by anyone noticing.
#[test]
fn the_shared_document_keeps_the_alpha_it_is_given() {
    use clayspace_model::{Alpha, SculptModel};

    let mut shared = shared();
    assert_eq!(SculptModel::alpha_name(&shared), None);

    SculptModel::set_alpha(
        &mut shared,
        Some(Alpha {
            name: "granito".into(),
            width: 2,
            height: 2,
            samples: vec![0.0, 1.0, 1.0, 0.0],
        }),
    );
    assert_eq!(
        SculptModel::alpha_name(&shared).as_deref(),
        Some("granito"),
        "the stamp did not reach the document, so nothing would stamp with it"
    );
}

/// Every *provided* method of every model trait the shared document
/// implements is overridden by it.
///
/// The tests above are hand-written, one per method someone remembered — which
/// is the same shape as the bug they guard against. Two were forgotten and
/// nothing said so: `begin_gesture` and `end_gesture` were never forwarded, so
/// no mesh gesture in the running application was ever previewed, and the
/// preview path went years without being reached. The tests that would have
/// exercised it drove the document directly.
///
/// So this one is not written per method. It reads the traits, finds every
/// method that has a body — a default the trait will quietly answer with — and
/// requires `shared.rs` to name it. Adding a provided method to a model trait
/// now fails here until it is forwarded.
///
/// Deliberately crude about Rust syntax: it is looking for `fn name(` at the
/// trait's indentation and whether the signature's line ends in `{`, which is
/// what the whole file is written in. A false positive here is a method that
/// gets forwarded needlessly, which costs a line; a false negative is the bug
/// above.
#[test]
fn every_provided_method_of_every_model_trait_is_forwarded() {
    let model = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../clayspace-model/src");
    let shared = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/shared.rs"),
    )
    .expect("read shared.rs");

    // The traits the shared document implements, and the file each lives in.
    let traits = [
        ("SculptModel", "sculpt.rs"),
        ("SceneModel", "scene.rs"),
        ("DocumentModel", "document.rs"),
        ("MaskModel", "mask.rs"),
        ("CurveModel", "curve.rs"),
        ("LatticeModel", "lattice.rs"),
        ("ArmatureModel", "armature.rs"),
        ("ExchangeModel", "exchange.rs"),
        ("ObjectModel", "shape.rs"),
    ];

    // Provided methods that are *derived* rather than inert: their default
    // body is written in terms of other trait methods, so it reaches the
    // document through those and forwarding it would only be a second copy of
    // the same composition. Everything else must be forwarded.
    const DERIVED: [&str; 3] = [
        // Composes active_representation, _editable, _visible and
        // _carries_geometry, all forwarded.
        "SculptModel::active_layer_state",
        // Composes curve(), forwarded.
        "CurveModel::curve_pivot",
        // Builds a refusal out of constants; asks the document nothing.
        "ObjectModel::no_objects_here",
    ];

    let mut missing = Vec::new();
    for (name, file) in traits {
        let source = std::fs::read_to_string(model.join(file)).expect("read a model source");
        for method in provided_methods(&source, name) {
            let full = format!("{name}::{method}");
            if !DERIVED.contains(&full.as_str()) && !shared.contains(&format!("fn {method}(")) {
                missing.push(full);
            }
        }
    }

    assert!(
        missing.is_empty(),
        "the shared document does not forward these, so the trait's default \
         answers on the document's behalf and nothing reports it: {missing:?}"
    );
}

/// The names of the methods `trait_name` gives a body to.
fn provided_methods(source: &str, trait_name: &str) -> Vec<String> {
    let Some(start) = source.find(&format!("pub trait {trait_name} {{")) else {
        panic!("{trait_name} is not in the source this test reads");
    };
    let mut methods = Vec::new();
    let mut depth = 0usize;
    for line in source[start..].lines().skip(1) {
        // The trait's own body is at one level of indentation; anything deeper
        // belongs to a method that has one, which is what makes it provided.
        let opens = line.matches('{').count();
        let closes = line.matches('}').count();
        if depth == 0 && line.starts_with('}') {
            break;
        }
        let trimmed = line.trim_start();
        if depth == 0 {
            if let Some(rest) = trimmed.strip_prefix("fn ") {
                // A body on this line, or opened by it: either way, provided.
                if line.trim_end().ends_with('{') {
                    let name: String = rest
                        .chars()
                        .take_while(|c| *c != '(' && *c != '<')
                        .collect();
                    methods.push(name);
                }
            }
        }
        depth = depth + opens - closes.min(depth + opens);
    }
    methods
}
