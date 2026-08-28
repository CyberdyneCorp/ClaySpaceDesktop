//! Which subtool is active, as the viewport says it.
//!
//! The stack has always said it and the viewport never did: every visible
//! carried layer arrived as one concatenated buffer and every visible SDF layer
//! as one merged surface, so a sculptor looking at the clay had no way of
//! telling which of the forms in front of them a dab would land on. These are
//! the two halves of the cue — a tint on a carried subtool, a box around an SDF
//! one — and the assertion that neither of them is anything but presentation.
//!
//! ```sh
//! cargo test -p clayspace-app --test visual_active_subtool
//! open target/visual
//! ```

mod support;

use std::path::Path;

use clayspace_engine::{BackendPolicy, ClayDocument};
use clayspace_model::{
    BrushSettings, Combine, CombineSettings, ExchangeModel, ExportSettings, GestureSample,
    LayerKey, ObjectModel, Representation, SceneModel, SculptModel, Shape, ToolKind,
};
use clayspace_view::{Camera, GpuMesh, Image, LatticeView};
use support::Harness;

/// How far apart the subtools stand, so each owns its own part of the frame.
const APART: f32 = 0.9;

/// The cell a test grid is worked at. Coarse: these captures are about which
/// form is tinted, not about how finely it is meshed.
const CELL: f32 = 0.05;

/// Three voxel subtools in a row, each with material of its own.
///
/// Voxel rather than mesh layers because a grid is the representation a
/// sculptor makes *inside* the application, and because both of them arrive in
/// the same carried buffer — whichever is tinted, it is the same span.
fn three_grids() -> Option<(ClayDocument, Vec<LayerKey>)> {
    let policy = BackendPolicy::discover(None).ok()?;
    let mut document = ClayDocument::new(policy).ok()?;
    let mut keys = Vec::new();
    for (at, name) in ["Esquerda", "Meio", "Direita"].iter().enumerate() {
        document.add_voxel_layer(name, CELL).ok()?;
        let key = document.scene().active?;
        deposit(&mut document, (at as f32 - 1.0) * APART);
        keys.push(key);
    }
    Some((document, keys))
}

/// A ridge of material dragged across the active grid, at an offset in x.
///
/// Through `apply_stroke`, which is the application's own path: filling a grid
/// directly would prove the renderer works and say nothing about whether what a
/// sculptor makes is what gets cued.
fn deposit(document: &mut ClayDocument, x: f32) {
    let brush = BrushSettings {
        size: 0.3,
        ..BrushSettings::default()
    };
    for step in 0..6 {
        let t = step as f32 / 5.0;
        let samples = [GestureSample {
            position: [x, -0.15 + t * 0.3, 0.0],
            pressure: 1.0,
            time: t,
        }];
        document
            .apply_stroke(ToolKind::Padrao, brush, &samples, [false; 3])
            .expect("the stroke was refused");
    }
}

/// Two SDF subtools, the second standing beside the first by its own layer
/// transform — which is the only way two SDF forms can be told apart at all,
/// since the surface merges them.
fn two_forms() -> Option<(ClayDocument, LayerKey)> {
    let policy = BackendPolicy::discover(None).ok()?;
    let mut document = ClayDocument::new(policy)
        .and_then(ClayDocument::with_starting_form)
        .ok()?;
    let second = document.add_layer("Segunda", Representation::Sdf).ok()?;
    document
        .place_object(
            Shape::Sphere,
            &[0.7],
            [0.0; 3],
            CombineSettings {
                op: Combine::Add,
                ..CombineSettings::default()
            },
        )
        .ok()?;
    document
        .set_layer_transform(second, [2.2, 0.0, 0.0], 1.0)
        .ok()?;
    Some((document, second))
}

