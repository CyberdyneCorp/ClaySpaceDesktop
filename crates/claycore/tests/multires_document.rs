//! What a `.clayspace` carries of a multiresolution surface, and what it does
//! not.
//!
//! ClayCore v0.78.0 states this first among its known limits, and it is the
//! one that costs a host the most: "a `.clayspace` does not carry a multires
//! hierarchy or an adaptive surface. Both are opaque and owning by design and
//! live beside the document; `clay_multires_serialize` gives you the bytes and
//! where they go is the host's decision. A host that saves only the document
//! saves only the cage's layer, not the sculpt on it."
//!
//! That sentence is easy to read as a note about plumbing and it is not. A
//! `clay_multires` is not a `clay_layer_id` — it is a free-standing owning
//! handle that took a *copy* of the cage on the way in, so the document it was
//! built from does not know it exists and cannot be made to. Saving is not
//! lossy here in the way a dropped attribute is lossy; the sculpt is simply
//! not among the things `clay_document_save` is being asked about.
//!
//! So these are tripwires held the way this repository holds them elsewhere:
//! the limit measured rather than described, failing the day the engine closes
//! it. `a_dab_on_a_hierarchy_does_not_reach_the_document_it_was_built_from` is
//! the whole claim in one comparison — the same document saved either side of
//! a dab, byte for byte identical — and
//! `the_hierarchys_bytes_are_the_hosts_to_place_and_they_come_back` is the
//! seam the host has to build against, run here so that the shape of the
//! answer is measured before anything depends on it.
//!
//! **For the phase that makes multires a representation in this application:**
//! these two together are the requirement. A `<path>.multires/` beside the
//! `.clayspace` is what the survey settled on and what the second test here
//! stands in for; whatever that side-car turns out to be, the first test is
//! the reason it cannot be skipped, and it should keep failing to save the
//! sculpt through `Document::save` for as long as the ABI says it does.

use claycore::{Document, Mesh, MeshBrush, MeshLayerDesc, MeshStamp, Multires, MultiresDesc};

// -- fixtures ---------------------------------------------------------------

/// A flat grid of quads, which is what a Catmull-Clark cage is supposed to be.
///
/// Built through a file for the reason `tests/multires.rs` gives: the C ABI
/// builds a mesh from an importer or from the mesher and offers no way to hand
/// it arrays, and the readers triangulate on the way in — which is fine for a
/// cage, since the subdivision rule is defined over faces of any arity.
fn cage(divisions: usize, half: f32, name: &str) -> Mesh {
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
    let path = scratch(&format!("cage-{name}"), "obj");
    std::fs::write(&path, text).expect("write the cage");
    let mesh = Mesh::load(&path).expect("load the cage");
    let _ = std::fs::remove_file(&path);
    mesh
}

/// A path in the temporary directory, unique to this process and this name.
fn scratch(name: &str, extension: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "claycore-multires-document-{name}-{}.{extension}",
        std::process::id()
    ))
}

/// A document holding one mesh layer, which is the cage, plus the hierarchy
/// built from it. The two are separate objects from here on.
fn cage_and_hierarchy(name: &str) -> (Document, Multires) {
    let mesh = cage(4, 2.0, name);
    let mut document = Document::new().expect("document");
    document
        .attach_mesh_layer(&mesh, &MeshLayerDesc::named("cage"))
        .expect("the cage is a mesh layer");
    let mut surface =
        Multires::from_mesh(&mesh, MultiresDesc::default()).expect("the cage is a hierarchy");
    surface.add_level().expect("subdivide once");
    surface.add_level().expect("subdivide twice");
    (document, surface)
}

/// One Draw dab at the hierarchy's finest level.
fn wrinkle(surface: &mut Multires) {
    let mut sculptor = surface.sculptor().expect("sculptor");
    sculptor
        .surface_mut()
        .set_sculpt_level(2)
        .expect("bind the finest level");
    sculptor.begin_stroke().expect("begin");
    sculptor
        .stamp(
            MeshStamp {
                verb: MeshBrush::Draw,
                center: [0.4, 0.0, -0.3],
                radius: 0.9,
                strength: 1.0,
                geodesic: false,
                ..Default::default()
            },
            None,
        )
        .expect("stamp");
}

