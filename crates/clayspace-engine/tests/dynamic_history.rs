//! Undo on an adaptive surface restores connectivity, not only positions.
//!
//! The fixture is a closed ball, because that is where a gesture is recorded
//! as the engine's reversible topology delta; an open sheet records the
//! surface's bytes instead (see `clayspace_engine::adaptive`), and
//! `tests/dynamic.rs` already covers that path. Every "exactly" below is a
//! digest over the drawn triangles by the bits of their positions, whatever
//! order the chunked buffer holds them in — the engine promises a replay
//! reproduces the surface exactly, not the buffer's layout.

use clayspace_engine::adaptive::{mesh_digest, Adaptive, Record, Replay};
use clayspace_engine::{BackendPolicy, ClayDocument};
use clayspace_model::{
    BrushSettings, ConversionSettings, Direction, ExchangeModel, GestureSample, ImportSettings,
    LayerKey, Representation, SceneModel, SculptModel, ToolKind,
};

// -- fixtures ---------------------------------------------------------------

/// A closed ball of radius one: `rings` bands of `segments` quads with a fan
/// at each pole, as (positions, triangles).
fn ball(rings: u32, segments: u32) -> (Vec<[f32; 3]>, Vec<[u32; 3]>) {
    let mut positions = vec![[0.0, 1.0, 0.0]];
    for ring in 1..rings {
        let phi = std::f32::consts::PI * ring as f32 / rings as f32;
        for seg in 0..segments {
            let theta = std::f32::consts::TAU * seg as f32 / segments as f32;
            positions.push([phi.sin() * theta.cos(), phi.cos(), phi.sin() * theta.sin()]);
        }
    }
    positions.push([0.0, -1.0, 0.0]);
    let bottom = positions.len() as u32 - 1;
    let at = |ring: u32, seg: u32| 1 + (ring - 1) * segments + seg % segments;
    let mut triangles = Vec::new();
    for seg in 0..segments {
        triangles.push([0, at(1, seg + 1), at(1, seg)]);
        triangles.push([bottom, at(rings - 1, seg), at(rings - 1, seg + 1)]);
    }
    for ring in 1..rings - 1 {
        for seg in 0..segments {
            let (a, b) = (at(ring, seg), at(ring, seg + 1));
            let (c, d) = (at(ring + 1, seg), at(ring + 1, seg + 1));
            triangles.push([a, b, d]);
            triangles.push([a, d, c]);
        }
    }
    (positions, triangles)
}

fn ball_mesh(rings: u32, segments: u32) -> claycore::Mesh {
    let (positions, triangles) = ball(rings, segments);
    let indices: Vec<u32> = triangles.into_iter().flatten().collect();
    claycore::Mesh::from_triangles(&positions, &indices).expect("a ball")
}

fn ball_obj(path: &std::path::Path) {
    let (positions, triangles) = ball(12, 16);
    let mut text = String::new();
    for [x, y, z] in positions {
        text.push_str(&format!("v {x} {y} {z}\n"));
    }
    for [a, b, c] in triangles {
        text.push_str(&format!("f {} {} {}\n", a + 1, b + 1, c + 1));
    }
    std::fs::write(path, text).expect("write the ball");
}

/// A document holding one imported ball, crossed in place into an adaptive
/// surface and active.
fn with_a_ball(who: &str) -> (ClayDocument, LayerKey) {
    let policy = BackendPolicy::discover(None).expect("discover backends");
    let mut document = ClayDocument::new(policy).expect("a document");
    let path = std::env::temp_dir().join(format!(
        "clayspace-dynamic-history-{who}-{}.obj",
        std::process::id()
    ));
    ball_obj(&path);
    document
        .import_mesh(&path, ImportSettings::default())
        .expect("import the ball");
    let _ = std::fs::remove_file(&path);
    let mesh = document
        .scene()
        .layers
        .iter()
        .find(|layer| layer.representation == Representation::Mesh)
        .map(|layer| layer.key)
        .expect("the ball is a mesh layer");
    document.set_active_layer(mesh).expect("activate the ball");
    let settings = ConversionSettings::default();
    let key = document
        .convert_layer_in_place(Direction::MeshToDynamic, settings.cell_size, settings.blur)
        .expect("a closed ball is an adaptive surface");
    (document, key)
}

