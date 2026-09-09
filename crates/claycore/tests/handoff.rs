//! The sculpt handoff, checked against the reader that consumes it.
//!
//! The verification here is deliberately **their** CLI accepting the file and
//! not our own parser agreeing with us. A round trip through a reader we wrote
//! would be the tautology this project has spent a week cataloguing: it would
//! prove the two halves of one implementation agree, which they will, and say
//! nothing about the pipeline.
//!
//! ```sh
//! cargo test -p claycore --test handoff -- --nocapture
//! ```
//!
//! Skipped where the retopology CLI is not present, which is every machine
//! that has not built the sibling engine — including CI. `CYBERREMESH_CLI`
//! names it explicitly; otherwise the sibling checkout's own build is tried.

use std::path::PathBuf;
use std::process::Command;

use claycore::{Document, Item, Mesh, MeshParams, Mesher, Op};

/// The retopology CLI, if this machine has one.
fn cli() -> Option<PathBuf> {
    if let Some(named) = std::env::var_os("CYBERREMESH_CLI") {
        let path = PathBuf::from(named);
        return path.is_file().then_some(path);
    }
    // The sibling checkout, where the engine's own presets put it.
    let sibling = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../CyberRemesherAndUV/build/cpu-headless/apps/cli/cyberremesh");
    sibling.canonicalize().ok().filter(|path| path.is_file())
}

/// A sculpt, as a mesh: the starting form, meshed watertight.
fn sculpt() -> Option<Mesh> {
    let mut document = Document::new().ok()?;
    let layer = document.add_sdf_layer("corpo").ok()?;
    // Two overlapping lobes rather than one sphere: a retopologiser given a
    // primitive is not being asked anything, and the seam is what a real
    // handoff carries.
    for x in [-0.35f32, 0.35] {
        let mut lobe = Item::sphere(0.5).ok()?;
        lobe.set_op(Op::Add).ok()?;
        lobe.set_position([x, 0.0, 0.0]).ok()?;
        document.add_item(layer, &lobe).ok()?;
    }
    document
        .mesh(MeshParams {
            voxel_size: Some(0.05),
            mesher: Mesher::MarchingTetrahedra,
            ..MeshParams::default()
        })
        .ok()
}