/// Which pixels two frames disagree about, as a mask over the frame.
///
/// The counts alone cannot say whether two cues landed on the *same* clay,
/// which is exactly the question "and the others stay plain" asks.
fn difference_mask(a: &Image, b: &Image) -> Vec<bool> {
    (0..a.height)
        .flat_map(|y| (0..a.width).map(move |x| (x, y)))
        .map(|(x, y)| {
            let (pa, pb) = (a.pixel(x, y), b.pixel(x, y));
            (0..3).any(|c| pa[c].abs_diff(pb[c]) > support::RENDER_NOISE)
        })
        .collect()
}

fn marked(mask: &[bool]) -> usize {
    mask.iter().filter(|on| **on).count()
}

/// A camera framing every subtool at once.
///
/// The union of the layers' own boxes and not `SculptModel::bounds`, which
/// answers for the *active* layer alone: framed by that, two of the three grids
/// stood outside the frame and activating one of them changed nothing on screen
/// for a reason that had nothing to do with the cue.
fn framing(document: &ClayDocument, keys: &[LayerKey]) -> Camera {
    let mut camera = Camera::default();
    let mut extent: Option<([f32; 3], [f32; 3])> = None;
    for key in keys {
        let Some((min, max)) = SceneModel::layer_bounds(document, *key) else {
            continue;
        };
        extent = Some(match extent {
            None => (min, max),
            Some((lo, hi)) => (
                std::array::from_fn(|i| lo[i].min(min[i])),
                std::array::from_fn(|i| hi[i].max(max[i])),
            ),
        });
    }
    match extent {
        Some((min, max)) => camera.frame_bounds(min.into(), max.into()),
        None => camera.frame_default(),
    }
    camera
}

/// The cue on a carried subtool: one of three grids drawn in the accent's hue
/// while its neighbours stay the clay they were.
#[test]
fn an_active_voxel_subtool_is_tinted_among_plain_ones() {
    let Some(mut harness) = Harness::new() else {
        return;
    };
    let Some((mut document, keys)) = three_grids() else {
        return;
    };

    let (vertices, indices, spans) = support::viewport_layers(&mut document);
    assert_eq!(
        spans.len(),
        3,
        "three grids with material in them offered {} spans; without one span \
         per subtool there is nothing to draw a cue against",
        spans.len()
    );
    assert!(
        spans.iter().all(|span| !span.indices.is_empty()),
        "a span named a subtool and no triangles"
    );

    let gpu = harness.gpu.clone();
    harness
        .renderer
        .set_mesh_layers(&gpu, &vertices, &indices, &spans);
    let camera = framing(&document, &keys);
    // Nothing in the surface slot: a voxel layer carries no SDF content, so the
    // whole picture is the carried buffer.
    let surface = GpuMesh::new(&gpu);

    harness.renderer.set_active_subtool(None);
    let plain = harness.capture(&surface, &camera, true, "active-subtool-none");

    harness.renderer.set_active_subtool(Some(keys[0]));
    let first = harness.capture(&surface, &camera, true, "active-subtool-first");

    harness.renderer.set_active_subtool(Some(keys[2]));
    let last = harness.capture(&surface, &camera, true, "active-subtool-last");

    let on_first = difference_mask(&plain, &first);
    let on_last = difference_mask(&plain, &last);
    assert!(
        marked(&on_first) > 200,
        "activating the left subtool changed {} pixels; the cue is invisible",
        marked(&on_first)
    );
    assert!(
        marked(&on_last) > 200,
        "activating the right subtool changed {} pixels; the cue is invisible",
        marked(&on_last)
    );

    // The spec's "the cue moves": the two activations mark different clay. If
    // the tint reached the whole buffer — one material for every span, which is
    // what the concatenated draw used to be — these masks would coincide.
    let both = on_first
        .iter()
        .zip(&on_last)
        .filter(|(a, b)| **a && **b)
        .count();
    assert!(
        both * 20 < marked(&on_first).min(marked(&on_last)),
        "the two cues overlap on {both} pixels of {} and {}; activating one \
         subtool is tinting another",
        marked(&on_first),
        marked(&on_last)
    );
}