fn brush() -> BrushSettings {
    BrushSettings {
        size: 0.5,
        intensity: 0.8,
        ..BrushSettings::default()
    }
}

/// The ball's front, from `from` to `to` over the surface at z ≈ 1, as seven
/// samples.
fn path(from: [f32; 2], to: [f32; 2]) -> Vec<GestureSample> {
    (0..=6)
        .map(|step| {
            let t = step as f32 / 6.0;
            let x = from[0] + (to[0] - from[0]) * t;
            let y = from[1] + (to[1] - from[1]) * t;
            let z = (1.0 - x * x - y * y).max(0.0).sqrt();
            GestureSample {
                position: [x, y, z],
                pressure: 1.0,
                time: t,
            }
        })
        .collect()
}

/// One gesture, sent the way the interface sends it: in segments, the whole
/// path so far for a dragging verb and the new samples for a stamping one.
fn gesture(
    document: &mut ClayDocument,
    tool: ToolKind,
    samples: &[GestureSample],
    symmetry: [bool; 3],
) -> bool {
    document.begin_gesture();
    let mut changed = false;
    for (start, end) in [(0, 3), (3, 5), (5, samples.len())] {
        let segment = if tool.is_path_driven() {
            &samples[..end]
        } else {
            &samples[start..end]
        };
        changed |= document
            .apply_stroke(tool, brush(), segment, symmetry)
            .expect("the segment is applied")
            .changed;
    }
    document.end_gesture();
    changed
}

fn draw(document: &mut ClayDocument, symmetry: [bool; 3]) -> bool {
    gesture(
        document,
        ToolKind::Padrao,
        &path([-0.3, 0.1], [0.3, 0.1]),
        symmetry,
    )
}

/// The drawn surface as (digest, triangles), independent of how the buffer
/// lays it out.
///
/// A chunked surface is drawn from slots with spare room (degenerate
/// triangles), with seam vertices duplicated per chunk, and a delta undo is
/// patched into those slots in place — so the same surface can sit in the
/// buffer in a different order. Each real triangle is read as its three
/// positions' bits, rotated to start at the least (winding kept), and the
/// sorted list is digested: equal exactly when the same triangles are drawn.
fn digest(document: &mut ClayDocument) -> (u64, usize) {
    let (positions, _, _, indices, _) = document.visible_mesh_geometry();
    let bits = |i: u32| positions[i as usize].map(f32::to_bits);
    let mut triangles: Vec<[[u32; 3]; 3]> = indices
        .chunks_exact(3)
        .filter(|t| t[0] != t[1] && t[1] != t[2] && t[0] != t[2])
        .map(|t| {
            let corners = [bits(t[0]), bits(t[1]), bits(t[2])];
            let least = (0..3).min_by_key(|&k| corners[k]).unwrap_or(0);
            [0, 1, 2].map(|k| corners[(least + k) % 3])
        })
        .collect();
    triangles.sort_unstable();
    let flat: Vec<[f32; 3]> = triangles
        .iter()
        .flatten()
        .map(|corner| corner.map(f32::from_bits))
        .collect();
    let order: Vec<u32> = (0..flat.len() as u32).collect();
    (mesh_digest(&flat, &order), triangles.len())
}

// -- the gesture ------------------------------------------------------------

/// A gesture drawn in three segments and mirrored across x is one undo entry,
/// however many triangles it split.
#[test]
fn a_dynamic_gesture_is_one_undo_entry() {
    let (mut document, _) = with_a_ball("one-entry");
    let (_, before) = digest(&mut document);
    let depth = document.history().depth;
    assert!(
        draw(&mut document, [true, false, false]),
        "the stroke lands"
    );
    let (_, after) = digest(&mut document);
    assert_ne!(before, after, "the stroke changed the topology");
    assert_eq!(document.history().depth, depth + 1);
    let held = document.dynamic_diagnostics();
    assert_eq!(held.history_steps, 1, "one record for the whole gesture");
    assert!(held.history_bytes > 0, "and it is priced");
}