/// How far the finest level stands off the flat sheet it was subdivided from.
fn relief(surface: &mut Multires) -> f32 {
    surface
        .copy_level_mesh(2)
        .expect("a level is a mesh")
        .positions()
        .iter()
        .map(|p| p[1].abs())
        .fold(0.0f32, f32::max)
}

// -- the limit --------------------------------------------------------------

/// The tripwire. A dab on a hierarchy changes nothing a `.clayspace` records.
///
/// Held as a byte comparison rather than as an absence, because an absence is
/// hard to assert and easy to assert vacuously: the same document is saved
/// before the dab and after it, and the two files are identical. There is no
/// route from `clay_multires_sculptor_stamp` to `clay_document_save`, so the
/// sculpt is not merely omitted from the file — it is not a thing the document
/// has an opinion about.
///
/// Measured on ClayCore v0.78.0: the dab takes the finest level's relief from
/// 0.000 to 0.883 and the hierarchy's own blob to 13,128 bytes, while the two
/// saves come out at 812 bytes each and byte for byte identical. The sculpt is
/// sixteen times the document and none of it is in the document.
///
/// When this fails, `clay_document_save` has started carrying the surface and
/// the side-car this application owes beside every `.clayspace` need not be
/// built at all — or, if it has been by then, comes out the way
/// `clayspace_engine::objects`'s table will when node readback is wrapped.
#[test]
fn a_dab_on_a_hierarchy_does_not_reach_the_document_it_was_built_from() {
    let (document, mut surface) = cage_and_hierarchy("unsaved");

    let flat = relief(&mut surface);
    assert!(
        flat < 1e-4,
        "the fixture's cage is not flat ({flat}), so a relief afterwards says \
         nothing about the dab"
    );

    let before_path = scratch("before", "clayspace");
    document.save(&before_path).expect("save the document");
    let before = std::fs::read(&before_path).expect("read what was written");

    wrinkle(&mut surface);
    let sculpted = relief(&mut surface);
    assert!(
        sculpted > 0.1,
        "the dab did not move the finest level ({sculpted}), so the comparison \
         below would pass for the wrong reason"
    );

    let after_path = scratch("after", "clayspace");
    document.save(&after_path).expect("save it again");
    let after = std::fs::read(&after_path).expect("read what was written");

    println!(
        "  relief {flat:.3} -> {sculpted:.3}; document {} -> {} bytes; blob {} bytes",
        before.len(),
        after.len(),
        surface
            .serialize()
            .expect("the hierarchy's own bytes")
            .len()
    );

    assert_eq!(
        before, after,
        "a `.clayspace` now changes when the hierarchy built from its cage is \
         sculpted. That is good news and it is the largest integration cost in \
         ClayCore v0.78.0 going away: the multires side-car is owed only \
         because a document carries the cage and not the sculpt on it, and it \
         is no longer owed"
    );

    let _ = std::fs::remove_file(&before_path);
    let _ = std::fs::remove_file(&after_path);
}

/// And what comes back through the document alone is the cage, unsculpted.
///
/// The other half of the same limit, said the way a host meets it: reopen the
/// file and the mesh layer is there with every vertex where it was put, which
/// is exactly what makes the loss silent. Nothing refuses, nothing warns, and
/// a sculptor who saved and reopened has a cage.
#[test]
fn a_reopened_document_holds_the_cage_with_no_sign_a_hierarchy_ever_stood_on_it() {
    let (document, mut surface) = cage_and_hierarchy("reopened");
    wrinkle(&mut surface);

    let path = scratch("cage-only", "clayspace");
    document.save(&path).expect("save the document");
    let mut back = Document::open(&path).expect("reopen it");
    let _ = std::fs::remove_file(&path);

    let layers = back.layer_ids().expect("the layers it came back with");
    assert_eq!(
        layers.len(),
        1,
        "the cage is the only layer in the document"
    );

    let (positions, _, _, indices) = back.read_mesh_layer("cage").expect("the cage reads back");
    assert!(
        positions.iter().all(|p| p[1].abs() < 1e-4),
        "the reopened cage is not flat, so it carried something of the sculpt"
    );
    println!(
        "  the cage came back as {} vertices and {} triangles, at y = 0 \
         throughout, against a hierarchy standing {:.3} off it",
        positions.len(),
        indices.len() / 3,
        relief(&mut surface)
    );
}