#[test]
fn the_retopology_engine_accepts_what_the_handoff_writer_produces() {
    let Some(cli) = cli() else {
        println!("no retopology CLI on this machine; skipping");
        return;
    };
    let Some(mesh) = sculpt() else {
        println!("no engine backend; skipping");
        return;
    };

    let dir = std::env::temp_dir().join(format!("handoff-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("a working directory");
    let handoff = dir.join("sculpt.ply");
    let out = dir.join("low.obj");

    mesh.save_handoff(&handoff, Some("ClaySpaceDesktop"), None, true)
        .expect("the handoff writer");

    let run = Command::new(&cli)
        .arg("--target")
        .arg(&handoff)
        .arg("--output")
        .arg(&out)
        .arg("--target-quads")
        .arg("500")
        .output()
        .expect("the CLI runs");

    let stderr = String::from_utf8_lossy(&run.stderr);
    println!(
        "{} -> exit {:?}\n{}",
        handoff.display(),
        run.status.code(),
        stderr.trim()
    );
    assert!(
        run.status.success(),
        "the retopology engine refused a file written by the handoff writer, \
         which is the one call that exists to satisfy its reader:\n{stderr}"
    );
    assert!(
        out.is_file(),
        "the CLI reported success and wrote no output"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// **What this file cannot yet test, and why it is not an omission.**
///
/// The other half of the guard is that `Mesh::save` produces a file their
/// reader *refuses* — it declares a mesh's quads as its faces, and
/// `handoff.cpp:403` is `if (face.size() != 3) return ParseError`. That is the
/// reason `save_handoff` exists.
///
/// It cannot be exercised from here because **this crate does not bind quad
/// meshing at all**: `clay_document_mesh_quads` has no wrapper, so the file
/// their reader would reject is one we currently have no way to produce. A
/// test asserting `save` is refused would, today, hand them a triangle mesh
/// and watch it be accepted — which would assert the opposite of the thing
/// and pass.
///
/// So the assertion is owed the day quad meshing is bound, and it is recorded
/// here rather than in a task list nobody greps.
#[test]
fn the_quad_export_refusal_is_owed_not_forgotten() {
    let Some(mesh) = sculpt() else {
        return;
    };
    // What holds today: the handoff writer is triangles whatever the mesh is,
    // which is the guarantee their reader depends on.
    assert_eq!(
        mesh.index_count() % 3,
        0,
        "the sculpt mesh is not triangulated, so the premise of the handoff \
         writer's guarantee has changed"
    );
}

/// The invariant their reader cannot check, checked here.
///
/// `readBuffers` (`handoff.cpp:702`) bounds every index against
/// `vertexCount` before dereferencing it, so a corrupted buffer gives them a
/// typed `ParseError` and never an out-of-bounds read. But it reserves from
/// `vertexCount` itself, and in a pointer-plus-count API there is nothing to
/// test that count against — the count *defines* the buffer's extent rather
/// than making a claim about it.
///
/// That is precisely what made their PLY header bug fixable and this
/// unfixable from their side: in a file, a declared count and the file's size
/// are independent quantities, so a small file making a large claim is
/// provably lying. Here there is no second quantity. So the guarantee has to
/// come from this side of the call, and it reduces to one sentence: **the
/// handoff writer must declare a vertex count matching the array it actually
/// wrote.** Their reader trusts it because it has nothing else to trust.
///
/// Written down as an executable invariant rather than a property nobody
/// recorded — and unlike the CLI test above this needs no sibling engine, so
/// it holds on CI too.
#[test]
fn the_handoff_declares_the_counts_it_actually_wrote() {
    let Some(mesh) = sculpt() else {
        println!("no engine backend; skipping");
        return;
    };
    let bytes = mesh
        .handoff_bytes(Some("ClaySpaceDesktop"), None, false)
        .expect("the handoff writer");
    let text = String::from_utf8(bytes).expect("an ASCII handoff is text");

    let (header, body) = text
        .split_once("end_header\n")
        .expect("a PLY handoff carries a header");
    let declared_at = |element: &str| -> usize {
        header
            .lines()
            .find_map(|line| line.strip_prefix(&format!("element {element} ")))
            .unwrap_or_else(|| panic!("the header declares an {element} count"))
            .trim()
            .parse()
            .expect("a numeric element count")
    };
    let declared = declared_at("vertex");
    let declared_faces = declared_at("face");

    assert_eq!(
        declared,
        mesh.vertex_count(),
        "the handoff declared {declared} vertices for a mesh of {}",
        mesh.vertex_count()
    );
    assert_eq!(
        declared_faces,
        mesh.index_count() / 3,
        "the handoff declared {declared_faces} faces for a mesh of {} triangles",
        mesh.index_count() / 3
    );

    // How many fields a vertex row carries, read from the header rather than
    // assumed: the writer carries normals, colour and a material mix beside
    // the position, and this has to tell a vertex row from a face row without
    // hardcoding which properties were written.
    let properties = header
        .lines()
        .skip_while(|line| !line.starts_with("element vertex "))
        .skip(1)
        .take_while(|line| line.starts_with("property "))
        .count();
    assert!(
        properties >= 3,
        "the header declares {properties} vertex properties, so the handoff \
         carries no positions"
    );

    let rows: Vec<&str> = body.lines().filter(|l| !l.trim().is_empty()).collect();

    // Exact, and against the header's own two counts. `rows > declared` would
    // be `N + F > N`, which holds for any mesh with faces at all — including
    // one whose vertex block is *short* of its declaration, which is
    // precisely the failure this exists to catch. A handoff declaring 100
    // vertices, writing 90, then 50 faces has 140 body rows and would satisfy
    // `> 100` while handing their reader an array ten short of the count it
    // strides by.
    assert_eq!(
        rows.len(),
        declared + declared_faces,
        "the handoff declared {declared} vertices and {declared_faces} faces, \
         which is {} rows, and the body holds {}",
        declared + declared_faces,
        rows.len()
    );

    // And the first `declared` rows really are vertex rows. The total alone
    // would accept a body of the right length with the split in the wrong
    // place; the field count is what pins the boundary between the blocks.
    for (index, row) in rows[..declared].iter().enumerate() {
        assert_eq!(
            row.split_whitespace().count(),
            properties,
            "vertex row {index} carries {} fields where the header declares \
             {properties}, so the vertex block ends before its declared count",
            row.split_whitespace().count()
        );
    }

    // Every face index lands inside the declared array. This is the check
    // their reader also makes; asserting it here means a writer that ever
    // stopped satisfying it fails on our side, where the cause is, rather
    // than as a `ParseError` from a foreign process.
    let mut faces = 0usize;
    for row in &rows[declared..] {
        let mut field = row.split_whitespace();
        let arity: usize = field.next().expect("a face row").parse().expect("an arity");
        assert_eq!(
            arity, 3,
            "the handoff wrote a {arity}-sided face; their reader is \
             triangles-only at handoff.cpp:403"
        );
        for index in field.take(arity) {
            let index: usize = index.parse().expect("a numeric index");
            assert!(
                index < declared,
                "a face indexes vertex {index} of {declared} — their reader \
                 would refuse this buffer, and rightly"
            );
        }
        faces += 1;
    }
    // No `faces == index_count / 3` here, deliberately: the exact row count
    // above already fixes `rows.len() - declared`, and this loop visits each
    // of those rows once, so such an assertion could not fail. It would read
    // like a third independent check and be a restatement of the first.
    println!("handoff: {declared} vertices, {faces} faces, all indices in bounds");
}
