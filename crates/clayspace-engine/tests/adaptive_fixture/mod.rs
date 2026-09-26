//! The adaptive-surface fixture the table walks share.
//!
//! Not the marched mesh the other carried fixtures use, and for a reason the
//! representation states: a marching pass leaves slivers with collinear
//! corners, and `clay_dynamic_surface_from_mesh` refuses a degenerate triangle
//! rather than repairing one. So the surface is read from a clean lat-long
//! sphere written as a file and imported — the route a sculptor's own mesh
//! takes — carrying vertex colour so the colour brushes have something to
//! write into.

use clayspace_engine::{BackendPolicy, ClayDocument};
use clayspace_model::{
    BrushSettings, ConversionSettings, Direction, ExchangeModel, GestureSample, ImportSettings,
    Representation, SceneModel, SculptModel, ToolKind,
};

/// A unit sphere about the origin, poles on ±z, wound outward, grey.
fn sphere_obj(path: &std::path::Path, rings: usize, segments: usize) {
    let mut text = String::new();
    let grey = "0.6 0.6 0.6";
    text.push_str(&format!("v 0 0 1 {grey}\n"));
    for ring in 1..rings {
        let polar = std::f32::consts::PI * ring as f32 / rings as f32;
        for segment in 0..segments {
            let azimuth = std::f32::consts::TAU * segment as f32 / segments as f32;
            text.push_str(&format!(
                "v {} {} {} {grey}\n",
                polar.sin() * azimuth.cos(),
                polar.sin() * azimuth.sin(),
                polar.cos()
            ));
        }
    }
    text.push_str(&format!("v 0 0 -1 {grey}\n"));
    // One-based, as OBJ counts.
    let at = |ring: usize, segment: usize| 2 + (ring - 1) * segments + segment % segments;
    let south = 2 + (rings - 1) * segments;
    for segment in 0..segments {
        text.push_str(&format!("f 1 {} {}\n", at(1, segment), at(1, segment + 1)));
    }
    for ring in 1..rings - 1 {
        for segment in 0..segments {
            let (a, b) = (at(ring, segment), at(ring, segment + 1));
            let (c, d) = (at(ring + 1, segment), at(ring + 1, segment + 1));
            text.push_str(&format!("f {a} {c} {d}\nf {a} {d} {b}\n"));
        }
    }
    for segment in 0..segments {
        text.push_str(&format!(
            "f {} {south} {}\n",
            at(rings - 1, segment),
            at(rings - 1, segment + 1)
        ));
    }
    std::fs::write(path, text).expect("write the sphere");
}

/// A document whose active layer is an adaptive sphere with a ridge across
/// its top, so the planing and smoothing verbs have something to act on.
pub fn adaptive_sphere() -> ClayDocument {
    use std::sync::atomic::{AtomicU32, Ordering};
    static NEXT: AtomicU32 = AtomicU32::new(0);
    // Unique per call: tests in one binary are threads of one process.
    let scratch = std::env::temp_dir().join(format!(
        "clayspace-adaptive-sphere-{}-{}.obj",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let policy = BackendPolicy::discover(None).expect("discover backends");
    let mut document = ClayDocument::new(policy).expect("a document");
    sphere_obj(&scratch, 24, 48);
    document
        .import_mesh(&scratch, ImportSettings::default())
        .expect("import the sphere");
    let _ = std::fs::remove_file(&scratch);
    let mesh = document
        .scene()
        .layers
        .iter()
        .find(|layer| layer.representation == Representation::Mesh)
        .map(|layer| layer.key)
        .expect("the sphere is a mesh layer");
    document
        .set_active_layer(mesh)
        .expect("activate the sphere");
    let settings = ConversionSettings::default();
    document
        .convert_layer_in_place(Direction::MeshToDynamic, settings.cell_size, settings.blur)
        .expect("a clean sphere is an adaptive surface");
    assert_eq!(document.active_representation(), Representation::Dynamic);
    for step in 0..7 {
        let t = step as f32 / 6.0;
        document
            .apply_stroke(
                ToolKind::Padrao,
                BrushSettings {
                    size: 0.2,
                    intensity: 1.0,
                    ..BrushSettings::default()
                },
                &[GestureSample {
                    position: [(t - 0.5) * 0.5, 0.0, 1.0],
                    pressure: 1.0,
                    time: t,
                }],
                [false; 3],
            )
            .expect("deposit");
    }
    document
}