/// The seam, run so that the host's half is measured rather than assumed.
///
/// `clay_multires_serialize` is the only route the sculpt has to disk, and the
/// blob round-trips exactly — same level count, same finest-level relief, and
/// the detail checksum the hierarchy answers with is the one it went in with.
/// So the side-car is buildable, and what it costs is a second file per
/// document rather than a lossy save.
///
/// This is not a tripwire. It stands beside the two above to say what the
/// application has to build, and to pin the shape of it before anything does.
#[test]
fn the_hierarchys_bytes_are_the_hosts_to_place_and_they_come_back() {
    let (_document, mut surface) = cage_and_hierarchy("sidecar");
    wrinkle(&mut surface);

    let sculpted = relief(&mut surface);
    let checksum = surface.detail_checksum().expect("a checksum");
    let bytes = surface.serialize().expect("the hierarchy's own bytes");

    let path = scratch("beside", "mrs");
    std::fs::write(&path, &bytes).expect("write the blob beside the document");
    let mut back =
        Multires::deserialize(&std::fs::read(&path).expect("read it back")).expect("deserialize");
    let _ = std::fs::remove_file(&path);

    assert_eq!(
        back.level_count(),
        surface.level_count(),
        "the blob came back with a different number of levels"
    );
    assert_eq!(
        back.detail_checksum().expect("a checksum"),
        checksum,
        "the detail did not survive the round trip the side-car would make"
    );
    let recovered = relief(&mut back);
    assert!(
        (recovered - sculpted).abs() < 1e-5,
        "the recovered hierarchy stands {recovered:.4} off the cage against \
         {sculpted:.4} going in"
    );
    println!(
        "  {} bytes carry the sculpt the document does not",
        bytes.len()
    );
}

// -- the two seams a host builds the representation on ----------------------

/// A hierarchy built over the layer's own cage, without the host copying it.
///
/// The route that matters for a host whose cage is already a document layer:
/// `Multires::from_mesh` wants a `&Mesh` and a layer's mesh is the document's
/// to lend, so the borrow that Rust will not hold twice is taken once inside
/// the crate. What comes back has to be the *same* hierarchy the copy route
/// builds, which is what is measured here rather than assumed: a level count,
/// a vertex count and a detail checksum, all three against a hierarchy built
/// the long way from the same triangles.
#[test]
fn a_hierarchy_is_built_over_a_layer_the_document_already_holds() {
    let mesh = cage(4, 2.0, "over-a-layer");
    let mut document = Document::new().expect("document");
    let layer = document
        .attach_mesh_layer(&mesh, &MeshLayerDesc::named("cage"))
        .expect("the cage is a mesh layer");

    let mut through_the_layer = document
        .multires_from_mesh_layer(layer, MultiresDesc::default())
        .expect("the layer's own mesh is a cage");
    let mut through_a_copy =
        Multires::from_mesh(&mesh, MultiresDesc::default()).expect("so is the copy");
    for surface in [&mut through_the_layer, &mut through_a_copy] {
        surface.add_level().expect("subdivide once");
        surface.add_level().expect("subdivide twice");
        wrinkle(surface);
    }

    assert_eq!(
        through_the_layer.level_count(),
        through_a_copy.level_count(),
        "the same cage subdivides to the same depth whichever way it arrived"
    );
    assert_eq!(
        through_the_layer.level_counts(2).expect("counts"),
        through_a_copy.level_counts(2).expect("counts"),
        "and to the same vertex and face counts"
    );
    assert_eq!(
        through_the_layer.detail_checksum().expect("checksum"),
        through_a_copy.detail_checksum().expect("checksum"),
        "and the same dab lands on the same coefficients, which is the whole \
         claim: the layer lent its mesh rather than a different one"
    );
    assert!(
        relief(&mut through_the_layer) > 0.0,
        "and the fixture actually sculpted something, so the equality above is \
         not two empty hierarchies agreeing"
    );
}