/// The cue on an SDF subtool: its box, because the merged surface cannot be
/// split per layer — the engine attributes no triangle to the layer it came
/// from, so there is nothing to tint.
#[test]
fn an_active_sdf_subtool_is_outlined() {
    let Some(mut harness) = Harness::new() else {
        return;
    };
    let Some((mut document, second)) = two_forms() else {
        return;
    };
    let gpu = harness.gpu.clone();
    let mut geometry = clayspace_app::SurfaceGeometry::new(&gpu);
    geometry
        .rebuild(&gpu, &mut document)
        .expect("mesh the forms");
    let camera = support::framed(&document);

    let bare = {
        harness.renderer.set_lattice(
            &gpu,
            LatticeView {
                points: &[],
                edges: &[],
                selected: &[],
                gizmo: None,
                outline: None,
                subtool_outline: None,
                handle: 0.0,
            },
        );
        harness.capture(geometry.mesh(), &camera, false, "active-subtool-sdf-none")
    };

    let bounds = SceneModel::layer_bounds(&document, second)
        .expect("the engine reports an SDF subtool's box");
    harness.renderer.set_lattice(
        &gpu,
        LatticeView {
            points: &[],
            edges: &[],
            selected: &[],
            gizmo: None,
            outline: None,
            subtool_outline: Some(bounds),
            handle: 0.0,
        },
    );
    let outlined = harness.capture(
        geometry.mesh(),
        &camera,
        false,
        "active-subtool-sdf-outline",
    );

    assert!(
        support::differing_pixels(&bare, &outlined) > 100,
        "outlining the active SDF subtool changed {} pixels; a cue nobody can \
         see is not a cue",
        support::differing_pixels(&bare, &outlined)
    );

    // The design's open question: the same box with the camera in close on the
    // subtool it wraps, which is what a sculptor working one form actually
    // looks at. Held to the same floor as the framed picture, because a box
    // whose every edge has left the frame is a cue that stops existing exactly
    // when the sculptor leans in.
    let mut close = Camera::default();
    let (min, max) = bounds;
    // Framed on the middle of the subtool rather than on the whole of it, which
    // is the case the question is about: a sculptor working a detail has the
    // form filling the frame and the box's edges outside it.
    let middle: [f32; 3] = std::array::from_fn(|i| (min[i] + max[i]) * 0.5);
    let inner: [f32; 3] = std::array::from_fn(|i| middle[i] - (max[i] - min[i]) * 0.22);
    let outer: [f32; 3] = std::array::from_fn(|i| middle[i] + (max[i] - min[i]) * 0.22);
    close.frame_bounds(inner.into(), outer.into());
    let dominant = harness.capture(
        geometry.mesh(),
        &close,
        false,
        "active-subtool-sdf-dominant",
    );
    harness.renderer.set_lattice(
        &gpu,
        LatticeView {
            points: &[],
            edges: &[],
            selected: &[],
            gizmo: None,
            outline: None,
            subtool_outline: None,
            handle: 0.0,
        },
    );
    let dominant_bare = harness.capture(
        geometry.mesh(),
        &close,
        false,
        "active-subtool-sdf-dominant-none",
    );
    assert!(
        support::differing_pixels(&dominant_bare, &dominant) > 100,
        "a subtool that fills the viewport lost its box entirely: {} pixels \
         differ",
        support::differing_pixels(&dominant_bare, &dominant)
    );
}