/// The undo gives back the exact connectivity and positions, and the redo the
/// exact post-gesture surface.
#[test]
fn undo_restores_connectivity() {
    let (mut document, _) = with_a_ball("connectivity");
    let before = digest(&mut document);
    assert!(draw(&mut document, [false; 3]));
    let after = digest(&mut document);
    assert_ne!(before.1, after.1, "the stroke changed the triangle count");

    assert!(document.undo().expect("undo"));
    assert_eq!(digest(&mut document), before, "back to the ball, exactly");
    assert!(document.redo().expect("redo"));
    assert_eq!(digest(&mut document), after, "and forward, exactly");
}

/// Undo, redo, undo, redo… converge on the same two digests: a redo lays down
/// the same connectivity, not an equivalent one.
#[test]
fn redo_is_deterministic() {
    let (mut document, _) = with_a_ball("deterministic");
    let before = digest(&mut document);
    assert!(draw(&mut document, [false; 3]));
    let after = digest(&mut document);
    for _ in 0..3 {
        assert!(document.undo().expect("undo"));
        assert_eq!(digest(&mut document), before);
        assert!(document.redo().expect("redo"));
        assert_eq!(digest(&mut document), after);
    }
}

/// A dragging verb is laid down again from its anchor on every segment. Its
/// undo takes back the whole drag, not the last segment's.
#[test]
fn a_dragged_gesture_is_undone_whole() {
    let (mut document, _) = with_a_ball("drag");
    let before = digest(&mut document);
    let depth = document.history().depth;
    assert!(gesture(
        &mut document,
        ToolKind::Mover,
        &path([0.0, 0.0], [0.0, 0.4]),
        [false; 3],
    ));
    assert_eq!(document.history().depth, depth + 1);
    assert_ne!(digest(&mut document), before);
    assert!(document.undo().expect("undo"));
    assert_eq!(digest(&mut document), before);
}

/// N gestures across several verbs, undone N times, give back the starting
/// digest; redone N times, the finishing one.
#[test]
fn n_gestures_undone_n_times_return_to_the_start() {
    let (mut document, _) = with_a_ball("series");
    let start = digest(&mut document);
    let strokes = [
        (ToolKind::Padrao, path([-0.3, 0.0], [0.3, 0.0])),
        (ToolKind::Mover, path([0.1, 0.1], [0.1, 0.4])),
        (ToolKind::Inflar, path([-0.2, -0.3], [0.2, -0.3])),
        (ToolKind::Argila, path([-0.3, 0.3], [0.3, 0.3])),
        (ToolKind::Puxar, path([0.0, -0.1], [0.0, 0.2])),
        (ToolKind::Suavizar, path([-0.3, 0.0], [0.3, 0.0])),
    ];
    let mut landed = 0;
    for (tool, samples) in &strokes {
        landed += usize::from(gesture(&mut document, *tool, samples, [false; 3]));
    }
    assert!(landed >= 4, "only {landed} of the strokes landed");
    let finish = digest(&mut document);
    assert_eq!(document.dynamic_diagnostics().history_steps, landed);
    for _ in 0..landed {
        assert!(document.undo().expect("undo"));
    }
    assert_eq!(digest(&mut document), start);
    for _ in 0..landed {
        assert!(document.redo().expect("redo"));
    }
    assert_eq!(digest(&mut document), finish);
}

/// The spec's mixed history: a stroke after a crossing, undone twice, gives
/// back the pre-stroke surface and then the pre-crossing mesh — and forward
/// again lands on the stroke's exact surface, because the crossing's redo
/// brings back the same surface the stroke's record was taken on.
#[test]
fn a_stroke_after_a_crossing_undoes_in_order_and_redoes_exactly() {
    let (mut document, key) = with_a_ball("mixed");
    let crossed = digest(&mut document);
    assert!(draw(&mut document, [false; 3]));
    let sculpted = digest(&mut document);

    assert!(document.undo().expect("undo the stroke"));
    assert_eq!(digest(&mut document), crossed);
    assert!(document.undo().expect("undo the crossing"));
    assert!(document.scene().layer(key).is_none());
    assert_eq!(document.dynamic_diagnostics().held, 0);

    assert!(document.redo().expect("redo the crossing"));
    assert_eq!(digest(&mut document), crossed);
    assert!(document.redo().expect("redo the stroke"));
    assert_eq!(digest(&mut document), sculpted);
}