/// A layer that is not a mesh layer has no cage to lend.
#[test]
fn a_field_layer_is_not_a_cage_and_says_so_rather_than_building_nothing() {
    let mut document = Document::new().expect("document");
    let field = document.add_sdf_layer("campo").expect("an SDF layer");
    let refused = document
        .multires_from_mesh_layer(field, MultiresDesc::default())
        .expect_err("a field layer holds no triangles");
    assert_eq!(
        refused.error.kind(),
        claycore::ErrorKind::NotFound,
        "the layer exists and its mesh does not, which is what NOT_FOUND says \
         here: {refused}"
    );
}

/// The bake back: a level's mesh takes the layer's place as one undo step.
///
/// This is the other half of the seam. A hierarchy's sculpt reaches a
/// `.clayspace` only as the *triangles of one level*, and that is what
/// crossing back out of the representation is — so the replacement has to land
/// on the layer that is already there rather than on a new one beside it, or
/// every reference the host holds to that row breaks.
#[test]
fn a_level_takes_the_cages_place_on_the_layer_it_came_from() {
    let mesh = cage(4, 2.0, "bake-back");
    let mut document = Document::new().expect("document");
    let layer = document
        .attach_mesh_layer(&mesh, &MeshLayerDesc::named("cage"))
        .expect("the cage is a mesh layer");
    document.enable_undo().expect("undo");

    let mut surface = document
        .multires_from_mesh_layer(layer, MultiresDesc::default())
        .expect("a hierarchy");
    surface.add_level().expect("subdivide once");
    surface.add_level().expect("subdivide twice");
    wrinkle(&mut surface);

    let flat = document.read_mesh_layer("cage").expect("the cage").0;
    let cage_vertices = flat.len();
    let standing = flat.iter().map(|p| p[1].abs()).fold(0.0f32, f32::max);
    assert_eq!(standing, 0.0, "the cage is a flat sheet before the bake");

    let before = document.mesh_layer_revision(layer).expect("a revision");
    let baked = surface.copy_level_mesh(2).expect("the finest level");
    let baked_vertices = baked.vertex_count();
    document
        .replace_mesh_layer(layer, &baked, before)
        .expect("the level replaces the cage");

    let (positions, ..) = document.read_mesh_layer("cage").expect("the layer");
    assert_eq!(
        positions.len(),
        baked_vertices,
        "the layer holds the level's vertices now, not the cage's {cage_vertices}"
    );
    assert!(
        positions.iter().map(|p| p[1].abs()).fold(0.0f32, f32::max) > 0.0,
        "and the wrinkle came with them"
    );
    assert_ne!(
        document.mesh_layer_revision(layer).expect("a revision"),
        before,
        "a wholesale replacement moves the revision, which is what tells a \
         cache built over the old triangles that it is wrong"
    );

    // One step, and it puts the cage back.
    assert!(document.undo().expect("undo"), "the bake is undoable");
    let (back, ..) = document.read_mesh_layer("cage").expect("the layer");
    assert_eq!(
        back.len(),
        cage_vertices,
        "undoing the replacement restores the cage, which is what makes it one \
         undo step rather than a layer swap the host has to unwind itself"
    );
}

/// A replacement handed a revision the layer has moved past is refused.
///
/// The check exists for a host that did slow work off the layer and wants to
/// commit it: if the layer was rebuilt while that work ran, committing would
/// overwrite what the sculptor did in the meantime.
#[test]
fn a_replacement_that_did_not_see_the_last_one_is_refused_rather_than_applied() {
    let mesh = cage(4, 2.0, "stale-commit");
    let mut document = Document::new().expect("document");
    let layer = document
        .attach_mesh_layer(&mesh, &MeshLayerDesc::named("cage"))
        .expect("the cage is a mesh layer");
    let mut surface = document
        .multires_from_mesh_layer(layer, MultiresDesc::default())
        .expect("a hierarchy");
    surface.add_level().expect("subdivide");

    let stale = document.mesh_layer_revision(layer).expect("a revision");
    let first = surface.copy_level_mesh(1).expect("level one");
    document
        .replace_mesh_layer(layer, &first, stale)
        .expect("the first commit sees what it expects");

    let second = surface.copy_level_mesh(1).expect("level one again");
    let refused = document
        .replace_mesh_layer(layer, &second, stale)
        .expect_err("the layer has moved since");
    assert_eq!(
        refused.kind(),
        claycore::ErrorKind::ForwardVersion,
        "and it is refused as a version that has been overtaken rather than as \
         a malformed argument: {refused}"
    );
}
