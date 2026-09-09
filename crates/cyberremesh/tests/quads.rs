//! A sculpt, across the bridge, back as quads.
//!
//! The first thing in this workspace that exercises both engines in one
//! process. ClayCore marches a field into triangles; the handoff buffer
//! profile carries them across with no file between two libraries already in
//! one address space; the retopologiser returns quads.

use claycore::{Document, Item, MeshParams, Mesher, Op};
use cyberremesh::{remesh, Mesh, QuadMethod, RemeshParams, Unwatched};

/// Two overlapping lobes, marched watertight — a sculpt rather than a
/// primitive, so the retopologiser is being asked something.
fn sculpt() -> Option<(Vec<f32>, Vec<f32>, Vec<u32>)> {
    let mut document = Document::new().ok()?;
    let layer = document.add_sdf_layer("corpo").ok()?;
    for x in [-0.35f32, 0.35] {
        let mut lobe = Item::sphere(0.5).ok()?;
        lobe.set_op(Op::Add).ok()?;
        lobe.set_position([x, 0.0, 0.0]).ok()?;
        document.add_item(layer, &lobe).ok()?;
    }
    let mesh = document
        .mesh(MeshParams {
            voxel_size: Some(0.04),
            mesher: Mesher::MarchingTetrahedra,
            ..MeshParams::default()
        })
        .ok()?;
    let positions: Vec<f32> = mesh.positions().iter().flat_map(|p| *p).collect();
    // `None` where the mesh was meshed without gradients. The buffer profile
    // accepts that — unlike the PLY profile, which requires normals — so an
    // empty slice is a real answer here rather than a failure.
    let normals: Vec<f32> = mesh
        .normals()
        .map(|normals| normals.iter().flat_map(|n| *n).collect())
        .unwrap_or_default();
    Some((positions, normals, mesh.indices().to_vec()))
}

fn across() -> Option<Mesh> {
    let (positions, normals, indices) = sculpt()?;
    Some(
        Mesh::from_handoff(
            &positions,
            &normals,
            &indices,
            claycore::HANDOFF_VERSION,
            "ClaySpaceDesktop",
        )
        .expect("the handoff buffer profile carries a marched sculpt"),
    )
}

#[test]
fn a_sculpt_crosses_the_bridge_and_comes_back_as_quads() {
    cyberremesh::set_max_worker_threads(4).expect("the cap");
    let Some(source) = across() else {
        println!("no engine backend; skipping");
        return;
    };
    let triangles_in = source.triangle_count();
    assert!(triangles_in > 0, "the bridge carried nothing");
    assert_eq!(
        source.face_count(),
        triangles_in,
        "the sculpt arrived with faces that are not its triangles, so the \
         handoff carried something other than a triangulation"
    );

    let quads = remesh(
        &source,
        RemeshParams {
            target_quads: 500,
            ..RemeshParams::default()
        },
        &mut Unwatched,
    )
    .expect("the retopologiser accepts a marched sculpt");

    println!(
        "{triangles_in} triangles in -> {} faces / {} triangles out",
        quads.face_count(),
        quads.triangle_count()
    );
    assert!(quads.face_count() > 0, "the retopology produced nothing");
    // **The property that says these are quads.** The engine carries quads
    // beside the triangulation rather than instead of it, so a quad mesh has
    // fewer faces than triangles — two triangles to a quad — where a triangle
    // mesh has exactly as many.
    assert!(
        quads.face_count() < quads.triangle_count(),
        "the result has {} faces and {} triangles, which is what a triangle \
         mesh looks like: no quads were produced",
        quads.face_count(),
        quads.triangle_count()
    );
    assert!(quads.vertex_count() > 0);
}

/// Every method the interface offers is one the engine accepts.
///
/// Walked rather than spot-checked: the list is ours and the enumerants are
/// theirs, so a method added here without a value behind it fails on the row
/// that was added rather than the first time a sculptor picks it.
#[test]
fn every_offered_quad_method_runs() {
    cyberremesh::set_max_worker_threads(4).expect("the cap");
    let Some(source) = across() else {
        return;
    };
    for method in QuadMethod::ALL {
        let outcome = remesh(
            &source,
            RemeshParams {
                target_quads: 300,
                method,
                ..RemeshParams::default()
            },
            &mut Unwatched,
        );
        match outcome {
            Ok(quads) => println!(
                "  {:<18} {} faces, {} triangles",
                method.label(),
                quads.face_count(),
                quads.triangle_count()
            ),
            Err(e) => panic!("{} was offered and refused: {e}", method.label()),
        }
    }
}

/// The authored edges are not the triangulation's edges.
///
/// This is the assertion the retopology feature actually rests on, and it was
/// missing: `a_sculpt_crosses_the_bridge_and_comes_back_as_quads` proves the
/// engine *made* quads, and nothing proved a wireframe could be drawn from
/// them. A host that fetches `triangle_indices` for its overlay draws
/// triangles from a quad mesh and the sculptor sees no quads at all — which is
/// exactly what happened here.
///
/// A closed pure-quad mesh of F faces has 2F edges (Euler, each edge shared by
/// two quads). Its fan triangulation has 3F: the same 2F plus one diagonal per
/// quad. So `edges < 3F` is the observable that says the diagonals are absent,
/// and it is two-sided — a buffer that had included them could not satisfy it.
#[test]
fn the_edges_are_the_quads_and_not_their_triangulation() {
    cyberremesh::set_max_worker_threads(4).expect("the cap");
    let Some(source) = across() else {
        return;
    };
    let quads = remesh(
        &source,
        RemeshParams {
            target_quads: 400,
            method: QuadMethod::ZRemesher,
            ..RemeshParams::default()
        },
        &mut Unwatched,
    )
    .expect("the retopology");

    assert!(
        quads.render_order_is_positions_order(),
        "the index buffers are in render order and the positions are not, so a \
         wireframe drawn from these edges would join the wrong points"
    );

    let faces = quads.face_count();
    let edges = quads.edge_count();
    let indices = quads.edge_indices();
    println!(
        "  {faces} faces, {} triangles, {edges} authored edges",
        quads.triangle_count()
    );

    assert_eq!(indices.len(), edges * 2, "two vertex indices per edge");
    assert!(edges > 0, "a retopologised mesh with no edges");
    assert!(
        edges < faces * 3,
        "{edges} edges for {faces} faces — a fan triangulation of F quads has \
         3F edges and the quads themselves have 2F, so this buffer is \
         carrying the diagonals a wireframe must not draw"
    );
    let bound = quads.vertex_count() as u32;
    assert!(
        indices.iter().all(|&v| v < bound),
        "an edge indexes a vertex the mesh does not have"
    );
}