/// A new gesture after an undo ends the redo line, and the record it
/// dropped stops being counted against the budget.
#[test]
fn a_new_gesture_after_an_undo_drops_the_redo_and_its_bytes() {
    let (mut document, _) = with_a_ball("redo-line");
    assert!(draw(&mut document, [false; 3]));
    assert!(document.undo().expect("undo"));
    assert_eq!(document.dynamic_diagnostics().history_steps, 1);
    assert!(gesture(
        &mut document,
        ToolKind::Inflar,
        &path([-0.2, -0.3], [0.2, -0.3]),
        [false; 3],
    ));
    let held = document.dynamic_diagnostics();
    assert_eq!(held.history_steps, 1, "the undone stroke's record is gone");
    assert!(!document.history().can_redo);
}

// -- what a record costs ----------------------------------------------------

/// One Draw gesture on a closed ball of `rings × segments`, as (faces before,
/// delta encoded bytes, delta resident bytes, surface bytes).
fn price_one_gesture(rings: u32, segments: u32) -> (u64, u64, u64, usize) {
    let mut adaptive = Adaptive::from_mesh(&ball_mesh(rings, segments)).expect("reads");
    let faces = adaptive.stats().faces;
    let snapshot = adaptive.bytes(0).expect("bytes").len();
    let samples: Vec<[f32; 5]> = (0..=12)
        .map(|step| {
            let x = -0.3 + step as f32 * 0.05;
            [x, 0.1, (1.0 - x * x - 0.01).sqrt(), 1.0, step as f32]
        })
        .collect();
    let stamp = claycore::MeshStamp {
        verb: claycore::MeshBrush::Draw,
        radius: 0.25,
        strength: 0.8,
        ..claycore::MeshStamp::default()
    };
    assert!(adaptive
        .stroke(
            &[(samples, stamp)],
            &claycore::StrokePreset::default(),
            &Adaptive::topology(),
            None,
        )
        .expect("the stroke"));
    let before = adaptive.digest().expect("digest");
    let Some(Record::Delta(delta)) = adaptive.close_gesture() else {
        panic!("a closed ball records a delta");
    };
    let stats = delta.stats().expect("stats");
    let record = adaptive
        .step(Record::Delta(delta), Replay::Revert)
        .expect("undo");
    let record = adaptive.step(record, Replay::Apply).expect("redo");
    assert_eq!(adaptive.digest().expect("digest"), before);
    assert_eq!(record.weight() as u64, stats.resident_bytes);
    (faces, stats.encoded_bytes, stats.resident_bytes, snapshot)
}

/// History memory per Dynamic gesture at two model sizes, delta against the
/// snapshot the history recorded before it.
///
/// Run in release to read the figures:
/// `cargo test --release -p clayspace-engine --test dynamic_history -- --nocapture price`
#[test]
fn price_a_gesture_at_two_model_sizes() {
    let mut figures = Vec::new();
    for (rings, segments) in [(48, 64), (192, 256)] {
        let (faces, encoded, resident, snapshot) = price_one_gesture(rings, segments);
        println!(
            "{faces} faces: delta {encoded} B encoded, {resident} B resident; \
             snapshot {snapshot} B"
        );
        figures.push((resident, snapshot as u64));
    }
    let (small, large) = (figures[0], figures[1]);
    assert!(
        large.1 > small.1 * 8,
        "the snapshot follows the surface: {} then {}",
        small.1,
        large.1
    );
    assert!(
        large.0 < large.1,
        "on the larger model the delta costs less than the snapshot: {} against {}",
        large.0,
        large.1
    );
}