/// The cue is presentation and nothing else.
///
/// The requirement says so outright — "it SHALL NOT alter the geometry the
/// document holds, meshes or exports" — and the way to break it is not exotic:
/// tinting by writing the vertex colours would look identical on screen and
/// would walk straight into every exported file.
#[test]
fn the_cue_leaves_no_trace_in_what_the_document_hands_out() {
    let Some((mut document, keys)) = three_grids() else {
        return;
    };

    document
        .set_active_layer(keys[0])
        .expect("activate the first subtool");
    let before = document.visible_mesh_geometry();
    document
        .set_active_layer(keys[2])
        .expect("activate the third subtool");
    let after = document.visible_mesh_geometry();

    assert_eq!(
        before.0, after.0,
        "activating a different subtool moved a vertex"
    );
    assert_eq!(
        before.1, after.1,
        "activating a different subtool turned a normal"
    );
    assert_eq!(
        before.2, after.2,
        "activating a different subtool painted the clay; the tint has leaked \
         out of the material and into the geometry"
    );
    assert_eq!(
        before.3, after.3,
        "activating a different subtool changed a triangle"
    );
    assert_eq!(
        before.4, after.4,
        "activating a different subtool moved the spans; the cue must choose \
         among them, not rearrange them"
    );
}

/// And the same claim through the door the requirement actually names.
///
/// Over two SDF subtools, which is the pair an export actually writes: the
/// exported field is the union of every visible SDF layer, and which of them is
/// active is a fact about the interface that the file must never learn.
#[test]
fn the_cue_stays_out_of_the_export() {
    let Some((mut document, second)) = two_forms() else {
        return;
    };
    let first = document
        .scene()
        .layers
        .first()
        .map(|layer| layer.key)
        .expect("a document has a layer");
    let directory = std::env::temp_dir().join("clayspace-active-subtool-export");
    std::fs::create_dir_all(&directory).expect("a directory to export into");

    let mut exported = Vec::new();
    for (at, key) in [first, second].into_iter().enumerate() {
        document.set_active_layer(key).expect("activate a subtool");
        let path = directory.join(format!("cue-{at}.obj"));
        document
            .export_mesh(&path, ExportSettings::default())
            .expect("the export was refused");
        exported.push(read_obj(&path));
        std::fs::remove_file(&path).ok();
    }

    let (first, second) = (&exported[0], &exported[1]);
    assert!(
        !first.0.is_empty(),
        "the export wrote no vertices at all, so it can say nothing about what \
         reached the file"
    );
    assert_eq!(
        first.0.len(),
        second.0.len(),
        "the two exports carry different vertex counts, so the active subtool \
         reached the file"
    );
    assert_eq!(
        first.1, second.1,
        "the two exports carry different faces, so the active subtool reached \
         the file"
    );
    // Whole `v` lines, not the first three numbers of one: OBJ writes vertex
    // colour as three more numbers on the same line, and colour is precisely
    // where a cue drawn into the clay rather than into the material would end
    // up. Compared with a tolerance because the exporter prints floats and the
    // last digit is not the claim — `export_determinism.rs` measured that.
    let differing = first
        .0
        .iter()
        .zip(&second.0)
        .filter(|(a, b)| a.len() != b.len() || a.iter().zip(*b).any(|(x, y)| (x - y).abs() > 1e-4))
        .count();
    assert_eq!(
        differing, 0,
        "{differing} exported vertices differ in position or attributes when a \
         different subtool is made active; the cue reached the file"
    );
}

/// An OBJ's `v` lines, whole, and its `f` lines.
///
/// Whole because a `v` line carries the position and, where the writer has one,
/// the vertex colour after it — and both halves have to be the same across two
/// exports for "carries no trace of the cue" to mean anything.
fn read_obj(path: &Path) -> (Vec<Vec<f32>>, Vec<String>) {
    let text = std::fs::read_to_string(path).expect("read the exported file");
    let mut vertices = Vec::new();
    let mut faces = Vec::new();
    for line in text.lines() {
        let mut parts = line.split_whitespace();
        match parts.next() {
            Some("v") => vertices.push(parts.filter_map(|n| n.parse().ok()).collect()),
            Some("f") => faces.push(line.to_string()),
            _ => {}
        }
    }
    (vertices, faces)
}
